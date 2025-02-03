export class WasmLedgerMap {
    private labels: string[];
    private data: Map<string, Uint8Array>;
    private blocksCount: number;

    constructor(labels?: string[]) {
        this.labels = labels || [];
        this.data = new Map();
        this.blocksCount = 0;
    }

    init(): void {
        this.data = new Map();
        this.blocksCount = 0;
    }

    upsert(label: string, key: Uint8Array, value: Uint8Array): void {
        this.data.set(`${label}:${new TextDecoder().decode(key)}`, value);
    }

    get(label: string, key: Uint8Array): Uint8Array {
        const value = this.data.get(`${label}:${new TextDecoder().decode(key)}`);
        if (value === undefined) {
            throw new Error(`No value found for label '${label}' and key '${new TextDecoder().decode(key)}'`);
        }
        return value;
    }

    delete(label: string, key: Uint8Array): void {
        this.data.delete(`${label}:${new TextDecoder().decode(key)}`);
    }

    commit_block(): void {
        this.blocksCount++;
    }

    get_blocks_count(): number {
        return this.blocksCount;
    }

    get_latest_block_hash(): Uint8Array { return new Uint8Array([1, 2, 3]); }
    refresh(): void { }
    get_next_block_start_pos(): number { return 100; }
    get_data_partition_start(): number { return 0; }
    get_persistent_storage_size(): number { return 1024 * 1024; }
    read_persistent_storage(_offset: number, _buffer: Uint8Array): void { }
    write_persistent_storage(_offset: number, _data: Uint8Array): void { }
}

export default function init(m: WasmLedgerMap): void {
    if (m) { m.init() };
}
