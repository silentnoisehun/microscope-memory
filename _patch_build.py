import re

with open("D:/codex/microscope-memory/src/build.rs", "r", encoding="utf8") as f:
    content = f.read()

old = """        let provider: Box<dyn crate::embeddings::EmbeddingProvider> =
            if config.embedding.provider == \"candle\" {
                match crate::embeddings::CandleEmbeddingProvider::new(&config.embedding.model) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        eprintln!(
                            \"  {} candle init: {} — falling back to mock\",
                            \"WARN\".yellow(), e
                        );
                        Box::new(crate::embeddings::MockEmbeddingProvider::new(
                            config.embedding.dim,
                        ))
                    }
                }
            } else {
                Box::new(crate::embeddings::MockEmbeddingProvider::new(
                    config.embedding.dim,
                ))
            };"""

new = """        let provider: Box<dyn crate::embeddings::EmbeddingProvider> =
            if config.embedding.provider == \"candle\" {
                match crate::embeddings::CandleEmbeddingProvider::new(&config.embedding.model) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        eprintln!(
                            \"  {} candle init: {} — falling back to mock\",
                            \"WARN\".yellow(), e
                        );
                        Box::new(crate::embeddings::MockEmbeddingProvider::new(
                            config.embedding.dim,
                        ))
                    }
                }
            } else if config.embedding.provider == \"python\" {
                let script_path = std::path::Path::new(&config.paths.layers_dir)
                    .parent()
                    .map(|p| p.join(\"embed.py\"))
                    .unwrap_or_else(|| std::path::PathBuf::from(\"embed.py\"));
                match crate::embeddings::PythonEmbeddingProvider::new(
                    &script_path.to_string_lossy(),
                    &config.embedding.model,
                ) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        eprintln!(
                            \"  {} python init: {} — falling back to mock\",
                            \"WARN\".yellow(), e
                        );
                        Box::new(crate::embeddings::MockEmbeddingProvider::new(
                            config.embedding.dim,
                        ))
                    }
                }
            } else {
                Box::new(crate::embeddings::MockEmbeddingProvider::new(
                    config.embedding.dim,
                ))
            };"""

if old in content:
    content = content.replace(old, new)
    with open("D:/codex/microscope-memory/src/build.rs", "w", encoding="utf8") as f:
        f.write(content)
    print("build.rs patched successfully")
else:
    print("ERROR: old pattern not found in build.rs")
    # Print context around the area
    idx = content.find("let provider: Box<dyn crate::embeddings::EmbeddingProvider>")
    if idx >= 0:
        print(content[idx:idx+800])
    else:
        print("Pattern 'let provider: Box<dyn crate::embeddings::EmbeddingProvider>' not found at all")
