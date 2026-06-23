with open("src/cli.rs","r",encoding="utf-8") as f:
    c = f.read()
old = '    Mermaid {\n        #[arg(short, long, default_value = "8080")]\n        port: u16,\n    },\n}'
new = '    Mermaid {\n        #[arg(short, long, default_value = "8080")]\n        port: u16,\n    },\n    /// Introspect - self-reflection: the system thinks about itself\n    Introspect,\n}'
c = c.replace(old, new, 1)
with open("src/cli.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Introspect command added")
