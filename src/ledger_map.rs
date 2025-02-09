use crate::errors::LedgerError;
use crate::ledger_entry::{
    EntryKey, EntryValue, LedgerBlock, LedgerBlockHeader, LedgerEntry, Operation,
};
use crate::metadata::Metadata;
use crate::partition_table;
use crate::platform_specific::{
    persistent_storage_read, persistent_storage_size_bytes, persistent_storage_write,
};
use crate::{debug, info, warn};
use crate::{platform_specific, AHashSet};
use anyhow::Result;
use async_stream::try_stream;
use borsh::to_vec;
use futures::{
    pin_mut,
    stream::{Stream, TryStreamExt},
};
use indexmap::IndexMap;
use sha2::Digest;
use std::cell::RefCell;

#[derive(Debug)]
pub struct LedgerMap {
    metadata: RefCell<Metadata>,
    labels_to_index: Option<AHashSet<String>>,
    entries: IndexMap<String, IndexMap<EntryKey, LedgerEntry>>,
    next_block_entries: IndexMap<String, IndexMap<EntryKey, LedgerEntry>>,
    current_timestamp_nanos: fn() -> u64,
}

impl LedgerMap {
    /// Create a new LedgerMap instance.
    /// If `labels_to_index` is `None`, then all labels will be indexed.
    /// Note that iterating over non-indexed labels will not be possible through .iter()
    pub async fn new(labels_to_index: Option<Vec<String>>) -> anyhow::Result<Self> {
        let mut result = LedgerMap {
            metadata: RefCell::new(Metadata::new().await),
            labels_to_index: labels_to_index.map(AHashSet::from_iter),
            entries: IndexMap::new(),
            next_block_entries: IndexMap::new(),
            current_timestamp_nanos: platform_specific::get_timestamp_nanos,
        };
        result.refresh_ledger().await?;
        Ok(result)
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub async fn new_with_path(
        labels_to_index: Option<Vec<String>>,
        path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        platform_specific::set_backing_file(path).map_err(|e| anyhow::format_err!("{:?}", e))?;
        Self::new(labels_to_index).await
    }

    #[cfg(all(target_arch = "wasm32", feature = "browser"))]
    pub async fn new_with_path(
        labels_to_index: Option<Vec<String>>,
        _path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        Self::new(labels_to_index).await
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub fn get_file_path(&self) -> Option<std::path::PathBuf> {
        platform_specific::get_backing_file_path()
    }

    #[cfg(all(target_arch = "wasm32", feature = "browser"))]
    pub fn get_file_path(&self) -> Option<std::path::PathBuf> {
        None
    }

    #[cfg(test)]
    fn with_timestamp_fn(self, get_timestamp_nanos: fn() -> u64) -> Self {
        LedgerMap {
            current_timestamp_nanos: get_timestamp_nanos,
            ..self
        }
    }

    pub fn begin_block(&mut self) -> anyhow::Result<()> {
        if !&self.next_block_entries.is_empty() {
            return Err(anyhow::format_err!("There is already an open transaction."));
        } else {
            self.next_block_entries.clear();
        }
        Ok(())
    }

    pub async fn commit_block(&mut self) -> anyhow::Result<()> {
        if self.next_block_entries.is_empty() {
            // debug!("Commit of empty block invoked, skipping");
        } else {
            info!(
                "Commit non-empty block, with {} entries",
                self.next_block_entries.len()
            );
            let mut block_entries = Vec::new();
            for (label, values) in self.next_block_entries.iter() {
                if match &self.labels_to_index {
                    Some(labels_to_index) => labels_to_index.contains(label),
                    None => true,
                } {
                    self.entries
                        .entry(label.clone())
                        .or_default()
                        .extend(values.clone())
                };
                for (_key, entry) in values.iter() {
                    block_entries.push(entry.clone());
                }
            }
            let block_timestamp = (self.current_timestamp_nanos)();
            let parent_hash = self.metadata.borrow().get_last_block_chain_hash().to_vec();
            let block = LedgerBlock::new(block_entries, block_timestamp, parent_hash);
            self._persist_block(block).await?;
            self.next_block_entries.clear();
        }
        Ok(())
    }

    pub fn get<S: AsRef<str>>(&self, label: S, key: &[u8]) -> Result<EntryValue, LedgerError> {
        fn lookup<'a>(
            map: &'a IndexMap<String, IndexMap<EntryKey, LedgerEntry>>,
            label: &String,
            key: &[u8],
        ) -> Option<&'a LedgerEntry> {
            match map.get(label) {
                Some(entries) => entries.get(key),
                None => None,
            }
        }

        let label = label.as_ref().to_string();
        for map in [&self.next_block_entries, &self.entries] {
            if let Some(entry) = lookup(map, &label, key) {
                match entry.operation() {
                    Operation::Upsert => {
                        return Ok(entry.value().to_vec());
                    }
                    Operation::Delete => {
                        return Err(LedgerError::EntryNotFound);
                    }
                }
            }
        }

        Err(LedgerError::EntryNotFound)
    }

