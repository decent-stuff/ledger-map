# LedgerMap

## Overview

LedgerMap provides a persistent key-value storage system with blockchain-like properties, implemented in Rust and compiled to WebAssembly.

The primary feature of this library is its ability to store data in an append-only fashion, effectively forming a ledger. Additionally, it supports data integrity checks using SHA-256 checksums for ledger entries.

This library is designed for use in smart contract environments, and compiles to `wasm32`, `x86_64`, and `aarch64` architectures.
The WebAssembly builds are intended for browser usage.

## Features

- Persistent key-value storage using browser's localStorage
- Data integrity protected with SHA-256 checksums
- Block-based storage with chain hashing
- Multiple label support for organizing data
- TypeScript support

## Installation

### Rust

Add LedgerMap to your `Cargo.toml`:

```toml
[dependencies]
ledger-map = "0.4.0"
```

### Node.js and Typescript

```bash
npm install ledger-map-wasm
```

## Usage

### Rust

Here is a basic example to get you started:

```rust
 use ledger_map::{LedgerMap};
 use env_logger::Env;

fn main() {
    // Optional: set log level to info by default
    env_logger::try_init_from_env(Env::default().default_filter_or("info")).unwrap();

    // Optional: Use custom file path for the persistent storage
    // let ledger_path = Some(std::path::PathBuf::from("/tmp/ledger_map/test_data.bin"));
    let ledger_path = None;

    // Create a new LedgerMap instance, and index all labels for quick search
    let mut ledger_map = LedgerMap::new_with_path(None, ledger_path).expect("Failed to create LedgerMap");

    // Insert a few new entries, each with a separate label
    ledger_map.upsert("Label1", b"key1".to_vec(), b"value1".to_vec()).unwrap();
    ledger_map.upsert("Label2", b"key2".to_vec(), b"value2".to_vec()).unwrap();
    ledger_map.commit_block().unwrap();

    // Retrieve all entries
    let entries = ledger_map.iter(None).collect::<Vec<_>>();
    println!("All entries: {:?}", entries);

    // Iterate only over entries with the Label1 label
    let entries = ledger_map.iter(Some("Label1")).collect::<Vec<_>>();
    println!("Label1 entries: {:?}", entries);

    // Iterate only over entries with the Label2 label
    let entries = ledger_map.iter(Some("Label2")).collect::<Vec<_>>();
    println!("Label2 entries: {:?}", entries);

    // Delete an entry from Label1
    ledger_map.delete("Label1", b"key1".to_vec()).unwrap();
    ledger_map.commit_block().unwrap();

    // Label1 entries are now empty
    assert_eq!(ledger_map.iter(Some("Label1")).count(), 0);

    // Label2 entries still exist
    assert_eq!(ledger_map.iter(Some("Label2")).count(), 1);
}
```

### Typescript

```typescript
import { LedgerMapWrapper } from "ledger-map-wasm";

async function example() {
  // Initialize
  const ledger = new LedgerMapWrapper();
  await ledger.initialize();

  // Store data
  const key = new Uint8Array([1, 2, 3]);
  const value = new Uint8Array([4, 5, 6]);

  ledger.beginBlock();
  ledger.upsert("my-label", key, value);
  ledger.commitBlock();

  // Retrieve data
  const retrieved = ledger.get("my-label", key);
  console.log(retrieved); // Uint8Array [4, 5, 6]

  // Delete data
  ledger.beginBlock();
  ledger.delete("my-label", key);
  ledger.commitBlock();
}
```

Typescript tests: there are tests that run with npm (node.js) and tests that require a browser.

The first class of tests can be ran with

```bash
npm run test
```

The second class of tests have to be run with

```bash
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' wasm-pack test --chrome --features browser
```

## API

### `LedgerMapWrapper`

Main class for interacting with the ledger.

#### Methods

- `async initialize(labels?: string[]): Promise<void>`
  Initialize the ledger. Optionally specify labels to index.

- `upsert(label: string, key: Uint8Array, value: Uint8Array): void`
  Store or update a value.

- `get(label: string, key: Uint8Array): Uint8Array`
  Retrieve a value.

- `delete(label: string, key: Uint8Array): void`
  Delete a value.

- `beginBlock(): void`
  Start a new block of operations.

- `commitBlock(): void`
  Commit the current block of operations.

- `getBlocksCount(): number`
  Get the total number of blocks.

- `getLatestBlockHash(): Uint8Array`
  Get the hash of the latest block.

- `refreshLedger(): void`
  Reload the ledger from storage.

## Development

### Building

```bash
npm run build
```

### Testing

```bash
npm test
```

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Contributions are welcome.
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
