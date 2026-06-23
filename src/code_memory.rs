//! Code Memory — dedikált kódmemória réteg kódoló agentek számára
//!
//! Claude Code, Cline, Kilo Code, OpenCode és más kódoló agentek
//! memória rétege. Tárolja és visszakeresi:
//! - Kódrészletek és függvények
//! - Projektstruktúra és függőségek
//! - Hiba → megoldás párok
//! - Kódolási konvenciók
//! - Refaktorálási előzmények
//!
//! Használat:
//!   microscope-mem code store --func "fn parse()" --file src/parser.rs
//!   microscope-mem code recall --query "hogyan oldottuk meg a borrow checkert"

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub enum CodeEntryType {
    /// Függvény/metódus definíció
    Function,
    /// Osztály/trait/struct definíció
    Type,
    /// Import/modul referenciák
    Import,
    /// Hiba → megoldás pár
    ErrorSolution,
    /// Projekt konfiguráció
    Config,
    /// Függőség (crate, package)
    Dependency,
    /// Kódolási konvenció
    Convention,
    /// Fájl/projekt struktúra
    Structure,
    /// Megjegyzés/dokumentáció
    Note,
}

#[derive(Debug, Clone)]
pub struct CodeEntry {
    pub id: u64,
    pub entry_type: CodeEntryType,
    pub title: String,
    pub code: String,
    pub file_path: String,
    pub language: String,
    pub project: String,
    pub symbols: Vec<String>,
    pub tags: Vec<String>,
    pub solution: Option<String>,
    pub created_ms: u64,
    pub access_count: u64,
}

#[derive(Debug, Clone)]
pub struct CodeQuery {
    pub query: String,
    pub language: Option<String>,
    pub entry_type: Option<CodeEntryType>,
    pub project: Option<String>,
    pub file: Option<String>,
    pub k: usize,
}

pub struct CodeMemory {
    entries: Arc<RwLock<Vec<CodeEntry>>>,
    next_id: Arc<RwLock<u64>>,
    project_index: Arc<RwLock<HashMap<String, Vec<u64>>>>,
    symbol_index: Arc<RwLock<HashMap<String, Vec<u64>>>>,
}

