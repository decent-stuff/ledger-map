use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::Storage;

thread_local! {
    static LOCAL_STORAGE: RefCell<Option<Storage>> = RefCell::new(None);
    static NUM_PAGES_ALLOCATED: RefCell<u64> = RefCell::new(0);
}

fn num_pages_allocated_get() -> u64 {
    NUM_PAGES_ALLOCATED.with(|n| *n.borrow())
}

fn num_pages_allocated_set(n: u64) {
    NUM_PAGES_ALLOCATED.with(|num| *num.borrow_mut() = n);
}

fn num_pages_allocated_inc() {
    NUM_PAGES_ALLOCATED.with(|n| *n.borrow_mut() += 1);
}

// Re-export macros for use in other modules
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        web_sys::console::debug_1(&format!($($arg)*).into());
    }};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        web_sys::console::info_1(&format!($($arg)*).into());
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        web_sys::console::warn_1(&format!($($arg)*).into());
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        web_sys::console::error_1(&format!($($arg)*).into());
    }};
}

// Export functions that match the interface expected by the rest of the codebase
pub fn export_debug(msg: &str) {
    debug!("{}", msg);
}

pub fn export_info(msg: &str) {
    info!("{}", msg);
}

pub fn export_warn(msg: &str) {
    warn!("{}", msg);
}

pub fn export_error(msg: &str) {
    error!("{}", msg);
}

fn is_storage_initialized() -> bool {
    LOCAL_STORAGE.with(|ls| ls.borrow().is_some())
}

#[wasm_bindgen(start)]
pub async fn init_storage() {
    if is_storage_initialized() {
        return;
    }
    info!("Initializing storage");

    let window = web_sys::window().expect("no global window exists");
    let storage = window
        .local_storage()
        .expect("no local storage exists")
        .expect("failed to get local storage");

    // Initialize storage in a separate scope to avoid borrow issues
    LOCAL_STORAGE.with(|ls| {
        *ls.borrow_mut() = Some(storage);
        info!("Storage initialized successfully");
    })
}

#[wasm_bindgen]
pub fn clear_storage() {
    web_sys::window()
        .expect("no global window exists")
        .local_storage()
        .expect("no local storage exists")
        .expect("failed to get local storage")
        .clear()
        .expect("failed to clear local storage");

    LOCAL_STORAGE.with(|ls| {
        *ls.borrow_mut() = None;
    });

    num_pages_allocated_set(0);
}

// Use 1MB chunks for efficient storage
pub const PERSISTENT_STORAGE_PAGE_SIZE: u64 = 1024 * 1024;

// Storage functions that match the interface expected by LedgerMap

fn get_chunk_key(chunk_index: u64) -> String {
    format!("ledger_map_chunk_{}", chunk_index)
}

fn get_chunk_index_and_offset(offset: u64) -> (u64, usize) {
    let chunk_index = offset / PERSISTENT_STORAGE_PAGE_SIZE;
    let chunk_offset = (offset % PERSISTENT_STORAGE_PAGE_SIZE) as usize;
    (chunk_index, chunk_offset)
}

fn encode_bytes(bytes: &[u8]) -> String {
    // Use base64 encoding since LocalStorage only supports strings
    BASE64.encode(bytes)
}

fn decode_bytes(data: &str) -> Vec<u8> {
    // Decode base64 string back to bytes
    BASE64.decode(data).unwrap_or_default()
}

pub async fn persistent_storage_read(offset: u64, buf: &mut [u8]) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }

    let storage_size = persistent_storage_size_bytes().await;
    if offset >= storage_size {
        // Fill buffer with zeros if reading beyond storage size
        buf.fill(0);
        return Ok(());
    }

    LOCAL_STORAGE.with(|storage| {
        if let Some(storage) = &*storage.borrow() {
            let (mut chunk_index, chunk_offset) = get_chunk_index_and_offset(offset);
            let mut bytes_read = 0;

            // Read data from chunks until buffer is full or no more data
            while bytes_read < buf.len() {
                let chunk_key = get_chunk_key(chunk_index);

                match storage.get_item(&chunk_key) {
                    Ok(Some(data)) => {
                        let chunk_data = decode_bytes(&data);
                        if chunk_data.is_empty() {
                            // Fill remaining buffer with zeros for empty chunks
                            buf[bytes_read..].fill(0);
                            break;
                        }

                        // Calculate read positions
                        let chunk_start = if bytes_read == 0 { chunk_offset } else { 0 };
                        let remaining = buf.len() - bytes_read;
                        let chunk_available = chunk_data.len().saturating_sub(chunk_start);
                        let copy_size = remaining.min(chunk_available);

                        if copy_size == 0 {
                            // Fill remaining buffer with zeros if no more data to copy
                            buf[bytes_read..].fill(0);
                            break;
                        }

                        // Copy data from this chunk
                        buf[bytes_read..bytes_read + copy_size]
                            .copy_from_slice(&chunk_data[chunk_start..chunk_start + copy_size]);

                        bytes_read += copy_size;
                        chunk_index += 1;
                    }
                    Ok(None) | Err(_) => {
                        // Fill remaining buffer with zeros if chunk doesn't exist
                        buf[bytes_read..].fill(0);
                        break;
                    }
                }
            }

            // Fill any remaining buffer space with zeros
            if bytes_read < buf.len() {
                buf[bytes_read..].fill(0);
            }
        }
        Ok(())
    })
}

