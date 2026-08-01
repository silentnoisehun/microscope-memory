//! Shared types used across native and WASM targets.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// 16-byte project identifier. BLAKE3-derived from a project root path,
/// or the all-zero `GLOBAL` context shared across projects.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(pub [u8; 16]);

impl ProjectId {
    /// Global context — visible to every project when included.
    pub const GLOBAL: Self = ProjectId([0u8; 16]);

    /// Derive a project id from a filesystem path.
    pub fn from_path(path: &str) -> Self {
        let hash = blake3::hash(path.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&hash.as_bytes()[..16]);
        ProjectId(bytes)
    }

    /// True for the zeroed global project id.
    pub fn is_global(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in &self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

impl FromStr for ProjectId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("global") {
            return Ok(ProjectId::GLOBAL);
        }
        let s = s.trim_start_matches("0x");
        if s.len() != 32 {
            return Err(format!("ProjectId must be 32 hex chars, got {}", s.len()));
        }
        let mut bytes = [0u8; 16];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let chunk =
                std::str::from_utf8(chunk).map_err(|_| "invalid utf-8 in hex".to_string())?;
            bytes[i] = u8::from_str_radix(chunk, 16).map_err(|e| format!("invalid hex: {}", e))?;
        }
        Ok(ProjectId(bytes))
    }
}

impl Serialize for ProjectId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ProjectId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Block header: packed, mmap-ready.
///
/// v4 (MSC4) layout appends `project_id`, `importance` and `flags` to the
/// legacy 32-byte spatial header. The first 32 bytes keep their old offsets so
/// existing fixed-offset reads remain valid.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct BlockHeader {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zoom: f32,
    pub depth: u8,
    pub layer_id: u8,
    pub data_offset: u32,
    pub data_len: u16,
    pub parent_idx: u32,
    pub child_count: u16,
    pub crc16: [u8; 2],
    pub project_id: ProjectId,
    pub importance: u8,
    pub flags: u8,
}

/// Alias used by the project-isolation protocol.
pub type MemoryBlockHeader = BlockHeader;

impl BlockHeader {
    /// Zero-copy visibility check for project isolation.
    #[inline(always)]
    pub fn is_visible_to(&self, active: ProjectId, include_global: bool) -> bool {
        self.project_id == active || (include_global && self.project_id.is_global())
    }
}

/// Query options used by project-scoped search and Hebbian propagation.
#[derive(Clone)]
pub struct MemoryQueryOptions {
    pub active_project: ProjectId,
    pub include_global: bool,
    pub min_importance: u8,
    pub max_results: usize,
}

impl Default for MemoryQueryOptions {
    fn default() -> Self {
        Self {
            active_project: ProjectId::GLOBAL,
            include_global: true,
            min_importance: 0,
            max_results: usize::MAX,
        }
    }
}

/// Meta header: 48 bytes at start of meta.bin
#[repr(C, packed)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct MetaHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub block_count: u32,
    pub depth_count: u32,
}

/// Append entry for the in-memory append log
pub struct AppendEntry {
    pub text: String,
    pub layer_id: u8,
    pub importance: u8,
    pub depth: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub emotion: [f32; 21],
    pub project_id: ProjectId,
}
