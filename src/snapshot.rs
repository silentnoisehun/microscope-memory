//! Snapshot: .mscope archive format for backup/restore/diff.
//!
//! Format:
//!   [magic "MSEX" 4B][version u32][file_count u32]
//!   Per file: [name_len u16][name bytes][data_len u64][data bytes]

use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

const MAGIC: &[u8; 4] = b"MSEX";
const VERSION: u32 = 1;

/// Files that compose a microscope index.
const INDEX_FILES: &[&str] = &[
    "meta.bin",
    "microscope.bin",
    "data.bin",
    "merkle.bin",
    "append.bin",
    "embeddings.bin",
];

/// Export all index files from output_dir into a single .mscope archive.
pub fn export(output_dir: &Path, archive_path: &Path) -> Result<(), String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    for &name in INDEX_FILES {
        let path = output_dir.join(name);
        if path.exists() {
            let data = fs::read(&path).map_err(|e| format!("read {}: {}", name, e))?;
            files.push((name.to_string(), data));
        }
    }

    if files.is_empty() {
        return Err("no index files found to export".to_string());
    }

    let f = fs::File::create(archive_path).map_err(|e| format!("create archive: {}", e))?;
    let mut w = BufWriter::new(f);

    // Header
    w.write_all(MAGIC).map_err(|e| e.to_string())?;
    w.write_all(&VERSION.to_le_bytes())
        .map_err(|e| e.to_string())?;
    w.write_all(&(files.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;

    // Files
    let mut total_size = 12u64; // header
    for (name, data) in &files {
        let name_bytes = name.as_bytes();
        w.write_all(&(name_bytes.len() as u16).to_le_bytes())
            .map_err(|e| e.to_string())?;
        w.write_all(name_bytes).map_err(|e| e.to_string())?;
        w.write_all(&(data.len() as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
        w.write_all(data).map_err(|e| e.to_string())?;
        total_size += 2 + name_bytes.len() as u64 + 8 + data.len() as u64;
    }

    w.flush().map_err(|e| e.to_string())?;

    println!(
        "  Exported {} files ({:.1} KB) → {}",
        files.len(),
        total_size as f64 / 1024.0,
        archive_path.display()
    );
    for (name, data) in &files {
        println!("    {}: {:.1} KB", name, data.len() as f64 / 1024.0);
    }

    Ok(())
}

/// Import a .mscope archive into output_dir.
pub fn import(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    let f = fs::File::open(archive_path).map_err(|e| format!("open archive: {}", e))?;
    let mut r = BufReader::new(f);

    // Header
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err(format!("invalid magic: expected MSEX, got {:?}", magic));
    }

    let mut ver_buf = [0u8; 4];
    r.read_exact(&mut ver_buf).map_err(|e| e.to_string())?;
    let version = u32::from_le_bytes(ver_buf);
    if version > VERSION {
        return Err(format!(
            "unsupported version: {} (max: {})",
            version, VERSION
        ));
    }

    let mut count_buf = [0u8; 4];
    r.read_exact(&mut count_buf).map_err(|e| e.to_string())?;
    let file_count = u32::from_le_bytes(count_buf);

    fs::create_dir_all(output_dir).map_err(|e| format!("create output dir: {}", e))?;

    for _ in 0..file_count {
        // Name
        let mut name_len_buf = [0u8; 2];
        r.read_exact(&mut name_len_buf).map_err(|e| e.to_string())?;
        let name_len = u16::from_le_bytes(name_len_buf) as usize;
        let mut name_buf = vec![0u8; name_len];
        r.read_exact(&mut name_buf).map_err(|e| e.to_string())?;
        let name = String::from_utf8(name_buf).map_err(|e| e.to_string())?;

        // Data
        let mut data_len_buf = [0u8; 8];
        r.read_exact(&mut data_len_buf).map_err(|e| e.to_string())?;
        let data_len = u64::from_le_bytes(data_len_buf) as usize;
        let mut data = vec![0u8; data_len];
        r.read_exact(&mut data).map_err(|e| e.to_string())?;

        // Sanitize filename (only allow known index files)
        if !INDEX_FILES.contains(&name.as_str()) {
            println!("    Skipping unknown file: {}", name);
            continue;
        }

        let out_path = output_dir.join(&name);
        fs::write(&out_path, &data).map_err(|e| format!("write {}: {}", name, e))?;
        println!("    {}: {:.1} KB", name, data.len() as f64 / 1024.0);
    }

    println!("  Imported {} files → {}", file_count, output_dir.display());
    Ok(())
}

/// Compare two .mscope archives: Merkle root + per-file size diff.
pub fn diff(a_path: &Path, b_path: &Path) -> Result<(), String> {
    let a_files = read_archive(a_path)?;
    let b_files = read_archive(b_path)?;

    println!("  {} vs {}", a_path.display(), b_path.display());

    // Compare Merkle roots if both have meta.bin
    let a_root = extract_merkle_root(&a_files);
    let b_root = extract_merkle_root(&b_files);
    match (a_root, b_root) {
        (Some(ar), Some(br)) => {
            if ar == br {
                println!("  Merkle root: {} (identical)", hex_str(&ar));
            } else {
                println!("  Merkle root A: {}", hex_str(&ar));
                println!("  Merkle root B: {}", hex_str(&br));
                println!("  DIFF Merkle roots differ — data changed");
            }
        }
        _ => println!("  (cannot compare Merkle roots — meta.bin missing)"),
    }

    // Per-file size comparison
    let all_names: std::collections::BTreeSet<&str> = a_files
        .keys()
        .chain(b_files.keys())
        .map(|s| s.as_str())
        .collect();

    for name in all_names {
        let a_size = a_files.get(name).map(|d| d.len());
        let b_size = b_files.get(name).map(|d| d.len());
        match (a_size, b_size) {
            (Some(a), Some(b)) => {
                let delta = b as i64 - a as i64;
                let sign = if delta >= 0 { "+" } else { "" };
                let status = if a == b { "=" } else { "~" };
                println!(
                    "  {} {}: {} → {} ({}{} bytes)",
                    status, name, a, b, sign, delta
                );
            }
            (Some(a), None) => println!("  - {}: {} (removed)", name, a),
            (None, Some(b)) => println!("  + {}: {} (added)", name, b),
            (None, None) => {}
        }
    }

    // Block count comparison
    let a_blocks = extract_block_count(&a_files);
    let b_blocks = extract_block_count(&b_files);
    if let (Some(a), Some(b)) = (a_blocks, b_blocks) {
        println!(
            "  Blocks: {} → {} ({}{})",
            a,
            b,
            if b >= a { "+" } else { "" },
            b as i64 - a as i64
        );
    }

    Ok(())
}

fn read_archive(path: &Path) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
    let f = fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let mut r = BufReader::new(f);
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err(format!("invalid magic in {}", path.display()));
    }

    let mut ver_buf = [0u8; 4];
    r.read_exact(&mut ver_buf).map_err(|e| e.to_string())?;
    let mut count_buf = [0u8; 4];
    r.read_exact(&mut count_buf).map_err(|e| e.to_string())?;
    let file_count = u32::from_le_bytes(count_buf);

    let mut files = std::collections::HashMap::new();
    for _ in 0..file_count {
        let mut name_len_buf = [0u8; 2];
        r.read_exact(&mut name_len_buf).map_err(|e| e.to_string())?;
        let name_len = u16::from_le_bytes(name_len_buf) as usize;
        let mut name_buf = vec![0u8; name_len];
        r.read_exact(&mut name_buf).map_err(|e| e.to_string())?;
        let name = String::from_utf8(name_buf).map_err(|e| e.to_string())?;

        let mut data_len_buf = [0u8; 8];
        r.read_exact(&mut data_len_buf).map_err(|e| e.to_string())?;
        let data_len = u64::from_le_bytes(data_len_buf) as usize;
        let mut data = vec![0u8; data_len];
        r.read_exact(&mut data).map_err(|e| e.to_string())?;

        files.insert(name, data);
    }
    Ok(files)
}

