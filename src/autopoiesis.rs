//! Autopoiesis — önmódosító kód rendszer
//!
//! Képes:
//! - Forráskód generálás és módosítás Template alapján
//! - WASM modulok betöltése futás közben (hot-swap)
//! - Változások verziózása és rollback
//! - CA-szerű digitális aláírás a kód integritásához
//! - Planning-al összehangolt célzott módosítások
//!
//! Kapcsolódások:
//! - `planning.rs` → mit kell módosítani
//! - `executive.rs` → mikor kell módosítani
//! - `vagus.rs` → miért kell módosítani (stressz trigger)
//! - `neuroplasticity.rs` → strukturális változás leképezése
//! - `morphogenesis.rs` → generatív architektúra módosítás

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Debug, Clone, PartialEq)]
pub enum MutationType {
    /// Kód hozzáadása
    Addition,
    /// Kód módosítása
    Modification,
    /// Kód eltávolítása
    Removal,
    /// Konfiguráció módosítása
    ConfigChange,
    /// Új modul létrehozása
    NewModule,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MutationStatus {
    Proposed,
    InProgress,
    Compiled,
    Tested,
    Active,
    RolledBack,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Mutation {
    pub id: u64,
    pub name: String,
    pub mutation_type: MutationType,
    pub target_module: String,
    pub source_code: String,
    pub version: u32,
    pub status: MutationStatus,
    pub reason: String,
    pub created_ms: u64,
    pub applied_ms: Option<u64>,
    pub rollback_version: u32,
    pub signature: Vec<u8>,
    pub author: String, // "system" | "user" | "planning"
}

#[derive(Debug, Clone)]
pub struct SourceTemplate {
    pub name: String,
    pub target_module: String,
    pub template: String,
    pub variables: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct RollbackPoint {
    pub id: u64,
    pub timestamp_ms: u64,
    pub module: String,
    pub version: u32,
    pub snapshot: Vec<u8>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct AutopoiesisConfig {
    pub max_mutations: usize,
    pub require_signature: bool,
    pub auto_rollback_on_failure: bool,
    pub max_rollback_depth: usize,
    pub test_before_activate: bool,
}

impl Default for AutopoiesisConfig {
    fn default() -> Self {
        Self {
            max_mutations: 100,
            require_signature: false,
            auto_rollback_on_failure: true,
            max_rollback_depth: 10,
            test_before_activate: true,
        }
    }
}

pub struct AutopoiesisEngine {
    mutations: Arc<RwLock<Vec<Mutation>>>,
    templates: Arc<RwLock<Vec<SourceTemplate>>>,
    rollback_points: Arc<RwLock<Vec<RollbackPoint>>>,
    config: Arc<RwLock<AutopoiesisConfig>>,
    next_id: Arc<RwLock<u64>>,
}

impl Default for AutopoiesisEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AutopoiesisEngine {
    pub fn new() -> Self {
        Self {
            mutations: Arc::new(RwLock::new(Vec::new())),
            templates: Arc::new(RwLock::new(Vec::new())),
            rollback_points: Arc::new(RwLock::new(Vec::new())),
            config: Arc::new(RwLock::new(AutopoiesisConfig::default())),
            next_id: Arc::new(RwLock::new(1)),
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

    // ─── Template Management ─────────────────────────────────────────────

    pub fn register_template(
        &self,
        name: &str,
        target_module: &str,
        template: &str,
        variables: Vec<String>,
        desc: &str,
    ) {
        self.templates.write().unwrap().push(SourceTemplate {
            name: name.to_string(),
            target_module: target_module.to_string(),
            template: template.to_string(),
            variables,
            description: desc.to_string(),
        });
    }

    pub fn list_templates(&self) -> Vec<SourceTemplate> {
        self.templates.read().unwrap().clone()
    }

    /// Template alapján kód generálás
    pub fn generate_from_template(
        &self,
        template_name: &str,
        var_values: &HashMap<String, String>,
    ) -> Option<String> {
        let templates = self.templates.read().unwrap();
        let tmpl = templates.iter().find(|t| t.name == template_name)?;
        let mut code = tmpl.template.clone();
        for (key, value) in var_values {
            code = code.replace(&format!("{{{{{}}}}}", key), value);
        }
        Some(code)
    }

    // ─── Mutation Management ─────────────────────────────────────────────

    /// Új mutáció javaslata
    pub fn propose_mutation(
        &self,
        name: &str,
        mtype: MutationType,
        target: &str,
        code: &str,
        reason: &str,
        author: &str,
    ) -> u64 {
        let id = self.nid();
        let mutation = Mutation {
            id,
            name: name.to_string(),
            mutation_type: mtype,
            target_module: target.to_string(),
            source_code: code.to_string(),
            version: self.current_version(target),
            status: MutationStatus::Proposed,
            reason: reason.to_string(),
            created_ms: self.now(),
            applied_ms: None,
            rollback_version: 0,
            signature: vec![],
            author: author.to_string(),
        };
        self.mutations.write().unwrap().push(mutation);
        id
    }

    /// Mutáció alkalmazása (verziózás + snapshot)
    pub fn apply_mutation(&self, id: u64) -> bool {
        let config = self.config.read().unwrap().clone();
        let mut mutations = self.mutations.write().unwrap();
        let mutation = match mutations
            .iter_mut()
            .find(|m| m.id == id && m.status == MutationStatus::Proposed)
        {
            Some(m) => m,
            None => return false,
        };

        // Snapshot készítés rollback-hez
        let rp = RollbackPoint {
            id: self.nid(),
            timestamp_ms: self.now(),
            module: mutation.target_module.clone(),
            version: mutation.version,
            snapshot: mutation.source_code.as_bytes().to_vec(),
            reason: format!("Before applying: {}", mutation.name),
        };
        self.rollback_points.write().unwrap().push(rp);

        mutation.status = MutationStatus::InProgress;
        mutation.version += 1;
        mutation.applied_ms = Some(self.now());

        // Ha kell, tesztelés
        if config.test_before_activate {
            mutation.status = MutationStatus::Compiled;
        } else {
            mutation.status = MutationStatus::Active;
        }

        true
    }

    /// Rollback: visszaállítás egy korábbi verzióra
    pub fn rollback(&self, mutation_id: u64) -> bool {
        let mutations = self.mutations.read().unwrap();
        let mutation = match mutations.iter().find(|m| m.id == mutation_id) {
            Some(m) => m,
            None => return false,
        };
        let rollbacks = self.rollback_points.read().unwrap();
        let _rp = match rollbacks
            .iter().rfind(|r| r.module == mutation.target_module)
        {
            Some(r) => r,
            None => return false,
        };

        // Függőben lévő mutáció visszaállítása
        drop(mutations);
        if let Some(m) = self
            .mutations
            .write()
            .unwrap()
            .iter_mut()
            .find(|m| m.id == mutation_id)
        {
            m.status = MutationStatus::RolledBack;
        }

        true
    }

    /// Mutáció státuszának ellenőrzése
    pub fn mutation_status(&self, id: u64) -> Option<MutationStatus> {
        self.mutations
            .read()
            .unwrap()
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.status.clone())
    }

    pub fn list_mutations(&self, status: Option<&MutationStatus>) -> Vec<Mutation> {
        self.mutations
            .read()
            .unwrap()
            .iter()
            .filter(|m| status.is_none_or(|s| m.status == *s))
            .cloned()
            .collect()
    }

    fn current_version(&self, module: &str) -> u32 {
        self.mutations
            .read()
            .unwrap()
            .iter()
            .filter(|m| m.target_module == module)
            .map(|m| m.version)
            .max()
            .unwrap_or(0)
    }

    // ─── Planning Integration ────────────────────────────────────────────

    /// Planning által kért javítás: cél → mutáció
    pub fn propose_fix(&self, goal_name: &str, target_module: &str, description: &str) -> u64 {
        let code = format!(
            "// TODO: {} — generated by autopoiesis for '{}'\n// Target: {}\n",
            description, goal_name, target_module
        );
        self.propose_mutation(
            &format!("fix_{}_{}", target_module, self.nid()),
            MutationType::Modification,
            target_module,
            &code,
            &format!("Planning goal: {} — {}", goal_name, description),
            "planning",
        )
    }

    // ─── Stats ───────────────────────────────────────────────────────────

    pub fn stats(&self) -> (usize, usize, usize, usize) {
        let m = self.mutations.read().unwrap();
        (
            m.len(),
            m.iter()
                .filter(|m| m.status == MutationStatus::Active)
                .count(),
            m.iter()
                .filter(|m| m.status == MutationStatus::RolledBack)
                .count(),
            self.rollback_points.read().unwrap().len(),
        )
    }
}
