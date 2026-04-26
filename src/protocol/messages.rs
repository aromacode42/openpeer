//! Signaling wire protocol message types.
//!
//! Frame format (§5):
//!   MAGIC (2) + VER (1) + MSG_TY (1) + LEN (4) + PAYLOAD (LEN)
//! - MAGIC = 0x4F50 ("OP")
//! - VER   = 0x01
//! - All multi-byte integers: big-endian
//! - Strings: u16 BE length + UTF-8 bytes
//! - SocketAddr: u8 family (4=v4, 6=v6) + addr bytes + u16 port

use std::io;

/// Magic bytes identifying an OpenPeer frame.
pub const MAGIC: u16 = 0x4F50;
/// Protocol version byte.
pub const VERSION: u8 = 0x01;

/// All message type codes used in the signaling protocol.
pub mod msg_type {
    /// Client → Server: Register with peer ID + public key.
    pub const REGISTER: u8 = 0x01;
    /// Server → Client: Ack with assigned ID + public address.
    pub const REGISTER_ACK: u8 = 0x02;
    /// Client → Server: Request connection to a peer.
    pub const CONNECT_REQUEST: u8 = 0x03;
    /// Server → Client: Offer with peer's info + session ID.
    pub const CONNECT_OFFER: u8 = 0x04;
    /// Client → Server: Accept a connect offer.
    pub const CONNECT_ACCEPT: u8 = 0x05;
    /// Bidirectional: Relay candidate (STUN/TURN).
    pub const RELAY_CANDIDATE: u8 = 0x06;
    /// Client → Server: Keepalive heartbeat.
    pub const HEARTBEAT: u8 = 0x07;
    /// Client → Server: Graceful disconnect.
    pub const BYE: u8 = 0x08;
    /// Server → Client: Error.
    pub const ERROR: u8 = 0xFF;
}

// ---------------------------------------------------------------------------
// Wire-format primitives
// ---------------------------------------------------------------------------

/// Encode a u16 as big-endian bytes.
pub fn encode_u16_be(buf: &mut Vec<u8>, v: u16) {
    buf.push((v >> 8) as u8);
    buf.push(v as u8);
}

/// Decode a u16 from big-endian bytes.
pub fn decode_u16_be(bytes: &[u8]) -> io::Result<u16> {
    if bytes.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "need 2 bytes for u16",
        ));
    }
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Encode a u32 as big-endian bytes.
pub fn encode_u32_be(buf: &mut Vec<u8>, v: u32) {
    let b = v.to_be_bytes();
    buf.extend_from_slice(&b);
}

/// Decode a u32 from big-endian bytes.
pub fn decode_u32_be(bytes: &[u8]) -> io::Result<u32> {
    if bytes.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "need 4 bytes for u32",
        ));
    }
    let mut arr = [0u8; 4];
    arr.copy_from_slice(&bytes[..4]);
    Ok(u32::from_be_bytes(arr))
}

/// Encode a string as: u16 BE length + UTF-8 bytes.
pub fn encode_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    encode_u16_be(buf, bytes.len() as u16);
    buf.extend_from_slice(bytes);
}

/// Decode a string from: u16 BE length + UTF-8 bytes.
pub fn decode_string(bytes: &[u8]) -> io::Result<(&str, usize)> {
    if bytes.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "need 2 bytes for string len",
        ));
    }
    let len = decode_u16_be(bytes)? as usize;
    if bytes.len() < 2 + len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "string content truncated",
        ));
    }
    let s = std::str::from_utf8(&bytes[2..2 + len])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "string not valid UTF-8"))?;
    Ok((s, 2 + len))
}

/// Encode a SocketAddr (v4: 1+4+2 bytes, v6: 1+16+2 bytes).
pub fn encode_socketaddr(buf: &mut Vec<u8>, addr: &std::net::SocketAddr) {
    buf.push(if addr.is_ipv4() { 4 } else { 6 });
    match addr {
        std::net::SocketAddr::V4(v4) => {
            buf.extend_from_slice(&v4.ip().octets());
        }
        std::net::SocketAddr::V6(v6) => {
            buf.extend_from_slice(&v6.ip().octets());
        }
    }
    let port = addr.port();
    buf.push((port >> 8) as u8);
    buf.push(port as u8);
}

