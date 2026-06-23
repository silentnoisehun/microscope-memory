with open("src/cli.rs","r",encoding="utf-8") as f:
    c = f.read()
old = "    /// Introspect - self-reflection: the system thinks about itself\n    Introspect,\n}"
new = "    /// Introspect - self-reflection: the system thinks about itself\n    Introspect,\n    /// SelfModel - show the system'\''s self-model snapshot\n    SelfModel,\n    /// Curiosity - show what the system is curious about\n    Curiosity,\n    /// Monologue - generate an inner monologue (the system thinking)\n    Monologue,\n}"
c = c.replace(old, new, 1)
with open("src/cli.rs","w",encoding="utf-8") as f:
    f.write(c)
print("CLI commands added")
