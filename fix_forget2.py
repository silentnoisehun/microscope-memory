content = open("src/dream.rs", "r", encoding="utf-8").read()

# Add Step 8: Forget old internal thoughts after Step 7
old = '''    // Step 7: Predictive cache cleanup \u2014 remove predictions with very low confidence
    pred_cache.dream_cleanup();

    // Measure energy after'''

new = '''    // Step 7: Predictive cache cleanup \u2014 remove predictions with very low confidence
    pred_cache.dream_cleanup();

    // Step 8: Forget old internal thoughts (autonomous mode outputs)
    let forgotten = forget_old_thoughts(output_dir, block_count)?;

    // Measure energy after'''

content = content.replace(old, new)

# Also add forgotten to the DreamCycle struct
old_cycle = '''    pub pruned_activations: u32,
    pub energy_before: f32,
    pub energy_after: f32,'''

new_cycle = '''    pub pruned_activations: u32,
    pub forgotten_blocks: u32,
    pub energy_before: f32,
    pub energy_after: f32,'''

content = content.replace(old_cycle, new_cycle)

# Add forgotten to the DreamCycle construction
old_ok = '''        pruned_pairs,
        pruned_activations,
        energy_after,'''

new_ok = '''        pruned_pairs,
        pruned_activations,
        forgotten_blocks: forgotten,
        energy_after,'''

content = content.replace(old_ok, new_ok)

# Add forgotten to the binary read
old_read = '''                        pruned_activations: read_u32(&data, off + 24),
                        energy_before: read_f32(&data, off + 28),
                        energy_after: read_f32(&data, off + 36),'''

new_read = '''                        pruned_activations: read_u32(&data, off + 24),
                        forgotten_blocks: read_u32(&data, off + 28),
                        energy_before: read_f32(&data, off + 32),
                        energy_after: read_f32(&data, off + 40),'''

content = content.replace(old_read, new_read)

# Add forgotten to the binary write
old_write = '''            buf.extend_from_slice(&c.pruned_activations.to_le_bytes());
            buf.extend_from_slice(&c.energy_before.to_le_bytes());
            buf.extend_from_slice(&c.energy_after.to_le_bytes());'''

new_write = '''            buf.extend_from_slice(&c.pruned_activations.to_le_bytes());
            buf.extend_from_slice(&c.forgotten_blocks.to_le_bytes());
            buf.extend_from_slice(&c.energy_before.to_le_bytes());
            buf.extend_from_slice(&c.energy_after.to_le_bytes());'''

content = content.replace(old_write, new_write)

# Update test structs
content = content.replace(
    'pruned_activations: 2,\n                    energy_before: 10.0,\n                    energy_after: 8.2,',
    'pruned_activations: 2,\n                    forgotten_blocks: 0,\n                    energy_before: 10.0,\n                    energy_after: 8.2,'
)
content = content.replace(
    'pruned_activations: 1,\n                    energy_before: 9.0,\n                    energy_after: 7.0,',
    'pruned_activations: 1,\n                    forgotten_blocks: 0,\n                    energy_before: 9.0,\n                    energy_after: 7.0,'
)
content = content.replace(
    'pruned_activations: 2,\n                    energy_before: 10.0,\n                    energy_after: 8.0,',
    'pruned_activations: 2,\n                    forgotten_blocks: 0,\n                    energy_before: 10.0,\n                    energy_after: 8.0,'
)
content = content.replace(
    'pruned_activations: 1,\n                    energy_before: 9.0,\n                    energy_after: 7.0,',
    'pruned_activations: 1,\n                    forgotten_blocks: 0,\n                    energy_before: 9.0,\n                    energy_after: 7.0,'
)

open("src/dream.rs", "w", encoding="utf-8").write(content)
print("OK - forget lepes hozzaadva a dream_consolidate-hoz")
