import sys
content = open("src/autonomous.rs", "r", encoding="utf-8").read()

old = 'fn speak(&self, text: &str) {\n        if !self.config.tts_enabled { return; }\n        let safe_text = text.replace('"', "'").replace('\\n', " ");\n        let ps_script = format!(\n            "Add-Type -AssemblyName System.Speech;  = New-Object System.Speech.Synthesis.SpeechSynthesizer; .Speak('{}')",\n            safe_text\n        );\n        let _ = std::process::Command::new("powershell")\n            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])\n            .spawn();\n    }'

new = 'fn speak(&self, text: &str) {\n        if !self.config.tts_enabled { return; }\n        let safe_text = text.replace('"', " ").replace('\\n', " ");\n        // Edge TTS használata a jobb hangminőségért\n        let _ = std::process::Command::new("python")\n            .args(["-m", "edge_tts", "--voice", "en-US-AriaNeural", "--text", &safe_text, "--write-media", "tts_output.mp3"])\n            .spawn();\n        // Kis késleltetés, hogy a fájl elkészüljön, majd lejátszás\n        std::thread::sleep(std::time::Duration::from_millis(1500));\n        let _ = std::process::Command::new("powershell")\n            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(New-Object Media.SoundPlayer 'tts_output.mp3').PlaySync()")])\n            .spawn();\n    }'

if old in content:
    content = content.replace(old, new, 1)
    open("src/autonomous.rs", "w", encoding="utf-8").write(content)
    print("OK")
else:
    print("Not found")
    idx = content.find("fn speak")
    end = content.find("fn store_result")
    actual = content[idx:end]
    print("Old repr:", repr(old))
    print("Actual repr:", repr(actual))