import { LedgerCursor, CursorDirection, cursorFromData } from '../canister';

describe('LedgerCursor', () => {
    describe('constructor', () => {
        it('creates a cursor with default values', () => {
            const cursor = new LedgerCursor(
                BigInt(0),
                BigInt(100),
                BigInt(1000),
                BigInt(0)
            );
            expect(cursor.dataBeginPosition).toBe(BigInt(0));
            expect(cursor.position).toBe(BigInt(100));
            expect(cursor.dataEndPosition).toBe(BigInt(1000));
            expect(cursor.responseBytes).toBe(BigInt(0));
            expect(cursor.direction).toBe(CursorDirection.Forward);
            expect(cursor.more).toBe(false);
        });
    });

    describe('fromString', () => {
        it('parses cursor string correctly', () => {
            const cursorStr = 'data_begin_position=0&position=100&data_end_position=1000&response_bytes=50&direction=forward&more=true';
            const cursor = LedgerCursor.fromString(cursorStr);

            expect(cursor.dataBeginPosition).toBe(BigInt(0));
            expect(cursor.position).toBe(BigInt(100));
            expect(cursor.dataEndPosition).toBe(BigInt(1000));
            expect(cursor.responseBytes).toBe(BigInt(50));
            expect(cursor.direction).toBe(CursorDirection.Forward);
            expect(cursor.more).toBe(true);
        });

        it('handles missing values', () => {
            const cursorStr = 'position=100';
            const cursor = LedgerCursor.fromString(cursorStr);

            expect(cursor.dataBeginPosition).toBe(BigInt(0));
            expect(cursor.position).toBe(BigInt(100));
            expect(cursor.dataEndPosition).toBe(BigInt(0));
            expect(cursor.responseBytes).toBe(BigInt(0));
            expect(cursor.direction).toBe(CursorDirection.Forward);
            expect(cursor.more).toBe(false);
        });
    });

    describe('toRequestString', () => {
        it('formats request string correctly', () => {
            const cursor = new LedgerCursor(
                BigInt(0),
                BigInt(100),
                BigInt(1000),
                BigInt(50),
                CursorDirection.Forward,
                true
            );
            expect(cursor.toRequestString()).toBe('position=100');
        });
    });

    describe('toUrlEncString', () => {
        it('formats URL encoded string correctly', () => {
            const cursor = new LedgerCursor(
                BigInt(0),
                BigInt(100),
                BigInt(1000),
                BigInt(50),
                CursorDirection.Forward,
                true
            );
            expect(cursor.toUrlEncString()).toBe('position=100&response_bytes=50&direction=forward&more=true');
        });
    });
});

describe('cursorFromData', () => {
    it('calculates cursor for normal case', () => {
        const cursor = cursorFromData(
            BigInt(0),    // locLedgerStartDataLba
            BigInt(2048), // locStorageBytes
            BigInt(1000), // locNextWritePosition
            BigInt(100),  // reqStartPosition
            BigInt(512)   // fetchSizeBytesDefault
        );

        expect(cursor.dataBeginPosition).toBe(BigInt(0));
        expect(cursor.position).toBe(BigInt(100));
        expect(cursor.dataEndPosition).toBe(BigInt(1000));
        expect(cursor.responseBytes).toBe(BigInt(512));
        expect(cursor.direction).toBe(CursorDirection.Forward);
        expect(cursor.more).toBe(true);
    });

    it('handles case when request position is before start', () => {
        const cursor = cursorFromData(
            BigInt(100), // locLedgerStartDataLba
            BigInt(2048),
            BigInt(1000),
            BigInt(50),  // reqStartPosition before start
            BigInt(512)
        );

        expect(cursor.position).toBe(BigInt(100)); // Should adjust to start position
    });

    it('handles case when next write position exceeds storage', () => {
        const cursor = cursorFromData(
            BigInt(0),
            BigInt(1000), // locStorageBytes
            BigInt(2000), // locNextWritePosition exceeds storage
            BigInt(100),
            BigInt(512)
        );

        expect(cursor.dataEndPosition).toBe(BigInt(1000)); // Should adjust to storage size
    });

    it('handles case when response would exceed available data', () => {
        const cursor = cursorFromData(
            BigInt(0),
            BigInt(2048),
            BigInt(600),  // Only 600 bytes available
            BigInt(100),
            BigInt(1024)  // Trying to fetch 1024 bytes
        );

        expect(cursor.responseBytes).toBe(BigInt(500)); // Should only return available bytes
        expect(cursor.more).toBe(false);
    });
});
