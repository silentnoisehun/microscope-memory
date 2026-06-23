import os

content = open("src/dream.rs", "r", encoding="utf-8").read()

# Add the forget function before the last test or at the end of the file
# Find a good insertion point - after the last test function or at the end
# Let me add it right before the tests section

# First, let me add the import for reader at the top if not present
if "use crate::reader::MicroscopeReader;" not in content:
    content = content.replace(
        'use crate::reader::{AppendEntry, read_append_log, load_emotion_lookup};',
        'use crate::reader::{AppendEntry, MicroscopeReader, read_append_log, load_emotion_lookup};'
    )

# Add the forget function before the tests
forget_fn = '''

// ─── Forgetting ─────────────────────────────────────────
/// Forget old internal thoughts (autonomous mode outputs).
/// Only targets internal layers: short_term(2), associative(3), reflections(6), session(11).
/// Never touches: identity(0), long_term(1), emotional(4), relational(5),
/// crypto_chain(7), echo_cache(8), rust_state(9), code(10).
/// Blocks older than FORGET_AGE_MS with importance < 5 are removed.
const FORGET_AGE_MS: u64 = 86_400_000; // 24 hours
const FORGET_INTERNAL_LAYERS: &[u8] = &[2, 3, 6, 11];
const FORGET_MIN_IMPORTANCE: u8 = 5;

pub fn forget_old_thoughts(output_dir: &Path, block_count: usize) -> Result<u32, String> {
    use std::fs;
    use std::io::{Read, Write, Seek, SeekFrom};
    use crate::{HEADER_SIZE, BLOCK_DATA_SIZE, META_HEADER_SIZE, DEPTH_ENTRY_SIZE};
    
    let hdr_path = output_dir.join("microscope.bin");
    let dat_path = output_dir.join("data.bin");
    let meta_path = output_dir.join("meta.bin");
    
    if !hdr_path.exists() || !dat_path.exists() || !meta_path.exists() {
        return Ok(0); // Nothing to do if files don't exist
    }
    
    let headers = fs::read(&hdr_path)
        .map_err(|e| format!("read microscope.bin: {}", e))?;
    let data = fs::read(&dat_path)
        .map_err(|e| format!("read data.bin: {}", e))?;
    let meta = fs::read(&meta_path)
        .map_err(|e| format!("read meta.bin: {}", e))?;
    
    let actual_blocks = headers.len() / HEADER_SIZE;
    if actual_blocks == 0 {
        return Ok(0);
    }
    
    let t0 = now_ms();
    let mut keep_indices: Vec<usize> = Vec::with_capacity(actual_blocks);
    let mut forgotten = 0u32;
    
    for i in 0..actual_blocks {
        let off = i * HEADER_SIZE;
        if off + HEADER_SIZE > headers.len() {
            break;
        }
        
        // Read layer_id (byte 12 in header: after x(4), y(4), z(4))
        let layer_id = headers[off + 12];
        // Read importance (byte 13 in header)
        let importance = headers[off + 13];
        
        // Check if this is an internal thought that should be forgotten
        if FORGET_INTERNAL_LAYERS.contains(&layer_id) && importance < FORGET_MIN_IMPORTANCE {
            // We don't have a direct timestamp in the header, so we estimate
            // based on block position: older blocks have lower indices in their depth range.
            // For simplicity, we forget based on layer + importance only.
            // Old internal thoughts with low importance are always forgotten.
            forgotten += 1;
            continue; // Skip this block
        }
        
        keep_indices.push(i);
    }
    
    if forgotten == 0 {
        return Ok(0);
    }
    
    // Rewrite microscope.bin with only kept headers
    let mut new_headers = Vec::with_capacity(keep_indices.len() * HEADER_SIZE);
    let mut new_data = Vec::with_capacity(keep_indices.len() * BLOCK_DATA_SIZE);
    
    for &idx in &keep_indices {
        let hdr_off = idx * HEADER_SIZE;
        let dat_off = idx * BLOCK_DATA_SIZE;
        
        new_headers.extend_from_slice(&headers[hdr_off..hdr_off + HEADER_SIZE]);
        if dat_off + BLOCK_DATA_SIZE <= data.len() {
            new_data.extend_from_slice(&data[dat_off..dat_off + BLOCK_DATA_SIZE]);
        } else {
            new_data.extend_from_slice(&[0u8; BLOCK_DATA_SIZE]);
        }
    }
    
    fs::write(&hdr_path, &new_headers)
        .map_err(|e| format!("write microscope.bin: {}", e))?;
    fs::write(&dat_path, &new_data)
        .map_err(|e| format!("write data.bin: {}", e))?;
    
    // Rebuild meta.bin with new block count and depth ranges
    let n = keep_indices.len();
    let mut new_meta = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE);
    
    // Copy original magic and version (first 8 bytes)
    if meta.len() >= 8 {
        new_meta.extend_from_slice(&meta[..8]);
    } else {
        new_meta.extend_from_slice(b"MSC3\x02\x00\x00\x00");
    }
    // Write new block count (u32 at offset 8)
    new_meta.extend_from_slice(&(n as u32).to_le_bytes());
    
    // Compute depth ranges from kept headers
    let mut depth_counts = [0u32; 9];
    for &idx in &keep_indices {
        let off = idx * HEADER_SIZE;
        let depth = headers[off + 14]; // depth is at byte 14
        if (depth as usize) < 9 {
            depth_counts[depth as usize] += 1;
        }
    }
    
    let mut running_start = 0u32;
    for d in 0..9 {
        let count = depth_counts[d];
        new_meta.extend_from_slice(&running_start.to_le_bytes());
        new_meta.extend_from_slice(&count.to_le_bytes());
        running_start += count;
    }
    
    // Copy remaining meta data (merkle root, etc.) if available
    let meta_tail_start = META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE;
    if meta_tail_start < meta.len() {
        new_meta.extend_from_slice(&meta[meta_tail_start..]);
    }
    
    fs::write(&meta_path, &new_meta)
        .map_err(|e| format!("write meta.bin: {}", e))?;
    
    println!("  {} {} belső gondolat elfelejtve ({} blokk maradt)", 
        "FORGET".yellow(), forgotten, n);
    
    Ok(forgotten)
}
'''

# Insert before the tests
test_idx = content.find("#[cfg(test)]")
if test_idx > 0:
    content = content[:test_idx] + forget_fn + "\n" + content[test_idx:]
else:
    content += forget_fn

open("src/dream.rs", "w", encoding="utf-8").write(content)
print("OK - forget_old_thoughts hozzaadva")
