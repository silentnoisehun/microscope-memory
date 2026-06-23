content = open("src/autonomous.rs", "r", encoding="utf-8").read()

# Update the dream output to include forgotten blocks
old = '                    cycle.replayed_fingerprints, cycle.strengthened_pairs,\n                    cycle.pruned_pairs + cycle.pruned_activations,\n                    cycle.energy_before, cycle.energy_after);'
new = '                    cycle.replayed_fingerprints, cycle.strengthened_pairs,\n                    cycle.pruned_pairs + cycle.pruned_activations,\n                    cycle.forgotten_blocks,\n                    cycle.energy_before, cycle.energy_after);'

content = content.replace(old, new)

# Update the format string
old_fmt = '"💤 Álom: {} fingerprint, {} megerősítve, {} ritkítva, energia: {:.2} → {:.2}"'
new_fmt = '"💤 Álom: {} fingerprint, {} megerősítve, {} ritkítva, {} elfelejtve, energia: {:.2} → {:.2}"'

content = content.replace(old_fmt, new_fmt)

# Update TTS speak
old_tts = '"Álom konszolidáció kész. Energia: {:.2} -> {:.2}"'
new_tts = '"Álom konszolidáció kész. {} elfelejtve. Energia: {:.2} -> {:.2}"'

content = content.replace(old_tts, new_tts)

# Update the TTS format args
old_tts_args = 'cycle.energy_before, cycle.energy_after));\n                msg'
new_tts_args = 'cycle.forgotten_blocks, cycle.energy_before, cycle.energy_after));\n                msg'

content = content.replace(old_tts_args, new_tts_args)

open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK - autonomous frissitve")
