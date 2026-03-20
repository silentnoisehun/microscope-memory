//! SHP (Silent Hope Protocol) — binary wire format.
//!
//! Request:  [MSHP:4][cmd:1][payload_len:4][genome_hash:32][payload...]
//! Response: [MSHR:4][status:1][payload_len:4][genome_hash:32][payload...]
//!
//! All integers are little-endian. No JSON. No serde.

pub const SHP_REQUEST_MAGIC: [u8; 4] = *b"MSHP";
pub const SHP_RESPONSE_MAGIC: [u8; 4] = *b"MSHR";
pub const SHP_HEADER_SIZE: usize = 41; // 4 + 1 + 4 + 32
pub const SHP_DEFAULT_PORT: u16 = 7946;
pub const SHP_MAX_PAYLOAD: u32 = 1 << 20; // 1 MB

// ── Commands ──

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Ping    = 0x01,
    Store   = 0x02,
    Recall  = 0x03,
    Look    = 0x04,
    Find    = 0x05,
    Verify  = 0x06,
    Stats   = 0x07,
    Teach   = 0x08,
    /// Look returning raw SHP v1.0 packets (372 bytes each, zero-copy).
    LookPacket = 0x09,
}

impl Command {
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::Ping),
            0x02 => Some(Self::Store),
            0x03 => Some(Self::Recall),
            0x04 => Some(Self::Look),
            0x05 => Some(Self::Find),
            0x06 => Some(Self::Verify),
            0x07 => Some(Self::Stats),
            0x08 => Some(Self::Teach),
            0x09 => Some(Self::LookPacket),
            _ => None,
        }
    }
}

// ── Status ──

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok              = 0x00,
    Error           = 0x01,
    GenomeMismatch  = 0x02,
    InvalidCommand  = 0x03,
    PayloadTooLarge = 0x04,
    TeachApproved   = 0x05,
    TeachDenied     = 0x06,
}

impl Status {
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::Ok),
            0x01 => Some(Self::Error),
            0x02 => Some(Self::GenomeMismatch),
            0x03 => Some(Self::InvalidCommand),
            0x04 => Some(Self::PayloadTooLarge),
            0x05 => Some(Self::TeachApproved),
            0x06 => Some(Self::TeachDenied),
            _ => None,
        }
    }
}

// ── Headers ──

pub struct RequestHeader {
    pub cmd: Command,
    pub payload_len: u32,
    pub genome_hash: [u8; 32],
}

pub struct ResponseHeader {
    pub status: Status,
    pub payload_len: u32,
    pub genome_hash: [u8; 32],
}

impl RequestHeader {
    pub fn to_bytes(&self) -> [u8; SHP_HEADER_SIZE] {
        let mut buf = [0u8; SHP_HEADER_SIZE];
        buf[0..4].copy_from_slice(&SHP_REQUEST_MAGIC);
        buf[4] = self.cmd as u8;
        buf[5..9].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[9..41].copy_from_slice(&self.genome_hash);
        buf
    }

    pub fn from_bytes(buf: &[u8; SHP_HEADER_SIZE]) -> Option<Self> {
        if buf[0..4] != SHP_REQUEST_MAGIC {
            return None;
        }
        let cmd = Command::from_u8(buf[4])?;
        let payload_len = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        let mut genome_hash = [0u8; 32];
        genome_hash.copy_from_slice(&buf[9..41]);
        Some(RequestHeader { cmd, payload_len, genome_hash })
    }
}

impl ResponseHeader {
    pub fn to_bytes(&self) -> [u8; SHP_HEADER_SIZE] {
        let mut buf = [0u8; SHP_HEADER_SIZE];
        buf[0..4].copy_from_slice(&SHP_RESPONSE_MAGIC);
        buf[4] = self.status as u8;
        buf[5..9].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[9..41].copy_from_slice(&self.genome_hash);
        buf
    }

    pub fn from_bytes(buf: &[u8; SHP_HEADER_SIZE]) -> Option<Self> {
        if buf[0..4] != SHP_RESPONSE_MAGIC {
            return None;
        }
        let status = Status::from_u8(buf[4])?;
        let payload_len = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        let mut genome_hash = [0u8; 32];
        genome_hash.copy_from_slice(&buf[9..41]);
        Some(ResponseHeader { status, payload_len, genome_hash })
    }
}

// ── Result entry (used in Recall/Look/Find responses) ──

#[derive(Debug, Clone)]
pub struct ResultEntry {
    pub distance: f32,
    pub depth: u8,
    pub layer_id: u8,
    pub text: String,
}

// ── Payload encoding/decoding ──

/// Encode Store payload: [layer_id:u8][importance:u8][text_len:u16 LE][text...]
pub fn encode_store(layer_id: u8, importance: u8, text: &str) -> Vec<u8> {
    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(u16::MAX as usize) as u16;
    let mut buf = Vec::with_capacity(4 + text_bytes.len());
    buf.push(layer_id);
    buf.push(importance);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&text_bytes[..len as usize]);
    buf
}

