const crc32 = (buf: Uint8Array) => {
  let c = ~0 >>> 0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i] ?? 0;
    for (let k = 0; k < 8; k++) {
      const mask = -(c & 1);
      c = (c >>> 1) ^ (0xedb88320 & mask);
    }
  }
  return ~c >>> 0;
};

const getDosTimestamp = () => {
  const d = new Date();
  const dosTime =
    ((d.getHours() & 0x1f) << 11) |
    ((d.getMinutes() & 0x3f) << 5) |
    (Math.floor(d.getSeconds() / 2) & 0x1f);
  const dosDate =
    (((d.getFullYear() - 1980) & 0x7f) << 9) |
    (((d.getMonth() + 1) & 0xf) << 5) |
    (d.getDate() & 0x1f);
  return { dosTime, dosDate };
};

const u16 = (v: number) => {
  const b = new Uint8Array(2);
  const dv = new DataView(b.buffer);
  dv.setUint16(0, v, true);
  return b;
};

const u32 = (v: number) => {
  const b = new Uint8Array(4);
  const dv = new DataView(b.buffer);
  dv.setUint32(0, v, true);
  return b;
};

export const createZip = (entries: { name: string; data: Uint8Array }[]) => {
  const files: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  const { dosTime, dosDate } = getDosTimestamp();

  for (const e of entries) {
    const nameBytes = new TextEncoder().encode(e.name);
    const csum = crc32(e.data);
    const lfh = new Uint8Array([
      ...u32(0x04034b50),
      ...u16(20),
      ...u16(0),
      ...u16(0),
      ...u16(dosTime),
      ...u16(dosDate),
      ...u32(csum),
      ...u32(e.data.length),
      ...u32(e.data.length),
      ...u16(nameBytes.length),
      ...u16(0),
      ...nameBytes,
      ...e.data,
    ]);
    files.push(lfh);
    const cdfh = new Uint8Array([
      ...u32(0x02014b50),
      ...u16(20),
      ...u16(20),
      ...u16(0),
      ...u16(0),
      ...u16(dosTime),
      ...u16(dosDate),
      ...u32(csum),
      ...u32(e.data.length),
      ...u32(e.data.length),
      ...u16(nameBytes.length),
      ...u16(0),
      ...u16(0),
      ...u16(0),
      ...u16(0),
      ...u32(0),
      ...u32(offset),
      ...nameBytes,
    ]);
    central.push(cdfh);
    offset += lfh.length;
  }

  const centralDir = central.reduce(
    (a, b) => new Uint8Array([...a, ...b]),
    new Uint8Array(),
  );
  const filesBlob = files.reduce(
    (a, b) => new Uint8Array([...a, ...b]),
    new Uint8Array(),
  );
  const eocd = new Uint8Array([
    ...u32(0x06054b50),
    ...u16(0),
    ...u16(0),
    ...u16(entries.length),
    ...u16(entries.length),
    ...u32(centralDir.length),
    ...u32(filesBlob.length),
    ...u16(0),
  ]);
  return new Uint8Array([...filesBlob, ...centralDir, ...eocd]);
};
