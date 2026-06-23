import re

def fix_from_bytes(filepath):
    with open(filepath, "r", encoding="utf-8") as f:
        c = f.read()
    
    # Fix patterns like: u16::from_le_bytes(data[pos..pos+2].try_into().ok())? as usize
    # To: u16::from_le_bytes(data[pos..pos+2].try_into().map_err(|_| ())?) as usize
    # Actually, let me just use a different approach - unwrap_or(0)
    
    # Replace: u16::from_le_bytes(data[pos..pos+2].try_into().ok())? as usize
    # With: u16::from_le_bytes(data[pos..pos+2].try_into().unwrap_or([0;2])) as usize
    
    c = c.replace(
        'u16::from_le_bytes(data[pos..pos+2].try_into().ok())? as usize',
        'u16::from_le_bytes(data[pos..pos+2].try_into().unwrap_or([0;2])) as usize'
    )
    c = c.replace(
        'u16::from_le_bytes(data[pos..pos+2].try_into().ok())? as usize; pos += 2',
        'u16::from_le_bytes(data[pos..pos+2].try_into().unwrap_or([0;2])) as usize; pos += 2'
    )
    
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(c)

fix_from_bytes("src/self_model.rs")
fix_from_bytes("src/curiosity.rs")
fix_from_bytes("src/inner_monologue.rs")
print("Fixed all from_bytes methods")