/// Decode Store payload.
pub fn decode_store(payload: &[u8]) -> Option<(u8, u8, String)> {
    if payload.len() < 4 { return None; }
    let layer_id = payload[0];
    let importance = payload[1];
    let text_len = u16::from_le_bytes([payload[2], payload[3]]) as usize;
    if payload.len() < 4 + text_len { return None; }
    let text = String::from_utf8_lossy(&payload[4..4 + text_len]).into_owned();
    Some((layer_id, importance, text))
}

/// Encode query payload: [query_len:u16 LE][query...][k:u16 LE]
pub fn encode_query(query: &str, k: u16) -> Vec<u8> {
    let qb = query.as_bytes();
    let qlen = qb.len().min(u16::MAX as usize) as u16;
    let mut buf = Vec::with_capacity(4 + qb.len());
    buf.extend_from_slice(&qlen.to_le_bytes());
    buf.extend_from_slice(&qb[..qlen as usize]);
    buf.extend_from_slice(&k.to_le_bytes());
    buf
}

/// Decode query payload.
pub fn decode_query(payload: &[u8]) -> Option<(String, u16)> {
    if payload.len() < 4 { return None; }
    let qlen = u16::from_le_bytes([payload[0], payload[1]]) as usize;
    if payload.len() < 2 + qlen + 2 { return None; }
    let query = String::from_utf8_lossy(&payload[2..2 + qlen]).into_owned();
    let k = u16::from_le_bytes([payload[2 + qlen], payload[3 + qlen]]);
    Some((query, k))
}

/// Encode Look payload: [x:f32][y:f32][z:f32][zoom:u8][k:u16 LE]
pub fn encode_look(x: f32, y: f32, z: f32, zoom: u8, k: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(15);
    buf.extend_from_slice(&x.to_le_bytes());
    buf.extend_from_slice(&y.to_le_bytes());
    buf.extend_from_slice(&z.to_le_bytes());
    buf.push(zoom);
    buf.extend_from_slice(&k.to_le_bytes());
    buf
}

/// Decode Look payload.
pub fn decode_look(payload: &[u8]) -> Option<(f32, f32, f32, u8, u16)> {
    if payload.len() < 15 { return None; }
    let x = f32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let y = f32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let z = f32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let zoom = payload[12];
    let k = u16::from_le_bytes([payload[13], payload[14]]);
    Some((x, y, z, zoom, k))
}

/// Encode Teach payload: [q_len:u16][query...][r_len:u16][response...]
pub fn encode_teach(query: &str, response: &str) -> Vec<u8> {
    let qb = query.as_bytes();
    let rb = response.as_bytes();
    let qlen = qb.len().min(u16::MAX as usize) as u16;
    let rlen = rb.len().min(u16::MAX as usize) as u16;
    let mut buf = Vec::with_capacity(4 + qb.len() + rb.len());
    buf.extend_from_slice(&qlen.to_le_bytes());
    buf.extend_from_slice(&qb[..qlen as usize]);
    buf.extend_from_slice(&rlen.to_le_bytes());
    buf.extend_from_slice(&rb[..rlen as usize]);
    buf
}

/// Decode Teach payload.
pub fn decode_teach(payload: &[u8]) -> Option<(String, String)> {
    if payload.len() < 4 { return None; }
    let qlen = u16::from_le_bytes([payload[0], payload[1]]) as usize;
    if payload.len() < 2 + qlen + 2 { return None; }
    let query = String::from_utf8_lossy(&payload[2..2 + qlen]).into_owned();
    let roff = 2 + qlen;
    let rlen = u16::from_le_bytes([payload[roff], payload[roff + 1]]) as usize;
    if payload.len() < roff + 2 + rlen { return None; }
    let response = String::from_utf8_lossy(&payload[roff + 2..roff + 2 + rlen]).into_owned();
    Some((query, response))
}

/// Encode result entries: [count:u16][{dist:f32, depth:u8, layer:u8, len:u16, text...}...]
pub fn encode_results(results: &[ResultEntry]) -> Vec<u8> {
    let count = results.len().min(u16::MAX as usize) as u16;
    let mut buf = Vec::new();
    buf.extend_from_slice(&count.to_le_bytes());
    for r in results.iter().take(count as usize) {
        buf.extend_from_slice(&r.distance.to_le_bytes());
        buf.push(r.depth);
        buf.push(r.layer_id);
        let tb = r.text.as_bytes();
        let tlen = tb.len().min(u16::MAX as usize) as u16;
        buf.extend_from_slice(&tlen.to_le_bytes());
        buf.extend_from_slice(&tb[..tlen as usize]);
    }
    buf
}

