//! Framed TCP read/write helpers for the signaling protocol.
//!
//! Wraps a `TcpStream` with a length-prefixed frame layer.

use std::io::{self, Cursor, Read, Result as IoResult};

use crate::protocol::messages::{decode_header, encode_header, FrameHeader};

const HEADER_LEN: usize = 8;

/// Read the next frame from `stream`. Returns the header and a `Cursor<Bytes>`
/// positioned at the start of the payload.
pub fn read_frame<R: Read>(reader: &mut R) -> IoResult<(FrameHeader, Cursor<Vec<u8>>)> {
    let mut header_buf = [0u8; HEADER_LEN];
    reader.read_exact(&mut header_buf)?;
    let header = decode_header(&header_buf)?;

    let mut payload = vec![0u8; header.payload_len as usize];
    if !payload.is_empty() {
        reader.read_exact(&mut payload)?;
    }

    Ok((header, Cursor::new(payload)))
}

/// Write a raw frame to `writer` with the given `msg_type` and `payload`.
pub fn write_frame<W: io::Write>(writer: &mut W, msg_type: u8, payload: &[u8]) -> IoResult<()> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    encode_header(&mut frame, msg_type, payload.len() as u32);
    frame.extend_from_slice(payload);
    writer.write_all(&frame)
}

/// Read all bytes from `reader` until EOF.
pub fn read_to_end<R: Read>(reader: &mut R) -> IoResult<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

/// A frame decoder that handles partial reads.
pub struct FrameDecoder {
    /// True once we've seen a complete header.
    header_done: bool,
    /// The completed header, once known.
    header: Option<FrameHeader>,
    /// Bytes read but not yet processed.
    buf: Vec<u8>,
    /// How many bytes of the payload have been received.
    payload_received: usize,
}

impl FrameDecoder {
    /// Create a new frame decoder.
    pub fn new() -> Self {
        Self {
            header_done: false,
            header: None,
            buf: Vec::new(),
            payload_received: 0,
        }
    }

    /// Feed more bytes from the wire. Returns `Ok(Some((header, payload)))` when
    /// a complete frame is ready, `Ok(None)` if more bytes are needed.
    pub fn feed(&mut self, data: &[u8]) -> IoResult<Option<(FrameHeader, Vec<u8>)>> {
        self.buf.extend_from_slice(data);

        // Try to decode header first
        if !self.header_done {
            if self.buf.len() < HEADER_LEN {
                return Ok(None);
            }
            let header = decode_header(&self.buf[..HEADER_LEN])?;
            self.header = Some(header);
            self.header_done = true;
            // Only drain the header — keep any extra bytes (start of payload)
            self.buf.drain(..HEADER_LEN);
        }

        let payload_len = match self.header.as_ref() {
            Some(h) => h.payload_len as usize,
            None => return Ok(None),
        };
        let needed = payload_len - self.payload_received;

        if self.buf.len() < needed {
            return Ok(None);
        }

        // We have enough bytes for the complete payload
        let header = self.header.take().unwrap();
        let payload = self.buf[..needed].to_vec();
        self.payload_received = 0;
        self.header_done = false;
        self.buf.drain(..needed);

        Ok(Some((header, payload)))
    }

    /// Returns true if the decoder is idle (no partial state).
    pub fn is_idle(&self) -> bool {
        !self.header_done && self.header.is_none() && self.buf.is_empty()
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// A frame encoder that buffers complete frames.
pub struct FrameEncoder {
    buf: Vec<u8>,
    pos: usize,
}

impl FrameEncoder {
    /// Create a new encoder.
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            pos: 0,
        }
    }

    /// Enqueue a frame for sending.
    pub fn encode(&mut self, msg_type: u8, payload: &[u8]) {
        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        encode_header(&mut frame, msg_type, payload.len() as u32);
        frame.extend_from_slice(payload);
        self.buf.extend_from_slice(&frame);
    }

    /// Consume up to `n` bytes from the internal buffer. Returns how many
    /// bytes were consumed.
    pub fn drain_into(&mut self, out: &mut [u8]) -> IoResult<usize> {
        let available = self.buf.len() - self.pos;
        let to_write = available.min(out.len());
        out[..to_write].copy_from_slice(&self.buf[self.pos..self.pos + to_write]);
        self.pos += to_write;

        // Compact once fully drained
        if self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        }

        Ok(to_write)
    }

    /// Returns true if there is nothing left to drain.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// Total bytes buffered (sent + unsent).
    pub fn buffered(&self) -> usize {
        self.buf.len() - self.pos
    }
}