fn extract_merkle_root(files: &std::collections::HashMap<String, Vec<u8>>) -> Option<[u8; 32]> {
    let meta = files.get("meta.bin")?;
    if meta.len() < 4 || &meta[0..4] != b"MSC2" {
        return None;
    }
    let offset = crate::META_HEADER_SIZE + 9 * crate::DEPTH_ENTRY_SIZE;
    if meta.len() < offset + 32 {
        return None;
    }
    let mut root = [0u8; 32];
    root.copy_from_slice(&meta[offset..offset + 32]);
    Some(root)
}

fn extract_block_count(files: &std::collections::HashMap<String, Vec<u8>>) -> Option<u32> {
    let meta = files.get("meta.bin")?;
    if meta.len() < 12 {
        return None;
    }
    Some(u32::from_le_bytes(meta[8..12].try_into().ok()?))
}

fn hex_str(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_export_import_roundtrip() {
        let dir = std::env::temp_dir().join("mscope_snap_test");
        let _ = fs::create_dir_all(&dir);

        // Create fake index files
        let src_dir = dir.join("src");
        let _ = fs::create_dir_all(&src_dir);
        fs::write(src_dir.join("meta.bin"), b"MSC2testdata1234").unwrap();
        fs::write(src_dir.join("microscope.bin"), b"headers_here").unwrap();
        fs::write(src_dir.join("data.bin"), b"block_data_here").unwrap();

        // Export
        let archive = dir.join("test.mscope");
        export(&src_dir, &archive).unwrap();
        assert!(archive.exists());

        // Import
        let dst_dir = dir.join("dst");
        import(&archive, &dst_dir).unwrap();

        // Verify
        assert_eq!(
            fs::read(dst_dir.join("meta.bin")).unwrap(),
            b"MSC2testdata1234"
        );
        assert_eq!(
            fs::read(dst_dir.join("microscope.bin")).unwrap(),
            b"headers_here"
        );
        assert_eq!(
            fs::read(dst_dir.join("data.bin")).unwrap(),
            b"block_data_here"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
