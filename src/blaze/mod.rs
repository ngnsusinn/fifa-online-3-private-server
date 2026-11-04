pub mod server;
pub mod tdf;

use byteorder::{BigEndian, ByteOrder};

pub const HEADER_MIN: usize = 12;

pub const OPT_JUMBO_FRAME: u8 = 0x1;
pub const OPT_HAS_CONTEXT: u8 = 0x2;
pub const OPT_IMMEDIATE: u8 = 0x4;
pub const OPT_JUMBO_CONTEXT: u8 = 0x8;

pub const MSG_MESSAGE: u8 = 0;
pub const MSG_REPLY: u8 = 1;
pub const MSG_NOTIFICATION: u8 = 2;
pub const MSG_ERROR_REPLY: u8 = 3;
pub const MSG_PING: u8 = 4;
pub const MSG_PING_REPLY: u8 = 5;

// Component IDs
pub const COMP_AUTHENTICATION: u16 = 0x0001;
pub const COMP_GAME_MANAGER: u16 = 0x0004;
pub const COMP_REDIRECTOR: u16 = 0x0005;
pub const COMP_UTIL: u16 = 0x0009;

// Command IDs
pub const CMD_CREATE_GAME: u16 = 0x0001;
pub const CMD_JOIN_GAME: u16 = 0x0002;
pub const CMD_START_MATCHMAKING: u16 = 0x000A;

pub const CMD_LOGIN: u16 = 0x000C;
pub const CMD_LOGIN_PERSONA: u16 = 0x006E;

pub const CMD_GET_SERVER_INSTANCE: u16 = 0x0001;

pub const CMD_PING: u16 = 0x0002;
pub const CMD_PRE_AUTH: u16 = 0x0007;

// Default Ports
pub const DEFAULT_REDIR_PORT: u16 = 28000;
pub const DEFAULT_NOTI_PORT: u16 = 28001;

pub fn msg_type_name(msg_type: u8) -> &'static str {
    match msg_type {
        0 => "MESSAGE",
        1 => "REPLY",
        2 => "NOTIFICATION",
        3 => "ERROR_REPLY",
        4 => "PING",
        5 => "PING_REPLY",
        _ => "UNKNOWN",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub length: usize,
    pub component: u16,
    pub command: u16,
    pub error: u16,
    pub msgtype: u8,
    pub msgid: u32,
    pub userindex: u8,
    pub options: u8,
    pub header_len: usize,
}

impl Header {
    pub fn msgtype_name(&self) -> &'static str {
        msg_type_name(self.msgtype)
    }
}

pub fn parse_header(buf: &[u8]) -> Option<Header> {
    if buf.len() < HEADER_MIN {
        return None;
    }
    let size_lo = BigEndian::read_u16(&buf[0..2]) as usize;
    let component = BigEndian::read_u16(&buf[2..4]);
    let command = BigEndian::read_u16(&buf[4..6]);
    let error = BigEndian::read_u16(&buf[6..8]);
    let b8 = buf[8];
    let b9 = buf[9];
    let msgnum_lo = BigEndian::read_u16(&buf[10..12]) as u32;

    let msgtype = (b8 >> 4) & 0x0F;
    let userindex = b8 & 0x0F;
    let options = (b9 >> 4) & 0x0F;
    let msgnum = (((b9 & 0x0F) as u32) << 16) | msgnum_lo;

    let mut off = HEADER_MIN;
    let mut size_hi = 0usize;

    if (options & OPT_JUMBO_FRAME) != 0 {
        if buf.len() < off + 2 {
            return None;
        }
        size_hi = BigEndian::read_u16(&buf[off..off + 2]) as usize;
        off += 2;
    }

    if (options & OPT_HAS_CONTEXT) != 0 {
        let ctx_len = if (options & OPT_JUMBO_CONTEXT) != 0 { 8 } else { 4 };
        if buf.len() < off + ctx_len {
            return None;
        }
        off += ctx_len;
    }

    Some(Header {
        length: (size_hi << 16) | size_lo,
        component,
        command,
        error,
        msgtype,
        msgid: msgnum,
        userindex,
        options,
        header_len: off,
    })
}

pub fn frames_and_rest(buf: &[u8]) -> (Vec<(Header, Vec<u8>)>, Vec<u8>) {
    let mut frames = Vec::new();
    let mut off = 0;
    while off < buf.len() {
        if let Some(hdr) = parse_header(&buf[off..]) {
            let start = off + hdr.header_len;
            let end = start + hdr.length;
            if end > buf.len() {
                break;
            }
            frames.push((hdr, buf[start..end].to_vec()));
            off = end;
        } else {
            break;
        }
    }
    (frames, buf[off..].to_vec())
}

pub fn describe(hdr: &Header, payload: &[u8]) -> String {
    let hex_part = if payload.len() > 48 {
        format!("{} ...", hex::encode(&payload[..48]))
    } else {
        hex::encode(payload)
    };
    format!(
        "comp=0x{:04x} cmd=0x{:04x} {} id={} err={} len={} payload={}",
        hdr.component,
        hdr.command,
        hdr.msgtype_name(),
        hdr.msgid,
        hdr.error,
        hdr.length,
        hex_part
    )
}

pub fn build_frame(
    component: u16,
    command: u16,
    msgtype: u8,
    msgid: u32,
    body: &[u8],
    error: u16,
) -> Vec<u8> {
    let size = body.len();
    if size > 0xFFFF {
        panic!("body >64KiB needs JUMBO_FRAME encoding (not implemented)");
    }
    let mut frame = Vec::with_capacity(HEADER_MIN + size);
    let mut u16_buf = [0u8; 2];

    BigEndian::write_u16(&mut u16_buf, size as u16);
    frame.extend_from_slice(&u16_buf);

    BigEndian::write_u16(&mut u16_buf, component);
    frame.extend_from_slice(&u16_buf);

    BigEndian::write_u16(&mut u16_buf, command);
    frame.extend_from_slice(&u16_buf);

    BigEndian::write_u16(&mut u16_buf, error);
    frame.extend_from_slice(&u16_buf);

    let b8 = (msgtype & 0x0F) << 4;
    let b9 = ((msgid >> 16) & 0x0F) as u8;
    frame.push(b8);
    frame.push(b9);

    BigEndian::write_u16(&mut u16_buf, (msgid & 0xFFFF) as u16);
    frame.extend_from_slice(&u16_buf);

    frame.extend_from_slice(body);
    frame
}

// Simple hex helper for display
pub mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
    }
}