    pub fn upsert<S: AsRef<str>, K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &mut self,
        label: S,
        key: K,
        value: V,
    ) -> Result<(), LedgerError> {
        self._insert_entry_into_next_block(label, key, value, Operation::Upsert)
    }

    pub fn delete<S: AsRef<str>, K: AsRef<[u8]>>(
        &mut self,
        label: S,
        key: K,
    ) -> Result<(), LedgerError> {
        self._insert_entry_into_next_block(label, key, Vec::new(), Operation::Delete)
    }

    pub async fn refresh_ledger(&mut self) -> anyhow::Result<()> {
        let metadata = self.metadata.borrow_mut();
        drop(metadata);
        self.entries.clear();
        self.next_block_entries.clear();

        // If the backend is empty or non-existing, just return
        if persistent_storage_size_bytes().await == 0 {
            warn!("Persistent storage is empty");
            return Ok(());
        }

        let data_part_entry = partition_table::get_data_partition().await;
        if persistent_storage_size_bytes().await < data_part_entry.start_lba {
            warn!("No data found in persistent storage");
            return Ok(());
        }

        let mut expected_parent_hash = Vec::new();
        let mut updates = Vec::new();
        // Step 1: Read all Ledger Blocks
        {
            let stream = self.iter_raw();
            pin_mut!(stream);

            while let Ok(entry) = stream.try_next().await {
                let (block_header, ledger_block) = match entry {
                    Some(entry) => entry,
                    None => break,
                };

                if ledger_block.parent_hash() != expected_parent_hash {
                    return Err(anyhow::format_err!(
                        "Hash mismatch: expected parent hash {:?}, got {:?}",
                        expected_parent_hash,
                        ledger_block.parent_hash()
                    ));
                };

                let new_chain_hash = Self::_compute_block_chain_hash(
                    ledger_block.parent_hash(),
                    ledger_block.entries(),
                    ledger_block.timestamp(),
                )?;

                let next_block_start_pos = self.metadata.borrow().next_block_start_pos()
                    + block_header.jump_bytes_next_block() as u64;
                self.metadata.borrow_mut().update_from_appended_block(
                    &new_chain_hash,
                    ledger_block.timestamp(),
                    next_block_start_pos,
                );
                expected_parent_hash = new_chain_hash;

                updates.push(ledger_block);
            }
        }

        // Step 2: Add ledger entries into the index (self.entries) for quick search
        for ledger_block in updates.into_iter() {
            for ledger_entry in ledger_block.entries() {
                // Skip entries that are not in the labels_to_index
                if !match &self.labels_to_index {
                    Some(labels_to_index) => labels_to_index.contains(ledger_entry.label()),
                    None => true,
                } {
                    continue;
                }
                let entries = match self.entries.get_mut(ledger_entry.label()) {
                    Some(entries) => entries,
                    None => {
                        let new_map = IndexMap::new();
                        self.entries
                            .insert(ledger_entry.label().to_string(), new_map);
                        self.entries
                            .get_mut(ledger_entry.label())
                            .ok_or(anyhow::format_err!(
                                "Entry label {:?} not found",
                                ledger_entry.label()
                            ))?
                    }
                };

                match &ledger_entry.operation() {
                    Operation::Upsert => {
                        entries.insert(ledger_entry.key().to_vec(), ledger_entry.clone());
                    }
                    Operation::Delete => {
                        entries.swap_remove(&ledger_entry.key().to_vec());
                    }
                }
            }
        }
        debug!("Ledger refreshed successfully");

        Ok(())
    }

    pub fn next_block_iter(&self, label: Option<&str>) -> impl Iterator<Item = &LedgerEntry> {
        match label {
            Some(label) => self
                .next_block_entries
                .get(label)
                .map(|entries| entries.values())
                .unwrap_or_default()
                .filter(|entry| entry.operation() == Operation::Upsert)
                .collect::<Vec<_>>()
                .into_iter(),
            None => self
                .next_block_entries
                .values()
                .flat_map(|entries| entries.values())
                .filter(|entry| entry.operation() == Operation::Upsert)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    pub fn iter(&self, label: Option<&str>) -> impl Iterator<Item = &LedgerEntry> {
        match label {
            Some(label) => self
                .entries
                .get(label)
                .map(|entries| entries.values())
                .unwrap_or_default()
                .filter(|entry| entry.operation() == Operation::Upsert)
                .collect::<Vec<_>>()
                .into_iter(),
            None => self
                .entries
                .values()
                .flat_map(|entries| entries.values())
                .filter(|entry| entry.operation() == Operation::Upsert)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    pub fn iter_raw(&self) -> impl Stream<Item = Result<(LedgerBlockHeader, LedgerBlock)>> + '_ {
        try_stream! {
            let data_start = partition_table::get_data_partition().await.start_lba;
            let mut current_lba = data_start;

            loop {
                match self._persisted_block_read(current_lba).await {
                    Ok((block_header, ledger_block)) => {
                        let jump_next = block_header.jump_bytes_next_block() as u64;
                        // Yield the next item as a success
                        yield (block_header, ledger_block);

                        // Advance to the next block offset
                        current_lba += jump_next;
                    }
                    Err(LedgerError::BlockEmpty) => {
                        // End of stream
                        break;
                    }
                    Err(LedgerError::BlockCorrupted(e)) => {
                        // Cause the stream to fail with this error and stop
                        Err(anyhow::anyhow!("Failed to read Ledger block: {}", e))?;
                    }
                    Err(e) => {
                        // Also fail
                        Err(anyhow::anyhow!("Failed to read Ledger block: {}", e))?;
                    }
                }
            }
        }
    }

    pub fn get_blocks_count(&self) -> usize {
        self.metadata.borrow().num_blocks()
    }

    pub fn get_latest_block_hash(&self) -> Vec<u8> {
        self.metadata.borrow().get_last_block_chain_hash().to_vec()
    }

    pub fn get_latest_block_timestamp_ns(&self) -> u64 {
        self.metadata.borrow().get_last_block_timestamp_ns()
    }

    pub fn get_next_block_start_pos(&self) -> u64 {
        self.metadata.borrow().next_block_start_pos()
    }

    pub fn get_next_block_entries_count(&self, label: Option<&str>) -> usize {
        self.next_block_iter(label).count()
    }

    fn _compute_block_chain_hash(
        parent_block_hash: &[u8],
        block_entries: &[LedgerEntry],
        block_timestamp: u64,
    ) -> anyhow::Result<Vec<u8>> {
        let mut hasher = sha2::Sha256::new();
        hasher.update(parent_block_hash);
        for entry in block_entries.iter() {
            hasher.update(to_vec(entry)?);
        }
        hasher.update(block_timestamp.to_le_bytes());
        Ok(hasher.finalize().to_vec())
    }

    async fn _persist_block(&self, ledger_block: LedgerBlock) -> anyhow::Result<()> {
        let block_serialized_data = ledger_block.serialize()?;
        info!(
            "Appending block @timestamp {} with {} bytes data: {}",
            ledger_block.timestamp(),
            block_serialized_data.len(),
            ledger_block
        );
        // Prepare block header
        let jump_bytes_prev_block = (self
            .metadata
            .borrow()
            .tip_block_start_pos()
            .unwrap_or_default() as i64
            - self.metadata.borrow().next_block_start_pos() as i64)
            as i32;
        let jump_bytes_next_block =
            (block_serialized_data.len() + LedgerBlockHeader::sizeof()) as u32;
        let serialized_block_header =
            LedgerBlockHeader::new(jump_bytes_prev_block, jump_bytes_next_block).serialize()?;

        // First persist block header
        let write_pos = self.metadata.borrow().next_block_start_pos();
        persistent_storage_write(write_pos, &serialized_block_header).await;

        // Then persist block data
        let write_pos =
            self.metadata.borrow().next_block_start_pos() + LedgerBlockHeader::sizeof() as u64;
        persistent_storage_write(write_pos, &block_serialized_data).await;

        let new_chain_hash = Self::_compute_block_chain_hash(
            ledger_block.parent_hash(),
            ledger_block.entries(),
            ledger_block.timestamp(),
        )?;
        let next_block_start_pos =
            self.metadata.borrow().next_block_start_pos() + jump_bytes_next_block as u64;
        self.metadata.borrow_mut().update_from_appended_block(
            &new_chain_hash,
            ledger_block.timestamp(),
            next_block_start_pos,
        );

        // Finally, persist 0u32 to mark the end of the block chain
        let write_pos =
            self.metadata.borrow().next_block_start_pos() + jump_bytes_next_block as u64;
        persistent_storage_write(write_pos, &[0u8; size_of::<u32>()]).await;
        Ok(())
    }

    async fn _persisted_block_read(
        &self,
        offset: u64,
    ) -> Result<(LedgerBlockHeader, LedgerBlock), LedgerError> {
        // Find out how many bytes we need to read ==> block len in bytes
        let mut buf = [0u8; size_of::<LedgerBlockHeader>()];
        persistent_storage_read(offset, &mut buf)
            .await
            .map_err(|e| LedgerError::BlockCorrupted(e.to_string()))?;

        let block_header = LedgerBlockHeader::deserialize(buf.as_ref())?;
        let block_len_bytes = block_header.jump_bytes_next_block();

        debug!(
            "Reading persisted block of {} bytes at offset 0x{:0x}",
            block_len_bytes, offset
        );

        // Read the block as raw bytes
        let mut buf = vec![0u8; block_len_bytes as usize];
        persistent_storage_read(offset + LedgerBlockHeader::sizeof() as u64, &mut buf)
            .await
            .map_err(|e| LedgerError::Other(e.to_string()))?;

        let block = LedgerBlock::deserialize(buf.as_ref(), block_header.block_version())
            .map_err(|err| LedgerError::BlockCorrupted(err.to_string()))?;

        Ok((block_header, block))
    }

    fn _insert_entry_into_next_block<S: AsRef<str>, K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &mut self,
        label: S,
        key: K,
        value: V,
        operation: Operation,
    ) -> Result<(), LedgerError> {
        let entry = LedgerEntry::new(label.as_ref(), key, value, operation);
        match self.next_block_entries.get_mut(entry.label()) {
            Some(entries) => {
                entries.insert(entry.key().to_vec(), entry);
            }
            None => {
                let mut new_map = IndexMap::new();
                new_map.insert(entry.key().to_vec(), entry);
                self.next_block_entries
                    .insert(label.as_ref().to_string(), new_map);
            }
        };

        Ok(())
    }
}

#[cfg(test)]
#[path = "ledger_map_tests.rs"]
mod ledger_map_tests;
