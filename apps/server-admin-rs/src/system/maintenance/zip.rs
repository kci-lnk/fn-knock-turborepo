use super::*;

pub(super) fn create_password_protected_zip(
    file_name: &str,
    content: &[u8],
    password: &str,
    modified_at_ms: i64,
) -> anyhow::Result<Vec<u8>> {
    let file_name_bytes = file_name.as_bytes();
    let crc = crc32_buffer(content);
    let compressed = deflate_raw(content)?;
    let mut encryptor = ZipCryptoEncryptor::new(password);
    let mut encryption_header = rand::random::<[u8; 12]>();
    encryption_header[11] = (crc >> 24) as u8;

    let mut encrypted_data = Vec::with_capacity(12 + compressed.len());
    encrypted_data.extend(encryptor.encrypt(&encryption_header));
    encrypted_data.extend(encryptor.encrypt(&compressed));

    let compressed_size = encrypted_data.len() as u32;
    let uncompressed_size = content.len() as u32;
    let (dos_time, dos_date) = dos_datetime(modified_at_ms);
    let flags = 0x0001_u16;
    let compression_method = 8_u16;

    let mut local_header = Vec::new();
    write_u32(&mut local_header, 0x04034b50);
    write_u16(&mut local_header, 20);
    write_u16(&mut local_header, flags);
    write_u16(&mut local_header, compression_method);
    write_u16(&mut local_header, dos_time);
    write_u16(&mut local_header, dos_date);
    write_u32(&mut local_header, crc);
    write_u32(&mut local_header, compressed_size);
    write_u32(&mut local_header, uncompressed_size);
    write_u16(&mut local_header, file_name_bytes.len() as u16);
    write_u16(&mut local_header, 0);
    local_header.extend_from_slice(file_name_bytes);

    let central_directory_offset = (local_header.len() + encrypted_data.len()) as u32;
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

    let mut output = Vec::with_capacity(
        local_header.len() + encrypted_data.len() + central_directory.len() + end.len(),
    );
    output.extend(local_header);
    output.extend(encrypted_data);
    output.extend(central_directory);
    output.extend(end);
    Ok(output)
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

    fn encrypt(&mut self, plain: &[u8]) -> Vec<u8> {
        plain
            .iter()
            .map(|byte| {
                let encrypted = *byte ^ self.decrypt_byte();
                self.update_keys(*byte);
                encrypted
            })
            .collect()
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

pub(super) fn deflate_raw(content: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(content)?;
    encoder.finish()
}

pub(super) fn crc32_buffer(buffer: &[u8]) -> u32 {
    let mut crc = 0xffffffff_u32;
    for byte in buffer {
        crc = crc32_update(crc, *byte);
    }
    crc ^ 0xffffffff
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
        .unwrap_or_else(|_| time::OffsetDateTime::UNIX_EPOCH);
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
