import re
content = open("src/autonomous.rs", "r", encoding="utf-8").read()

start = content.find("fn speak(&self, text: &str)")
end = content.find("fn store_result", start)
old_func = content[start:end]

new_func = '    fn speak(&self, text: &str) {\n        if !self.config.tts_enabled { return; }\n        let safe_text = text.replace('"', " ").replace("\\n", " ");\n        // Edge TTS használata a jobb hangminőségért\n        let _ = std::process::Command::new("python")\n            .args(["-m", "edge_tts", "--voice", "en-US-AriaNeural", "--text", &safe_text, "--write-media", "tts_output.mp3"])\n            .spawn();\n        // Kis késleltetés, hogy a fájl elkészüljön, majd lejátszás\n        std::thread::sleep(std::time::Duration::from_millis(1500));\n        let _ = std::process::Command::new("powershell")\n            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(New-Object Media.SoundPlayer 'tts_output.mp3').PlaySync()")])\n            .spawn();\n    }\n\n    '

content = content.replace(old_func, new_func, 1)
open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK - replaced")