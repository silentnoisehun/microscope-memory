import re

# Fix self_model.rs - remove extra ? operators
with open("src/self_model.rs","r",encoding="utf-8") as f:
    c = f.read()
c = c.replace("try_into().ok()?)?", "try_into().ok())?")
c = c.replace("try_into().ok()?)?", "try_into().ok())?")
with open("src/self_model.rs","w",encoding="utf-8") as f:
    f.write(c)

# Fix curiosity.rs - remove extra ? operators and fix blocks -> result_blocks
with open("src/curiosity.rs","r",encoding="utf-8") as f:
    c = f.read()
c = c.replace("try_into().ok()?)?", "try_into().ok())?")
c = c.replace("try_into().ok()?)?", "try_into().ok())?")
c = c.replace("p.blocks.len()", "p.result_blocks.len()")
with open("src/curiosity.rs","w",encoding="utf-8") as f:
    f.write(c)

# Fix inner_monologue.rs - remove extra ? operators
with open("src/inner_monologue.rs","r",encoding="utf-8") as f:
    c = f.read()
c = c.replace("try_into().ok()?)?", "try_into().ok())?")
c = c.replace("try_into().ok()?)?", "try_into().ok())?")
with open("src/inner_monologue.rs","w",encoding="utf-8") as f:
    f.write(c)

print("All fixes applied")
