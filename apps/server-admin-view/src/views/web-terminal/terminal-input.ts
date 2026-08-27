import {
  ASCII_TERMINAL_RESPONSE_PATTERN,
  LEGACY_MOUSE_SEQUENCE_PREFIX,
  REMOTE_RESPONSE_CODEPOINT_SAMPLE_LIMIT,
  textEncoder,
} from "./terminal-runtime";

export const encodeInputToBase64 = (value: string): string => {
  const bytes = encodeTerminalInputToBytes(value);
  let binary = "";
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary);
};

const appendUtf8Bytes = (target: number[], value: string): void => {
  textEncoder.encode(value).forEach((byte) => target.push(byte));
};

export const encodeTerminalInputToBytes = (value: string): Uint8Array => {
  if (!value.includes(LEGACY_MOUSE_SEQUENCE_PREFIX)) {
    return textEncoder.encode(value);
  }

  const bytes: number[] = [];
  let cursor = 0;

  while (cursor < value.length) {
    const sequenceStart = value.indexOf(LEGACY_MOUSE_SEQUENCE_PREFIX, cursor);
    if (sequenceStart === -1 || sequenceStart + 6 > value.length) {
      appendUtf8Bytes(bytes, value.slice(cursor));
      break;
    }

    appendUtf8Bytes(bytes, value.slice(cursor, sequenceStart));
    bytes.push(0x1b, 0x5b, 0x4d);
    for (let offset = 3; offset < 6; offset += 1) {
      bytes.push(value.charCodeAt(sequenceStart + offset) & 0xff);
    }
    cursor = sequenceStart + 6;
  }

  return Uint8Array.from(bytes);
};

export const getInputByteLength = (value: string): number =>
  encodeTerminalInputToBytes(value).byteLength;

/**
 * Splits terminal input without cutting a Unicode code point or the six-byte
 * legacy X10 mouse sequence understood by `encodeTerminalInputToBytes`.
 */
export const splitTerminalInputByByteLength = (
  value: string,
  maxBytes: number,
): string[] => {
  if (!value) return [];
  if (!Number.isSafeInteger(maxBytes) || maxBytes < 1) {
    throw new RangeError("Terminal input chunk size must be positive");
  }

  const chunks: string[] = [];
  let chunk = "";
  let chunkBytes = 0;
  let cursor = 0;
  while (cursor < value.length) {
    const legacyMouseSequence = value.startsWith(
      LEGACY_MOUSE_SEQUENCE_PREFIX,
      cursor,
    );
    const tokenLength = legacyMouseSequence
      ? Math.min(6, value.length - cursor)
      : (value.codePointAt(cursor) ?? 0) > 0xffff
        ? 2
        : 1;
    const token = value.slice(cursor, cursor + tokenLength);
    const tokenBytes = getInputByteLength(token);
    if (chunk && chunkBytes + tokenBytes > maxBytes) {
      chunks.push(chunk);
      chunk = "";
      chunkBytes = 0;
    }
    // `maxBytes` is much larger than every atomic token in production. Keep
    // this branch total for callers using a smaller value in tests or tools.
    if (!chunk && tokenBytes > maxBytes) {
      chunks.push(token);
    } else {
      chunk += token;
      chunkBytes += tokenBytes;
    }
    cursor += tokenLength;
  }
  if (chunk) chunks.push(chunk);
  return chunks;
};

const hasAsciiControlByte = (value: string): boolean => {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code < 0x20 || code === 0x7f) {
      return true;
    }
  }
  return false;
};

export const isSafeRemoteTerminalResponse = (value: string): boolean =>
  value.length > 0 &&
  ASCII_TERMINAL_RESPONSE_PATTERN.test(value) &&
  (value.includes("\u001b") || hasAsciiControlByte(value));

export const summarizeTerminalResponseCodePoints = (value: string): string =>
  Array.from(value)
    .slice(0, REMOTE_RESPONSE_CODEPOINT_SAMPLE_LIMIT)
    .map((char) => `U+${char.codePointAt(0)?.toString(16).toUpperCase()}`)
    .join(" ");

export const decodeBase64ToBytes = (value: string): Uint8Array => {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
};

export const buildTerminalSizeKey = (cols: number, rows: number): string =>
  `${cols}x${rows}`;

export const encodeCtrlInput = (value: string): string | null => {
  if (value.length !== 1) return null;
  const directMap: Record<string, string> = {
    " ": "\u0000",
    "@": "\u0000",
    "`": "\u0000",
    "2": "\u0000",
    "[": "\u001b",
    "{": "\u001b",
    "3": "\u001b",
    "\\": "\u001c",
    "|": "\u001c",
    "4": "\u001c",
    "]": "\u001d",
    "}": "\u001d",
    "5": "\u001d",
    "^": "\u001e",
    "~": "\u001e",
    "6": "\u001e",
    _: "\u001f",
    "7": "\u001f",
    "?": "\u007f",
    "8": "\u007f",
  };

  if (directMap[value]) {
    return directMap[value];
  }

  const code = value.toUpperCase().charCodeAt(0);
  if (code >= 65 && code <= 90) {
    return String.fromCharCode(code - 64);
  }

  return null;
};
