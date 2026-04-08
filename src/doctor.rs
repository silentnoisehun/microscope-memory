//! doctor.rs — Integrity diagnostics and automatic repair for Microscope Memory.
//! (v0.7.0 Public Beta - Reliability Phase)

use std::fs;
use std::path::Path;
use colored::Colorize;
use crate::config::Config;
use crate::reader::{MicroscopeReader};
use crate::merkle::MerkleTree;

pub fn run_doctor(config: &Config, fix: bool) -> Result<(), String> {
    println!("{}", "=".repeat(60));
    println!("  {}", "MICROSCOPE MEMORY DOCTOR — Integrity Scan".cyan().bold());
    println!("{}", "=".repeat(60));

    let output_dir = Path::new(&config.paths.output_dir);
    let mut artifacts_missing = false;

    // 1. Check core artifacts
    let core_files = ["meta.bin", "microscope.bin", "data.bin"];
    for file in &core_files {
        let p = output_dir.join(file);
        if !p.exists() {
            println!("  [{}] {} is missing", "ERR".red(), file);
            artifacts_missing = true;
        } else {
            println!("  [{}] {} present ({:.1} KB)", 
                "OK".green(), 
                file, 
                fs::metadata(&p).map(|m| m.len() as f64 / 1024.0).unwrap_or(0.0)
            );
        }
    }

    if artifacts_missing {
        println!("\n  {} Core artifacts are missing. Run 'build' to regenerate.", "WARN:".yellow());
        return Ok(());
    }

    // 2. Comprehensive Integrity Scan
    let reader = MicroscopeReader::open(config)?;
    println!("\n  {} scanning {} blocks...", "INTEGRITY:".yellow(), reader.block_count);
    
    let mut bad_crc = 0;
    for i in 0..reader.block_count {
        let h = reader.header(i);
        let stored_crc = u16::from_le_bytes(h.crc16);
        if stored_crc == 0 { continue; }

        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        if end > reader.data.len() {
            bad_crc += 1;
            continue;
        }

        let computed = crate::crc16_ccitt(&reader.data[start..end]);
        if computed != stored_crc {
            bad_crc += 1;
        }
    }

    if bad_crc == 0 {
        println!("  [{}] All block CRCs verified.", "OK".green());
    } else {
        println!("  [{}] {} block(s) have CRC mismatches!", "FAIL".red(), bad_crc);
    }

    // 3. Merkle Root Check
    let merkle_path = output_dir.join("merkle.bin");
    if merkle_path.exists() {
        if let Ok(merkle_data) = fs::read(&merkle_path) {
            if let Some(tree) = MerkleTree::from_bytes(&merkle_data) {
                println!("  [{}] Merkle tree loaded (Root: {})", "OK".green(), crate::hex_str(&tree.root));
            }
        }
    }

    // 4. Append Log Health & Repair
    println!("\n  {} checking append log...", "RECOVERY:".yellow());
    let append_path = output_dir.join("append.bin");
    if append_path.exists() {
        let data = fs::read(&append_path).map_err(|e| e.to_string())?;
        if data.is_empty() {
            println!("  [{}] Append log is empty.", "OK".green());
        } else {
            let mut pos = 0;
            let is_v2 = data.len() >= 4 && &data[0..4] == b"APv2";
            if is_v2 { pos = 4; }
            let header_size = if is_v2 { 19 } else { 18 };
            
            let mut valid_pos = pos;
            let mut count = 0;
            let mut corrupted = false;

            while pos + header_size <= data.len() {
                let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
                if pos + header_size + len > data.len() {
                    corrupted = true;
                    break;
                }
                pos += header_size + len;
                valid_pos = pos;
                count += 1;
            }

            if pos < data.len() {
                corrupted = true;
            }

            if corrupted {
                println!("  [{}] Found corrupted data at tail. Valid entries: {}", "WARN".yellow(), count);
                if fix {
                    println!("  [{}] Truncating append.bin to last valid position ({} bytes)...", "FIX".green(), valid_pos);
                    let f = fs::OpenOptions::new().write(true).open(&append_path).map_err(|e| e.to_string())?;
                    f.set_len(valid_pos as u64).map_err(|e| e.to_string())?;
                    println!("  [{}] Truncation successful.", "DONE".green());
                } else {
                    println!("  [{}] Run with --fix to truncate and recover the log.", "INFO".cyan());
                }
            } else {
                println!("  [{}] Append log is healthy. Entries: {}", "OK".green(), count);
            }
        }
    } else {
        println!("  [{}] No append log found.", "INFO".cyan());
    }

    println!("\n{}", "Status: Diagnostics complete.".bold());
    println!("{}", "=".repeat(60));

    Ok(())
}
