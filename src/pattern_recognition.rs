//! Pattern Recognition — minta felismerő és tanuló rendszer a Microscope Memory számára
//!
//! Képes:
//! - Szekvenciális minták detektálására (gyakori gondolatmenetek)
//! - Temporális mintákra (napi ritmusok, ciklikus aktivitás)
//! - Strukturális motívumokra (architektúra gráfokban)
//! - Cluster mintákra (memória térbeli csoportosulások)
//! - Cross-domain korrelációra (minták átfedése különböző rétegek között)
//!
//! Kapcsolódások:
//! - morphogenesis.rs → strukturális motívum detektálás
//! - temporal_archetype.rs → időbeli mintázatok
//! - thought_graph.rs → gondolatmenet minták
//! - hebbian.rs → ko-aktivációs minták
//! - archetype.rs → kikristályosodott minták
//! - heuristic_decision.rs → minta-alapú döntéshozatal

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
// ─── Alap Típusok ───────────────────────────────────────────────────────────

/// Minta típusok
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum PatternType {
    /// Ismétlődő szekvencia (gondolatmenet, műveleti sor)
    Sequence,
    /// Időbeli ritmus (napi, heti ciklus)
    Temporal,
    /// Strukturális motívum (algráf minta)
    Structural,
    /// Térbeli klaszter (memória csoportosulás)
    Cluster,
    /// Több domainen átívelő minta
    CrossDomain,
    /// Ismeretlen / egyedi
    Unknown,
}

/// Minta jelentőség
#[derive(Debug, Clone)]
pub struct PatternSignificance {
    /// 0.0 - 1.0: mennyire biztos a minta
    pub confidence: f64,
    /// Hányszor detektáltuk
    pub frequency: u64,
    /// Utolsó egyezés időbélyege (unix ms)
    pub last_matched_ms: u64,
    /// Átlagos hasonlóság az egyezéseknél
    pub avg_similarity: f64,
    /// Tanulási ráta (mennyire adaptálódik új adathoz)
    pub learning_rate: f64,
}

impl Default for PatternSignificance {
    fn default() -> Self {
        Self {
            confidence: 0.1,
            frequency: 0,
            last_matched_ms: 0,
            avg_similarity: 0.0,
            learning_rate: 0.3,
        }
    }
}

/// Felismert minta sablon
#[derive(Debug, Clone)]
pub struct RecognizedPattern {
    pub id: u64,
    pub name: String,
    pub pattern_type: PatternType,
    /// Sablon adat (típustól függő)
    pub template: PatternTemplate,
    pub significance: PatternSignificance,
    pub tags: Vec<String>,
    /// Domain ahonnan származik (pl. "thought", "architecture", "memory")
    pub domain: String,
    pub metadata: HashMap<String, String>,
}

/// Minta sablon adat
#[derive(Debug, Clone)]
pub enum PatternTemplate {
    /// Szekvencia: elemek sorozata (pl. block id-k)
    Sequence(Vec<String>),
    /// Temporális: {id, hour, day_of_week, frequency}
    Temporal(Vec<TemporalSlice>),
    /// Strukturális: adjacency list minta
    Structural(Vec<(String, String, f64)>),
    /// Cluster: középpont + sugár + elemek
    Cluster {
        center: (f64, f64, f64),
        radius: f64,
        elements: Vec<String>,
    },
    /// Minták kombinációja
    Composite(Vec<u64>),
}

/// Időbeli szelet
#[derive(Debug, Clone)]
pub struct TemporalSlice {
    pub hour: u8,
    pub day_of_week: u8,
    pub frequency: f64,
    pub avg_intensity: f64,
    pub activity_ids: Vec<String>,
}

/// Minta egyezés eredménye
#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub pattern_id: u64,
    pub target_description: String,
    pub similarity: f64,
    pub timestamp_ms: u64,
    pub matched_elements: Vec<String>,
    pub details: String,
}

/// Konfigurációs paraméterek
#[derive(Debug, Clone)]
pub struct RecognitionConfig {
    /// Minimális hasonlóság az egyezéshez (0.0 - 1.0)
    pub min_similarity: f64,
    /// Minimális confidence az új minta elfogadásához
    pub min_confidence: f64,
    /// Hány találat után tekintünk egy mintát "megerősítettnek"
    pub confirmation_threshold: u64,
    /// Maximális minta szám a könyvtárban
    pub max_patterns: usize,
    /// Cluster távolság küszöb
    pub cluster_radius: f64,
    /// Szekvencia ablak méret
    pub sequence_window: usize,
    /// Tanulási ráta (0.0 - 1.0)
    pub learning_rate: f64,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            min_similarity: 0.6,
            min_confidence: 0.3,
            confirmation_threshold: 3,
            max_patterns: 1000,
            cluster_radius: 0.15,
            sequence_window: 10,
            learning_rate: 0.3,
        }
    }
}
// ─── Pattern Recognizer ──────────────────────────────────────────────────────

