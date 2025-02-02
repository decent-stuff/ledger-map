use crate::platform_specific_wasm32_browser::{
    clear_storage, init_storage, PERSISTENT_STORAGE_PAGE_SIZE,
};
use crate::wasm::WasmLedgerMap;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

// Configure to run in both Node.js and browser
wasm_bindgen_test_configure!(run_in_browser);

// Helper function to create a test ledger with some data
fn create_test_ledger() -> WasmLedgerMap {
    clear_storage();
    init_storage();
    let mut ledger = WasmLedgerMap::new(None).expect("Failed to create WasmLedgerMap");

    // Add some test entries
    ledger.upsert("label1", b"key1", b"value1").unwrap();
    ledger.upsert("label2", b"key2", b"value2").unwrap();
    ledger.commit_block().unwrap();

    ledger.upsert("label1", b"key3", b"value3").unwrap();
    ledger.commit_block().unwrap();

    ledger
}

#[wasm_bindgen_test]
fn test_basic_operations() {
    clear_storage();
    init_storage();
    let mut ledger = WasmLedgerMap::new(None).expect("Failed to create WasmLedgerMap");

    // Test upsert and get
    let key = b"test_key".to_vec();
    let value = b"test_value".to_vec();
    ledger.upsert("test_label", &key, &value).unwrap();

    ledger.commit_block().unwrap();

    let retrieved = ledger.get("test_label", &key).unwrap();
    assert_eq!(retrieved, value);

    // Test delete
    ledger.delete("test_label", &key).unwrap();
    ledger.commit_block().unwrap();

    let get_result = ledger.get("test_label", &key);
    assert!(get_result.is_err());
}

#[wasm_bindgen_test]
fn test_multiple_labels() {
    clear_storage();
    init_storage();
    let mut ledger = WasmLedgerMap::new(None).expect("Failed to create WasmLedgerMap");

    let key1 = b"key1".to_vec();
    let value1 = b"value1".to_vec();
    let key2 = b"key2".to_vec();
    let value2 = b"value2".to_vec();

    ledger.upsert("label1", &key1, &value1).unwrap();
    ledger.upsert("label2", &key2, &value2).unwrap();

    ledger.commit_block().unwrap();

    let get1 = ledger.get("label1", &key1);
    assert_eq!(get1.unwrap(), value1);

    let get2 = ledger.get("label2", &key2);
    assert_eq!(get2.unwrap(), value2);
}

#[wasm_bindgen_test]
fn test_block_operations() {
    clear_storage();
    init_storage();
    let mut ledger = WasmLedgerMap::new(None).expect("Failed to create WasmLedgerMap");

    let key = b"block_test_key".to_vec();
    let value1 = b"value1".to_vec();
    let value2 = b"value2".to_vec();

    // First block
    ledger.upsert("test", &key, &value1).unwrap();
    ledger.commit_block().unwrap();

    let blocks_count = ledger.get_blocks_count();
    assert_eq!(blocks_count, 1);

    let get1 = ledger.get("test", &key);
    assert_eq!(get1.unwrap(), value1);

    // Second block
    ledger.upsert("test", &key, &value2).unwrap();
    ledger.commit_block().unwrap();

    let blocks_count = ledger.get_blocks_count();
    assert_eq!(blocks_count, 2);

    let get2 = ledger.get("test", &key);
    assert_eq!(get2.unwrap(), value2);

    // Verify block hash exists
    let hash = ledger.get_latest_block_hash();
    assert!(hash.length() > 0);
}

#[wasm_bindgen_test]
fn test_persistence() {
    clear_storage();
    init_storage();
    let mut ledger = WasmLedgerMap::new(None).expect("Failed to create WasmLedgerMap");

    let key = b"persist_key".to_vec();
    let value = b"persist_value".to_vec();

    ledger.upsert("test", &key, &value).unwrap();
    ledger.commit_block().unwrap();

    // Refresh ledger (simulates reload)
    ledger.refresh().unwrap();

    // Verify data persisted
    let get_result = ledger.get("test", &key);
    assert_eq!(get_result.unwrap(), value);
}

