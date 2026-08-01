//! Stable identity for index blocks and persisted cognitive state.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const BLOCK_ID_BYTES: usize = 32;
const BLOCK_IDS_MAGIC: &[u8; 4] = b"BID1";
const STATE_IDS_MAGIC: &[u8; 4] = b"SID1";

pub type BlockId = [u8; BLOCK_ID_BYTES];

pub fn compute(
    layer_id: u8,
    depth: u8,
    parent_id: Option<&BlockId>,
    data: &[u8],
    duplicate_ordinal: u32,
) -> BlockId {
    let mut hasher = Sha256::new();
    hasher.update(b"microscope-block-id-v1\0");
    hasher.update([layer_id, depth]);
    hasher.update(parent_id.copied().unwrap_or([0; BLOCK_ID_BYTES]));
    hasher.update((data.len() as u32).to_le_bytes());
    hasher.update(data);
    hasher.update(duplicate_ordinal.to_le_bytes());
    hasher.finalize().into()
}

pub fn write_block_ids(path: &Path, ids: &[BlockId]) -> Result<(), String> {
    write_ids(path, BLOCK_IDS_MAGIC, ids)
}

pub fn write_state_ids(path: &Path, ids: &[BlockId]) -> Result<(), String> {
    write_ids(path, STATE_IDS_MAGIC, ids)
}

pub fn read_block_ids(path: &Path, expected_count: usize) -> Option<Vec<BlockId>> {
    read_ids(path, BLOCK_IDS_MAGIC, expected_count)
}

pub fn read_state_ids(path: &Path) -> Option<Vec<BlockId>> {
    let data = fs::read(path).ok()?;
    if data.len() < 8 || &data[..4] != STATE_IDS_MAGIC {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    read_ids_from_bytes(&data, count)
}

fn write_ids(path: &Path, magic: &[u8; 4], ids: &[BlockId]) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(8 + ids.len() * BLOCK_ID_BYTES);
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        bytes.extend_from_slice(id);
    }
    let tmp = path.with_extension("bin.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write {}: {}", path.display(), e))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {}", path.display(), e))
}

fn read_ids(path: &Path, magic: &[u8; 4], expected_count: usize) -> Option<Vec<BlockId>> {
    let data = fs::read(path).ok()?;
    if data.len() < 8 || &data[..4] != magic {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    if count != expected_count {
        return None;
    }
    read_ids_from_bytes(&data, count)
}

fn read_ids_from_bytes(data: &[u8], count: usize) -> Option<Vec<BlockId>> {
    if data.len() != 8 + count * BLOCK_ID_BYTES {
        return None;
    }
    let mut ids = Vec::with_capacity(count);
    for chunk in data[8..].chunks_exact(BLOCK_ID_BYTES) {
        ids.push(chunk.try_into().ok()?);
    }
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_depends_on_parent_and_duplicate_ordinal() {
        let parent = compute(1, 2, None, b"parent", 0);
        let first = compute(1, 3, Some(&parent), b"same", 0);
        let duplicate = compute(1, 3, Some(&parent), b"same", 1);
        assert_ne!(first, duplicate);
    }
}
