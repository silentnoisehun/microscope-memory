with open("src/narrative_memory.rs","r",encoding="utf-8") as f:
    c = f.read()
c = c.replace(
    "u16::from_le_bytes(data[pos..pos+2].try_into().ok()?)? as usize",
    "u16::from_le_bytes(data[pos..pos+2].try_into().unwrap_or([0;2])) as usize"
)
with open("src/narrative_memory.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Fixed")
