use crate::platform_specific_wasm32_browser::{
    clear_storage, init_storage, PERSISTENT_STORAGE_PAGE_SIZE,
};
use crate::wasm::WasmLedgerMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

// Configure to run in browser
wasm_bindgen_test_configure!(run_in_browser);

// Helper function to create a test ledger with some data
fn create_test_ledger() -> WasmLedgerMap {
    clear_storage();
    init_storage();
    let mut ledger = WasmLedgerMap::new(None).expect("Failed to create WasmLedgerMap");

    // Add some test data
    let test_data = b"test data for storage operations";
    ledger.write_persistent_storage(0, test_data).unwrap();
    ledger.commit_block().unwrap();

    ledger
}

#[wasm_bindgen_test]
fn test_storage_position_methods() {
    let ledger = create_test_ledger();

    // Test next block start position
    let next_pos = ledger.get_next_block_start_pos();
    assert!(next_pos > 0);

    // Test data partition start
    let partition_start = ledger.get_data_partition_start();
    assert!(partition_start >= 0);

    // Test persistent storage size
    let storage_size = ledger.get_persistent_storage_size();
    assert!(storage_size > 0);
}

#[wasm_bindgen_test]
fn test_persistent_storage_operations() {
    let mut ledger = create_test_ledger();

    // Test writing to storage
    let test_data = b"test data for storage";
    ledger.write_persistent_storage(0, test_data).unwrap();

    // Test reading from storage
    let mut read_buffer = vec![0u8; test_data.len()];
    ledger.read_persistent_storage(0, &mut read_buffer).unwrap();
    assert_eq!(&read_buffer, test_data);

    // Test reading beyond written data
    let mut empty_buffer = vec![0u8; 10];
    ledger
        .read_persistent_storage(test_data.len() as u64, &mut empty_buffer)
        .unwrap();
    assert_eq!(&empty_buffer, &vec![0u8; 10]);
}

#[wasm_bindgen_test]
fn test_cross_chunk_operations() {
    let mut ledger = create_test_ledger();

    // Create test data that spans multiple chunks
    let chunk_size = PERSISTENT_STORAGE_PAGE_SIZE as usize;
    let data_size = chunk_size + 1024; // Slightly more than one chunk
    let test_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

    // Write data spanning chunks
    ledger.write_persistent_storage(0, &test_data).unwrap();

    // Read back in chunks
    let mut read_buffer = vec![0u8; chunk_size];
    ledger.read_persistent_storage(0, &mut read_buffer).unwrap();
    assert_eq!(&read_buffer, &test_data[..chunk_size]);

    let mut read_buffer2 = vec![0u8; 1024];
    ledger
        .read_persistent_storage(chunk_size as u64, &mut read_buffer2)
        .unwrap();
    assert_eq!(&read_buffer2, &test_data[chunk_size..]);
}

#[wasm_bindgen_test]
async fn test_canister_data_fetch() {
    use candid::{Decode, Encode};
    use ic_agent::{Agent, Identity};

    // Initialize test ledger
    let mut ledger = create_test_ledger();

    // Create test data
    let test_data = b"test data for canister fetch";
    ledger.write_persistent_storage(0, test_data).unwrap();
    ledger.commit_block().unwrap();

    // Create agent for canister interaction
    let agent = Agent::builder()
        .with_url("https://icp-api.io")
        .build()
        .expect("Failed to create agent");

    // Get current position
    let current_pos = ledger.get_next_block_start_pos();
    let cursor = format!("position={}", current_pos);

    // Prepare bytes before if needed
    let bytes_before = if current_pos > 1024 {
        let mut buf = vec![0u8; 1024];
        ledger
            .read_persistent_storage(current_pos - 1024, &mut buf)
            .unwrap();
        Some(buf)
    } else {
        None
    };

    // Call canister data_fetch method
    let response = agent
        .query(&"ggi4a-wyaaa-aaaai-actqq-cai".parse().unwrap())
        .method_name("data_fetch")
        .with_arg(Encode!(&Some(cursor), &bytes_before).unwrap())
        .call()
        .await
        .expect("Failed to call data_fetch");

    // Decode response
    let (new_cursor, data): (String, Vec<u8>) = Decode!(response.as_slice()).unwrap();

    // Write received data to storage
    if !data.is_empty() {
        ledger.write_persistent_storage(current_pos, &data).unwrap();
        ledger.refresh().unwrap();
    }

    // Verify data was written correctly
    let mut read_buffer = vec![0u8; data.len()];
    ledger
        .read_persistent_storage(current_pos, &mut read_buffer)
        .unwrap();
    assert_eq!(&read_buffer, &data);
}

#[wasm_bindgen_test]
async fn test_canister_data_fetch_with_verification() {
    use candid::{Decode, Encode};
    use ic_agent::{Agent, Identity};

    // Initialize test ledger
    let mut ledger = create_test_ledger();

    // Create test data with known pattern
    let test_data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    ledger.write_persistent_storage(0, &test_data).unwrap();
    ledger.commit_block().unwrap();

    // Create agent for canister interaction
    let agent = Agent::builder()
        .with_url("https://icp-api.io")
        .build()
        .expect("Failed to create agent");

    // Get current position
    let current_pos = ledger.get_next_block_start_pos();
    let cursor = format!("position={}", current_pos);

    // Call canister data_fetch method
    let response = agent
        .query(&"ggi4a-wyaaa-aaaai-actqq-cai".parse().unwrap())
        .method_name("data_fetch")
        .with_arg(Encode!(&Some(cursor), &None::<Vec<u8>>).unwrap())
        .call()
        .await
        .expect("Failed to call data_fetch");

    // Decode response
    let (new_cursor, data): (String, Vec<u8>) = Decode!(response.as_slice()).unwrap();

    // Write received data to storage
    if !data.is_empty() {
        ledger.write_persistent_storage(current_pos, &data).unwrap();
        ledger.refresh().unwrap();

        // Verify data integrity
        let mut read_buffer = vec![0u8; data.len()];
        ledger
            .read_persistent_storage(current_pos, &mut read_buffer)
            .unwrap();
        assert_eq!(&read_buffer, &data);

        // Parse cursor to verify position
        let new_pos = new_cursor
            .split('&')
            .find(|s| s.starts_with("position="))
            .and_then(|s| s.split('=').nth(1))
            .and_then(|s| s.parse::<u64>().ok())
            .expect("Failed to parse cursor position");

        assert!(
            new_pos >= current_pos,
            "New position should be >= current position"
        );
    }
}
