//! Binary key store — `keys.bin`
//!
//! Zero-JSON, pure bincode. ORA védi a kulcsokat.
//! Formátum: MKEY magic + version + KeyEntry[].
//!
//! Nem titkosítva — ORA maga a védelem (bináris formátum, fájlrendszer szintű hozzáférés).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// MKEY magic bytes
const MAGIC: [u8; 4] = [0x4D, 0x4B, 0x45, 0x59]; // "MKEY"
const VERSION: u8 = 1;

/// A teljes kulcstár
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyStore {
    pub magic: [u8; 4],
    pub version: u8,
    pub entries: Vec<KeyEntry>,
}

impl Default for KeyStore {
    fn default() -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            entries: Vec::new(),
        }
    }
}

/// Egy API kulcs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyEntry {
    /// "openai" | "gemini" | "ollama" | ...
    pub service: String,
    /// Maga a kulcs (pl. "sk-..." vagy "AIza...")
    pub key: String,
    /// 0 = elsődleges, 1 = másodlagos, 2 = harmadlagos...
    pub priority: u8,
    /// Ha tudjuk, mennyi quota maradt (None = nem tudjuk)
    pub quota_remaining: Option<f64>,
    /// Utolsó hibaüzenet (pl. "429 Too Many Requests")
    #[serde(default)]
    pub last_error: Option<String>,
    /// Letiltva, mert többször is 429-et kapott
    #[serde(default)]
    pub disabled: bool,
    /// Létrehozás időpontja (unix timestamp)
    pub created_at: i64,
}

impl KeyStore {
    /// Betölti a kulcstárat a megadott útvonalról.
    /// Ha a fájl nem létezik, üres kulcstárat ad vissza.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.as_ref().exists() {
            return Ok(Self {
                magic: MAGIC,
                version: VERSION,
                entries: Vec::new(),
            });
        }
        let bytes = fs::read(path.as_ref())?;
        let store: KeyStore = bincode::deserialize(&bytes)?;
        if store.magic != MAGIC {
            return Err("Invalid keys.bin magic bytes".into());
        }
        if store.version != VERSION {
            return Err(format!("Unsupported keys.bin version: {}", store.version).into());
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

    /// Kulcs lekérése service név alapján, priority szerint (legkisebb szám = legmagasabb priority)
    pub fn get(&self, service: &str) -> Option<&KeyEntry> {
        self.entries
            .iter()
            .filter(|e| e.service == service && !e.disabled)
            .min_by_key(|e| e.priority)
    }

    /// Az összes nem letiltott kulcs lekérése egy service-hez, priority sorrendben
    pub fn get_all(&self, service: &str) -> Vec<&KeyEntry> {
        let mut v: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.service == service && !e.disabled)
            .collect();
        v.sort_by_key(|e| e.priority);
        v
    }

    /// Kulcs beállítása (hozzáadás vagy frissítés)
    pub fn set(&mut self, service: &str, key: String, priority: u8) {
        // Ha már van ilyen service + priority, frissítsük
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.service == service && e.priority == priority)
        {
            existing.key = key;
            existing.last_error = None;
            existing.disabled = false;
            existing.created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            return;
        }
        // Különben új bejegyzés
        self.entries.push(KeyEntry {
            service: service.to_string(),
            key,
            priority,
            quota_remaining: None,
            last_error: None,
            disabled: false,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        });
    }

    /// Kulcs eltávolítása
    pub fn remove(&mut self, service: &str, priority: u8) {
        self.entries
            .retain(|e| !(e.service == service && e.priority == priority));
    }

    /// Hiba bejegyzése egy kulcshoz — ha quota hiba, letiltja
    pub fn record_error(&mut self, service: &str, priority: u8, error: &str) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.service == service && e.priority == priority)
        {
            entry.last_error = Some(error.to_string());
            // 429 = quota elfogyott, 401 = auth hiba → letiltjuk
            if error.contains("429") || error.contains("401") || error.contains("403") {
                entry.disabled = true;
            }
        }
    }

    /// Sikeres hívás után: töröljük a hibát, opcionálisan frissítjük a quota-t
    pub fn record_success(&mut self, service: &str, priority: u8) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.service == service && e.priority == priority)
        {
            entry.last_error = None;
            // Ha újra működik, engedélyezzük
            entry.disabled = false;
        }
    }

    /// Összes kulcs újraengedélyezése (pl. időzítő alapján)
    pub fn reset_all(&mut self) {
        for entry in &mut self.entries {
            entry.disabled = false;
            entry.last_error = None;
        }
    }

    /// Kulcsok listázása (key nélkül, csak metaadatok)
    pub fn list(&self) -> Vec<KeyEntryInfo> {
        self.entries
            .iter()
            .map(|e| KeyEntryInfo {
                service: e.service.clone(),
                priority: e.priority,
                quota_remaining: e.quota_remaining,
                last_error: e.last_error.clone(),
                disabled: e.disabled,
                created_at: e.created_at,
                key_preview: if e.key.len() > 8 {
                    format!("{}...", &e.key[..8])
                } else {
                    "***".to_string()
                },
            })
            .collect()
    }
}

/// Publikus kulcs info (key nélkül)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyEntryInfo {
    pub service: String,
    pub priority: u8,
    pub quota_remaining: Option<f64>,
    pub last_error: Option<String>,
    pub disabled: bool,
    pub created_at: i64,
    pub key_preview: String,
}

/// Alapértelmezett útvonal a keys.bin-hez
pub fn default_keys_path(output_dir: &str) -> String {
    format!("{}/keys.bin", output_dir)
}
