content = open("src/autonomous.rs", "r", encoding="utf-8").read()
content = content.replace("en-US-AriaNeural", "hu-HU-NoemiNeural")
open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK - NoemiNeural")