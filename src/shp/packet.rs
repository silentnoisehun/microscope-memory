//! SHP v1.0 — zero-copy binary packet format.
//!
//! Fixed 372-byte packet for carrying memory blocks over the wire.
//! Every field is at a known offset — no parsing, no allocation, mmap-ready.
//!
//! | Offset | Type      | Name         | Description                              |
//! |--------|-----------|--------------|------------------------------------------|
//! | 0-3    | [u8; 4]   | MAGIC        | "SHP!" (0x53 0x48 0x50 0x21)             |
//! | 4-35   | [u8; 32]  | GENOME_ROOT  | Hope Genome Merkle root hash             |
//! | 36-47  | (f32,f32,f32) | COORDS   | 3D spatial coordinates (X, Y, Z)         |
//! | 48-51  | f32       | ZOOM         | Depth level normalized (0.0-1.0)         |
//! | 52-83  | [u8; 32]  | BLOCK_HASH   | SHA-256 of DATA                          |
//! | 84-339 | [u8; 256] | DATA         | Raw memory block (UTF-8 text, zero-padded)|
//! | 340-371| [u8; 32]  | MERKLE_PROOF | Merkle branch proof to root              |
//!
//! Total: 372 bytes. Fixed. No JSON. No serde. Zero-copy.

use sha2::{Sha256, Digest};

/// SHP v1.0 magic bytes: "SHP!" (0x53 0x48 0x50 0x21)
pub const SHP_MAGIC: [u8; 4] = [0x53, 0x48, 0x50, 0x21];

/// Total packet size in bytes.
pub const SHP_PACKET_SIZE: usize = 372;

/// The fixed-size SHP v1.0 packet.
///
/// `#[repr(C, packed)]` ensures zero-copy compatibility with mmap/network buffers.
/// Every field at a deterministic offset — no padding, no alignment games.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ShpPacket {
    /// Magic bytes: "SHP!" — identifies this as an SHP v1.0 packet.
    pub magic: [u8; 4],
    /// Hope Genome Merkle root hash — authenticates the sender.
    /// If this doesn't match the receiver's compiled genome root, the packet is dropped.
    pub genome_root: [u8; 32],
    /// 3D spatial coordinates of the memory block.
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Normalized zoom/depth level (depth / 8.0, range 0.0-1.0).
    pub zoom: f32,
    /// SHA-256 hash of the DATA field — content integrity.
    pub block_hash: [u8; 32],
    /// The raw memory block data (256 bytes, UTF-8 text, zero-padded).
    pub data: [u8; 256],
    /// Merkle branch proof — proves this block is part of the global tree.
    pub merkle_proof: [u8; 32],
}

// Compile-time size assertion
const _: () = assert!(std::mem::size_of::<ShpPacket>() == SHP_PACKET_SIZE);

impl ShpPacket {
    /// Create a new SHP packet from a memory block.
    pub fn new(
        genome_root: [u8; 32],
        x: f32, y: f32, z: f32,
        zoom: f32,
        text: &str,
        merkle_proof: [u8; 32],
    ) -> Self {
        let mut data = [0u8; 256];
        let bytes = text.as_bytes();
        let len = bytes.len().min(256);
        data[..len].copy_from_slice(&bytes[..len]);

        let block_hash = sha256_data(&data);

        ShpPacket {
            magic: SHP_MAGIC,
            genome_root,
            x, y, z,
            zoom,
            block_hash,
            data,
            merkle_proof,
        }
    }

    /// Interpret a raw byte buffer as an SHP packet (zero-copy).
    ///
    /// Returns `None` if:
    /// - Buffer is too small
    /// - Magic bytes don't match
    pub fn from_bytes(buf: &[u8; SHP_PACKET_SIZE]) -> Option<&ShpPacket> {
        if buf[0..4] != SHP_MAGIC {
            return None;
        }
        // SAFETY: ShpPacket is #[repr(C, packed)] and exactly SHP_PACKET_SIZE bytes.
        // The buffer is properly sized. All bit patterns are valid for the field types.
        Some(unsafe { &*(buf.as_ptr() as *const ShpPacket) })
    }

    /// Serialize this packet to a byte array.
    pub fn to_bytes(&self) -> [u8; SHP_PACKET_SIZE] {
        let mut buf = [0u8; SHP_PACKET_SIZE];
        // SAFETY: ShpPacket is #[repr(C, packed)] and exactly SHP_PACKET_SIZE bytes.
        let src = unsafe {
            std::slice::from_raw_parts(
                self as *const ShpPacket as *const u8,
                SHP_PACKET_SIZE,
            )
        };
        buf.copy_from_slice(src);
        buf
    }

    /// Validate the packet's integrity.
    pub fn validate(&self) -> PacketValidation {
        let mut result = PacketValidation {
            magic_ok: self.magic == SHP_MAGIC,
            hash_ok: false,
            genome_ok: false,
        };

        // Verify block hash matches data
        let computed = sha256_data(&self.data);
        result.hash_ok = computed == self.block_hash;

        result
    }

    /// Validate against a known genome root.
    pub fn validate_with_genome(&self, expected_root: &[u8; 32]) -> PacketValidation {
        let mut v = self.validate();
        v.genome_ok = self.genome_root == *expected_root;
        v
    }

