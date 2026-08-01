import re

with open("D:/codex/microscope-memory/src/embeddings.rs", "r", encoding="utf8") as f:
    content = f.read()

# Use Mutex for interior mutability
content = content.replace(
    "pub struct PythonEmbeddingProvider {",
    "pub struct PythonEmbeddingProvider {"
)

content = content.replace(
    "    stdin: std::process::ChildStdin,",
    "    stdin: std::sync::Mutex<std::process::ChildStdin>,"
)

content = content.replace(
    "    stdout: std::process::ChildStdout,",
    "    stdout: std::sync::Mutex<std::process::ChildStdout>,"
)

# Fix embed() to use Mutex
old_embed = """    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        use std::io::{Read, Write};

        // Write text + newline to stdin
        let mut stdin_guard = &self.stdin;
        writeln!(stdin_guard, \"{}\", text)
            .map_err(|e| EmbeddingError::ApiError(format!(\"write stdin: {}\", e)))?;
        stdin_guard.flush()
            .map_err(|e| EmbeddingError::ApiError(format!(\"flush stdin: {}\", e)))?;

        // Read embedding: dim * f32 bytes
        let mut buf = vec![0u8; self.dim * 4];
        let mut stdout_guard = &mut self.stdout;
        stdout_guard.read_exact(&mut buf)
            .map_err(|e| EmbeddingError::ApiError(format!(\"read stdout: {}\", e)))?;

        let embedding: Vec<f32> = buf
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();

        Ok(embedding)
    }"""

new_embed = """    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        use std::io::{Read, Write};

        // Write text + newline to stdin
        let mut stdin_guard = self.stdin.lock().unwrap();
        writeln!(stdin_guard, \"{}\", text)
            .map_err(|e| EmbeddingError::ApiError(format!(\"write stdin: {}\", e)))?;
        stdin_guard.flush()
            .map_err(|e| EmbeddingError::ApiError(format!(\"flush stdin: {}\", e)))?;
        drop(stdin_guard);

        // Read embedding: dim * f32 bytes
        let mut buf = vec![0u8; self.dim * 4];
        let mut stdout_guard = self.stdout.lock().unwrap();
        stdout_guard.read_exact(&mut buf)
            .map_err(|e| EmbeddingError::ApiError(format!(\"read stdout: {}\", e)))?;

        let embedding: Vec<f32> = buf
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();

        Ok(embedding)
    }"""

if old_embed in content:
    content = content.replace(old_embed, new_embed)
    with open("D:/codex/microscope-memory/src/embeddings.rs", "w", encoding="utf8") as f:
        f.write(content)
    print("OK - embed() patched")
else:
    print("NOT FOUND - embed()")
    idx = content.find("fn embed(&self, text:")
    if idx >= 0:
        print(content[idx:idx+600])
    else:
        print("embed() not found at all")