#[wasm_bindgen_test]
fn test_wasm_block_entries() {
    let ledger = create_test_ledger();

    // Test getting all entries
    let entries = ledger.get_block_entries(None);
    info!("Entries: {:#?}", entries);
    assert_eq!(entries.length(), 3); // Should have all 3 entries

    // Test getting entries for specific label
    let label1_entries = ledger.get_block_entries(Some("label1".to_string()));
    assert_eq!(label1_entries.length(), 2); // Should have 2 entries for label1

    let label2_entries = ledger.get_block_entries(Some("label2".to_string()));
    assert_eq!(label2_entries.length(), 1); // Should have 1 entry for label2

    // Verify entry contents
    let entry = label2_entries.get(0).dyn_into::<JsValue>().unwrap();

    // Verify we can access WasmLedgerMapEntry properties
    let entry_obj = js_sys::Object::from(entry);
    assert!(js_sys::Reflect::has(&entry_obj, &"label".into()).unwrap());
    assert!(js_sys::Reflect::has(&entry_obj, &"key".into()).unwrap());
    assert!(js_sys::Reflect::has(&entry_obj, &"value".into()).unwrap());
    assert!(js_sys::Reflect::has(&entry_obj, &"operation".into()).unwrap());
}

#[wasm_bindgen_test]
fn test_wasm_next_block_entries() {
    let mut ledger = create_test_ledger();

    // Add entries to next block
    ledger.upsert("label3", b"key4", b"value4").unwrap();
    ledger.upsert("label3", b"key5", b"value5").unwrap();

    // Test next block entries count
    assert_eq!(ledger.get_next_block_entries_count(None), 2);
    assert_eq!(
        ledger.get_next_block_entries_count(Some("label3".to_string())),
        2
    );

    // Test getting next block entries
    let next_entries = ledger.get_next_block_entries(None);
    assert_eq!(next_entries.length(), 2);

    // Verify entries are cleared after commit
    ledger.commit_block().unwrap();
    assert_eq!(ledger.get_next_block_entries_count(None), 0);
}

#[wasm_bindgen_test]
fn test_wasm_block_metadata() {
    let ledger = create_test_ledger();

    // Test block count
    assert_eq!(ledger.get_blocks_count(), 2);

    // Test latest block hash
    let hash = ledger.get_latest_block_hash();
    assert!(hash.length() > 0);

    // Test latest block timestamp
    let timestamp = ledger.get_latest_block_timestamp();
    assert!(timestamp > 0);
}

// Tests for chunked storage implementation
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
mod storage_tests {
    use super::*;
    use crate::platform_specific_wasm32_browser::{
        clear_storage, init_storage, persistent_storage_read, persistent_storage_size_bytes,
        persistent_storage_write,
    };

    #[wasm_bindgen_test]
    fn test_basic_read_write() {
        clear_storage();
        init_storage();

        let data = b"Hello, World!";
        persistent_storage_write(0, data);

        let mut buf = vec![0; data.len()];
        persistent_storage_read(0, &mut buf).unwrap();
        assert_eq!(&buf, data);

        // Verify size
        assert_eq!(
            persistent_storage_size_bytes(),
            PERSISTENT_STORAGE_PAGE_SIZE
        );
    }

    #[wasm_bindgen_test]
    fn test_cross_chunk_read_write() {
        clear_storage();
        init_storage();

        // Create test data that spans multiple chunks / pages
        let chunk_size = PERSISTENT_STORAGE_PAGE_SIZE; // 1MB
        let data_size = chunk_size as usize * 2 + 512 * 1024; // 2.5MB to ensure crossing chunks
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        // Write the data
        persistent_storage_write(0, &data);

        // Verify total size
        assert_eq!(
            persistent_storage_size_bytes(),
            3 * PERSISTENT_STORAGE_PAGE_SIZE
        );

        // Test cases for cross-chunk reads
        let test_cases = vec![
            // Read exactly at chunk boundary
            (chunk_size, 1024),
            // Read spanning chunk boundary
            (chunk_size - 512, 1024),
            // Read spanning multiple chunks
            (chunk_size - 512, chunk_size as usize),
            // Read from middle of one chunk to middle of another
            (chunk_size + 256 * 1024, 512 * 1024),
        ];

        for (offset, size) in test_cases {
            let mut buf = vec![0; size];
            persistent_storage_read(offset, &mut buf).unwrap();

            // Calculate expected data for this range
            let start = offset as usize;
            let end = start + size;
            let expected: Vec<u8> = (start..end).map(|i| (i % 256) as u8).collect();

            assert_eq!(
                buf, expected,
                "Failed at offset {} with size {}",
                offset, size
            );
        }
    }