    /// Extract the text content from the DATA field (strips zero padding).
    pub fn text(&self) -> &str {
        let end = self.data.iter().position(|&b| b == 0).unwrap_or(256);
        std::str::from_utf8(&self.data[..end]).unwrap_or("")
    }

    /// Read coordinates as a tuple.
    pub fn coords(&self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }
}

/// Result of packet validation.
#[derive(Debug, Clone, Copy)]
pub struct PacketValidation {
    /// Magic bytes are "SHP!"
    pub magic_ok: bool,
    /// SHA-256 of DATA matches BLOCK_HASH
    pub hash_ok: bool,
    /// GENOME_ROOT matches expected genome root
    pub genome_ok: bool,
}

impl PacketValidation {
    /// True if all checks pass.
    pub fn is_valid(&self) -> bool {
        self.magic_ok && self.hash_ok && self.genome_ok
    }
}

/// Build an ShpPacket from a MicroscopeReader block.
pub fn packet_from_block(
    reader: &crate::MicroscopeReader,
    block_idx: usize,
    genome_root: [u8; 32],
    merkle_proof: [u8; 32],
) -> ShpPacket {
    let hdr = reader.header(block_idx);
    let text = reader.text(block_idx);
    ShpPacket::new(
        genome_root,
        hdr.x, hdr.y, hdr.z,
        hdr.zoom,
        text,
        merkle_proof,
    )
}

/// SHA-256 of a 256-byte data block.
fn sha256_data(data: &[u8; 256]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_size_is_372() {
        assert_eq!(std::mem::size_of::<ShpPacket>(), 372);
        assert_eq!(SHP_PACKET_SIZE, 372);
    }

    #[test]
    fn packet_roundtrip() {
        let genome = [0xAB; 32];
        let merkle = [0xCD; 32];
        let pkt = ShpPacket::new(genome, 0.25, 0.5, 0.75, 0.375, "hello world", merkle);

        let bytes = pkt.to_bytes();
        let parsed = ShpPacket::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.magic, SHP_MAGIC);
        assert_eq!(parsed.genome_root, genome);
        assert!((parsed.x - 0.25).abs() < f32::EPSILON);
        assert!((parsed.y - 0.5).abs() < f32::EPSILON);
        assert!((parsed.z - 0.75).abs() < f32::EPSILON);
        assert!((parsed.zoom - 0.375).abs() < f32::EPSILON);
        assert_eq!(parsed.text(), "hello world");
        assert_eq!(parsed.merkle_proof, merkle);
    }

    #[test]
    fn packet_hash_integrity() {
        let pkt = ShpPacket::new([0; 32], 0.0, 0.0, 0.0, 0.0, "test data", [0; 32]);
        let v = pkt.validate();
        assert!(v.magic_ok);
        assert!(v.hash_ok);
    }

    #[test]
    fn packet_tampered_data_detected() {
        let mut pkt = ShpPacket::new([0; 32], 0.0, 0.0, 0.0, 0.0, "original", [0; 32]);
        // Tamper with data
        pkt.data[0] = b'X';
        let v = pkt.validate();
        assert!(v.magic_ok);
        assert!(!v.hash_ok, "tampered data should fail hash check");
    }

    #[test]
    fn packet_bad_magic_rejected() {
        let mut bytes = [0u8; SHP_PACKET_SIZE];
        bytes[0..4].copy_from_slice(b"XXXX");
        assert!(ShpPacket::from_bytes(&bytes).is_none());
    }

    #[test]
    fn packet_genome_validation() {
        let genome = [0xAB; 32];
        let pkt = ShpPacket::new(genome, 0.0, 0.0, 0.0, 0.0, "test", [0; 32]);

        let v_good = pkt.validate_with_genome(&genome);
        assert!(v_good.genome_ok);

        let v_bad = pkt.validate_with_genome(&[0xFF; 32]);
        assert!(!v_bad.genome_ok);
    }

    #[test]
    fn packet_zero_padded_text() {
        let pkt = ShpPacket::new([0; 32], 0.0, 0.0, 0.0, 0.0, "short", [0; 32]);
        assert_eq!(pkt.text(), "short");
        // Verify rest is zero-padded
        assert_eq!(pkt.data[5], 0);
        assert_eq!(pkt.data[255], 0);
    }

    #[test]
    fn packet_max_data() {
        let long_text = "A".repeat(300); // longer than 256
        let pkt = ShpPacket::new([0; 32], 0.0, 0.0, 0.0, 0.0, &long_text, [0; 32]);
        assert_eq!(pkt.text().len(), 256); // truncated to 256
    }

    #[test]
    fn packet_full_validation() {
        let genome = [0xAB; 32];
        let pkt = ShpPacket::new(genome, 0.1, 0.2, 0.3, 0.5, "validated packet", [0xCD; 32]);
        let v = pkt.validate_with_genome(&genome);
        assert!(v.is_valid(), "full validation should pass");
    }

    #[test]
    fn packet_coords_readback() {
        let pkt = ShpPacket::new([0; 32], 0.123, 0.456, 0.789, 0.5, "coords test", [0; 32]);
        let (x, y, z) = pkt.coords();
        assert!((x - 0.123).abs() < f32::EPSILON);
        assert!((y - 0.456).abs() < f32::EPSILON);
        assert!((z - 0.789).abs() < f32::EPSILON);
    }
}
