content = open("src/autonomous.rs", "r", encoding="utf-8").read()
start = content.find("fn speak(&self, text: &str)")
end = content.find("fn store_result", start)

new_func = '    fn speak(&self, text: &str) {\n'
new_func += '        if !self.config.tts_enabled { return; }\n'
new_func += '        let safe_text = text.replace("\"", " ").replace("\\n", " ");\n'
new_func += '        // Edge TTS - headless MP3 generalas\n'
new_func += '        let _ = std::process::Command::new("python")\n'
new_func += '            .args(["-m", "edge_tts", "--voice", "en-US-AriaNeural", "--text", &safe_text, "--write-media", "tts_output.mp3"])\n'
new_func += '            .spawn();\n'
new_func += '        std::thread::sleep(std::time::Duration::from_millis(2000));\n'
new_func += '        // Headless lejatszas ffplay-el (-nodisp = no window, -autoexit = kilepes utan)\n'
new_func += '        let _ = std::process::Command::new("ffplay")\n'
new_func += '            .args(["-nodisp", "-autoexit", "-loglevel", "quiet", "tts_output.mp3"])\n'
new_func += '            .spawn();\n'
new_func += '    }\n\n    '

content = content[:start] + new_func + content[end:]
open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK")