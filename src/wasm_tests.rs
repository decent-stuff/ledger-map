use crate::{LedgerMap, WasmLedgerMap};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// Helper function to create a test ledger with some data
fn create_test_ledger() -> WasmLedgerMap {
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
    let mut ledger = LedgerMap::new(None).expect("Failed to create LedgerMap");

    // Test upsert and get
    let key = b"test_key".to_vec();
    let value = b"test_value".to_vec();
    ledger
        .upsert("test_label", key.clone(), value.clone())
        .unwrap();
    ledger.commit_block().unwrap();

    let retrieved = ledger.get("test_label", &key).unwrap();
    assert_eq!(retrieved, value);

    // Test delete
    ledger.delete("test_label", key.clone()).unwrap();
    ledger.commit_block().unwrap();

    assert!(ledger.get("test_label", &key).is_err());
}

#[wasm_bindgen_test]
fn test_multiple_labels() {
    let mut ledger = LedgerMap::new(None).expect("Failed to create LedgerMap");

    let key1 = b"key1".to_vec();
    let value1 = b"value1".to_vec();
    let key2 = b"key2".to_vec();
    let value2 = b"value2".to_vec();

    ledger
        .upsert("label1", key1.clone(), value1.clone())
        .unwrap();
    ledger
        .upsert("label2", key2.clone(), value2.clone())
        .unwrap();
    ledger.commit_block().unwrap();

    assert_eq!(ledger.get("label1", &key1).unwrap(), value1);
    assert_eq!(ledger.get("label2", &key2).unwrap(), value2);
}

#[wasm_bindgen_test]
fn test_block_operations() {
    let mut ledger = LedgerMap::new(None).expect("Failed to create LedgerMap");

    let key = b"block_test_key".to_vec();
    let value1 = b"value1".to_vec();
    let value2 = b"value2".to_vec();

    // First block
    ledger.begin_block().unwrap();
    ledger.upsert("test", key.clone(), value1.clone()).unwrap();
    ledger.commit_block().unwrap();

    assert_eq!(ledger.get_blocks_count(), 1);
    assert_eq!(ledger.get("test", &key).unwrap(), value1);

    // Second block
    ledger.begin_block().unwrap();
    ledger.upsert("test", key.clone(), value2.clone()).unwrap();
    ledger.commit_block().unwrap();

    assert_eq!(ledger.get_blocks_count(), 2);
    assert_eq!(ledger.get("test", &key).unwrap(), value2);

    // Verify block hash exists
    let hash = ledger.get_latest_block_hash();
    assert!(!hash.is_empty());
}

#[wasm_bindgen_test]
fn test_persistence() {
    let mut ledger = LedgerMap::new(None).expect("Failed to create LedgerMap");

    let key = b"persist_key".to_vec();
    let value = b"persist_value".to_vec();

    ledger.upsert("test", key.clone(), value.clone()).unwrap();
    ledger.commit_block().unwrap();

    // Refresh ledger (simulates reload)
    ledger.refresh_ledger().unwrap();

    // Verify data persisted
    assert_eq!(ledger.get("test", &key).unwrap(), value);
}

#[wasm_bindgen_test]
fn test_wasm_block_entries() {
    let ledger = create_test_ledger();

    // Test getting all entries
    let entries = ledger.get_block_entries(None);
    assert_eq!(entries.length(), 3); // Should have all 3 entries

    // Test getting entries for specific label
    let label1_entries = ledger.get_block_entries(Some("label1".to_string()));
    assert_eq!(label1_entries.length(), 2); // Should have 2 entries for label1

    let label2_entries = ledger.get_block_entries(Some("label2".to_string()));
    assert_eq!(label2_entries.length(), 1); // Should have 1 entry for label2

    // Verify entry contents
    let entry = label2_entries.get(0).dyn_into::<JsValue>().unwrap();
    let entry: JsValue = entry.into();
    assert!(entry.has_type::<crate::WasmLedgerEntry>());
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
