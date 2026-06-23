content = open("src/reader.rs", "r", encoding="utf-8").read()

# Add a new function after store_memory_with_emotion
old = "/// Variant of `store_memory` that also writes to the timeline log and,"
new = """/// Store memory to append log and timeline only (NOT to layer files).
/// Used for temporary/internal thoughts that should not persist through rebuilds.
pub fn store_memory_temporary(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
) -> Result<(), String> {
    let _lock = FileLock::acquire(config)?;
    let (x, y, z) = content_coords_blended(text, layer, config.search.semantic_weight);
    let lid = layer_to_id(layer);
    let depth = auto_depth(text);

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let needs_magic = !append_path.exists()
        || fs::metadata(&append_path)
            .map(|m| m.len() == 0)
            .unwrap_or(true);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&append_path)
        .map_err(|e| format!("open append log: {}", e))?;

    let write = |f: &mut fs::File, data: &[u8]| -> Result<(), String> {
        f.write_all(data)
            .map_err(|e| format!("write append log: {}", e))
    };

    if needs_magic {
        write(&mut file, b"APv2")?;
    }

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    write(&mut file, &(len as u32).to_le_bytes())?;
    write(&mut file, &[lid])?;
    write(&mut file, &[importance])?;
    write(&mut file, &[depth])?;
    write(&mut file, &x.to_le_bytes())?;
    write(&mut file, &y.to_le_bytes())?;
    write(&mut file, &z.to_le_bytes())?;
    write(&mut file, &text_bytes[..len])?;

    // Timeline log (always)
    let output_dir = Path::new(&config.paths.output_dir);
    let entry = crate::timeline::TimelineEntry {
        ts_ms: crate::timeline::now_epoch_ms(),
        layer_id: lid,
        importance,
        depth,
        status: crate::timeline::STATUS_NORMAL,
        text: text.to_string(),
    };
    if let Err(e) = crate::timeline::append_entry(&output_dir.join("timeline.bin"), &entry) {
        eprintln!("  {} append timeline: {}", "WARN".yellow(), e);
    }

    Ok(())
}

/// Variant of `store_memory` that also writes to the timeline log and,"""

content = content.replace(old, new)

open("src/reader.rs", "w", encoding="utf-8").write(content)
print("OK - store_memory_temporary hozzaadva")
