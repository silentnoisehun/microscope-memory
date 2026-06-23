content = open("src/dream.rs", "r", encoding="utf-8").read()

# Update DreamStats
content = content.replace(
    '    pub total_pruned_activations: u64,\n}',
    '    pub total_pruned_activations: u64,\n    pub total_forgotten_blocks: u64,\n}'
)

# Update stats calculation
content = content.replace(
    '            total_pruned_activations: self\n                .cycles\n                .iter()\n                .map(|c| c.pruned_activations as u64)\n                .sum(),',
    '            total_pruned_activations: self\n                .cycles\n                .iter()\n                .map(|c| c.pruned_activations as u64)\n                .sum(),\n            total_forgotten_blocks: self\n                .cycles\n                .iter()\n                .map(|c| c.forgotten_blocks as u64)\n                .sum(),'
)

open("src/dream.rs", "w", encoding="utf-8").write(content)
print("OK - DreamStats frissitve")
