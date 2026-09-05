use super::*;

const BACKUP_CHUNK_SIZE: usize = 64 * 1024;

// Keep the total archive bounded without ever growing a contiguous allocation
// to its full size. Freed multi-megabyte Vec capacities can remain resident in
// system allocators long after a completed export.
pub(super) struct BackupArchiveBuffer {
    chunks: Vec<Vec<u8>>,
    len: usize,
    limit: usize,
}

impl BackupArchiveBuffer {
    fn new(limit: usize) -> Self {
        Self {
            chunks: Vec::new(),
            len: 0,
            limit,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn chunks(&self) -> &[Vec<u8>] {
        &self.chunks
    }

    pub(super) fn into_chunks(self) -> Vec<Vec<u8>> {
        self.chunks
    }

    fn patch_u32(&mut self, offset: usize, value: u32) {
        for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.chunks[(offset + index) / BACKUP_CHUNK_SIZE]
                [(offset + index) % BACKUP_CHUNK_SIZE] = byte;
        }
    }

    #[cfg(test)]
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.chunks.into_iter().flatten().collect()
    }

    #[cfg(test)]
    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        let mut buffer = Self::new(MAX_BACKUP_ARCHIVE_SIZE);
        buffer.write_all(bytes).unwrap();
        buffer
    }
}

impl Write for BackupArchiveBuffer {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len();
        if count > self.limit.saturating_sub(self.len) {
            return Err(io::Error::other("Backup export is too large"));
        }
        while !bytes.is_empty() {
            if self
                .chunks
                .last()
                .is_none_or(|chunk| chunk.len() == BACKUP_CHUNK_SIZE)
            {
                self.chunks.push(Vec::with_capacity(
                    BACKUP_CHUNK_SIZE.min(self.limit - self.len),
                ));
            }
            let chunk = self.chunks.last_mut().unwrap();
            let copied = bytes.len().min(BACKUP_CHUNK_SIZE - chunk.len());
            chunk.extend_from_slice(&bytes[..copied]);
            self.len += copied;
            bytes = &bytes[copied..];
        }
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BackupCrcWriter {
    len: usize,
    crc: u32,
    limit: usize,
}

impl Write for BackupCrcWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.len) {
            return Err(io::Error::other("Backup export is too large"));
        }
        for &byte in bytes {
            self.crc = crc32_update(self.crc, byte);
        }
        self.len += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BackupEncryptWriter<'a> {
    output: &'a mut BackupArchiveBuffer,
    encryptor: ZipCryptoEncryptor,
    scratch: [u8; BACKUP_CHUNK_SIZE],
}

