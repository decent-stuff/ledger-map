import { TextEncoder, TextDecoder } from 'node:util';

if (typeof global.TextEncoder === 'undefined') {
    (global as any).TextEncoder = TextEncoder;
}

if (typeof global.TextDecoder === 'undefined') {
    (global as any).TextDecoder = TextDecoder;
}
