//! Binary Zen key store — `zen_keys.bin`
//!
//! Zero-JSON, pure bincode. ORA kulcsai binárisan.
//! Formátum: ZKEY magic + version + Provider[] + Model[].
//!
//! Váltja ki a zen_keys.json-t — gyorsabb, kisebb, biztonságosabb.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// ZKEY magic bytes
const MAGIC: [u8; 4] = [0x5A, 0x4B, 0x45, 0x59]; // "ZKEY"
const VERSION: u8 = 1;

/// A teljes Zen kulcstár
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZenKeyStore {
    pub magic: [u8; 4],
    pub version: u8,
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
}

/// Egy API provider (openai, groq, sambanova, ...)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Provider {
    /// "openai" | "groq" | "sambanova" | ...
    pub name: String,
    /// API base URL
    pub base_url: String,
    /// Kulcs rotációs stratégia
    pub rotation: RotationStrategy,
    /// API kulcsok (priority sorrendben)
    pub keys: Vec<KeyEntry>,
}

/// Kulcs rotációs stratégia
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum RotationStrategy {
    RoundRobin,
    Priority,
}

impl RotationStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RoundRobin => "round-robin",
            Self::Priority => "priority",
        }
    }
}

impl FromStr for RotationStrategy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "priority" => Ok(Self::Priority),
            _ => Ok(Self::RoundRobin),
        }
    }
}

/// Egy API kulcs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyEntry {
    /// Maga a kulcs
    pub key: String,
    /// 0 = elsődleges, 1 = másodlagos, ...
    pub priority: u8,
    /// Ha tudjuk, mennyi quota maradt
    pub quota_remaining: Option<f64>,
    /// Utolsó hibaüzenet
    #[serde(default)]
    pub last_error: Option<String>,
    /// Letiltva (pl. 429 miatt)
    #[serde(default)]
    pub disabled: bool,
}

/// Egy modell definíció
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Model {
    /// Modell ID (pl. "deepseek-v4-flash-free")
    pub id: String,
    /// Provider neve (None = default / openai)
    pub provider: Option<String>,
    /// Endpoint típus: "chat/completions" vagy "messages"
    pub endpoint: String,
    /// Ingyenes modell?
    pub free: bool,
    /// Priority (0 = legmagasabb)
    pub priority: u8,
}

impl ZenKeyStore {
    /// Betöltés bináris fájlból
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.as_ref().exists() {
            return Ok(Self {
                magic: MAGIC,
                version: VERSION,
                providers: Vec::new(),
                models: Vec::new(),
            });
        }
        let bytes = fs::read(path.as_ref())?;
        let store: ZenKeyStore = bincode::deserialize(&bytes)?;
        if store.magic != MAGIC {
            return Err("Invalid zen_keys.bin magic bytes".into());
        }
        if store.version != VERSION {
            return Err(format!("Unsupported zen_keys.bin version: {}", store.version).into());
        }
        Ok(store)
    }

    /// Mentés bincode formátumban
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = bincode::serialize(self)?;
        let tmp = path.as_ref().with_extension("bin.tmp");
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, path.as_ref())?;
        Ok(())
    }

    /// Kulcs lekérése provider + priority szerint (legkisebb priority = első)
    pub fn get_key(&self, provider: &str) -> Option<&KeyEntry> {
        self.providers
            .iter()
            .find(|p| p.name == provider)
            .and_then(|p| p.keys.iter().find(|k| !k.disabled))
    }

    /// Összes aktív kulcs egy providerhez
    pub fn get_all_keys(&self, provider: &str) -> Vec<&KeyEntry> {
        self.providers
            .iter()
            .find(|p| p.name == provider)
            .map(|p| {
                let mut v: Vec<_> = p.keys.iter().filter(|k| !k.disabled).collect();
                v.sort_by_key(|k| k.priority);
                v
            })
            .unwrap_or_default()
    }

    /// Modell lekérése ID alapján
    pub fn get_model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Alapértelmezett modell (legmagasabb priority, free)
    pub fn default_model(&self) -> Option<&Model> {
        self.models.iter().min_by_key(|m| m.priority)
    }

    /// Ingyenes modellek listája
    pub fn free_models(&self) -> Vec<&Model> {
        let mut v: Vec<_> = self.models.iter().filter(|m| m.free).collect();
        v.sort_by_key(|m| m.priority);
        v
    }

    /// Hiba bejegyzése — letiltja a kulcsot 429/401/403 esetén
    pub fn record_error(&mut self, provider: &str, key_idx: usize, error: &str) {
        if let Some(p) = self.providers.iter_mut().find(|p| p.name == provider) {
            if let Some(entry) = p.keys.get_mut(key_idx) {
                entry.last_error = Some(error.to_string());
                if error.contains("429") || error.contains("401") || error.contains("403") {
                    entry.disabled = true;
                }
            }
        }
    }

    /// Sikeres hívás után
    pub fn record_success(&mut self, provider: &str, key_idx: usize) {
        if let Some(p) = self.providers.iter_mut().find(|p| p.name == provider) {
            if let Some(entry) = p.keys.get_mut(key_idx) {
                entry.last_error = None;
                entry.disabled = false;
            }
        }
    }

    /// JSON-ból importálás (zen_keys.json formátum)
    pub fn import_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json: serde_json::Value = serde_json::from_str(json_str)?;

        let mut providers = Vec::new();

        // OpenAI/Zen provider
        if let Some(base_url) = json["base_url"].as_str() {
            let rotation = json["rotation"]
                .as_str()
                .unwrap_or("round-robin")
                .parse()
                .unwrap_or(RotationStrategy::RoundRobin);
            let keys: Vec<KeyEntry> = json["keys"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(i, k)| KeyEntry {
                            key: k.as_str().unwrap_or("").to_string(),
                            priority: i as u8,
                            quota_remaining: None,
                            last_error: None,
                            disabled: false,
                        })
                        .collect()
                })
                .unwrap_or_default();
            providers.push(Provider {
                name: "openai".to_string(),
                base_url: base_url.to_string(),
                rotation,
                keys,
            });
        }

        // Groq
        if let Some(groq) = json["groq"].as_object() {
            let base_url = groq["base_url"]
                .as_str()
                .unwrap_or("https://api.groq.com/openai/v1")
                .to_string();
            let rotation = groq["rotation"]
                .as_str()
                .unwrap_or("round-robin")
                .parse()
                .unwrap_or(RotationStrategy::RoundRobin);
            let keys: Vec<KeyEntry> = groq["keys"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(i, k)| KeyEntry {
                            key: k.as_str().unwrap_or("").to_string(),
                            priority: i as u8,
                            quota_remaining: None,
                            last_error: None,
                            disabled: false,
                        })
                        .collect()
                })
                .unwrap_or_default();
            providers.push(Provider {
                name: "groq".to_string(),
                base_url,
                rotation,
                keys,
            });
        }

        // SambaNova
        if let Some(sn) = json["sambanova"].as_object() {
            let base_url = sn["base_url"]
                .as_str()
                .unwrap_or("https://api.sambanova.ai/v1")
                .to_string();
            let rotation = sn["rotation"]
                .as_str()
                .unwrap_or("round-robin")
                .parse()
                .unwrap_or(RotationStrategy::RoundRobin);
            let keys: Vec<KeyEntry> = sn["keys"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(i, k)| KeyEntry {
                            key: k.as_str().unwrap_or("").to_string(),
                            priority: i as u8,
                            quota_remaining: None,
                            last_error: None,
                            disabled: false,
                        })
                        .collect()
                })
                .unwrap_or_default();
            providers.push(Provider {
                name: "sambanova".to_string(),
                base_url,
                rotation,
                keys,
            });
        }

        // Modellek
        let models: Vec<Model> = json["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|m| Model {
                        id: m["id"].as_str().unwrap_or("unknown").to_string(),
                        provider: m["provider"].as_str().map(|s| s.to_string()),
                        endpoint: m["endpoint"]
                            .as_str()
                            .unwrap_or("chat/completions")
                            .to_string(),
                        free: m["free"].as_bool().unwrap_or(false),
                        priority: m["priority"].as_u64().unwrap_or(99) as u8,
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            magic: MAGIC,
            version: VERSION,
            providers,
            models,
        })
    }

    /// Szöveges statisztika
    pub fn stats(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Providers: {}\n", self.providers.len()));
        for p in &self.providers {
            let active = p.keys.iter().filter(|k| !k.disabled).count();
            let total = p.keys.len();
            s.push_str(&format!(
                "  {}: {}/{} keys active, rotation={}\n",
                p.name,
                active,
                total,
                p.rotation.as_str()
            ));
        }
        s.push_str(&format!("Models: {}\n", self.models.len()));
        for m in &self.models {
            let prov = m.provider.as_deref().unwrap_or("openai");
            s.push_str(&format!(
                "  #{} {} [{}] {} {}\n",
                m.priority,
                m.id,
                prov,
                m.endpoint,
                if m.free { "FREE" } else { "PAID" }
            ));
        }
        s
    }
}

/// Alapértelmezett útvonal a zen_keys.bin-hez
pub fn default_zen_keys_path(output_dir: &str) -> String {
    format!("{}/zen_keys.bin", output_dir)
}
