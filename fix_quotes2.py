content = open("src/autonomous.rs", "r", encoding="utf-8").read()
content = content.replace('replace("\n", " ")', "replace('\\n', \" \")")
open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("Fixed")