import init, { WasmLedgerMap } from '../../dist/wasm';
import { createLedgerCanister, LedgerCursor, cursorFromData } from './canister';

export interface LedgerMapOptions {
    labels?: string[];
    canisterId?: string;
}

export class LedgerMap {
    private instance: WasmLedgerMap | null = null;

    /**
     * Initialize the LedgerMap instance
     * @param options Configuration options
     */
    async initialize(options: LedgerMapOptions = {}): Promise<void> {
        await init();
        this.instance = new WasmLedgerMap(options.labels);
    }

    /**
     * Store or update a value
     * @param label The label for the entry
     * @param key The key as Uint8Array
     * @param value The value as Uint8Array
     */
    upsert(label: string, key: Uint8Array, value: Uint8Array): void {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        this.instance.upsert(label, key, value);
    }

    /**
     * Retrieve a value
     * @param label The label for the entry
     * @param key The key as Uint8Array
     * @returns The value as Uint8Array
     */
    get(label: string, key: Uint8Array): Uint8Array {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        return this.instance.get(label, key);
    }

    /**
     * Delete an entry
     * @param label The label for the entry
     * @param key The key as Uint8Array
     */
    delete(label: string, key: Uint8Array): void {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        this.instance.delete(label, key);
    }

    /**
     * Commit the current block of operations
     */
    commitBlock(): void {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        this.instance.commit_block();
    }

    /**
     * Get the total number of blocks
     */
    getBlocksCount(): number {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        return this.instance.get_blocks_count();
    }

    /**
     * Get the hash of the latest block
     */
    getLatestBlockHash(): Uint8Array {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        return this.instance.get_latest_block_hash();
    }

    /**
     * Reload the ledger from storage
     */
    refreshLedger(): void {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }
        this.instance.refresh();
    }

    /**
     * Fetch data from the ledger canister
     * @returns Promise that resolves when the data has been fetched and processed
     */
    async fetchFromCanister(): Promise<void> {
        if (!this.instance) {
            throw new Error('LedgerMap not initialized. Call initialize() first.');
        }

        const canister = await createLedgerCanister();

        // Get the current position in the local ledger
        const nextBlockStartPos = this.instance.get_next_block_start_pos();
        const dataPartitionStart = this.instance.get_data_partition_start();
        const storageSize = this.instance.get_persistent_storage_size();

        // Create a cursor for the current position
        const cursor = cursorFromData(
            BigInt(dataPartitionStart),
            BigInt(storageSize),
            BigInt(nextBlockStartPos),
            BigInt(nextBlockStartPos)
        );

        // Get some bytes before the current position for verification
        let bytesBefore: Uint8Array | undefined;
        const BYTES_BEFORE_LEN = 1024; // 1KB of data before
        if (cursor.position > BigInt(BYTES_BEFORE_LEN)) {
            bytesBefore = new Uint8Array(BYTES_BEFORE_LEN);
            // Read bytes before the current position
            const beforePos = cursor.position - BigInt(BYTES_BEFORE_LEN);
            this.instance.read_persistent_storage(beforePos, bytesBefore);
        }

        // Fetch data from the canister
        const result = await canister.data_fetch(
            cursor.toRequestString(),
            bytesBefore
        );

        if (result.Err) {
            throw new Error(`Failed to fetch data: ${result.Err}`);
        }

        if (!result.Ok) {
            throw new Error('No data received from canister');
        }

        const [cursorStr, data] = result.Ok;
        const remoteCursor = LedgerCursor.fromString(cursorStr);

        // Verify the remote cursor position is not behind our local position
        if (remoteCursor.position < cursor.position) {
            throw new Error(
                `Ledger canister has less data than available locally ${remoteCursor.position} < ${cursor.position} bytes`
            );
        }

        // Write the new data to persistent storage
        if (data.length > 0) {
            this.instance.write_persistent_storage(remoteCursor.position, data);
            this.refreshLedger();
        }
    }
}

export default LedgerMap;
