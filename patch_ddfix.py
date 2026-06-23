with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
c = c.replace(
    "match microscope_memory::daydream::daydream(&config, &reader, output_dir, &seed_text, steps)",
    "match microscope_memory::daydream::daydream(&config, &seed_text, steps)"
)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Fixed daydream call")
