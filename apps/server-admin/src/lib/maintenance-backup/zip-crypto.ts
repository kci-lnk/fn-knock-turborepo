import { randomBytes } from "node:crypto";
import { deflateRawSync } from "node:zlib";

const CRC32_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let i = 0; i < table.length; i += 1) {
    let value = i;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[i] = value >>> 0;
  }
  return table;
})();

const crc32Update = (crc: number, byte: number): number =>
  ((CRC32_TABLE[(crc ^ byte) & 0xff] ?? 0) ^ (crc >>> 8)) >>> 0;

const crc32Buffer = (buffer: Buffer): number => {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = crc32Update(crc, byte);
  }
  return (crc ^ 0xffffffff) >>> 0;
};

const toDosDateTime = (date: Date): { time: number; date: number } => {
  const year = Math.min(2107, Math.max(1980, date.getFullYear()));
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const hours = date.getHours();
  const minutes = date.getMinutes();
  const seconds = Math.floor(date.getSeconds() / 2);

  return {
    time: ((hours & 0x1f) << 11) | ((minutes & 0x3f) << 5) | (seconds & 0x1f),
    date: ((year - 1980) << 9) | ((month & 0xf) << 5) | (day & 0x1f),
  };
};

const createZipCryptoEncryptor = (password: string) => {
  let key0 = 0x12345678;
  let key1 = 0x23456789;
  let key2 = 0x34567890;

  const updateKeys = (byte: number) => {
    key0 = crc32Update(key0, byte);
    key1 = (Math.imul((key1 + (key0 & 0xff)) >>> 0, 134775813) + 1) >>> 0;
    key2 = crc32Update(key2, key1 >>> 24);
  };

  for (const byte of Buffer.from(password, "utf-8")) {
    updateKeys(byte);
  }

  const decryptByte = (): number => {
    const temp = (key2 | 2) & 0xffff;
    return (Math.imul(temp, temp ^ 1) >>> 8) & 0xff;
  };

  return (plain: Buffer): Buffer => {
    const encrypted = Buffer.allocUnsafe(plain.length);
    for (let index = 0; index < plain.length; index += 1) {
      const byte = plain[index] ?? 0;
      encrypted[index] = byte ^ decryptByte();
      updateKeys(byte);
    }
    return encrypted;
  };
};

const writeUInt16LE = (value: number): Buffer => {
  const buffer = Buffer.allocUnsafe(2);
  buffer.writeUInt16LE(value & 0xffff, 0);
  return buffer;
};

const writeUInt32LE = (value: number): Buffer => {
  const buffer = Buffer.allocUnsafe(4);
  buffer.writeUInt32LE(value >>> 0, 0);
  return buffer;
};

export const createPasswordProtectedZip = (
  fileName: string,
  content: Buffer,
  password: string,
  modifiedAt = new Date(),
): Buffer => {
  const fileNameBuffer = Buffer.from(fileName, "utf-8");
  const crc = crc32Buffer(content);
  const compressedContent = deflateRawSync(content, { level: 9 });
  const encrypt = createZipCryptoEncryptor(password);
  const encryptionHeader = randomBytes(12);
  encryptionHeader[11] = (crc >>> 24) & 0xff;
  const encryptedData = Buffer.concat([
    encrypt(encryptionHeader),
    encrypt(compressedContent),
  ]);
  const compressedSize = encryptedData.length;
  const uncompressedSize = content.length;
  const { time: dosTime, date: dosDate } = toDosDateTime(modifiedAt);
  const flags = 0x0001;
  const compressionMethod = 8;

  const localHeader = Buffer.concat([
    writeUInt32LE(0x04034b50),
    writeUInt16LE(20),
    writeUInt16LE(flags),
    writeUInt16LE(compressionMethod),
    writeUInt16LE(dosTime),
    writeUInt16LE(dosDate),
    writeUInt32LE(crc),
    writeUInt32LE(compressedSize),
    writeUInt32LE(uncompressedSize),
    writeUInt16LE(fileNameBuffer.length),
    writeUInt16LE(0),
    fileNameBuffer,
  ]);
  const centralDirectoryOffset = localHeader.length + encryptedData.length;
  const centralDirectory = Buffer.concat([
    writeUInt32LE(0x02014b50),
    writeUInt16LE(20),
    writeUInt16LE(20),
    writeUInt16LE(flags),
    writeUInt16LE(compressionMethod),
    writeUInt16LE(dosTime),
    writeUInt16LE(dosDate),
    writeUInt32LE(crc),
    writeUInt32LE(compressedSize),
    writeUInt32LE(uncompressedSize),
    writeUInt16LE(fileNameBuffer.length),
    writeUInt16LE(0),
    writeUInt16LE(0),
    writeUInt16LE(0),
    writeUInt16LE(0),
    writeUInt32LE(0),
    writeUInt32LE(0),
    fileNameBuffer,
  ]);
  const endOfCentralDirectory = Buffer.concat([
    writeUInt32LE(0x06054b50),
    writeUInt16LE(0),
    writeUInt16LE(0),
    writeUInt16LE(1),
    writeUInt16LE(1),
    writeUInt32LE(centralDirectory.length),
    writeUInt32LE(centralDirectoryOffset),
    writeUInt16LE(0),
  ]);

  return Buffer.concat([
    localHeader,
    encryptedData,
    centralDirectory,
    endOfCentralDirectory,
  ]);
};
