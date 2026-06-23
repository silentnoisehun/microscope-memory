import re
with open("src/autonomous.rs", "r", encoding="utf-8") as f:
    content = f.read()

old_speak = """    fn speak(&self, text: &str) {
        if !self.config.tts_enabled { return; }
        let safe_text = text.replace('"', "'").replace('\n', " ");
        let ps_script = format!(
            "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Speak('{}')",
            safe_text
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .spawn();
    }"""

new_speak = """    fn speak(&self, text: &str) {
        if !self.config.tts_enabled { return; }
        let safe_text = text.replace('"', " ").replace('\n', " ");
        // Edge TTS használata a jobb hangminőségért
        let _ = std::process::Command::new("python")
            .args(["-m", "edge_tts", "--voice", "en-US-AriaNeural", "--text", &safe_text, "--write-media", "tts_output.mp3"])
            .spawn();
        // Kis késleltetés, hogy a fájl elkészüljön, majd lejátszás
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(New-Object Media.SoundPlayer 'tts_output.mp3').PlaySync()")])
            .spawn();
    }"""

if old_speak in content:
    content = content.replace(old_speak, new_speak, 1)
    with open("src/autonomous.rs", "w", encoding="utf-8") as f:
        f.write(content)
    print("OK - replaced")
else:
    print("Old string not found")
    m = re.search(r'fn speak\([^}]+\)', content)
    if m:
        print("Found:", repr(m.group()))