//! Print one embedding vector for cross-provider verification.

fn main() {
    let mut args = std::env::args().skip(1);
    let provider = args.next().unwrap_or_else(|| "candle".to_string());
    let model = args.next().unwrap_or_else(|| {
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2".to_string()
    });
    let text = args
        .next()
        .unwrap_or_else(|| "teszt magyar memória".to_string());
    let mut config = microscope_memory::config::Config::default();
    config.embedding.provider = provider;
    config.embedding.model = model;
    config.embedding.dim = 384;
    let p = microscope_memory::embeddings::provider_from_config(&config.embedding, 384);
    let vector = p.embed(&text).expect("embedding");
    println!(
        "{}",
        vector
            .iter()
            .map(|v| format!("{:.8}", v))
            .collect::<Vec<_>>()
            .join(",")
    );
}