impl Default for CodeMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeMemory {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            next_id: Arc::new(RwLock::new(1)),
            project_index: Arc::new(RwLock::new(HashMap::new())),
            symbol_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn nid(&self) -> u64 {
        let mut id = self.next_id.write().unwrap();
        let n = *id;
        *id += 1;
        n
    }
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &self,
        entry_type: CodeEntryType,
        title: &str,
        code: &str,
        file_path: &str,
        language: &str,
        project: &str,
        symbols: Vec<String>,
        tags: Vec<String>,
    ) -> u64 {
        let id = self.nid();
        let entry = CodeEntry {
            id,
            entry_type,
            title: title.to_string(),
            code: code.to_string(),
            file_path: file_path.to_string(),
            language: language.to_string(),
            project: project.to_string(),
            symbols: symbols.clone(),
            tags,
            solution: None,
            created_ms: self.now(),
            access_count: 0,
        };

        self.entries.write().unwrap().push(entry);

        // Indexek
        self.project_index
            .write()
            .unwrap()
            .entry(project.to_string())
            .or_default()
            .push(id);
        for sym in &symbols {
            self.symbol_index
                .write()
                .unwrap()
                .entry(sym.to_lowercase())
                .or_default()
                .push(id);
        }

        id
    }

    pub fn store_error_solution(
        &self,
        error: &str,
        solution: &str,
        file_path: &str,
        language: &str,
        project: &str,
    ) -> u64 {
        let id = self.nid();
        let entry = CodeEntry {
            id,
            entry_type: CodeEntryType::ErrorSolution,
            title: format!("Error: {}", error.chars().take(80).collect::<String>()),
            code: error.to_string(),
            file_path: file_path.to_string(),
            language: language.to_string(),
            project: project.to_string(),
            symbols: vec![],
            tags: vec!["error".to_string(), "solution".to_string()],
            solution: Some(solution.to_string()),
            created_ms: self.now(),
            access_count: 0,
        };
        self.entries.write().unwrap().push(entry);
        self.project_index
            .write()
            .unwrap()
            .entry(project.to_string())
            .or_default()
            .push(id);
        id
    }

    pub fn recall(&self, query: &CodeQuery) -> Vec<CodeEntry> {
        let entries = self.entries.read().unwrap();
        let q = query.query.to_lowercase();
        let mut scored: Vec<(f64, &CodeEntry)> = Vec::new();

        for entry in entries.iter() {
            let mut score = 0.0;

            // Kulcsszó egyezés
            if entry.title.to_lowercase().contains(&q) {
                score += 3.0;
            }
            if entry.code.to_lowercase().contains(&q) {
                score += 2.0;
            }
            if let Some(ref sol) = entry.solution {
                if sol.to_lowercase().contains(&q) {
                    score += 2.0;
                }
            }

            // Szimbólum egyezés
            for sym in &entry.symbols {
                if sym.to_lowercase().contains(&q) {
                    score += 2.0;
                }
            }

            // Szűrők
            if let Some(ref lang) = query.language {
                if entry.language != *lang {
                    score -= 1.0;
                }
            }
            if let Some(ref etype) = query.entry_type {
                if entry.entry_type != *etype {
                    score -= 1.0;
                }
            }
            if let Some(ref proj) = query.project {
                if entry.project != *proj {
                    score -= 0.5;
                }
            }

            // Access count boost
            score += (entry.access_count as f64) * 0.1;

            if score > 0.0 {
                scored.push((score, entry));
            }
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.truncate(query.k);

        scored
            .into_iter()
            .map(|(_, e)| {
                let mut entry = e.clone();
                entry.access_count += 1;
                entry
            })
            .collect()
    }

    pub fn recall_by_symbol(&self, symbol: &str) -> Vec<CodeEntry> {
        let sym_idx = self.symbol_index.read().unwrap();
        let sym = symbol.to_lowercase();
        let ids: Vec<u64> = sym_idx.get(&sym).cloned().unwrap_or_default();

        let entries = self.entries.read().unwrap();
        ids.iter()
            .filter_map(|id| entries.iter().find(|e| e.id == *id))
            .cloned()
            .collect()
    }

    pub fn recall_by_project(&self, project: &str) -> Vec<CodeEntry> {
        let proj_idx = self.project_index.read().unwrap();
        let ids: Vec<u64> = proj_idx.get(project).cloned().unwrap_or_default();

        let entries = self.entries.read().unwrap();
        ids.iter()
            .filter_map(|id| entries.iter().find(|e| e.id == *id))
            .cloned()
            .collect()
    }

    pub fn list_by_type(&self, etype: CodeEntryType) -> Vec<CodeEntry> {
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .filter(|e| e.entry_type == etype)
            .cloned()
            .collect()
    }

    pub fn get(&self, id: u64) -> Option<CodeEntry> {
        self.entries
            .read()
            .unwrap()
            .iter()
            .find(|e| e.id == id)
            .cloned()
    }

    pub fn stats(&self) -> (usize, usize, Vec<(String, usize)>) {
        let entries = self.entries.read().unwrap();
        let total = entries.len();
        let errors = entries
            .iter()
            .filter(|e| e.entry_type == CodeEntryType::ErrorSolution)
            .count();

        let mut projects: HashMap<String, usize> = HashMap::new();
        for e in entries.iter() {
            *projects.entry(e.project.clone()).or_default() += 1;
        }
        let mut sorted: Vec<_> = projects.into_iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.1));

        (total, errors, sorted)
    }
}

impl std::fmt::Display for CodeEntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeEntryType::Function => write!(f, "Function"),
            CodeEntryType::Type => write!(f, "Type"),
            CodeEntryType::Import => write!(f, "Import"),
            CodeEntryType::ErrorSolution => write!(f, "ErrorSolution"),
            CodeEntryType::Config => write!(f, "Config"),
            CodeEntryType::Dependency => write!(f, "Dependency"),
            CodeEntryType::Convention => write!(f, "Convention"),
            CodeEntryType::Structure => write!(f, "Structure"),
            CodeEntryType::Note => write!(f, "Note"),
        }
    }
}