/// Decode a SocketAddr from wire format.
pub fn decode_socketaddr(bytes: &[u8]) -> io::Result<(&std::net::SocketAddr, usize)> {
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "need 1 byte for family",
        ));
    }
    match bytes[0] {
        4 => {
            if bytes.len() < 7 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "need 7 bytes for v4 addr",
                ));
            }
            let mut ip = [0u8; 4];
            ip.copy_from_slice(&bytes[1..5]);
            let port = u16::from_be_bytes([bytes[5], bytes[6]]);
            let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::from(ip));
            let addr = std::net::SocketAddr::new(ip, port);
            Ok((Box::leak(Box::new(addr)), 7))
        }
        6 => {
            if bytes.len() < 19 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "need 19 bytes for v6 addr",
                ));
            }
            let mut ip = [0u8; 16];
            ip.copy_from_slice(&bytes[1..17]);
            let port = u16::from_be_bytes([bytes[17], bytes[18]]);
            let ip = std::net::IpAddr::V6(std::net::Ipv6Addr::from(ip));
            let addr = std::net::SocketAddr::new(ip, port);
            Ok((Box::leak(Box::new(addr)), 19))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown address family: {}", other),
        )),
    }
}

// ---------------------------------------------------------------------------
// Frame codec
// ---------------------------------------------------------------------------

/// A decoded frame header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    /// Magic bytes (must be 0x4F50).
    pub magic: u16,
    /// Protocol version (must be 0x01).
    pub version: u8,
    /// Message type byte.
    pub msg_type: u8,
    /// Payload length in bytes.
    pub payload_len: u32,
}

/// Encode a frame header into `buf`.
pub fn encode_header(buf: &mut Vec<u8>, msg_type: u8, payload_len: u32) {
    encode_u16_be(buf, MAGIC);
    buf.push(VERSION);
    buf.push(msg_type);
    encode_u32_be(buf, payload_len);
}

/// Decode a frame header from the beginning of `bytes`. Returns the header and
/// the number of header bytes consumed.
pub fn decode_header(bytes: &[u8]) -> io::Result<FrameHeader> {
    if bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("need 8 header bytes, got {}", bytes.len()),
        ));
    }
    let magic = decode_u16_be(&bytes[..2])?;
    if magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad magic: expected 0x{:04X}, got 0x{:04X}", MAGIC, magic),
        ));
    }
    let version = bytes[2];
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported protocol version: {}", version),
        ));
    }
    let msg_type = bytes[3];
    let payload_len = decode_u32_be(&bytes[4..8])?;
    Ok(FrameHeader {
        magic,
        version,
        msg_type,
        payload_len,
    })
}

// ---------------------------------------------------------------------------
// Convenience frame builders
// ---------------------------------------------------------------------------

/// Encode a full Register message (type 0x01).
pub fn encode_register(peer_id: &str, pubkey: &[u8; 32]) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_string(&mut payload, peer_id);
    payload.extend_from_slice(pubkey);
    let mut frame = Vec::new();
    encode_header(&mut frame, msg_type::REGISTER, payload.len() as u32);
    frame.extend_from_slice(&payload);
    frame
}

/// Encode a full RegisterAck message (type 0x02).
pub fn encode_register_ack(assigned_id: &str, public_addr: &std::net::SocketAddr) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_string(&mut payload, assigned_id);
    encode_socketaddr(&mut payload, public_addr);
    let mut frame = Vec::new();
    encode_header(&mut frame, msg_type::REGISTER_ACK, payload.len() as u32);
    frame.extend_from_slice(&payload);
    frame
}

/// Encode a full Error message (type 0xFF).
pub fn encode_error(code: u16, msg: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_u16_be(&mut payload, code);
    encode_string(&mut payload, msg);
    let mut frame = Vec::new();
    encode_header(&mut frame, msg_type::ERROR, payload.len() as u32);
    frame.extend_from_slice(&payload);
    frame
}

/// Encode a full Heartbeat message (type 0x07, empty payload).
pub fn encode_heartbeat() -> Vec<u8> {
    let mut frame = Vec::new();
    encode_header(&mut frame, msg_type::HEARTBEAT, 0);
    frame
}

