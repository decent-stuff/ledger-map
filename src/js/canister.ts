import { Actor, ActorMethod, ActorSubclass, HttpAgent } from '@dfinity/agent';
import { IDL } from '@dfinity/candid';

export const CANISTER_ID = 'ggi4a-wyaaa-aaaai-actqq-cai';
export const API_HOST = 'https://icp-api.io';

export enum CursorDirection {
    Forward = 'forward',
    Backward = 'backward'
}

export class LedgerCursor {
    constructor(
        public dataBeginPosition: bigint,
        public position: bigint,
        public dataEndPosition: bigint,
        public responseBytes: bigint,
        public direction: CursorDirection = CursorDirection.Forward,
        public more: boolean = false
    ) { }

    static fromString(s: string): LedgerCursor {
        const params = new URLSearchParams(s);
        return new LedgerCursor(
            BigInt(params.get('data_begin_position') || '0'),
            BigInt(params.get('position') || '0'),
            BigInt(params.get('data_end_position') || '0'),
            BigInt(params.get('response_bytes') || '0'),
            (params.get('direction') as CursorDirection) || CursorDirection.Forward,
            params.get('more') === 'true'
        );
    }

    toRequestString(): string {
        return `position=${this.position}`;
    }

    toUrlEncString(): string {
        return `position=${this.position}&response_bytes=${this.responseBytes}&direction=${this.direction}&more=${this.more}`;
    }

    toString(): string {
        return `(position 0x${this.position.toString(16)}) ${this.toUrlEncString()}`;
    }
}

// Define the IDL interface explicitly
interface CandidIDL {
    Variant: <T extends Record<string, any>>(fields: T) => any;
    Tuple: (...types: any[]) => any;
    Text: any;
    Vec: (t: any) => any;
    Nat8: any;
    Opt: (t: any) => any;
    Func: (args: any[], ret: any[], annotations: string[]) => any;
    Service: (methods: Record<string, any>) => any;
}

export const idlFactory = ({ IDL }: { IDL: CandidIDL }) => {
    const ResultData = IDL.Variant({
        'Ok': IDL.Tuple(IDL.Text, IDL.Vec(IDL.Nat8)),
        'Err': IDL.Text
    });

    return IDL.Service({
        'data_fetch': IDL.Func(
            [IDL.Opt(IDL.Text), IDL.Opt(IDL.Vec(IDL.Nat8))],
            [ResultData],
            ['query']
        )
    });
};

export interface LedgerCanisterService {
    data_fetch: ActorMethod<[string | undefined, Uint8Array | undefined], {
        Ok?: [string, Uint8Array];
        Err?: string;
    }>;
}

const MAX_RETRIES = 3;
const RETRY_DELAY = 3000;

export async function createLedgerCanister(): Promise<ActorSubclass<LedgerCanisterService>> {
    const agent = new HttpAgent({ host: API_HOST });
    await agent.fetchRootKey();

    return Actor.createActor<LedgerCanisterService>(idlFactory, {
        agent,
        canisterId: CANISTER_ID,
    });
}

export function cursorFromData(
    locLedgerStartDataLba: bigint,
    locStorageBytes: bigint,
    locNextWritePosition: bigint,
    reqStartPosition: bigint,
    fetchSizeBytesDefault: bigint = BigInt(1024 * 1024) // 1MB default
): LedgerCursor {
    // Handle edge case: reqStartPosition is before the start of the data partition
    const responseStartPosition = locLedgerStartDataLba > reqStartPosition ? locLedgerStartDataLba : reqStartPosition;

    // Handle edge case: locNextWritePosition is beyond the end of the persistent storage
    const adjustedNextWritePosition = locNextWritePosition > locStorageBytes ? locStorageBytes : locNextWritePosition;

    // Start - end position ==> size
    // size is ideally equal to FETCH_SIZE_BYTES_DEFAULT, but there may not be enough data in the persistent storage
    const responseEndPosition = (responseStartPosition + fetchSizeBytesDefault) > adjustedNextWritePosition
        ? adjustedNextWritePosition
        : responseStartPosition + fetchSizeBytesDefault;

    if (responseStartPosition >= locStorageBytes || responseStartPosition >= responseEndPosition) {
        return new LedgerCursor(
            locLedgerStartDataLba,
            adjustedNextWritePosition,
            adjustedNextWritePosition,
            BigInt(0),
            CursorDirection.Forward,
            false
        );
    }

    return new LedgerCursor(
        locLedgerStartDataLba,
        responseStartPosition,
        adjustedNextWritePosition,
        responseEndPosition - responseStartPosition,
        CursorDirection.Forward,
        responseEndPosition < adjustedNextWritePosition
    );
}
