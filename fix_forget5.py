content = open("src/dream.rs", "r", encoding="utf-8").read()

# Fix DreamCycle struct - add forgotten_blocks after pruned_activations
old = "    pub pruned_activations: u32,\n    pub consolidated_patterns: u32,\n    pub energy_before: f32,\n    pub energy_after: f32,"
new = "    pub pruned_activations: u32,\n    pub consolidated_patterns: u32,\n    pub forgotten_blocks: u32,\n    pub energy_before: f32,\n    pub energy_after: f32,"
content = content.replace(old, new)

# Fix DreamStats struct
old = "    pub total_strengthened: u64,\n    pub total_replayed: u64,"
new = "    pub total_strengthened: u64,\n    pub total_replayed: u64,\n    pub total_forgotten_blocks: u64,"
content = content.replace(old, new)

# Fix CYCLE_BYTES
old = "const CYCLE_BYTES: usize = 40; // 8+4+4+4+4+4+4+4+4"
new = "const CYCLE_BYTES: usize = 44; // 8+4+4+4+4+4+4+4+4+4"
content = content.replace(old, new)

# Fix binary read offsets
old = "                        pruned_activations: read_u32(&data, off + 24),\n                        consolidated_patterns: read_u32(&data, off + 28),\n                        energy_before: read_f32(&data, off + 32),\n                        energy_after: read_f32(&data, off + 36),"
new = "                        pruned_activations: read_u32(&data, off + 24),\n                        consolidated_patterns: read_u32(&data, off + 28),\n                        forgotten_blocks: read_u32(&data, off + 32),\n                        energy_before: read_f32(&data, off + 36),\n                        energy_after: read_f32(&data, off + 40),"
content = content.replace(old, new)

# Fix binary write
old = "            buf.extend_from_slice(&c.pruned_activations.to_le_bytes());\n            buf.extend_from_slice(&c.consolidated_patterns.to_le_bytes());\n            buf.extend_from_slice(&c.energy_before.to_le_bytes());\n            buf.extend_from_slice(&c.energy_after.to_le_bytes());"
new = "            buf.extend_from_slice(&c.pruned_activations.to_le_bytes());\n            buf.extend_from_slice(&c.consolidated_patterns.to_le_bytes());\n            buf.extend_from_slice(&c.forgotten_blocks.to_le_bytes());\n            buf.extend_from_slice(&c.energy_before.to_le_bytes());\n            buf.extend_from_slice(&c.energy_after.to_le_bytes());"
content = content.replace(old, new)

# Fix DreamCycle construction in dream_consolidate
old = "        consolidated_patterns,\n        energy_before,"
new = "        consolidated_patterns,\n        forgotten_blocks: forgotten,\n        energy_before,"
content = content.replace(old, new)

# Fix stats calculation
old = "                .map(|c| c.pruned_activations as u64)\n                .sum(),"
new = "                .map(|c| c.pruned_activations as u64)\n                .sum(),\n            total_forgotten_blocks: self\n                .cycles\n                .iter()\n                .map(|c| c.forgotten_blocks as u64)\n                .sum(),"
content = content.replace(old, new)

# Fix test structs - update all test DreamCycle instances
# Test 1
content = content.replace(
    "pruned_activations: 2,\n                    consolidated_patterns: 1,\n                    energy_before: 10.0,\n                    energy_after: 8.2,",
    "pruned_activations: 2,\n                    consolidated_patterns: 1,\n                    forgotten_blocks: 0,\n                    energy_before: 10.0,\n                    energy_after: 8.2,"
)
# Test 2
content = content.replace(
    "pruned_activations: 1,\n                    consolidated_patterns: 0,\n                    energy_before: 9.0,\n                    energy_after: 7.0,",
    "pruned_activations: 1,\n                    consolidated_patterns: 0,\n                    forgotten_blocks: 0,\n                    energy_before: 9.0,\n                    energy_after: 7.0,"
)

open("src/dream.rs", "w", encoding="utf-8").write(content)
print("OK")