impl Default for FrameEncoder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_decoder_idle_when_new() {
        let dec = FrameDecoder::new();
        assert!(dec.is_idle());
    }

    #[test]
    fn frame_decoder_partial_header_returns_none() {
        let mut dec = FrameDecoder::new();
        let result = dec
            .feed(&[0x4F, 0x50, 0x01, 0x07, 0x00, 0x00, 0x00])
            .unwrap();
        assert!(result.is_none());
        assert!(!dec.is_idle());
    }

    #[test]
    fn frame_decoder_complete_header_idle() {
        let mut dec = FrameDecoder::new();
        // heartbeat frame: 8 header + 0 payload
        let frame = crate::protocol::encode_heartbeat();
        let result = dec.feed(&frame).unwrap();
        assert!(result.is_some());
        let (header, payload) = result.unwrap();
        assert_eq!(header.msg_type, crate::protocol::msg_type::HEARTBEAT);
        assert!(payload.is_empty());
        assert!(dec.is_idle());
    }

    #[test]
    fn frame_decoder_multiple_frames() {
        let mut dec = FrameDecoder::new();
        let hb = crate::protocol::encode_heartbeat();
        let bye = crate::protocol::encode_bye();
        let mut combined = hb.clone();
        combined.extend_from_slice(&bye);

        let (header1, payload1) = dec.feed(&combined[..hb.len()]).unwrap().unwrap();
        assert_eq!(header1.msg_type, crate::protocol::msg_type::HEARTBEAT);
        assert!(payload1.is_empty());

        let (header2, payload2) = dec.feed(&combined[hb.len()..]).unwrap().unwrap();
        assert_eq!(header2.msg_type, crate::protocol::msg_type::BYE);
        assert!(payload2.is_empty());
    }

    #[test]
    fn frame_decoder_partial_then_complete() {
        let mut dec = FrameDecoder::new();
        // Heartbeat frame: 8 header + 0 payload = 8 bytes total.
        // Feed just the header — decoder should be idle waiting for payload.
        let hb = crate::protocol::encode_heartbeat();
        assert_eq!(hb.len(), 8);

        let result = dec.feed(&hb[..4]).unwrap();
        assert!(result.is_none());
        assert!(
            !dec.is_idle(),
            "should not be idle — waiting for 0 payload bytes"
        );

        // Now feed the remaining 4 header bytes — completes the frame
        let result = dec.feed(&hb[4..]).unwrap();
        assert!(result.is_some());
        let (header, payload) = result.unwrap();
        assert_eq!(header.msg_type, crate::protocol::msg_type::HEARTBEAT);
        assert!(payload.is_empty());
        assert!(dec.is_idle(), "decoder should be idle after complete frame");
    }

    #[test]
    fn frame_decoder_single_chunk_completes() {
        let mut dec = FrameDecoder::new();
        let hb = crate::protocol::encode_heartbeat();
        // Feed whole frame in one call
        let result = dec.feed(&hb).unwrap().unwrap();
        assert_eq!(result.0.msg_type, crate::protocol::msg_type::HEARTBEAT);
        assert!(result.1.is_empty());
        assert!(dec.is_idle());
    }

    #[test]
    fn frame_encoder_encode_then_drain() {
        let mut enc = FrameEncoder::new();
        enc.encode(crate::protocol::msg_type::HEARTBEAT, &[]);

        let mut out = [0u8; 64];
        let n = enc.drain_into(&mut out).unwrap();
        assert_eq!(n, 8); // header only for heartbeat
        assert_eq!(&out[..2], &[0x4F, 0x50]);

        assert!(enc.is_empty());
        assert_eq!(enc.buffered(), 0);
    }

    #[test]
    fn frame_encoder_drain_in_chunks() {
        let mut enc = FrameEncoder::new();
        enc.encode(crate::protocol::msg_type::HEARTBEAT, &[]);

        let mut out = [0u8; 3];
        let n1 = enc.drain_into(&mut out).unwrap();
        assert_eq!(n1, 3);
        assert_eq!(enc.buffered(), 5);

        let mut out2 = [0u8; 10];
        let n2 = enc.drain_into(&mut out2).unwrap();
        assert_eq!(n2, 5);
        assert!(enc.is_empty());
    }

    #[test]
    fn frame_encoder_multiple_frames() {
        let mut enc = FrameEncoder::new();
        enc.encode(crate::protocol::msg_type::HEARTBEAT, &[]);
        enc.encode(crate::protocol::msg_type::BYE, &[]);

        let mut all = Vec::new();
        let mut buf = [0u8; 64];
        while !enc.is_empty() {
            let n = enc.drain_into(&mut buf).unwrap();
            all.extend_from_slice(&buf[..n]);
        }

        assert_eq!(all.len(), 16); // 2 × 8-byte headers
        assert_eq!(&all[0..2], &[0x4F, 0x50]);
        assert_eq!(all[3], crate::protocol::msg_type::HEARTBEAT);
        assert_eq!(&all[8..10], &[0x4F, 0x50]);
        assert_eq!(all[11], crate::protocol::msg_type::BYE);
    }

    #[test]
    fn roundtrip_frame_via_encoder_decoder() {
        let mut enc = FrameEncoder::new();
        let original_payload = b"hello world";
        enc.encode(0x07, original_payload);

        let mut raw = Vec::new();
        let mut buf = [0u8; 64];
        while !enc.is_empty() {
            let n = enc.drain_into(&mut buf).unwrap();
            raw.extend_from_slice(&buf[..n]);
        }

        let mut dec = FrameDecoder::new();
        let (header, payload) = dec.feed(&raw).unwrap().unwrap();
        assert_eq!(header.msg_type, 0x07);
        assert_eq!(&payload[..], original_payload);
        assert!(dec.is_idle());
    }
}
