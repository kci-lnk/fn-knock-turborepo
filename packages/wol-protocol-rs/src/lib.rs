use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::fmt;

type HmacSha256 = Hmac<Sha256>;

pub const PACKET_LEN: usize = 94;
pub const HEADER_LEN: usize = 62;
pub const PROTOCOL_VERSION: u8 = 2;
pub const REQUEST_MAGIC: [u8; 4] = *b"FNWL";
pub const ACK_MAGIC: [u8; 4] = *b"FNWA";
const REQUEST_DOMAIN: &[u8] = b"fn-knock/wol/request/v2\0";
const ACK_DOMAIN: &[u8] = b"fn-knock/wol/ack/v2\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Command {
    Wake = 1,
    Probe = 2,
    Status = 3,
}

impl TryFrom<u8> for Command {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Wake),
            2 => Ok(Self::Probe),
            3 => Ok(Self::Status),
            _ => Err(ProtocolError::UnsupportedCommand),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum AckStatus {
    Ok = 0,
    ClockSkew = 1,
    InvalidTarget = 2,
    BroadcastFailed = 3,
    InternalError = 4,
    TargetOnline = 5,
    TargetOffline = 6,
    TargetUnknown = 7,
}

impl TryFrom<u16> for AckStatus {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::ClockSkew),
            2 => Ok(Self::InvalidTarget),
            3 => Ok(Self::BroadcastFailed),
            4 => Ok(Self::InternalError),
            5 => Ok(Self::TargetOnline),
            6 => Ok(Self::TargetOffline),
            7 => Ok(Self::TargetUnknown),
            _ => Err(ProtocolError::UnsupportedStatus),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const ZERO: Self = Self([0; 6]);

    pub fn from_bytes(bytes: [u8; 6]) -> Result<Self, ProtocolError> {
        let value = Self(bytes);
        if !value.is_valid_target() {
            return Err(ProtocolError::InvalidMac);
        }
        Ok(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    pub fn is_valid_target(&self) -> bool {
        self.0 != [0; 6] && self.0 != [0xff; 6] && self.0[0] & 1 == 0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl std::str::FromStr for MacAddress {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let compact = value
            .trim()
            .chars()
            .filter(|character| !matches!(character, ':' | '-'))
            .collect::<String>();
        if compact.len() != 12 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ProtocolError::InvalidMac);
        }
        let mut bytes = [0_u8; 6];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16)
                .map_err(|_| ProtocolError::InvalidMac)?;
        }
        Self::from_bytes(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestPacket {
    pub command: Command,
    pub relay_id: [u8; 16],
    pub key_version: u32,
    pub timestamp: u64,
    pub request_id: [u8; 16],
    pub target_mac: [u8; 6],
    /// Preferred IPv4 address for a status probe. All zeroes mean unknown.
    pub target_ipv4: [u8; 4],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AckPacket {
    pub command: Command,
    pub status: AckStatus,
    pub relay_id: [u8; 16],
    pub key_version: u32,
    pub timestamp: u64,
    pub request_id: [u8; 16],
    pub target_mac: [u8; 6],
    /// IPv4 address observed by the relay while probing the target.
    pub target_ipv4: [u8; 4],
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ProtocolError {
    #[error("packet length is invalid")]
    InvalidLength,
    #[error("packet magic is invalid")]
    InvalidMagic,
    #[error("protocol version is unsupported")]
    UnsupportedVersion,
    #[error("command is unsupported")]
    UnsupportedCommand,
    #[error("acknowledgement status is unsupported")]
    UnsupportedStatus,
    #[error("packet signature is invalid")]
    InvalidSignature,
    #[error("packet reserved flags are not zero")]
    NonZeroReserved,
    #[error("MAC address is invalid")]
    InvalidMac,
}

pub fn encode_request(packet: &RequestPacket, psk: &[u8]) -> [u8; PACKET_LEN] {
    let mut output = [0_u8; PACKET_LEN];
    encode_common(
        &mut output,
        REQUEST_MAGIC,
        packet.command,
        0,
        packet.relay_id,
        packet.key_version,
        packet.timestamp,
        packet.request_id,
        packet.target_mac,
        packet.target_ipv4,
    );
    write_signature(&mut output, psk, REQUEST_DOMAIN);
    output
}

pub fn decode_request(input: &[u8], psk: &[u8]) -> Result<RequestPacket, ProtocolError> {
    verify_packet(input, psk, REQUEST_MAGIC, REQUEST_DOMAIN)?;
    if input[6..8] != [0, 0] {
        return Err(ProtocolError::NonZeroReserved);
    }
    Ok(RequestPacket {
        command: Command::try_from(input[5])?,
        relay_id: array_at(input, 8),
        key_version: u32::from_be_bytes(array_at(input, 24)),
        timestamp: u64::from_be_bytes(array_at(input, 28)),
        request_id: array_at(input, 36),
        target_mac: array_at(input, 52),
        target_ipv4: array_at(input, 58),
    })
}

pub fn encode_ack(packet: &AckPacket, psk: &[u8]) -> [u8; PACKET_LEN] {
    let mut output = [0_u8; PACKET_LEN];
    encode_common(
        &mut output,
        ACK_MAGIC,
        packet.command,
        packet.status as u16,
        packet.relay_id,
        packet.key_version,
        packet.timestamp,
        packet.request_id,
        packet.target_mac,
        packet.target_ipv4,
    );
    write_signature(&mut output, psk, ACK_DOMAIN);
    output
}

pub fn decode_ack(input: &[u8], psk: &[u8]) -> Result<AckPacket, ProtocolError> {
    verify_packet(input, psk, ACK_MAGIC, ACK_DOMAIN)?;
    Ok(AckPacket {
        command: Command::try_from(input[5])?,
        status: AckStatus::try_from(u16::from_be_bytes(array_at(input, 6)))?,
        relay_id: array_at(input, 8),
        key_version: u32::from_be_bytes(array_at(input, 24)),
        timestamp: u64::from_be_bytes(array_at(input, 28)),
        request_id: array_at(input, 36),
        target_mac: array_at(input, 52),
        target_ipv4: array_at(input, 58),
    })
}

pub fn magic_packet(mac: MacAddress) -> [u8; 102] {
    let mut output = [0_u8; 102];
    output[..6].fill(0xff);
    for chunk in output[6..].chunks_exact_mut(6) {
        chunk.copy_from_slice(mac.as_bytes());
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn encode_common(
    output: &mut [u8; PACKET_LEN],
    magic: [u8; 4],
    command: Command,
    status_or_flags: u16,
    relay_id: [u8; 16],
    key_version: u32,
    timestamp: u64,
    request_id: [u8; 16],
    target_mac: [u8; 6],
    target_ipv4: [u8; 4],
) {
    output[..4].copy_from_slice(&magic);
    output[4] = PROTOCOL_VERSION;
    output[5] = command as u8;
    output[6..8].copy_from_slice(&status_or_flags.to_be_bytes());
    output[8..24].copy_from_slice(&relay_id);
    output[24..28].copy_from_slice(&key_version.to_be_bytes());
    output[28..36].copy_from_slice(&timestamp.to_be_bytes());
    output[36..52].copy_from_slice(&request_id);
    output[52..58].copy_from_slice(&target_mac);
    output[58..62].copy_from_slice(&target_ipv4);
}

fn write_signature(output: &mut [u8; PACKET_LEN], psk: &[u8], domain: &[u8]) {
    let signature = signature(psk, domain, &output[..HEADER_LEN]);
    output[HEADER_LEN..].copy_from_slice(&signature);
}

fn verify_packet(
    input: &[u8],
    psk: &[u8],
    magic: [u8; 4],
    domain: &[u8],
) -> Result<(), ProtocolError> {
    if input.len() != PACKET_LEN {
        return Err(ProtocolError::InvalidLength);
    }
    if input[..4] != magic {
        return Err(ProtocolError::InvalidMagic);
    }
    if input[4] != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    let mut verifier = HmacSha256::new_from_slice(psk).expect("HMAC accepts keys of any size");
    verifier.update(domain);
    verifier.update(&input[..HEADER_LEN]);
    verifier
        .verify_slice(&input[HEADER_LEN..])
        .map_err(|_| ProtocolError::InvalidSignature)
}

fn signature(psk: &[u8], domain: &[u8], header: &[u8]) -> [u8; 32] {
    let mut signer = HmacSha256::new_from_slice(psk).expect("HMAC accepts keys of any size");
    signer.update(domain);
    signer.update(header);
    signer.finalize().into_bytes().into()
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> [u8; N] {
    let mut output = [0_u8; N];
    output.copy_from_slice(&input[offset..offset + N]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RequestPacket {
        RequestPacket {
            command: Command::Wake,
            relay_id: [0x11; 16],
            key_version: 7,
            timestamp: 1_700_000_000,
            request_id: [0x22; 16],
            target_mac: [0x02, 0x11, 0x22, 0x33, 0x44, 0x55],
            target_ipv4: [192, 168, 1, 20],
        }
    }

    #[test]
    fn request_and_ack_round_trip_and_detect_tampering() {
        let psk = [0x33; 32];
        let encoded = encode_request(&request(), &psk);
        assert_eq!(encoded.len(), PACKET_LEN);
        assert_eq!(decode_request(&encoded, &psk).unwrap(), request());

        let mut tampered = encoded;
        tampered[52] ^= 1;
        assert_eq!(
            decode_request(&tampered, &psk),
            Err(ProtocolError::InvalidSignature)
        );
        let mut reserved = encoded;
        reserved[6] = 1;
        write_signature(&mut reserved, &psk, REQUEST_DOMAIN);
        assert_eq!(
            decode_request(&reserved, &psk),
            Err(ProtocolError::NonZeroReserved)
        );

        let ack = AckPacket {
            command: Command::Wake,
            status: AckStatus::Ok,
            relay_id: request().relay_id,
            key_version: request().key_version,
            timestamp: request().timestamp + 1,
            request_id: request().request_id,
            target_mac: request().target_mac,
            target_ipv4: request().target_ipv4,
        };
        assert_eq!(decode_ack(&encode_ack(&ack, &psk), &psk).unwrap(), ack);
    }

    #[test]
    fn request_and_ack_match_v2_golden_vectors() {
        let psk = [0x33; 32];
        assert_eq!(
            hex::encode(encode_request(&request(), &psk)),
            "464e574c020100001111111111111111111111111111111100000007000000006553f10022222222222222222222222222222222021122334455c0a8011415904b6f047c3bd903cf2a8de9b92d2e75ee25ae3a3d88c7ac3f1c386249173f"
        );
        let ack = AckPacket {
            command: Command::Wake,
            status: AckStatus::Ok,
            relay_id: request().relay_id,
            key_version: request().key_version,
            timestamp: request().timestamp + 1,
            request_id: request().request_id,
            target_mac: request().target_mac,
            target_ipv4: request().target_ipv4,
        };
        assert_eq!(
            hex::encode(encode_ack(&ack, &psk)),
            "464e5741020100001111111111111111111111111111111100000007000000006553f10122222222222222222222222222222222021122334455c0a801142ff699bec4299cfba52dc31cd24f400766bcc525976ff75b95226984b8881d5f"
        );
    }

    #[test]
    fn normalizes_common_mac_formats_and_rejects_non_unicast_values() {
        for value in ["02:11:22:33:44:55", "02-11-22-33-44-55", "021122334455"] {
            assert_eq!(
                value.parse::<MacAddress>().unwrap().to_string(),
                "02:11:22:33:44:55"
            );
        }
        for value in [
            "",
            "02.11.22.33.44.55",
            "00:00:00:00:00:00",
            "FF:FF:FF:FF:FF:FF",
            "01:11:22:33:44:55",
        ] {
            assert_eq!(value.parse::<MacAddress>(), Err(ProtocolError::InvalidMac));
        }
    }

    #[test]
    fn builds_standard_magic_packet() {
        let mac = "02:11:22:33:44:55".parse::<MacAddress>().unwrap();
        let packet = magic_packet(mac);
        assert_eq!(&packet[..6], &[0xff; 6]);
        assert!(
            packet[6..]
                .chunks_exact(6)
                .all(|chunk| chunk == mac.as_bytes())
        );
    }
}