/// A fő minta felismerő rendszer
pub struct PatternRecognizer {
    /// Felismert minták könyvtára
    patterns: Arc<RwLock<Vec<RecognizedPattern>>>,
    /// Egyezési előzmények
    match_history: Arc<RwLock<VecDeque<PatternMatch>>>,
    /// Konfiguráció
    config: Arc<RwLock<RecognitionConfig>>,
    /// Következő ID
    next_id: Arc<RwLock<u64>>,
    /// Domain index (pattern_id → domain)
    domain_index: Arc<RwLock<HashMap<String, Vec<u64>>>>,
}

impl Default for PatternRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternRecognizer {
    pub fn new() -> Self {
        Self {
            patterns: Arc::new(RwLock::new(Vec::new())),
            match_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            config: Arc::new(RwLock::new(RecognitionConfig::default())),
            next_id: Arc::new(RwLock::new(1)),
            domain_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_config(config: RecognitionConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            ..Self::new()
        }
    }

    fn next_id(&self) -> u64 {
        let mut id = self.next_id.write().unwrap();
        let n = *id;
        *id += 1;
        n
    }

    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    // ─── Pattern Register ─────────────────────────────────────────────────

    /// Új minta regisztrálása
    pub fn register_pattern(
        &self,
        name: &str,
        ptype: PatternType,
        template: PatternTemplate,
        domain: &str,
        tags: Vec<String>,
    ) -> u64 {
        let id = self.next_id();
        let pattern = RecognizedPattern {
            id,
            name: name.to_string(),
            pattern_type: ptype,
            template,
            significance: PatternSignificance::default(),
            tags,
            domain: domain.to_string(),
            metadata: HashMap::new(),
        };

        self.patterns.write().unwrap().push(pattern);
        self.domain_index
            .write()
            .unwrap()
            .entry(domain.to_string())
            .or_default()
            .push(id);

        id
    }

    /// Minta törlése
    pub fn forget_pattern(&self, id: u64) -> bool {
        let mut patterns = self.patterns.write().unwrap();
        if let Some(pos) = patterns.iter().position(|p| p.id == id) {
            let p = patterns.remove(pos);
            if let Some(ids) = self.domain_index.write().unwrap().get_mut(&p.domain) {
                ids.retain(|&i| i != id);
            }
            true
        } else {
            false
        }
    }

    /// Minták listázása típus szerint
    pub fn list_patterns(
        &self,
        ptype: Option<PatternType>,
        domain: Option<&str>,
    ) -> Vec<RecognizedPattern> {
        self.patterns
            .read()
            .unwrap()
            .iter()
            .filter(|p| {
                let type_match = ptype.as_ref().is_none_or(|t| p.pattern_type == *t);
                let domain_match = domain.is_none_or(|d| p.domain == d);
                type_match && domain_match
            })
            .cloned()
            .collect()
    }

    /// Minta keresés név alapján
    pub fn find_pattern(&self, name: &str) -> Option<RecognizedPattern> {
        self.patterns
            .read()
            .unwrap()
            .iter()
            .find(|p| p.name == name)
            .cloned()
    }

    /// Minta lekérése ID alapján
    pub fn get_pattern(&self, id: u64) -> Option<RecognizedPattern> {
        self.patterns
            .read()
            .unwrap()
            .iter()
            .find(|p| p.id == id)
            .cloned()
    }

    // ─── Szekvencia Detektálás ─────────────────────────────────────────────

    /// Szekvencia minta felismerés: gyakori részsorozatok keresése
    /// Bemenet: elemek sorozata (pl. block id-k, műveletek, események)
    /// Kimenet: ismétlődő minták confidenciával
    pub fn find_sequences(&self, elements: &[String], domain: &str) -> Vec<RecognizedPattern> {
        let config = self.config.read().unwrap().clone();
        let window = config.sequence_window;
        let min_sim = config.min_similarity;
        let mut patterns = Vec::new();

        if elements.len() < 2 {
            return patterns;
        }

        // Gyakori 2-3-4 hosszúságú részsorozatok
        let mut seq_counts: HashMap<Vec<String>, u64> = HashMap::new();

        for len in 2..=window.min(elements.len()).min(5) {
            for i in 0..=elements.len().saturating_sub(len) {
                let subseq: Vec<String> = elements[i..i + len].to_vec();
                *seq_counts.entry(subseq).or_insert(0) += 1;
            }
        }

        // Statisztikai szűrés
        let total = elements.len() as f64;
        for (seq, count) in &seq_counts {
            let freq = *count as f64 / total;
            if freq >= min_sim && seq.len() >= 2 {
                let confidence = (freq * (1.0 - 1.0 / (seq.len() as f64))).min(1.0);

                let pattern = RecognizedPattern {
                    id: self.next_id(),
                    name: format!("seq_{}_{}", domain, seq.first().unwrap_or(&"?".to_string())),
                    pattern_type: PatternType::Sequence,
                    template: PatternTemplate::Sequence(seq.clone()),
                    significance: PatternSignificance {
                        confidence,
                        frequency: *count,
                        last_matched_ms: self.now_ms(),
                        avg_similarity: freq,
                        learning_rate: config.learning_rate,
                    },
                    tags: vec!["auto_detected".to_string(), format!("len_{}", seq.len())],
                    domain: domain.to_string(),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("frequency".to_string(), format!("{:.2}", freq));
                        m.insert("sample_count".to_string(), count.to_string());
                        m
                    },
                };

                // Ellenőrizzük, hogy már létezik-e hasonló
                let exists = self.patterns.read().unwrap().iter().any(|p| {
                    if let PatternTemplate::Sequence(ref existing) = p.template {
                        existing == seq
                    } else {
                        false
                    }
                });

                if !exists {
                    let id = pattern.id;
                    self.patterns.write().unwrap().push(pattern.clone());
                    self.domain_index
                        .write()
                        .unwrap()
                        .entry(domain.to_string())
                        .or_default()
                        .push(id);
                }

                patterns.push(pattern);
            }
        }

        patterns.sort_by(|a, b| {
            b.significance
                .confidence
                .partial_cmp(&a.significance.confidence)
                .unwrap()
        });
        patterns
    }

