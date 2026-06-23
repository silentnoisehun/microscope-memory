with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
old = "narrative.session_count % microscope_memory::self_reflect::AUTO_REFLECT_INTERVAL"
new = "(narrative.session_count as usize) % microscope_memory::self_reflect::AUTO_REFLECT_INTERVAL"
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Fixed type mismatch")
