use anyhow::Result;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::Storage;

thread_local! {
    static LOCAL_STORAGE: RefCell<Option<Storage>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn init_storage() {
    let window = web_sys::window().expect("no global window exists");
    let storage = window
        .local_storage()
        .expect("no local storage exists")
        .expect("failed to get local storage");

    LOCAL_STORAGE.with(|ls| {
        *ls.borrow_mut() = Some(storage);
    });
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

pub const PERSISTENT_STORAGE_PAGE_SIZE: u64 = 64 * 1024;

// Storage functions that match the interface expected by LedgerMap
pub fn persistent_storage_read(offset: u64, buf: &mut [u8]) -> Result<(), String> {
    LOCAL_STORAGE.with(|storage| {
        if let Some(storage) = &*storage.borrow() {
            if let Ok(Some(data)) = storage.get_item(&format!("ledger_map_{}", offset)) {
                let bytes = hex::decode(data).map_err(|e| e.to_string())?;
                let len = buf.len().min(bytes.len());
                buf[..len].copy_from_slice(&bytes[..len]);
            }
        }
        Ok(())
    })
}

pub fn persistent_storage_write(offset: u64, buf: &[u8]) {
    LOCAL_STORAGE.with(|storage| {
        if let Some(storage) = &*storage.borrow() {
            let _ = storage.set_item(&format!("ledger_map_{}", offset), &hex::encode(buf));
        }
    });
}

pub fn persistent_storage_size_bytes() -> u64 {
    LOCAL_STORAGE.with(|storage| {
        if let Some(storage) = &*storage.borrow() {
            storage.length().unwrap_or(0) as u64 * 2 // Approximate since we store hex encoded
        } else {
            0
        }
    })
}

pub fn persistent_storage_grow(_additional_pages: u64) -> Result<u64, String> {
    // No-op for browser implementation
    Ok(persistent_storage_size_bytes())
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