impl Write for BackupEncryptWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for source in bytes.chunks(BACKUP_CHUNK_SIZE) {
            let encrypted = &mut self.scratch[..source.len()];
            encrypted.copy_from_slice(source);
            self.encryptor.encrypt_in_place(encrypted);
            self.output.write_all(encrypted)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

pub(super) fn create_password_protected_json_zip(
    file_name: &str,
    payload: &Value,
    password: &str,
    modified_at_ms: i64,
) -> anyhow::Result<BackupArchiveBuffer> {
    create_password_protected_zip_streaming(
        file_name,
        |writer| {
            serde_json::to_writer_pretty(writer, payload)?;
            Ok(())
        },
        password,
        modified_at_ms,
        MAX_BACKUP_ARCHIVE_SIZE,
    )
}

fn create_password_protected_zip_streaming(
    file_name: &str,
    write_content: impl Fn(&mut dyn Write) -> anyhow::Result<()>,
    password: &str,
    modified_at_ms: i64,
    limit: usize,
) -> anyhow::Result<BackupArchiveBuffer> {
    let file_name_bytes = file_name.as_bytes();
    anyhow::ensure!(
        file_name_bytes.len() <= u16::MAX as usize,
        "Backup filename is too large"
    );
    // ZipCrypto's header needs CRC before compression. Count and checksum a
    // serialization pass, then serialize again directly into the compressor.
    let mut checksum = BackupCrcWriter {
        len: 0,
        crc: 0xffffffff,
        limit,
    };
    write_content(&mut checksum)?;
    let crc = checksum.crc ^ 0xffffffff;
    let uncompressed_size = u32::try_from(checksum.len)?;
    let (dos_time, dos_date) = dos_datetime(modified_at_ms);
    let flags = 0x0001_u16;
    let compression_method = 8_u16;

    let mut header = Vec::new();
    write_u32(&mut header, 0x04034b50);
    write_u16(&mut header, 20);
    write_u16(&mut header, flags);
    write_u16(&mut header, compression_method);
    write_u16(&mut header, dos_time);
    write_u16(&mut header, dos_date);
    write_u32(&mut header, crc);
    // Patched after compression; write directly into the final archive buffer.
    write_u32(&mut header, 0);
    write_u32(&mut header, uncompressed_size);
    write_u16(&mut header, file_name_bytes.len() as u16);
    write_u16(&mut header, 0);
    header.extend_from_slice(file_name_bytes);
    let mut output = BackupArchiveBuffer::new(limit);
    output.write_all(&header)?;

    let data_start = output.len();
    let mut encryption_header = rand::random::<[u8; 12]>();
    encryption_header[11] = (crc >> 24) as u8;
    let mut encryptor = ZipCryptoEncryptor::new(password);
    encryptor.encrypt_in_place(&mut encryption_header);
    output.write_all(&encryption_header)?;
    let writer = BackupEncryptWriter {
        output: &mut output,
        encryptor,
        scratch: [0; BACKUP_CHUNK_SIZE],
    };
    let encoder = DeflateEncoder::new(writer, Compression::best());
    // Serde may emit very small writes around JSON escapes and punctuation.
    // Batch them without creating a buffer proportional to the whole document.
    let mut buffered = io::BufWriter::with_capacity(BACKUP_CHUNK_SIZE, encoder);
    write_content(&mut buffered)?;
    buffered
        .into_inner()
        .map_err(|error| error.into_error())?
        .finish()?
        .flush()?;
    let compressed_size = (output.len() - data_start) as u32;
    output.patch_u32(18, compressed_size);

    let central_directory_offset = output.len() as u32;
    let mut central_directory = Vec::new();
    write_u32(&mut central_directory, 0x02014b50);
    write_u16(&mut central_directory, 20);
    write_u16(&mut central_directory, 20);
    write_u16(&mut central_directory, flags);
    write_u16(&mut central_directory, compression_method);
    write_u16(&mut central_directory, dos_time);
    write_u16(&mut central_directory, dos_date);
    write_u32(&mut central_directory, crc);
    write_u32(&mut central_directory, compressed_size);
    write_u32(&mut central_directory, uncompressed_size);
    write_u16(&mut central_directory, file_name_bytes.len() as u16);
    write_u16(&mut central_directory, 0);
    write_u16(&mut central_directory, 0);
    write_u16(&mut central_directory, 0);
    write_u16(&mut central_directory, 0);
    write_u32(&mut central_directory, 0);
    write_u32(&mut central_directory, 0);
    central_directory.extend_from_slice(file_name_bytes);

    let mut end = Vec::new();
    write_u32(&mut end, 0x06054b50);
    write_u16(&mut end, 0);
    write_u16(&mut end, 0);
    write_u16(&mut end, 1);
    write_u16(&mut end, 1);
    write_u32(&mut end, central_directory.len() as u32);
    write_u32(&mut end, central_directory_offset);
    write_u16(&mut end, 0);

    output.write_all(&central_directory)?;
    output.write_all(&end)?;
    Ok(output)
}

#[cfg(test)]
pub(super) fn create_password_protected_zip(
    file_name: &str,
    content: &[u8],
    password: &str,
    modified_at_ms: i64,
) -> anyhow::Result<Vec<u8>> {
    Ok(create_password_protected_zip_streaming(
        file_name,
        |writer| {
            writer.write_all(content)?;
            Ok(())
        },
        password,
        modified_at_ms,
        MAX_BACKUP_ARCHIVE_SIZE,
    )?
    .into_bytes())
}

struct ZipCryptoEncryptor {
    key0: u32,
    key1: u32,
    key2: u32,
}

impl ZipCryptoEncryptor {
    fn new(password: &str) -> Self {
        let mut this = Self {
            key0: 0x12345678,
            key1: 0x23456789,
            key2: 0x34567890,
        };
        for byte in password.as_bytes() {
            this.update_keys(*byte);
        }
        this
    }

    fn encrypt_in_place(&mut self, data: &mut [u8]) {
        for byte in data {
            let plain = *byte;
            *byte ^= self.decrypt_byte();
            self.update_keys(plain);
        }
    }

    fn update_keys(&mut self, byte: u8) {
        self.key0 = crc32_update(self.key0, byte);
        self.key1 = self
            .key1
            .wrapping_add(self.key0 & 0xff)
            .wrapping_mul(134775813)
            .wrapping_add(1);
        self.key2 = crc32_update(self.key2, (self.key1 >> 24) as u8);
    }

