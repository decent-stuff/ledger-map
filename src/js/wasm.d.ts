export class WasmLedgerMap {
    constructor(labels?: string[]);
    upsert(label: string, key: Uint8Array, value: Uint8Array): void;
    get(label: string, key: Uint8Array): Uint8Array;
    delete(label: string, key: Uint8Array): void;
    commit_block(): void;
    get_blocks_count(): number;
    get_latest_block_hash(): Uint8Array;
    refresh(): void;
    get_next_block_start_pos(): number;
    get_data_partition_start(): number;
    get_persistent_storage_size(): number;
    read_persistent_storage(offset: number, buffer: Uint8Array): void;
    write_persistent_storage(offset: number, data: Uint8Array): void;
}

export default function init(): Promise<void>;