    #[wasm_bindgen_test]
    fn test_offset_alignment_edge_cases() {
        clear_storage();
        init_storage();

        let chunk_size = 1024 * 1024;

        // Write at chunk boundary
        let data1 = b"boundary";
        persistent_storage_write(chunk_size, data1);

        // Write just before chunk boundary
        let data2 = b"before_boundary";
        persistent_storage_write(chunk_size - 5, data2);

        // Write spanning chunk boundary
        let data3 = b"spanning_boundary";
        persistent_storage_write(chunk_size - 5, data3);

        // Verify all writes
        let mut buf = vec![0; 20];
        persistent_storage_read(chunk_size - 5, &mut buf).unwrap();
        assert_eq!(&buf[..data3.len()], data3);
    }

    #[wasm_bindgen_test]
    fn test_zero_length_operations() {
        clear_storage();
        init_storage();

        // Write empty data
        let empty: &[u8] = &[];
        persistent_storage_write(1000, empty);

        // Read zero bytes
        let mut buf = Vec::new();
        persistent_storage_read(1000, &mut buf).unwrap();
        assert_eq!(buf.len(), 0);
    }

    #[wasm_bindgen_test]
    fn test_read_beyond_data() {
        clear_storage();
        init_storage();

        let data = b"test data";
        persistent_storage_write(0, data);

        // Read beyond written data
        let mut buf = vec![0; 10];
        persistent_storage_read(data.len() as u64 + 100, &mut buf).unwrap();
        assert_eq!(&buf, &vec![0; 10]);

        // Read partially beyond data
        let mut buf2 = vec![0; 20];
        persistent_storage_read(5, &mut buf2).unwrap();
        assert_eq!(&buf2[..4], &data[5..]);
        assert_eq!(&buf2[4..], &vec![0; 16]);
    }

    #[wasm_bindgen_test]
    fn test_large_sparse_writes() {
        clear_storage();
        init_storage();

        let chunk_size = 1024 * 1024;

        // Write at the start
        let data1 = b"start";
        persistent_storage_write(0, data1);

        // Write at end of first chunk
        let data2 = b"end_chunk1";
        persistent_storage_write(chunk_size - data2.len() as u64, data2);

        // Write at start of second chunk
        let data3 = b"start_chunk2";
        persistent_storage_write(chunk_size, data3);

        // Verify all writes
        let mut buf1 = vec![0; data1.len()];
        persistent_storage_read(0, &mut buf1).unwrap();
        assert_eq!(&buf1, data1);

        let mut buf2 = vec![0; data2.len()];
        persistent_storage_read(chunk_size - data2.len() as u64, &mut buf2).unwrap();
        assert_eq!(&buf2, data2);

        let mut buf3 = vec![0; data3.len()];
        persistent_storage_read(chunk_size, &mut buf3).unwrap();
        assert_eq!(&buf3, data3);
    }

    #[wasm_bindgen_test]
    fn test_size_calculation() {
        clear_storage();
        init_storage();

        assert_eq!(persistent_storage_size_bytes(), 0);

        // Write some data
        let data = b"test data";
        persistent_storage_write(0, data);
        assert_eq!(
            persistent_storage_size_bytes(),
            PERSISTENT_STORAGE_PAGE_SIZE
        );

        // Write at a higher offset
        let offset = 1024 * 1024; // 1MB
        persistent_storage_write(offset, data);
        assert_eq!(
            persistent_storage_size_bytes(),
            2 * PERSISTENT_STORAGE_PAGE_SIZE
        );
    }
}
