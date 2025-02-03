import { LedgerMap } from '../index';
import { createLedgerCanister } from '../canister';
import { Actor } from '@dfinity/agent';

// Mock the wasm initialization
jest.mock('../../../dist/wasm', () => ({
    __esModule: true,
    default: jest.fn().mockResolvedValue(undefined),
    WasmLedgerMap: jest.fn().mockImplementation(() => ({
        upsert: jest.fn(),
        get: jest.fn(),
        delete: jest.fn(),
        commit_block: jest.fn(),
        get_blocks_count: jest.fn().mockReturnValue(0),
        get_latest_block_hash: jest.fn().mockReturnValue(new Uint8Array([1, 2, 3])),
        refresh: jest.fn(),
        get_next_block_start_pos: jest.fn().mockReturnValue(100),
        get_data_partition_start: jest.fn().mockReturnValue(0),
        get_persistent_storage_size: jest.fn().mockReturnValue(1024 * 1024),
        read_persistent_storage: jest.fn(),
        write_persistent_storage: jest.fn()
    }))
}));

// Mock the canister actor
jest.mock('@dfinity/agent', () => ({
    Actor: {
        createActor: jest.fn()
    },
    HttpAgent: jest.fn().mockImplementation(() => ({
        fetchRootKey: jest.fn().mockResolvedValue(undefined)
    }))
}));

describe('LedgerMap with canister integration', () => {
    let ledgerMap: LedgerMap;

    beforeEach(async () => {
        ledgerMap = new LedgerMap();
        await ledgerMap.initialize();
    });

    describe('fetchFromCanister', () => {
        it('fetches and processes data from canister successfully', async () => {
            // Mock successful canister response
            const mockActor = {
                data_fetch: jest.fn().mockResolvedValue({
                    Ok: [
                        'position=200&response_bytes=100&direction=forward&more=false',
                        new Uint8Array([1, 2, 3, 4])
                    ]
                })
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await expect(ledgerMap.fetchFromCanister()).resolves.not.toThrow();

            // Verify canister interaction
            expect(mockActor.data_fetch).toHaveBeenCalledWith(
                'position=100',
                undefined
            );
        });

        it('handles canister error response', async () => {
            // Mock error response from canister
            const mockActor = {
                data_fetch: jest.fn().mockResolvedValue({
                    Err: 'Failed to fetch data'
                })
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await expect(ledgerMap.fetchFromCanister()).rejects.toThrow(
                'Failed to fetch data'
            );
        });

        it('handles case when canister has less data', async () => {
            // Mock response where canister position is behind local position
            const mockActor = {
                data_fetch: jest.fn().mockResolvedValue({
                    Ok: [
                        'position=50&response_bytes=50&direction=forward&more=false',
                        new Uint8Array([1, 2, 3, 4])
                    ]
                })
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await expect(ledgerMap.fetchFromCanister()).rejects.toThrow(
                'Ledger canister has less data than available locally'
            );
        });

        it('handles empty data response', async () => {
            // Mock response with empty data
            const mockActor = {
                data_fetch: jest.fn().mockResolvedValue({
                    Ok: [
                        'position=200&response_bytes=0&direction=forward&more=false',
                        new Uint8Array([])
                    ]
                })
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await expect(ledgerMap.fetchFromCanister()).resolves.not.toThrow();
        });

        it('fetches with bytes before when position allows', async () => {
            // Mock the read_persistent_storage implementation
            const mockWasmInstance = {
                get_next_block_start_pos: jest.fn().mockReturnValue(2000),
                get_data_partition_start: jest.fn().mockReturnValue(0),
                get_persistent_storage_size: jest.fn().mockReturnValue(1024 * 1024),
                read_persistent_storage: jest.fn(),
                write_persistent_storage: jest.fn(),
                refresh: jest.fn()
            };
            (ledgerMap as any).instance = mockWasmInstance;

            const mockActor = {
                data_fetch: jest.fn().mockResolvedValue({
                    Ok: [
                        'position=2000&response_bytes=100&direction=forward&more=false',
                        new Uint8Array([1, 2, 3, 4])
                    ]
                })
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await ledgerMap.fetchFromCanister();

            // Verify that read_persistent_storage was called for bytes before
            expect(mockWasmInstance.read_persistent_storage).toHaveBeenCalled();
            // Verify that data_fetch was called with bytesBefore
            expect(mockActor.data_fetch.mock.calls[0][1]).toBeInstanceOf(Uint8Array);
        });

        it('handles incremental data fetching', async () => {
            const mockWasmInstance = {
                get_next_block_start_pos: jest.fn().mockReturnValue(0),
                get_data_partition_start: jest.fn().mockReturnValue(0),
                get_persistent_storage_size: jest.fn().mockReturnValue(1024 * 1024),
                read_persistent_storage: jest.fn(),
                write_persistent_storage: jest.fn(),
                refresh: jest.fn()
            };
            (ledgerMap as any).instance = mockWasmInstance;

            // Mock a sequence of responses with more=true
            const mockActor = {
                data_fetch: jest.fn()
                    .mockResolvedValueOnce({
                        Ok: [
                            'position=0&response_bytes=100&direction=forward&more=true',
                            new Uint8Array(Array(100).fill(1))
                        ]
                    })
                    .mockResolvedValueOnce({
                        Ok: [
                            'position=100&response_bytes=100&direction=forward&more=false',
                            new Uint8Array(Array(100).fill(2))
                        ]
                    })
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await ledgerMap.fetchFromCanister();

            // Verify write_persistent_storage was called for each chunk
            expect(mockWasmInstance.write_persistent_storage).toHaveBeenCalledTimes(2);
            expect(mockWasmInstance.refresh).toHaveBeenCalledTimes(2);
        });

        it('handles network errors gracefully', async () => {
            const mockActor = {
                data_fetch: jest.fn().mockRejectedValue(new Error('Network error'))
            };
            (Actor.createActor as jest.Mock).mockReturnValue(mockActor);

            await expect(ledgerMap.fetchFromCanister()).rejects.toThrow('Network error');
        });
    });
});