    fn decrypt_byte(&self) -> u8 {
        let temp = (self.key2 | 2) & 0xffff;
        (((temp.wrapping_mul(temp ^ 1)) >> 8) & 0xff) as u8
    }
}

pub(super) fn crc32_update(crc: u32, byte: u8) -> u32 {
    let mut value = (crc ^ u32::from(byte)) & 0xff;
    for _ in 0..8 {
        value = if value & 1 != 0 {
            0xedb88320 ^ (value >> 1)
        } else {
            value >> 1
        };
    }
    value ^ (crc >> 8)
}

pub(super) fn dos_datetime(ms: i64) -> (u16, u16) {
    let timestamp = ms.div_euclid(1000);
    let utc = time::OffsetDateTime::from_unix_timestamp(timestamp)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let local = time::UtcOffset::current_local_offset()
        .map(|offset| utc.to_offset(offset))
        .unwrap_or(utc);
    let year = local.year().clamp(1980, 2107);
    let month = u8::from(local.month()) as u16;
    let day = local.day() as u16;
    let hours = local.hour() as u16;
    let minutes = local.minute() as u16;
    let seconds = (local.second() / 2) as u16;
    let time = ((hours & 0x1f) << 11) | ((minutes & 0x3f) << 5) | (seconds & 0x1f);
    let date = (((year - 1980) as u16) << 9) | ((month & 0xf) << 5) | (day & 0x1f);
    (time, date)
}

pub(super) fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmented_backup_buffer_enforces_total_and_chunk_bounds() {
        let mut output = BackupArchiveBuffer::new(BACKUP_CHUNK_SIZE + 17);
        output.write_all(&vec![7; BACKUP_CHUNK_SIZE]).unwrap();
        assert!(output.write_all(&[8; 18]).is_err());
        assert_eq!(output.len(), BACKUP_CHUNK_SIZE);
        output.write_all(&[8; 17]).unwrap();
        assert_eq!(output.len(), BACKUP_CHUNK_SIZE + 17);
        assert_eq!(output.chunks().len(), 2);
        assert!(
            output
                .chunks()
                .iter()
                .all(|chunk| chunk.capacity() <= BACKUP_CHUNK_SIZE)
        );
        assert!(output.write_all(&[9]).is_err());
    }

    #[test]
    fn repeated_streamed_json_exports_preserve_content_and_small_allocations() {
        let mut random = 0x1234_5678_9abc_def0_u64;
        let value = (0..256 * 1024)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                char::from(b' ' + (random % 95) as u8)
            })
            .collect::<String>();
        let payload = json!({ "entries": [{ "value": value }], "entry_count": 1 });
        let expected = serde_json::to_vec_pretty(&payload).unwrap();
        for _ in 0..4 {
            let archive = create_password_protected_json_zip(
                KNOCK_BACKUP_JSON_FILENAME,
                &payload,
                KNOCK_BACKUP_PASSWORD,
                1_704_067_200_000,
            )
            .unwrap();
            assert!(archive.chunks().len() > 1);
            assert!(archive.len() <= MAX_BACKUP_ARCHIVE_SIZE);
            assert_eq!(
                archive.len(),
                archive.chunks().iter().map(Vec::len).sum::<usize>()
            );
            assert!(
                archive
                    .chunks()
                    .iter()
                    .all(|chunk| chunk.len() <= BACKUP_CHUNK_SIZE
                        && chunk.capacity() <= BACKUP_CHUNK_SIZE)
            );
            let bytes = archive.into_bytes();
            assert_eq!(
                read_backup_json_from_archive_native(&bytes)
                    .unwrap()
                    .as_bytes(),
                expected
            );
        }
    }

    #[test]
    fn streamed_zip_limits_both_source_json_and_final_archive() {
        let oversized_source = create_password_protected_zip_streaming(
            KNOCK_BACKUP_JSON_FILENAME,
            |writer| {
                writer.write_all(&[b'x'; 1024])?;
                Ok(())
            },
            KNOCK_BACKUP_PASSWORD,
            1_704_067_200_000,
            1023,
        );
        assert!(
            oversized_source
                .err()
                .unwrap()
                .to_string()
                .contains("too large")
        );
        // Even an empty source needs ZIP headers, encryption and a directory.
        let oversized_archive = create_password_protected_zip_streaming(
            KNOCK_BACKUP_JSON_FILENAME,
            |_| Ok(()),
            KNOCK_BACKUP_PASSWORD,
            1_704_067_200_000,
            64,
        );
        assert!(
            oversized_archive
                .err()
                .unwrap()
                .to_string()
                .contains("too large")
        );
    }
}
