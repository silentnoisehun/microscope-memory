with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
c = c.replace("            let _ = hf.save_state(output_dir);\n", "")
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Removed save_state call")