pub async fn persistent_storage_write(offset: u64, buf: &[u8]) {
    init_storage().await;
    let size_bytes_prev = persistent_storage_size_bytes().await;
    let size_bytes_expected = offset + buf.len() as u64;
    if size_bytes_expected > size_bytes_prev {
        persistent_storage_grow(
            (size_bytes_expected - size_bytes_prev) / PERSISTENT_STORAGE_PAGE_SIZE + 1,
        )
        .await
        .unwrap();
    }
    LOCAL_STORAGE.with(|storage| {
        if let Some(storage) = &*storage.borrow() {
            let (mut chunk_index, chunk_offset) = get_chunk_index_and_offset(offset);
            let mut bytes_written = 0;

            while bytes_written < buf.len() {
                let chunk_key = get_chunk_key(chunk_index);

                // Read existing chunk or create new one
                let mut chunk_data = if let Ok(Some(data)) = storage.get_item(&chunk_key) {
                    decode_bytes(&data)
                } else {
                    Vec::new()
                };

                // Calculate write positions
                let chunk_start = if bytes_written == 0 { chunk_offset } else { 0 };
                let remaining = buf.len() - bytes_written;
                let space_in_chunk = PERSISTENT_STORAGE_PAGE_SIZE as usize - chunk_start;
                let copy_size = remaining.min(space_in_chunk);

                // Ensure chunk is large enough
                if chunk_start + copy_size > chunk_data.len() {
                    chunk_data.resize(chunk_start + copy_size, 0);
                }

                // Copy data to this chunk
                chunk_data[chunk_start..chunk_start + copy_size]
                    .copy_from_slice(&buf[bytes_written..bytes_written + copy_size]);

                // Save chunk
                storage
                    .set_item(&chunk_key, &encode_bytes(&chunk_data))
                    .unwrap_or_else(|e| panic!("Failed to write to storage: {:?}", e));

                bytes_written += copy_size;
                chunk_index += 1;
            }
        } else {
            panic!("Storage not initialized");
        }
    })
}

/// LedgerMap first calls this function to determine the size of the persistent storage
pub async fn persistent_storage_size_bytes() -> u64 {
    init_storage().await;
    num_pages_allocated_get() * PERSISTENT_STORAGE_PAGE_SIZE
}

/// Grows the persistent storage by the specified number of pages
/// Returns the *previous* number of pages
pub async fn persistent_storage_grow(additional_pages: u64) -> Result<u64, String> {
    let num_pages_prev = num_pages_allocated_get();
    LOCAL_STORAGE.with(|storage| {
        if let Some(storage) = &*storage.borrow() {
            for i in 0..additional_pages {
                let chunk_key = get_chunk_key(num_pages_prev + i);
                storage
                    .set_item(&chunk_key, &"")
                    .unwrap_or_else(|e| panic!("Failed to write to storage: {:?}", e));
                num_pages_allocated_inc();
            }
        } else {
            warn!("Storage not initialized");
        }
    });

    Ok(num_pages_prev)
}

pub fn set_backing_file(_path: Option<std::path::PathBuf>) -> Result<(), String> {
    Ok(()) // No-op for browser implementation
}

pub fn get_backing_file_path() -> Option<std::path::PathBuf> {
    None // No backing file in browser implementation
}

pub fn get_timestamp_nanos() -> u64 {
    (js_sys::Date::now() * 1_000_000.0) as u64
}
