//! Explicit memory — declarative knowledge (facts, events, concepts)
//! Facts, events, concepts that can be consciously recalled and reported

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Fact {
    pub statement: String,
    pub confidence: f32,    // 0.0-1.0
    pub source: String,
    pub timestamp_ms: u64,
    pub access_count: u32,
    pub verified: bool,
}

#[derive(Clone, Debug)]
pub struct Concept {
    pub name: String,
    pub definition: String,
    pub examples: Vec<String>,
    pub related_concepts: Vec<String>,
    pub abstraction_level: f32, // 0.0 = concrete, 1.0 = abstract
}

#[derive(Clone, Debug)]
pub struct Event {
    pub description: String,
    pub timestamp_ms: u64,
    pub participants: Vec<String>,
    pub location: String,
    pub emotional_significance: f32,
    pub detail_level: f32, // 0.0 = vague, 1.0 = detailed
}

pub struct ExplicitMemory {
    pub facts: HashMap<String, Fact>,
    pub concepts: HashMap<String, Concept>,
    pub events: Vec<Event>,
    pub relationships: HashMap<String, Vec<String>>, // concept -> related concepts
}

impl ExplicitMemory {
    pub fn new() -> Self {
        Self {
            facts: HashMap::new(),
            concepts: HashMap::new(),
            events: Vec::new(),
            relationships: HashMap::new(),
        }
    }

    /// Store a declarative fact
    pub fn store_fact(&mut self, statement: &str, source: &str, confidence: f32) {
        let key = statement.to_lowercase();
        self.facts.entry(key.clone())
            .and_modify(|f| {
                f.access_count += 1;
                f.confidence = (f.confidence + confidence) / 2.0;
            })
            .or_insert_with(|| Fact {
                statement: statement.to_string(),
                confidence: confidence.clamp(0.0, 1.0),
                source: source.to_string(),
                timestamp_ms: Self::now_ms(),
                access_count: 1,
                verified: confidence > 0.8,
            });
    }

    /// Define a concept
    pub fn define_concept(&mut self, name: &str, definition: &str, abstraction: f32) {
        self.concepts.insert(name.to_lowercase(), Concept {
            name: name.to_string(),
            definition: definition.to_string(),
            examples: Vec::new(),
            related_concepts: Vec::new(),
            abstraction_level: abstraction.clamp(0.0, 1.0),
        });
    }

    /// Add example to concept
    pub fn add_example(&mut self, concept: &str, example: &str) {
        if let Some(c) = self.concepts.get_mut(&concept.to_lowercase()) {
            c.examples.push(example.to_string());
        }
    }

    /// Record an event
    pub fn record_event(&mut self, description: &str, location: &str, emotional_sig: f32) {
        self.events.push(Event {
            description: description.to_string(),
            timestamp_ms: Self::now_ms(),
            participants: Vec::new(),
            location: location.to_string(),
            emotional_significance: emotional_sig.clamp(0.0, 1.0),
            detail_level: 0.5,
        });
    }

    /// Link concepts
    pub fn relate_concepts(&mut self, concept1: &str, concept2: &str) {
        let key1 = concept1.to_lowercase();
        let key2 = concept2.to_lowercase();
        
        self.relationships.entry(key1.clone())
            .or_insert_with(Vec::new)
            .push(key2.clone());
        
        self.relationships.entry(key2)
            .or_insert_with(Vec::new)
            .push(key1);
    }

    /// Retrieve fact with confidence
    pub fn recall_fact(&mut self, statement: &str) -> Option<(String, f32)> {
        let key = statement.to_lowercase();
        if let Some(fact) = self.facts.get_mut(&key) {
            fact.access_count += 1;
            return Some((fact.statement.clone(), fact.confidence));
        }
        None
    }

    /// Get concept definition
    pub fn get_concept(&self, name: &str) -> Option<&Concept> {
        self.concepts.get(&name.to_lowercase())
    }

    /// Get all facts sorted by confidence
    pub fn high_confidence_facts(&self, min_confidence: f32) -> Vec<&Fact> {
        let mut facts: Vec<_> = self.facts.values()
            .filter(|f| f.confidence >= min_confidence)
            .collect();
        facts.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        facts
    }