    /// Adott szekvencia egyeztetése ismert mintákkal
    pub fn match_sequence(&self, sequence: &[String]) -> Vec<PatternMatch> {
        let config = self.config.read().unwrap().clone();
        let mut matches = Vec::new();
        let now = self.now_ms();

        for pattern in self.patterns.read().unwrap().iter() {
            if let PatternTemplate::Sequence(ref pattern_seq) = pattern.template {
                // Részsorozat keresés (pattern hosszabb lehet)
                if pattern_seq.len() > sequence.len() {
                    continue;
                }

                for i in 0..=sequence.len().saturating_sub(pattern_seq.len()) {
                    let window = &sequence[i..i + pattern_seq.len()];
                    let sim = self.sequence_similarity(window, pattern_seq);

                    if sim >= config.min_similarity {
                        matches.push(PatternMatch {
                            pattern_id: pattern.id,
                            target_description: format!(
                                "{:?}..{:?}",
                                window.first().unwrap_or(&"".to_string()),
                                window.last().unwrap_or(&"".to_string())
                            ),
                            similarity: sim,
                            timestamp_ms: now,
                            matched_elements: window.to_vec(),
                            details: format!(
                                "Matched sequence pattern '{}' ({})",
                                pattern.name, pattern.domain
                            ),
                        });

                        // Frissítés: frequency növelés
                        if let Some(p) = self
                            .patterns
                            .write()
                            .unwrap()
                            .iter_mut()
                            .find(|p| p.id == pattern.id)
                        {
                            p.significance.frequency += 1;
                            p.significance.last_matched_ms = now;
                            p.significance.avg_similarity =
                                p.significance.avg_similarity * 0.7 + sim * 0.3;
                            p.significance.confidence = (p.significance.confidence + 0.05).min(1.0);
                        }
                    }
                }
            }
        }

        self.match_history.write().unwrap().extend(matches.clone());
        matches
    }

    /// Két szekvencia hasonlósága (Levenshtein normalizált)
    fn sequence_similarity(&self, a: &[String], b: &[String]) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let max_len = a.len().max(b.len()) as f64;
        let mut matches = 0u64;

        for i in 0..a.len().min(b.len()) {
            if a[i] == b[i] {
                matches += 1;
            }
        }