/// Decode result entries.
pub fn decode_results(payload: &[u8]) -> Vec<ResultEntry> {
    if payload.len() < 2 { return Vec::new(); }
    let count = u16::from_le_bytes([payload[0], payload[1]]) as usize;
    let mut results = Vec::with_capacity(count);
    let mut off = 2;
    for _ in 0..count {
        if off + 8 > payload.len() { break; }
        let distance = f32::from_le_bytes([payload[off], payload[off+1], payload[off+2], payload[off+3]]);
        let depth = payload[off + 4];
        let layer_id = payload[off + 5];
        let tlen = u16::from_le_bytes([payload[off + 6], payload[off + 7]]) as usize;
        off += 8;
        if off + tlen > payload.len() { break; }
        let text = String::from_utf8_lossy(&payload[off..off + tlen]).into_owned();
        off += tlen;
        results.push(ResultEntry { distance, depth, layer_id, text });
    }
    results
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_header_roundtrip() {
        let hdr = RequestHeader {
            cmd: Command::Recall,
            payload_len: 12345,
            genome_hash: [0xAB; 32],
        };
        let bytes = hdr.to_bytes();
        let parsed = RequestHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.cmd, Command::Recall);
        assert_eq!(parsed.payload_len, 12345);
        assert_eq!(parsed.genome_hash, [0xAB; 32]);
    }

    #[test]
    fn response_header_roundtrip() {
        let hdr = ResponseHeader {
            status: Status::TeachDenied,
            payload_len: 999,
            genome_hash: [0xCD; 32],
        };
        let bytes = hdr.to_bytes();
        let parsed = ResponseHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.status, Status::TeachDenied);
        assert_eq!(parsed.payload_len, 999);
    }

    #[test]
    fn invalid_magic_rejected() {
        let mut bytes = [0u8; SHP_HEADER_SIZE];
        bytes[0..4].copy_from_slice(b"XXXX");
        assert!(RequestHeader::from_bytes(&bytes).is_none());
        assert!(ResponseHeader::from_bytes(&bytes).is_none());
    }

    #[test]
    fn store_roundtrip() {
        let encoded = encode_store(3, 7, "hello world");
        let (lid, imp, text) = decode_store(&encoded).unwrap();
        assert_eq!(lid, 3);
        assert_eq!(imp, 7);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn query_roundtrip() {
        let encoded = encode_query("rust memory", 10);
        let (query, k) = decode_query(&encoded).unwrap();
        assert_eq!(query, "rust memory");
        assert_eq!(k, 10);
    }

    #[test]
    fn look_roundtrip() {
        let encoded = encode_look(0.25, 0.5, 0.75, 3, 5);
        let (x, y, z, zoom, k) = decode_look(&encoded).unwrap();
        assert!((x - 0.25).abs() < f32::EPSILON);
        assert!((y - 0.5).abs() < f32::EPSILON);
        assert!((z - 0.75).abs() < f32::EPSILON);
        assert_eq!(zoom, 3);
        assert_eq!(k, 5);
    }

    #[test]
    fn teach_roundtrip() {
        let encoded = encode_teach("What is Ora?", "Ora is an AI partner");
        let (q, r) = decode_teach(&encoded).unwrap();
        assert_eq!(q, "What is Ora?");
        assert_eq!(r, "Ora is an AI partner");
    }

    #[test]
    fn results_roundtrip() {
        let entries = vec![
            ResultEntry { distance: 0.123, depth: 3, layer_id: 1, text: "hello".into() },
            ResultEntry { distance: 0.456, depth: 5, layer_id: 2, text: "world".into() },
        ];
        let encoded = encode_results(&entries);
        let decoded = decode_results(&encoded);
        assert_eq!(decoded.len(), 2);
        assert!((decoded[0].distance - 0.123).abs() < f32::EPSILON);
        assert_eq!(decoded[0].text, "hello");
        assert_eq!(decoded[1].depth, 5);
        assert_eq!(decoded[1].text, "world");
    }

    #[test]
    fn command_from_u8_all() {
        assert_eq!(Command::from_u8(0x01), Some(Command::Ping));
        assert_eq!(Command::from_u8(0x08), Some(Command::Teach));
        assert_eq!(Command::from_u8(0x00), None);
        assert_eq!(Command::from_u8(0xFF), None);
    }

    #[test]
    fn status_from_u8_all() {
        assert_eq!(Status::from_u8(0x00), Some(Status::Ok));
        assert_eq!(Status::from_u8(0x06), Some(Status::TeachDenied));
        assert_eq!(Status::from_u8(0xFF), None);
    }

    #[test]
    fn empty_payload_decode() {
        assert!(decode_store(&[]).is_none());
        assert!(decode_query(&[]).is_none());
        assert!(decode_look(&[]).is_none());
        assert!(decode_teach(&[]).is_none());
        assert!(decode_results(&[]).is_empty());
    }
}