    /// Get recent events
    pub fn recent_events(&self, count: usize) -> Vec<&Event> {
        let mut events = self.events.iter().collect::<Vec<_>>();
        events.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));
        events.into_iter().take(count).collect()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("explicit_memory.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"EXPL");
        data.push(1);

        // Facts
        let fact_count = self.facts.len() as u32;
        data.extend_from_slice(&fact_count.to_le_bytes());
        for (_, fact) in &self.facts {
            let stmt_bytes = fact.statement.as_bytes();
            data.extend_from_slice(&(stmt_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(stmt_bytes);
            data.extend_from_slice(&fact.confidence.to_le_bytes());
            let src_bytes = fact.source.as_bytes();
            data.push(src_bytes.len() as u8);
            data.extend_from_slice(src_bytes);
            data.extend_from_slice(&fact.timestamp_ms.to_le_bytes());
            data.extend_from_slice(&fact.access_count.to_le_bytes());
            data.push(fact.verified as u8);
        }

        // Concepts
        let concept_count = self.concepts.len() as u32;
        data.extend_from_slice(&concept_count.to_le_bytes());
        for (_, concept) in &self.concepts {
            let name_bytes = concept.name.as_bytes();
            data.push(name_bytes.len() as u8);
            data.extend_from_slice(name_bytes);
            let def_bytes = concept.definition.as_bytes();
            data.extend_from_slice(&(def_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(def_bytes);
            data.extend_from_slice(&concept.abstraction_level.to_le_bytes());
            
            let ex_count = concept.examples.len() as u8;
            data.push(ex_count);
            for example in &concept.examples {
                let ex_bytes = example.as_bytes();
                data.extend_from_slice(&(ex_bytes.len() as u16).to_le_bytes());
                data.extend_from_slice(ex_bytes);
            }
        }

        // Events
        let event_count = self.events.len() as u32;
        data.extend_from_slice(&event_count.to_le_bytes());
        for event in &self.events {
            let desc_bytes = event.description.as_bytes();
            data.extend_from_slice(&(desc_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(desc_bytes);
            data.extend_from_slice(&event.timestamp_ms.to_le_bytes());
            let loc_bytes = event.location.as_bytes();
            data.push(loc_bytes.len() as u8);
            data.extend_from_slice(loc_bytes);
            data.extend_from_slice(&event.emotional_significance.to_le_bytes());
            data.extend_from_slice(&event.detail_level.to_le_bytes());
        }

        fs::write(&path, data).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("explicit_memory.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..4] != b"EXPL" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let mut facts = HashMap::new();
        let mut concepts = HashMap::new();
        let events = Vec::new();

        // Read facts
        if idx + 4 <= data.len() {
            let fact_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..fact_count {
                if idx + 2 > data.len() { break; }
                let stmt_len = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                if idx + stmt_len > data.len() { break; }
                let statement = String::from_utf8_lossy(&data[idx..idx+stmt_len]).to_string();
                idx += stmt_len;

                if idx + 4 > data.len() { break; }
                let confidence = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                if idx >= data.len() { break; }
                let src_len = data[idx] as usize;
                idx += 1;

                if idx + src_len > data.len() { break; }
                let source = String::from_utf8_lossy(&data[idx..idx+src_len]).to_string();
                idx += src_len;

                if idx + 13 > data.len() { break; }
                let timestamp_ms = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                     data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                let access_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let verified = data[idx] != 0;
                idx += 1;

                facts.insert(statement.to_lowercase(), Fact {
                    statement,
                    confidence,
                    source,
                    timestamp_ms,
                    access_count,
                    verified,
                });
            }
        }

        // Read concepts (simplified)
        if idx + 4 <= data.len() {
            let concept_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;
            
            for _ in 0..concept_count {
                if idx >= data.len() { break; }
                let name_len = data[idx] as usize;
                idx += 1;

                if idx + name_len + 6 > data.len() { break; }
                let name = String::from_utf8_lossy(&data[idx..idx+name_len]).to_string();
                idx += name_len;

                let def_len = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                if idx + def_len > data.len() { break; }
                let definition = String::from_utf8_lossy(&data[idx..idx+def_len]).to_string();
                idx += def_len;

                if idx + 4 > data.len() { break; }
                let abstraction_level = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                concepts.insert(name.clone(), Concept {
                    name,
                    definition,
                    examples: Vec::new(),
                    related_concepts: Vec::new(),
                    abstraction_level,
                });
            }
        }

        Ok(Self {
            facts,
            concepts,
            events,
            relationships: HashMap::new(),
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}