        matches as f64 / max_len
    }

    // ─── Temporális Minta Detektálás ───────────────────────────────────────

    /// Temporális minták keresése időbélyeges eseményekből
    /// Bemenet: {timestamp_ms, id, intensity} triplek
    /// Kimenet: napi/heti ritmusok
    pub fn find_temporal_patterns(
        &self,
        events: &[(u64, String, f64)],
        domain: &str,
    ) -> Vec<RecognizedPattern> {
        if events.is_empty() {
            return Vec::new();
        }
        let config = self.config.read().unwrap().clone();
        let mut patterns = Vec::new();

        // Csoportosítás óra × nap szerint
        let mut hourly: HashMap<(u8, u8), Vec<f64>> = HashMap::new(); // (hour, day) → intensities

        for (ts, _id, intensity) in events {
            let secs = *ts / 1000;
            let hour = ((secs / 3600) % 24) as u8;
            let day = ((secs / 86400) % 7) as u8;
            hourly
                .entry((hour, day))
                .or_default()
                .push(*intensity);
        }

        // Mintázatok keresése
        // 1. Csúcsidők (magas aktivitású órák)
        let mut peak_hours: Vec<(u8, u8, f64)> = hourly
            .iter()
            .map(|((h, d), intensities)| {
                let avg: f64 = intensities.iter().sum::<f64>() / intensities.len() as f64;
                (*h, *d, avg)
            })
            .filter(|(_, _, avg)| *avg > 0.5)
            .collect();

        peak_hours.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        if !peak_hours.is_empty() {
            let slices: Vec<TemporalSlice> = peak_hours
                .iter()
                .map(|(h, d, avg)| TemporalSlice {
                    hour: *h,
                    day_of_week: *d,
                    frequency: 1.0,
                    avg_intensity: *avg,
                    activity_ids: Vec::new(),
                })
                .collect();

            let confidence = (peak_hours.len() as f64 / 24.0).min(1.0) * 0.5;

            let pattern = RecognizedPattern {
                id: self.next_id(),
                name: format!("temporal_peak_{}", domain),
                pattern_type: PatternType::Temporal,
                template: PatternTemplate::Temporal(slices),
                significance: PatternSignificance {
                    confidence,
                    frequency: events.len() as u64,
                    last_matched_ms: self.now_ms(),
                    avg_similarity: confidence,
                    learning_rate: config.learning_rate,
                },
                tags: vec!["auto_detected".to_string(), "peak_hours".to_string()],
                domain: domain.to_string(),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("peak_count".to_string(), peak_hours.len().to_string());
                    m
                },
            };

            let id = pattern.id;
            self.patterns.write().unwrap().push(pattern.clone());
            self.domain_index
                .write()
                .unwrap()
                .entry(domain.to_string())
                .or_default()
                .push(id);

            patterns.push(pattern);
        }

        // 2. Napi ritmus (reggel/délután/este aktivitás)
        let day_parts: [(&str, u8, u8); 4] = [
            ("reggel", 6, 12),
            ("delutan", 12, 18),
            ("este", 18, 23),
            ("hajnal", 0, 6),
        ];

        for (part_name, start_h, end_h) in &day_parts {
            let count: usize = hourly
                .iter()
                .filter(|((h, _), _)| *h >= *start_h && *h <= *end_h)
                .map(|(_, v)| v.len())
                .sum();

            if count > 0 {
                let total_events: usize = events.len();
                let ratio = count as f64 / total_events as f64;
                if ratio > 0.4 {
                    let slices: Vec<TemporalSlice> = hourly
                        .iter()
                        .filter(|((h, _), _)| *h >= *start_h && *h <= *end_h)
                        .map(|((h, d), intensities)| {
                            let avg = intensities.iter().sum::<f64>() / intensities.len() as f64;
                            TemporalSlice {
                                hour: *h,
                                day_of_week: *d,
                                frequency: intensities.len() as f64,
                                avg_intensity: avg,
                                activity_ids: Vec::new(),
                            }
                        })
                        .collect();

                    let pattern = RecognizedPattern {
                        id: self.next_id(),
                        name: format!("{}_{}", part_name, domain),
                        pattern_type: PatternType::Temporal,
                        template: PatternTemplate::Temporal(slices),
                        significance: PatternSignificance {
                            confidence: ratio * 0.6,
                            frequency: count as u64,
                            last_matched_ms: self.now_ms(),
                            avg_similarity: ratio,
                            learning_rate: config.learning_rate,
                        },
                        tags: vec!["auto_detected".to_string(), "daily_rhythm".to_string()],
                        domain: domain.to_string(),
                        metadata: HashMap::new(),
                    };

                    let id = pattern.id;
                    self.patterns.write().unwrap().push(pattern.clone());
                    self.domain_index
                        .write()
                        .unwrap()
                        .entry(domain.to_string())
                        .or_default()
                        .push(id);

                    patterns.push(pattern);
                }
            }
        }

        patterns
    }
    // ─── Strukturális Motívum Detektálás ──────────────────────────────────

    /// Strukturális motívumok keresése gráfban (pl. morphogenesis által növesztett architektúrákban)
    /// Bemenet: adjacencia lista (from, to, weight)
    /// Kimenet: gyakori algráf minták (motívumok)
    pub fn find_motifs(
        &self,
        edges: &[(String, String, f64)],
        domain: &str,
    ) -> Vec<RecognizedPattern> {
        let config = self.config.read().unwrap().clone();
        let mut patterns = Vec::new();

        if edges.is_empty() {
            return patterns;
        }

        // Gyakori 2-3-4 node-os algráfok (motívumok)
        // 1. Fan-out: egy node → több node
        let mut fan_out: HashMap<String, Vec<String>> = HashMap::new();
        for (from, to, _) in edges {
            fan_out
                .entry(from.clone())
                .or_default()
                .push(to.clone());
        }

        for (node, targets) in &fan_out {
            if targets.len() >= 3 {
                let mut motif_edges: Vec<(String, String, f64)> = Vec::new();
                for t in targets {
                    motif_edges.push((node.clone(), t.clone(), 1.0));
                }

                let confidence = (targets.len() as f64 / 10.0).min(1.0);
                let pattern = RecognizedPattern {
                    id: self.next_id(),
                    name: format!("fan_out_{}", node),
                    pattern_type: PatternType::Structural,
                    template: PatternTemplate::Structural(motif_edges),
                    significance: PatternSignificance {
                        confidence: confidence * 0.5,
                        frequency: targets.len() as u64,
                        last_matched_ms: self.now_ms(),
                        avg_similarity: confidence,
                        learning_rate: config.learning_rate,
                    },
                    tags: vec!["auto_detected".to_string(), "fan_out".to_string()],
                    domain: domain.to_string(),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("node_count".to_string(), (targets.len() + 1).to_string());
                        m
                    },
                };

                let id = pattern.id;
                self.patterns.write().unwrap().push(pattern.clone());
                self.domain_index
                    .write()
                    .unwrap()
                    .entry(domain.to_string())
                    .or_default()
                    .push(id);
                patterns.push(pattern);
            }
        }

        // 2. Fan-in: több node → egy node
        let mut fan_in: HashMap<String, Vec<String>> = HashMap::new();
        for (from, to, _) in edges {
            fan_in
                .entry(to.clone())
                .or_default()
                .push(from.clone());
        }

        for (node, sources) in &fan_in {
            if sources.len() >= 3 {
                let mut motif_edges: Vec<(String, String, f64)> = Vec::new();
                for s in sources {
                    motif_edges.push((s.clone(), node.clone(), 1.0));
                }

                let pattern = RecognizedPattern {
                    id: self.next_id(),
                    name: format!("fan_in_{}", node),
                    pattern_type: PatternType::Structural,
                    template: PatternTemplate::Structural(motif_edges),
                    significance: PatternSignificance {
                        confidence: (sources.len() as f64 / 10.0).min(1.0) * 0.5,
                        frequency: sources.len() as u64,
                        last_matched_ms: self.now_ms(),
                        avg_similarity: 0.0,
                        learning_rate: config.learning_rate,
                    },
                    tags: vec!["auto_detected".to_string(), "fan_in".to_string()],
                    domain: domain.to_string(),
                    metadata: HashMap::new(),
                };

                let id = pattern.id;
                self.patterns.write().unwrap().push(pattern.clone());
                self.domain_index
                    .write()
                    .unwrap()
                    .entry(domain.to_string())
                    .or_default()
                    .push(id);
                patterns.push(pattern);
            }
        }

        // 3. Hub: magas fokszámú node-ok
        let mut degrees: HashMap<String, usize> = HashMap::new();
        for (from, to, _) in edges {
            *degrees.entry(from.clone()).or_insert(0) += 1;
            *degrees.entry(to.clone()).or_insert(0) += 1;
        }

        let avg_degree = degrees.values().sum::<usize>() as f64 / degrees.len().max(1) as f64;
        for (node, degree) in &degrees {
            if *degree as f64 > avg_degree * 2.0 && *degree >= 4 {
                let pattern = RecognizedPattern {
                    id: self.next_id(),
                    name: format!("hub_{}", node),
                    pattern_type: PatternType::Structural,
                    template: PatternTemplate::Structural(vec![(
                        node.clone(),
                        "__hub__".to_string(),
                        *degree as f64,
                    )]),
                    significance: PatternSignificance {
                        confidence: (*degree as f64 / 20.0).min(1.0) * 0.4,
                        frequency: *degree as u64,
                        last_matched_ms: self.now_ms(),
                        avg_similarity: 0.0,
                        learning_rate: config.learning_rate,
                    },
                    tags: vec!["auto_detected".to_string(), "hub".to_string()],
                    domain: domain.to_string(),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("degree".to_string(), degree.to_string());
                        m.insert("avg_degree".to_string(), format!("{:.1}", avg_degree));
                        m
                    },
                };

                let id = pattern.id;
                self.patterns.write().unwrap().push(pattern.clone());
                self.domain_index
                    .write()
                    .unwrap()
                    .entry(domain.to_string())
                    .or_default()
                    .push(id);
                patterns.push(pattern);
            }
        }

        patterns
    }

    /// Strukturális motívum keresés morphogenesis organizmusokban
    pub fn find_motifs_in_organism(
        &self,
        _nodes: &[crate::morphogenesis::MorphNode],
        connections: &[crate::morphogenesis::MorphConnection],
    ) -> Vec<RecognizedPattern> {
        let edges: Vec<(String, String, f64)> = connections
            .iter()
            .filter(|c| c.is_active)
            .map(|c| (c.from_node.to_string(), c.to_node.to_string(), c.weight))
            .collect();

        self.find_motifs(&edges, "morphogenesis")
    }

    // ─── Cluster Detektálás ────────────────────────────────────────────────

    /// Térbeli klaszterek keresése pontfelhőben (DBSCAN-szerű)
    /// Bemenet: (x, y, z) koordináták + id
    /// Kimenet: csoportosulások a memória térben
    pub fn find_clusters(
        &self,
        points: &[(f64, f64, f64, String)],
        domain: &str,
    ) -> Vec<RecognizedPattern> {
        let config = self.config.read().unwrap().clone();
        let mut patterns = Vec::new();

        if points.is_empty() {
            return patterns;
        }

        let eps = config.cluster_radius;
        let min_pts = 3;
        let mut visited: HashSet<usize> = HashSet::new();
        let mut clusters: Vec<Vec<usize>> = Vec::new();

        for i in 0..points.len() {
            if visited.contains(&i) {
                continue;
            }

            let mut neighbors: Vec<usize> = points
                .iter()
                .enumerate()
                .filter(|(j, _)| {
                    let dist = ((points[i].0 - points[*j].0).powi(2)
                        + (points[i].1 - points[*j].1).powi(2)
                        + (points[i].2 - points[*j].2).powi(2))
                    .sqrt();
                    dist <= eps
                })
                .map(|(j, _)| j)
                .collect();

            if neighbors.len() < min_pts {
                visited.insert(i);
                continue; // noise
            }

            // Cluster kiterjesztése
            let mut cluster: Vec<usize> = Vec::new();
            while let Some(seed) = neighbors.pop() {
                if visited.contains(&seed) {
                    continue;
                }
                visited.insert(seed);
                cluster.push(seed);

                let seed_neighbors: Vec<usize> = points
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| !visited.contains(j))
                    .filter(|(j, _)| {
                        let dist = ((points[seed].0 - points[*j].0).powi(2)
                            + (points[seed].1 - points[*j].1).powi(2)
                            + (points[seed].2 - points[*j].2).powi(2))
                        .sqrt();
                        dist <= eps
                    })
                    .map(|(j, _)| j)
                    .collect();

                neighbors.extend(seed_neighbors);
            }

            if cluster.len() >= min_pts {
                clusters.push(cluster);
            }
        }

        // Cluster minták létrehozása
        for (ci, cluster) in clusters.iter().enumerate() {
            let cluster_points: Vec<&(f64, f64, f64, String)> =
                cluster.iter().map(|&i| &points[i]).collect();

            // Középpont
            let cx = cluster_points.iter().map(|p| p.0).sum::<f64>() / cluster_points.len() as f64;
            let cy = cluster_points.iter().map(|p| p.1).sum::<f64>() / cluster_points.len() as f64;
            let cz = cluster_points.iter().map(|p| p.2).sum::<f64>() / cluster_points.len() as f64;

            // Átlagos távolság a középponttól
            let avg_dist: f64 = cluster_points
                .iter()
                .map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2) + (p.2 - cz).powi(2)).sqrt())
                .sum::<f64>()
                / cluster_points.len() as f64;

            let element_ids: Vec<String> = cluster_points.iter().map(|p| p.3.clone()).collect();
            let density =
                cluster.len() as f64 / (avg_dist * avg_dist * 4.0 * std::f64::consts::PI).max(0.01);

            let pattern = RecognizedPattern {
                id: self.next_id(),
                name: format!("cluster_{}_{}", domain, ci),
                pattern_type: PatternType::Cluster,
                template: PatternTemplate::Cluster {
                    center: (cx, cy, cz),
                    radius: avg_dist,
                    elements: element_ids.clone(),
                },
                significance: PatternSignificance {
                    confidence: (density / 100.0).min(1.0),
                    frequency: element_ids.len() as u64,
                    last_matched_ms: self.now_ms(),
                    avg_similarity: 1.0 - (avg_dist / 1.0).min(1.0),
                    learning_rate: config.learning_rate,
                },
                tags: vec!["auto_detected".to_string(), "cluster".to_string()],
                domain: domain.to_string(),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("size".to_string(), cluster.len().to_string());
                    m.insert("density".to_string(), format!("{:.3}", density));
                    m
                },
            };

            let id = pattern.id;
            self.patterns.write().unwrap().push(pattern.clone());
            self.domain_index
                .write()
                .unwrap()
                .entry(domain.to_string())
                .or_default()
                .push(id);
            patterns.push(pattern);
        }

        patterns
    }
    // ─── Cross-domain Korreláció ──────────────────────────────────────────

    /// Különböző domain-ekből származó minták korrelációja
    /// Keres olyan mintákat, amelyek több domain-ben is előfordulnak
    /// (pl. ugyanaz a szekvencia megjelenik thought és recall domain-ben is)
    pub fn cross_correlate(&self) -> Vec<RecognizedPattern> {
        let config = self.config.read().unwrap().clone();
        let mut composite_patterns = Vec::new();

        let patterns = self.patterns.read().unwrap();
        let domains: Vec<String> = self.domain_index.read().unwrap().keys().cloned().collect();

        if domains.len() < 2 {
            return Vec::new();
        }

        // Domain párok keresése, ahol hasonló minták vannak
        for i in 0..domains.len() {
            for j in (i + 1)..domains.len() {
                let d1 = &domains[i];
                let d2 = &domains[j];

                let p1: Vec<&RecognizedPattern> =
                    patterns.iter().filter(|p| p.domain == *d1).collect();
                let p2: Vec<&RecognizedPattern> =
                    patterns.iter().filter(|p| p.domain == *d2).collect();

                for pat1 in &p1 {
                    for pat2 in &p2 {
                        // Csak azonos típusú mintákat korrelálunk
                        if pat1.pattern_type != pat2.pattern_type {
                            continue;
                        }

                        let sim = match (&pat1.template, &pat2.template) {
                            (PatternTemplate::Sequence(s1), PatternTemplate::Sequence(s2)) => {
                                self.sequence_similarity(s1, s2)
                            }
                            (PatternTemplate::Structural(e1), PatternTemplate::Structural(e2)) => {
                                let common = e1.iter().filter(|e| e2.contains(e)).count();
                                let max = e1.len().max(e2.len()).max(1);
                                common as f64 / max as f64
                            }
                            _ => 0.0,
                        };

                        if sim > config.min_similarity * 1.2 {
                            // szigorúbb küszöb
                            let composite_id = self.next_id();
                            let composite = RecognizedPattern {
                                id: composite_id,
                                name: format!("cross_{}_{}", pat1.name, pat2.name),
                                pattern_type: PatternType::CrossDomain,
                                template: PatternTemplate::Composite(vec![pat1.id, pat2.id]),
                                significance: PatternSignificance {
                                    confidence: sim * 0.8,
                                    frequency: pat1.significance.frequency
                                        + pat2.significance.frequency,
                                    last_matched_ms: self.now_ms(),
                                    avg_similarity: sim,
                                    learning_rate: config.learning_rate,
                                },
                                tags: vec!["auto_detected".to_string(), "cross_domain".to_string()],
                                domain: format!("cross:{}:{}", d1, d2),
                                metadata: {
                                    let mut m = HashMap::new();
                                    m.insert("domain_a".to_string(), d1.clone());
                                    m.insert("domain_b".to_string(), d2.clone());
                                    m
                                },
                            };

                            composite_patterns.push(composite);
                        }
                    }
                }
            }
        }

        // Cross-domain minták regisztrálása
        for cp in &composite_patterns {
            let id = cp.id;
            self.patterns.write().unwrap().push(cp.clone());
            self.domain_index
                .write()
                .unwrap()
                .entry(cp.domain.clone())
                .or_default()
                .push(id);
        }

        composite_patterns
    }

    // ─── Konszolidáció ─────────────────────────────────────────────────────

    /// Hasonló minták összevonása, gyenge minták elfelejtése
    pub fn consolidate(&self) -> usize {
        let config = self.config.read().unwrap().clone();
        let mut patterns = self.patterns.write().unwrap();
        let before = patterns.len();

        // 1. Alacsony confidence-ű minták eltávolítása
        patterns.retain(|p| {
            p.significance.confidence >= config.min_confidence
                || p.significance.frequency >= config.confirmation_threshold
        });

        // 2. Hasonló minták összevonása (azonos típus + magas hasonlóság)
        let mut i = 0;
        while i < patterns.len() {
            let mut j = i + 1;
            while j < patterns.len() {
                let should_merge = patterns[i].pattern_type == patterns[j].pattern_type
                    && patterns[i].domain == patterns[j].domain
                    && match (&patterns[i].template, &patterns[j].template) {
                        (PatternTemplate::Sequence(s1), PatternTemplate::Sequence(s2)) => {
                            self.sequence_similarity(s1, s2) > 0.8
                        }
                        _ => false,
                    };

                if should_merge {
                    patterns[i].significance.frequency += patterns[j].significance.frequency;
                    patterns[i].significance.confidence = patterns[i]
                        .significance
                        .confidence
                        .max(patterns[j].significance.confidence);
                    patterns.remove(j);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }

        // 3. Max patterns limit
        while patterns.len() > config.max_patterns {
            // Leggyengébb eltávolítása
            let min_pos = (0..patterns.len())
                .min_by(|&a, &b| {
                    patterns[a]
                        .significance
                        .confidence
                        .partial_cmp(&patterns[b].significance.confidence)
                        .unwrap()
                })
                .unwrap_or(0);
            patterns.remove(min_pos);
        }

        before - patterns.len()
    }

    // ─── Statisztikák ──────────────────────────────────────────────────────

    /// Rendszer statisztikák
    pub fn stats(&self) -> (usize, usize, usize, f64) {
        let patterns = self.patterns.read().unwrap();

        let total = patterns.len();
        let seq_count = patterns
            .iter()
            .filter(|p| p.pattern_type == PatternType::Sequence)
            .count();
        let struct_count = patterns
            .iter()
            .filter(|p| p.pattern_type == PatternType::Structural)
            .count();
        let avg_confidence = if total > 0 {
            patterns
                .iter()
                .map(|p| p.significance.confidence)
                .sum::<f64>()
                / total as f64
        } else {
            0.0
        };

        (total, seq_count, struct_count, avg_confidence)
    }

    /// Friss egyezések lekérése
    pub fn recent_matches(&self, k: usize) -> Vec<PatternMatch> {
        self.match_history
            .read()
            .unwrap()
            .iter()
            .rev()
            .take(k)
            .cloned()
            .collect()
    }

    /// Konfiguráció frissítése
    pub fn set_config(&self, config: RecognitionConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Konfiguráció olvasása
    pub fn get_config(&self) -> RecognitionConfig {
        self.config.read().unwrap().clone()
    }

    /// Domain-ek listázása
    pub fn list_domains(&self) -> Vec<String> {
        self.domain_index.read().unwrap().keys().cloned().collect()
    }

    /// Domain szerinti minta szám
    pub fn domain_pattern_count(&self, domain: &str) -> usize {
        self.domain_index
            .read()
            .unwrap()
            .get(domain)
            .map(|ids| ids.len())
            .unwrap_or(0)
    }
}

// ─── Segédfüggvények ────────────────────────────────────────────────────────

/// PatternType Display
impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternType::Sequence => write!(f, "Sequence"),
            PatternType::Temporal => write!(f, "Temporal"),
            PatternType::Structural => write!(f, "Structural"),
            PatternType::Cluster => write!(f, "Cluster"),
            PatternType::CrossDomain => write!(f, "CrossDomain"),
            PatternType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for RecognizedPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pattern #{} [{}] '{}' — {} ({:.1}% confidence, {} freq)",
            self.id,
            self.pattern_type,
            self.name,
            self.domain,
            self.significance.confidence * 100.0,
            self.significance.frequency
        )
    }
}
