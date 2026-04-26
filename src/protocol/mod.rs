//! Wire-format signaling protocol: message types + binary codec.
//!
//! ## Frame format
//! - Header: 2-byte MAGIC + 1-byte VER + 1-byte MSG_TY + 4-byte u32 BE LEN
//! - Payload: LEN bytes of encoded message data
//!
//! ## Constants
//! - `MAGIC` = 0x4F50
//! - `VERSION` = 0x01
//!
//! ## Exports
//! - [`msg_type`] — message type code constants
//! - [`messages`] — message builders and primitive codecs
//! - [`codec`] — framed `TcpStream` read/write helpers

pub mod codec;
pub mod messages;

pub use messages::msg_type;
pub use messages::{
    decode_header, decode_socketaddr, decode_string, decode_u16_be, decode_u32_be, encode_bye,
    encode_error, encode_header, encode_heartbeat, encode_register, encode_register_ack,
    encode_socketaddr, encode_string, encode_u16_be, encode_u32_be, FrameHeader, MAGIC, VERSION,
};