/// Encode a full Bye message (type 0x08, empty payload).
pub fn encode_bye() -> Vec<u8> {
    let mut frame = Vec::new();
    encode_header(&mut frame, msg_type::BYE, 0);
    frame
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // --- Primitive encoding ---

    #[test]
    fn encode_decode_u16_roundtrip() {
        let mut buf = Vec::new();
        encode_u16_be(&mut buf, 0x1234);
        assert_eq!(buf.len(), 2);
        assert_eq!(buf, [0x12, 0x34]);
        let v = decode_u16_be(&buf).unwrap();
        assert_eq!(v, 0x1234);
    }

    #[test]
    fn encode_decode_u16_zero() {
        let mut buf = Vec::new();
        encode_u16_be(&mut buf, 0);
        assert_eq!(buf, [0, 0]);
        assert_eq!(decode_u16_be(&buf).unwrap(), 0);
    }

    #[test]
    fn encode_decode_u16_max() {
        let mut buf = Vec::new();
        encode_u16_be(&mut buf, 0xFFFF);
        let v = decode_u16_be(&buf).unwrap();
        assert_eq!(v, 0xFFFF);
    }

    #[test]
    fn decode_u16_truncated() {
        let err = decode_u16_be(&[0x12]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn encode_decode_u32_roundtrip() {
        let mut buf = Vec::new();
        encode_u32_be(&mut buf, 0x12345678);
        assert_eq!(buf.len(), 4);
        assert_eq!(buf, [0x12, 0x34, 0x56, 0x78]);
        let v = decode_u32_be(&buf).unwrap();
        assert_eq!(v, 0x12345678);
    }

    #[test]
    fn encode_decode_u32_zero() {
        let mut buf = Vec::new();
        encode_u32_be(&mut buf, 0);
        assert_eq!(buf, [0, 0, 0, 0]);
        assert_eq!(decode_u32_be(&buf).unwrap(), 0);
    }

    #[test]
    fn decode_u32_truncated() {
        let err = decode_u32_be(&[0, 0, 0]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn encode_decode_string_roundtrip() {
        let mut buf = Vec::new();
        encode_string(&mut buf, "hello");
        assert_eq!(&buf[..2], [0, 5]); // length = 5
        assert_eq!(&buf[2..], b"hello");
        let (s, n) = decode_string(&buf).unwrap();
        assert_eq!(s, "hello");
        assert_eq!(n, 7);
    }

    #[test]
    fn encode_decode_string_empty() {
        let mut buf = Vec::new();
        encode_string(&mut buf, "");
        assert_eq!(&buf[..2], [0, 0]);
        let (s, n) = decode_string(&buf).unwrap();
        assert_eq!(s, "");
        assert_eq!(n, 2);
    }

    #[test]
    fn encode_decode_string_unicode() {
        let mut buf = Vec::new();
        encode_string(&mut buf, "你好");
        let (s, _) = decode_string(&buf).unwrap();
        assert_eq!(s, "你好");
    }

    #[test]
    fn encode_decode_string_long() {
        let s = "a".repeat(300);
        let mut buf = Vec::new();
        encode_string(&mut buf, &s);
        let (decoded, _) = decode_string(&buf).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn decode_string_truncated_len() {
        let err = decode_string(&[0]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn decode_string_truncated_content() {
        // length says 10 but only 5 bytes follow
        let err = decode_string(&[0, 10, b'h', b'e', b'l', b'l', b'o']).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn decode_string_invalid_utf8() {
        // length 4 with non-UTF-8 bytes
        let err = decode_string(&[0, 4, 0x80, 0x81, 0x82, 0x83]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn encode_decode_socketaddr_v4() {
        let addr: std::net::SocketAddr = "192.168.1.1:8080".parse().unwrap();
        let mut buf = Vec::new();
        encode_socketaddr(&mut buf, &addr);
        assert_eq!(buf.len(), 7); // 1 + 4 + 2
        assert_eq!(buf[0], 4);
        assert_eq!(&buf[1..5], [192, 168, 1, 1]);
        assert_eq!(&buf[5..7], [0x1F, 0x90]); // 8080
        let (decoded, n) = decode_socketaddr(&buf).unwrap();
        assert_eq!(n, 7);
        assert_eq!(decoded, &addr);
    }

    #[test]
    fn encode_decode_socketaddr_v6() {
        let addr: std::net::SocketAddr = "[::1]:12345".parse().unwrap();
        let mut buf = Vec::new();
        encode_socketaddr(&mut buf, &addr);
        assert_eq!(buf.len(), 19); // 1 + 16 + 2
        assert_eq!(buf[0], 6);
        let (decoded, n) = decode_socketaddr(&buf).unwrap();
        assert_eq!(n, 19);
        assert_eq!(decoded, &addr);
    }

    #[test]
    fn decode_socketaddr_truncated_v4() {
        let err = decode_socketaddr(&[4, 0, 0, 0]).unwrap_err(); // only 4 bytes
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn decode_socketaddr_truncated_v6() {
        let err = decode_socketaddr(&[6]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn decode_socketaddr_unknown_family() {
        let err = decode_socketaddr(&[5]).unwrap_err();
        assert!(err.to_string().contains("unknown address family"));
    }

    // --- Frame header ---

    #[test]
    fn encode_decode_header_roundtrip() {
        let mut buf = Vec::new();
        encode_header(&mut buf, msg_type::REGISTER, 42);
        assert_eq!(buf.len(), 8);
        assert_eq!(&buf[..2], [0x4F, 0x50]); // MAGIC
        assert_eq!(buf[2], VERSION);
        assert_eq!(buf[3], msg_type::REGISTER);
        let (h, n) = decode_header(&buf).map(|h| (h, 8)).unwrap();
        assert_eq!(n, 8);
        assert_eq!(h.magic, MAGIC);
        assert_eq!(h.version, VERSION);
        assert_eq!(h.msg_type, msg_type::REGISTER);
        assert_eq!(h.payload_len, 42);
    }

    #[test]
    fn encode_header_produces_correct_total_frame_len() {
        let payload_len = 7;
        let mut buf = Vec::new();
        encode_header(&mut buf, msg_type::HEARTBEAT, payload_len);
        assert_eq!(buf.len(), 8);
        // total frame = header(8) + payload
        assert_eq!(8 + payload_len, 15);
    }

    #[test]
    fn decode_header_truncated() {
        let err = decode_header(&[0x4F, 0x50, 0x01, 0x01, 0x00, 0x00, 0x00]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn decode_header_bad_magic() {
        let buf = [0x00, 0x00, 0x01, 0x01, 0, 0, 0, 0];
        let err = decode_header(&buf).unwrap_err();
        assert!(err.to_string().contains("bad magic"));
    }

    #[test]
    fn decode_header_bad_version() {
        let buf = [0x4F, 0x50, 0x99, 0x01, 0, 0, 0, 0];
        let err = decode_header(&buf).unwrap_err();
        assert!(err.to_string().contains("unsupported protocol version"));
    }

    // --- Convenience builders ---

    #[test]
    fn encode_register_roundtrip() {
        let pubkey = [0xAB_u8; 32];
        let frame = encode_register("alice", &pubkey);
        let header = decode_header(&frame).unwrap();
        assert_eq!(header.msg_type, msg_type::REGISTER);
        // payload: u16 len(2) + "alice"(5) + pubkey(32) = 39
        assert_eq!(header.payload_len, 39);
        let payload = &frame[8..];
        let (peer_id, n) = decode_string(payload).unwrap();
        assert_eq!(peer_id, "alice");
        assert_eq!(&payload[n..n + 32], &pubkey);
    }

    #[test]
    fn encode_register_ack_roundtrip() {
        let addr: std::net::SocketAddr = "1.2.3.4:9999".parse().unwrap();
        let frame = encode_register_ack("alice", &addr);
        let header = decode_header(&frame).unwrap();
        assert_eq!(header.msg_type, msg_type::REGISTER_ACK);
        let payload = &frame[8..];
        let (assigned_id, n) = decode_string(payload).unwrap();
        assert_eq!(assigned_id, "alice");
        let (decoded_addr, n2) = decode_socketaddr(&payload[n..]).unwrap();
        assert_eq!(decoded_addr, &addr);
        assert_eq!(header.payload_len as usize, n + n2);
    }

    #[test]
    fn encode_error_roundtrip() {
        let frame = encode_error(404, "peer not found");
        let header = decode_header(&frame).unwrap();
        assert_eq!(header.msg_type, msg_type::ERROR);
        let payload = &frame[8..];
        let code = decode_u16_be(payload).unwrap();
        assert_eq!(code, 404);
        let (msg, _) = decode_string(&payload[2..]).unwrap();
        assert_eq!(msg, "peer not found");
    }

    #[test]
    fn encode_heartbeat_has_no_payload() {
        let frame = encode_heartbeat();
        let header = decode_header(&frame).unwrap();
        assert_eq!(header.msg_type, msg_type::HEARTBEAT);
        assert_eq!(header.payload_len, 0);
        assert_eq!(frame.len(), 8);
    }

    #[test]
    fn encode_bye_has_no_payload() {
        let frame = encode_bye();
        let header = decode_header(&frame).unwrap();
        assert_eq!(header.msg_type, msg_type::BYE);
        assert_eq!(header.payload_len, 0);
        assert_eq!(frame.len(), 8);
    }

    // --- String edge cases ---
    #[test]
    fn encode_decode_string_ascii_edge_cases() {
        for s in ["", "a", "hello", "hello world", "a]b]c", "Rust 🦀"] {
            let mut buf = Vec::new();
            encode_string(&mut buf, s);
            let (decoded, n) = decode_string(&buf).unwrap();
            assert_eq!(decoded, s, "roundtrip failed for {:?}", s);
            assert_eq!(n, buf.len());
        }
    }

    // --- u32 edge cases ---
    #[test]
    fn encode_decode_u32_edge_cases() {
        for v in [
            0u32,
            1,
            127,
            128,
            255,
            256,
            65535,
            65536,
            1_000_000,
            u32::MAX,
        ] {
            let mut buf = Vec::new();
            encode_u32_be(&mut buf, v);
            let decoded = decode_u32_be(&buf).unwrap();
            assert_eq!(decoded, v);
        }
    }
}
