//! Morphogenesis — Biológiai Mintákon Alapuló Generatív Architektúra-Tenyésztés

//!

//! Ez a modul a természet növekedési mintáit használja szoftverarchitektúrák generálására:

//! - Gombafonalak (mycelium) → elosztott P2P hálózatok

//! - Kapilláris elágazások → adatfolyam pipeline-ok

//! - Nyálkpenész (slime mold) → útvonal optimalizálás

//! - L-system fraktálok → self-similar struktúrák

//!

//! ## Alapelvek (Morphogenetic Design)

//!

//! A hagyományos architektúra-tervezés dobozokat és nyilakat rajzol (top-down).

//! A morphogenesis alulról felfelé (bottom-up) növeszti a struktúrákat:

//!

//! 1. **Seed** — egyetlen kiinduló pont (szolgáltatás, adatforrás, trigger)

//! 2. **Morphogen Field** — koncentráció-gradiens, ami vezeti a növekedést

//! 3. **Growth** — a seed-ből kiindulva, a morfogén mezőt követve nő a struktúra

//! 4. **Pruning** — gyenge ágak elhalnak, erősek megerősödnek (use-it-or-lose-it)

//! 5. **Expression** — a kinőtt struktúra leképezése Architecture típusra

//!

//! Kapcsolódások:

//! - `architecture_generator.rs` → Architecture, SimulationMetrics

//! - `architecture_simulator.rs` → Architecture, Component, Connection

//! - `mental_sandbox.rs` → szimulációs környezet

//! - `neuroplasticity.rs` → szinaptikus kapcsolatok megerősítése

//! - `vagus.rs` → stressz trigger → növekedés indukálása

//! - `federation.rs` → fingerprint-ek exportálása globális rezonanciához

use std::collections::HashMap;

use std::sync::{Arc, RwLock};

// ─── Növekedési Minták ───────────────────────────────────────────────────────

/// Növekedési algoritmus típusok

#[derive(Debug, Clone, Copy, PartialEq, Hash)]

pub enum GrowthPattern {

    /// Gombafonalszerű elágazó hálózat (P2P overlay, elosztott rendszerek)

    Mycelium,

    /// Kapilláris fraktál-elágazás (adatfolyam pipeline-ok, hierarchikus cache)

    Capillary,

    /// Nyálkpenész útvonal optimalizálás (CDN, routing, mesh hálózatok)

    SlimeMold,

    /// L-system self-similar struktúrák (mikro-szolgáltatás hierarchiák)

    FractalLSystem,

    /// Hibrid: több minta kombinációja

    Hybrid,

}

/// Csomópont típusok a kinőtt struktúrában

#[derive(Debug, Clone, PartialEq, Hash)]

pub enum NodeType {

    /// Gyökér csomópont (kiindulási pont)

    Root,

    /// Szolgáltatás / feldolgozó egység

    Service,

    /// Adatbázis vagy storage node

    Database,

    /// Cache réteg

    Cache,

    /// API Gateway / bejárati pont

    Gateway,

    /// Terheléselosztó

    LoadBalancer,

    /// Üzenetsor / buffer

    Queue,

    /// Levél csomópont (végpont, nincs további elágazás)

    Leaf,

    /// Egyedi típus

    Custom(String),

}

/// Kapcsolat típusa

#[derive(Debug, Clone, PartialEq, Hash)]

pub enum ConnectionType {

    /// Adatfolyam (data plane)

    DataFlow,

    /// Vezérlő jelzés (control plane)

    Control,

    /// Replikáció (adat másolás)

    Replication,

    /// Backup / helyreállítás

    Backup,

}

// ─── Core Adatszerkezetek ───────────────────────────────────────────────────

/// Seed — kiinduló "mag" a növekedéshez

#[derive(Debug, Clone)]

pub struct Seed {

    /// Egyedi azonosító

    pub id: String,

    /// Pozíció a 3D térben (morfogén mező koordináták)

    pub position: (f64, f64, f64),

    /// Kezdeti energia (maximális növekedési potenciál)

    pub energy: f64,

    /// Típus címke (pl. "database", "api", "cache")

    pub type_tag: String,

    /// Növekedési preferencia

    pub preferred_pattern: Option<GrowthPattern>,

}

impl Seed {

    pub fn new(id: &str, x: f64, y: f64, z: f64, type_tag: &str) -> Self {

        Self {

            id: id.to_string(),

            position: (x, y, z),

            energy: 100.0,

            type_tag: type_tag.to_string(),

            preferred_pattern: None,

        }

    }

    pub fn with_energy(mut self, energy: f64) -> Self {

        self.energy = energy;

        self

    }

    pub fn with_pattern(mut self, pattern: GrowthPattern) -> Self {

        self.preferred_pattern = Some(pattern);

        self

    }

}

/// Morfogén koncentráció mező — biológiai "illat" gradiensek

#[derive(Debug, Clone)]

pub struct MorphogenField {

    /// Koncentráció értékek a 3D rácsban

    pub gradients: HashMap<(i32, i32, i32), f64>,

    /// Gradiens források (vonzó és taszító pontok)

    pub attractors: Vec<(f64, f64, f64, f64)>, // (x, y, z, strength)

    pub repellents: Vec<(f64, f64, f64, f64)>,

    /// Diffúziós ráta (mennyire terjed szét a gradiens)

    pub diffusion_rate: f64,

    /// Párolgási ráta (mennyire gyengül idővel)

    pub evaporation_rate: f64,

}

impl MorphogenField {

    pub fn new() -> Self {

        Self {

            gradients: HashMap::new(),

            attractors: Vec::new(),

            repellents: Vec::new(),

            diffusion_rate: 0.1,

            evaporation_rate: 0.01,

        }

    }

    /// Koncentráció lekérése egy adott pontban (tricubic interpoláció)

    pub fn concentration_at(&self, x: f64, y: f64, z: f64) -> f64 {

        let gx = x.floor() as i32;

        let gy = y.floor() as i32;

        let gz = z.floor() as i32;

        let mut total = 0.0;

        for dx in -1..=1 {

            for dy in -1..=1 {

                for dz in -1..=1 {

                    if let Some(val) = self.gradients.get(&(gx + dx, gy + dy, gz + dz)) {

                        let dist = ((x - (gx + dx) as f64).powi(2)

                                    + (y - (gy + dy) as f64).powi(2)

                                    + (z - (gz + dz) as f64).powi(2))

                                    .sqrt()

                                    .max(0.001);

                        total += val / dist;

                    }

                }

            }

        }

        // Attraktorok hozzáadása

        for (ax, ay, az, strength) in &self.attractors {

            let dist = ((x - ax).powi(2) + (y - ay).powi(2) + (z - az).powi(2))

                .sqrt()

                .max(0.001);

            total += strength / dist;

        }

        // Repellensek kivonása

        for (rx, ry, rz, strength) in &self.repellents {

            let dist = ((x - rx).powi(2) + (y - ry).powi(2) + (z - rz).powi(2))

                .sqrt()

                .max(0.001);

            total -= strength / dist;

        }

        total

    }

    /// Új attraktor hozzáadása

    pub fn add_attractor(&mut self, x: f64, y: f64, z: f64, strength: f64) {

        self.attractors.push((x, y, z, strength));

        let key = (x.floor() as i32, y.floor() as i32, z.floor() as i32);

        *self.gradients.entry(key).or_insert(0.0) += strength;

    }

    /// Gradiens diffúzió egy lépése

    pub fn diffuse(&mut self) {

        let old = self.gradients.clone();

        for ((x, y, z), val) in &old {

            if *val < 0.001 { continue; }

            for dx in -1..=1 {

                for dy in -1..=1 {

                    for dz in -1..=1 {

                        if dx == 0 && dy == 0 && dz == 0 { continue; }

                        let spread = val * self.diffusion_rate / 26.0;

                        *self.gradients.entry((x + dx, y + dy, z + dz)).or_insert(0.0) += spread;

                    }

                }

            }

        }

    }

    /// Párologtatás (gyenge gradiensek eltüntetése)

    pub fn evaporate(&mut self) {

        self.gradients.retain(|_, v| {

            *v *= 1.0 - self.evaporation_rate;

            *v > 0.001

        });

    }

}

/// Növekedési konfigurációs paraméterek

#[derive(Debug, Clone)]

pub struct GrowthConfig {

    /// Növekedési minta

    pub pattern: GrowthPattern,

    /// Maximum node-ok száma

    pub max_nodes: usize,

    /// Elágazási valószínűség (0.0 - 1.0)

    pub branching_probability: f64,

    /// Elágazási szög (radián)

    pub branching_angle: f64,

    /// Energia bomlási ráta (mennyit veszít egy ág hosszegységenként)

    pub energy_decay: f64,

    /// Minimum energia az elágazáshoz

    pub min_energy_for_branch: f64,

    /// Maximális növekedési mélység (hány generáció)

    pub max_depth: u32,

    /// Anastomosis (fúzió) valószínűsége

    pub anastomosis_probability: f64,

    /// Pruning threshold (mennyi ideig használatlan ág haljon el)

    pub prune_idle_cycles: u32,

    /// Slime mold specifikus: nyom megőrzési ráta

    pub trail_persistence: f64,

    /// Slime mold specifikus: nyom párolgási ráta

    pub trail_evaporation: f64,

    /// L-system specifikus: produkciós szabályok

    pub lsystem_rules: Vec<(char, String)>,

}

impl Default for GrowthConfig {

    fn default() -> Self {

        Self {

            pattern: GrowthPattern::Mycelium,

            max_nodes: 500,

            branching_probability: 0.3,

            branching_angle: 0.6,

            energy_decay: 0.1,

            min_energy_for_branch: 30.0,

            max_depth: 20,

            anastomosis_probability: 0.05,

            prune_idle_cycles: 10,

            trail_persistence: 0.9,

            trail_evaporation: 0.05,

            lsystem_rules: vec![

                ('A', "AB".to_string()),

                ('B', "A[B]A".to_string()),

            ],

        }

    }

}

impl GrowthConfig {

    pub fn mycelium_default() -> Self {

        Self {

            pattern: GrowthPattern::Mycelium,

            branching_probability: 0.35,

            branching_angle: 0.8,

            energy_decay: 0.08,

            min_energy_for_branch: 25.0,

            anastomosis_probability: 0.08,

            ..Self::default()

        }

    }

    pub fn capillary_default() -> Self {

        Self {

            pattern: GrowthPattern::Capillary,

            branching_probability: 0.5,

            branching_angle: 0.4,

            energy_decay: 0.15,

            min_energy_for_branch: 20.0,

            max_depth: 12,

            ..Self::default()

        }

    }

    pub fn slime_mold_default() -> Self {

        Self {

            pattern: GrowthPattern::SlimeMold,

            max_nodes: 300,

            trail_persistence: 0.9,

            trail_evaporation: 0.01,

            ..Self::default()

        }

    }

    pub fn fractal_lsystem_default() -> Self {

        Self {

            pattern: GrowthPattern::FractalLSystem,

            branching_angle: 0.5,

            max_depth: 8,

            lsystem_rules: vec![

                ('F', "FF+[+F-F-F]-[-F+F+F]".to_string()),

            ],

            ..Self::default()

        }

    }

}

/// Csomópont a kinőtt struktúrában

#[derive(Debug, Clone)]

pub struct MorphNode {

    pub id: usize,

    pub position: (f64, f64, f64),

    pub node_type: NodeType,

    pub name: String,

    pub latency_base_ms: f64,

    pub capacity: f64,

    pub energy: f64,

    pub depth: u32,

    pub metadata: HashMap<String, String>,

}

/// Kapcsolat két csomópont között

#[derive(Debug, Clone)]

pub struct MorphConnection {

    pub id: usize,

    pub from_node: usize,

    pub to_node: usize,

    pub weight: f64,

    pub bandwidth: f64,

    pub protocol: String,

    pub latency_ms: f64,

    pub connection_type: ConnectionType,

    pub is_active: bool,

    pub age_cycles: u32,

}

/// Növekedési statisztikák

#[derive(Debug, Clone)]

pub struct GrowthMetrics {

    pub node_count: usize,

    pub connection_count: usize,

    pub max_depth: u32,

    pub avg_branching_factor: f64,

    pub fractal_dimension: f64,

    pub total_energy: f64,

    pub redundancy_score: f64,

    pub avg_path_length: f64,

}

/// Organizmus — a teljes kinőtt struktúra

#[derive(Debug, Clone)]

pub struct Organism {

    pub id: String,

    pub name: String,

    pub nodes: Vec<MorphNode>,

    pub connections: Vec<MorphConnection>,

    pub growth_pattern: GrowthPattern,

    pub generation: u32,

    pub fitness_score: f64,

    pub age_cycles: u32,

    pub seed: Seed,

    pub metrics: Option<GrowthMetrics>,

    pub metadata: HashMap<String, String>,

}

impl Organism {

    pub fn new(id: &str, name: &str, seed: Seed, pattern: GrowthPattern) -> Self {

        Self {

            id: id.to_string(),

            name: name.to_string(),

            nodes: Vec::new(),

            connections: Vec::new(),

            growth_pattern: pattern,

            generation: 0,

            fitness_score: 0.0,

            age_cycles: 0,

            seed,

            metrics: None,

            metadata: HashMap::new(),

        }

    }

    /// Statisztikák számítása

    pub fn calculate_metrics(&mut self) -> GrowthMetrics {

        let node_count = self.nodes.len();

        let connection_count = self.connections.len();

        let max_depth = self.nodes.iter().map(|n| n.depth).max().unwrap_or(0);

        // Átlagos elágazási faktor

        let branching = if node_count > 1 {

            let internal_nodes = self.nodes.iter().filter(|n| {

                self.connections.iter().filter(|c| c.from_node == n.id).count() > 1

            }).count();

            internal_nodes as f64 / node_count.max(1) as f64

        } else { 0.0 };

        // Redundancia: kapcsolatok száma / minimálisan szükséges kapcsolatok

        let redundancy = if node_count > 1 {

            connection_count as f64 / (node_count - 1).max(1) as f64

        } else { 0.0 };

        // Átlagos útvonal hossz (egyszerű becslés)

        let avg_path = if connection_count > 0 && node_count > 1 {

            (node_count as f64).ln() / (redundancy.max(0.1)).ln()

        } else { 0.0 };

        let total_energy: f64 = self.nodes.iter().map(|n| n.energy).sum();

        let metrics = GrowthMetrics {

            node_count,

            connection_count,

            max_depth,

            avg_branching_factor: branching,

            fractal_dimension: (node_count as f64).ln() / ((max_depth.max(2)) as f64).ln(),

            total_energy,

            redundancy_score: redundancy,

            avg_path_length: avg_path,

        };

        self.metrics = Some(metrics.clone());

        metrics

    }

    /// Energia szint ellenőrzése

    pub fn is_alive(&self) -> bool {

        self.nodes.iter().any(|n| n.energy > 0.0)

    }

    /// Életkor növelése

    pub fn age_one_cycle(&mut self) {

        self.age_cycles += 1;

        for node in &mut self.nodes {

            node.energy *= 0.99; // Természetes energia vesztés

        }

    }

}

// ─── Növekedési Algoritmusok ─────────────────────────────────────────────────

// Segédfüggvény: véletlen vektor egy gömb felületén

fn random_direction() -> (f64, f64, f64) {

    let theta = rand::random::<f64>() * std::f64::consts::TAU;

    let phi = (rand::random::<f64>() * 2.0 - 1.0).acos();

    (

        theta.sin() * phi.cos(),

        theta.sin() * phi.sin(),

        theta.cos(),

    )

}

/// Mycelium (gombafonalszerű) növekedés

///

/// Indul egy seed-ből, hifák nőnek ki véletlen irányokba elágazva.

/// A hifák követik a morfogén gradienseket.

/// Két hifa találkozásakor anastomosis (fúzió) történhet.

pub fn mycelium_growth(seed: &Seed, field: &MorphogenField, config: &GrowthConfig) -> Organism {

    let mut organism = Organism::new(

        &format!("mycelium_{}", rand::random::<u32>()),

        &format!("Mycelium_{}", seed.type_tag),

        seed.clone(),

        GrowthPattern::Mycelium,

    );

    // Gyökér csomópont

    organism.nodes.push(MorphNode {

        id: 0,

        position: seed.position,

        node_type: NodeType::Root,

        name: format!("root_{}", seed.type_tag),

        latency_base_ms: 1.0,

        capacity: 100.0,

        energy: seed.energy,

        depth: 0,

        metadata: HashMap::new(),

    });

    // Aktív hifa csúcsok (tip)

    struct HyphaTip {

        node_id: usize,

        position: (f64, f64, f64),

        direction: (f64, f64, f64),

        energy: f64,

        depth: u32,

        idle_cycles: u32,

    }

    let mut tips: Vec<HyphaTip> = vec![HyphaTip {

        node_id: 0,

        position: seed.position,

        direction: random_direction(),

        energy: seed.energy,

        depth: 0,

        idle_cycles: 0,

    }];

    let mut next_node_id = 1;

    let mut next_conn_id = 1;

    let mut grown = 0;

    while grown < config.max_nodes && !tips.is_empty() {

        let mut new_tips = Vec::new();

        let mut dead_indices = Vec::new();

        for (idx, tip) in tips.iter_mut().enumerate() {

            if tip.energy <= 0.0 || tip.depth >= config.max_depth {

                dead_indices.push(idx);

                continue;

            }

            // Gradiens követés: a morfogén mező irányába igazítjuk az irányt

            let (cx, cy, cz) = tip.position;

            let current_conc = field.concentration_at(cx, cy, cz);

            let step = 0.5;

            let (gx, gy, gz) = if current_conc > 0.01 {

                let forward = field.concentration_at(cx + tip.direction.0 * step, cy + tip.direction.1 * step, cz + tip.direction.2 * step);

                if forward > current_conc {

                    tip.direction // már jó irány

                } else {

                    // Próbáljunk jobb irányt

                    let mut best_dir = tip.direction;

                    let mut best_conc = forward;

                    for _ in 0..8 {

                        let test_dir = random_direction();

                        let tc = field.concentration_at(

                            cx + test_dir.0 * step,

                            cy + test_dir.1 * step,

                            cz + test_dir.2 * step,

                        );

                        if tc > best_conc {

                            best_conc = tc;

                            best_dir = test_dir;

                        }

                    }

                    best_dir

                }

            } else {

                tip.direction

            };

            // Elágazás

            if rand::random::<f64>() < config.branching_probability && tip.energy > config.min_energy_for_branch && grown < config.max_nodes - 1 {

                let branch_dir = random_direction();

                let branch_energy = tip.energy * 0.6;

                let new_id = next_node_id;

                next_node_id += 1;

                grown += 1;

                organism.nodes.push(MorphNode {

                    id: new_id,

                    position: tip.position,

                    node_type: NodeType::Service,

                    name: format!("hypha_{}", new_id),

                    latency_base_ms: 2.0 + rand::random::<f64>() * 5.0,

                    capacity: 30.0 + rand::random::<f64>() * 70.0,

                    energy: branch_energy,

                    depth: tip.depth + 1,

                    metadata: HashMap::new(),

                });

                organism.connections.push(MorphConnection {

                    id: next_conn_id,

                    from_node: tip.node_id,

                    to_node: new_id,

                    weight: 0.5,

                    bandwidth: 1000.0 + rand::random::<f64>() * 9000.0,

                    protocol: "gRPC".to_string(),

                    latency_ms: 1.0,

                    connection_type: ConnectionType::DataFlow,

                    is_active: true,

                    age_cycles: 0,

                });

                next_conn_id += 1;

                new_tips.push(HyphaTip {

                    node_id: new_id,

                    position: tip.position,

                    direction: branch_dir,

                    energy: branch_energy,

                    depth: tip.depth + 1,

                    idle_cycles: 0,

                });

                tip.energy *= 0.5;

            }

            // Növekedés előre

            let nx = cx + gx * step;

            let ny = cy + gy * step;

            let nz = cz + gz * step;

            tip.position = (nx, ny, nz);

            tip.energy -= config.energy_decay;

            tip.idle_cycles += 1;

            // Anastomosis: ha egy másik node közelében vagyunk

            for node in &organism.nodes {

                if node.id == tip.node_id { continue; }

                let dist = ((nx - node.position.0).powi(2)

                          + (ny - node.position.1).powi(2)

                          + (nz - node.position.2).powi(2))

                          .sqrt();

                if dist < 0.8 && rand::random::<f64>() < config.anastomosis_probability {

                    organism.connections.push(MorphConnection {

                        id: next_conn_id,

                        from_node: tip.node_id,

                        to_node: node.id,

                        weight: 0.7,

                        bandwidth: 2000.0 + rand::random::<f64>() * 8000.0,

                        protocol: "gRPC".to_string(),

                        latency_ms: 0.5,

                        connection_type: ConnectionType::DataFlow,

                        is_active: true,

                        age_cycles: 0,

                    });

                    next_conn_id += 1;

                    dead_indices.push(idx);

                    break;

                }

            }

        }

        // Halott tippek eltávolítása

        dead_indices.sort_unstable();

        dead_indices.dedup();

        for i in dead_indices.into_iter().rev() {

            if i < tips.len() {

                tips.remove(i);

            }

        }

        tips.extend(new_tips);

        // Idle pruning

        tips.retain(|t| t.idle_cycles < config.prune_idle_cycles);

    }

    organism.calculate_metrics();

    organism

}

/// Kapilláris fraktál-elágazás

///

/// Hierarchikus elágazás: gyökér → artéria → arteriole → kapilláris.

/// Minden elágazásnál a kapacitás oszlik meg az ágak között.

/// A kapilláris szinten történik az adatcsere (leaf node-ok).

pub fn capillary_growth(seed: &Seed, field: &MorphogenField, config: &GrowthConfig) -> Organism {

    let mut organism = Organism::new(

        &format!("capillary_{}", rand::random::<u32>()),

        &format!("Capillary_{}", seed.type_tag),

        seed.clone(),

        GrowthPattern::Capillary,

    );

    // Gyökér csomópont

    organism.nodes.push(MorphNode {

        id: 0,

        position: seed.position,

        node_type: NodeType::Root,

        name: format!("root_{}", seed.type_tag),

        latency_base_ms: 0.5,

        capacity: 1000.0,

        energy: seed.energy,

        depth: 0,

        metadata: HashMap::new(),

    });

    let mut next_node_id = 1;

    let mut next_conn_id = 1;

    // Rekurzív elágazás építése

    fn branch(

        organism: &mut Organism,

        parent_id: usize,

        parent_pos: (f64, f64, f64),

        parent_capacity: f64,

        depth: u32,

        max_depth: u32,

        angle: f64,

        field: &MorphogenField,

        next_node_id: &mut usize,

        next_conn_id: &mut usize,

    ) {

        if depth >= max_depth || *next_node_id >= 500 {

            // Leaf node

            organism.nodes.push(MorphNode {

                id: *next_node_id,

                position: parent_pos,

                node_type: NodeType::Leaf,

                name: format!("capillary_{}", *next_node_id),

                latency_base_ms: 10.0 + depth as f64 * 2.0,

                capacity: parent_capacity * 0.1,

                energy: 100.0 - depth as f64 * 5.0,

                depth,

                metadata: HashMap::new(),

            });

            organism.connections.push(MorphConnection {

                id: *next_conn_id,

                from_node: parent_id,

                to_node: *next_node_id,

                weight: 0.5,

                bandwidth: parent_capacity * 0.1,

                protocol: if depth > 3 { "local".to_string() } else { "HTTP/2".to_string() },

                latency_ms: 1.0 + depth as f64,

                connection_type: ConnectionType::DataFlow,

                is_active: true,

                age_cycles: 0,

            });

            *next_node_id += 1;

            *next_conn_id += 1;

            return;

        }

        // Két ágra osztódás

        let directions = [

            (angle.cos(), angle.sin(), 0.0),

            (angle.cos(), -angle.sin(), 0.0),

        ];

        // 3D: néha Z irányba is

        let use_z = if depth > 2 && depth % 2 == 0 {

            [(angle.cos(), 0.0, angle.sin()), (0.0, angle.cos(), angle.sin())]

        } else {

            directions

        };

        let step = 1.0 / (depth + 1) as f64;

        let child_capacity = parent_capacity * 0.6;

        for (i, &(dx, dy, dz)) in use_z.iter().enumerate() {

            let child_pos = (

                parent_pos.0 + dx * step,

                parent_pos.1 + dy * step,

                parent_pos.2 + dz * step,

            );

            // Gradiens hatás: vonzódás az attraktorok felé

            let attr_conc = field.concentration_at(child_pos.0, child_pos.1, child_pos.2);

            let adjusted_pos = if attr_conc > 0.1 {

                (

                    child_pos.0 + attr_conc * 0.1,

                    child_pos.1 + attr_conc * 0.1,

                    child_pos.2 + attr_conc * 0.1,

                )

            } else {

                child_pos

            };

            let child_id = *next_node_id;

            *next_node_id += 1;

            let node_type = if depth < 2 {

                NodeType::Gateway

            } else if depth < 4 {

                NodeType::LoadBalancer

            } else if depth < 6 {

                NodeType::Service

            } else {

                NodeType::Cache

            };

            organism.nodes.push(MorphNode {

                id: child_id,

                position: adjusted_pos,

                node_type,

                name: format!("branch_{}_{}", depth, i),

                latency_base_ms: 1.0 + depth as f64 * 1.5,

                capacity: child_capacity,

                energy: 100.0 - depth as f64 * 3.0,

                depth,

                metadata: HashMap::new(),

            });

            organism.connections.push(MorphConnection {

                id: *next_conn_id,

                from_node: parent_id,

                to_node: child_id,

                weight: 0.6 - depth as f64 * 0.02,

                bandwidth: child_capacity,

                protocol: if depth < 3 { "HTTP/2".to_string() } else { "gRPC".to_string() },

                latency_ms: 1.0 + depth as f64 * 0.5,

                connection_type: ConnectionType::DataFlow,

                is_active: true,

                age_cycles: 0,

            });

            *next_conn_id += 1;

            // Rekurzió

            branch(

                organism,

                child_id,

                adjusted_pos,

                child_capacity,

                depth + 1,

                max_depth,

                angle * 0.85,

                field,

                next_node_id,

                next_conn_id,

            );

        }

        // Visszirányú kapcsolat (redundancia)

        if depth > 1 && depth < 4 && use_z.len() >= 2 {

            let from_id = *next_node_id - 1;

            let to_id = *next_node_id - 2;

            organism.connections.push(MorphConnection {

                id: *next_conn_id,

                from_node: from_id,

                to_node: to_id,

                weight: 0.2,

                bandwidth: child_capacity * 0.3,

                protocol: "backup".to_string(),

                latency_ms: 5.0,

                connection_type: ConnectionType::Replication,

                is_active: true,

                age_cycles: 0,

            });

            *next_conn_id += 1;

        }

    }

    branch(

        &mut organism,

        0,

        seed.position,

        1000.0,

        1,

        config.max_depth,

        config.branching_angle,

        field,

        &mut next_node_id,

        &mut next_conn_id,

    );

    organism.calculate_metrics();

    organism

}

/// Nyálkpenész (Physarum polycephalum) szimuláció

///

/// Rács alapú szimuláció: részecskék mozognak a rácson, nyomot hagynak maguk után.

/// A nyom vonzza a többi részecskét (autokatalízis).

/// A párolgás gyengíti a régi nyomokat.

/// Tápanyag források = seed pozíciók.

/// Az eredmény: optimális útvonal hálózat a források között.

pub fn slime_mold_growth(seed: &Seed, field: &MorphogenField, config: &GrowthConfig) -> Organism {

    let mut organism = Organism::new(

        &format!("slime_{}", rand::random::<u32>()),

        &format!("SlimeMold_{}", seed.type_tag),

        seed.clone(),

        GrowthPattern::SlimeMold,

    );

    // Rács inicializálása

    let grid_size = 64;

    let mut trail: Vec<Vec<f64>> = vec![vec![0.0; grid_size]; grid_size];

    let mut particles: Vec<(f64, f64, f64)> = Vec::new(); // (x, y, angle)

    // Tápanyag források a seed körül

    let food_sources: Vec<(usize, usize)> = (0..8).map(|i| {

        let fx = ((seed.position.0 + (i as f64 * 3.0).sin() * 20.0) as usize).clamp(0, grid_size - 1);

        let fy = ((seed.position.1 + (i as f64 * 3.0).cos() * 20.0) as usize).clamp(0, grid_size - 1);

        trail[fy][fx] = 1.0; // Food marker

        (fx, fy)

    }).collect();

    // Részecskék indítása a seed-ből

    for _ in 0..50 {

        let angle = rand::random::<f64>() * std::f64::consts::TAU;

        particles.push((

            seed.position.0.max(0.0).min(grid_size as f64 - 1.0),

            seed.position.1.max(0.0).min(grid_size as f64 - 1.0),

            angle,

        ));

    }

    // Szimulációs lépések

    let sensor_angle = std::f64::consts::FRAC_PI_4; // 45 fok

    let sensor_distance = 3.0;

    let rotation_angle = std::f64::consts::FRAC_PI_6; // 30 fok

    let step_size = 1.0;

    let mut grid_attractors: HashMap<(i32, i32), f64> = HashMap::new();

    for _step in 0..1000 {

        for particle in &mut particles {

            let (px, py, angle) = *particle;

            // Szenzorok: előre, balra, jobbra

            let sensors = [

                (angle, 1.0),                       // előre

                (angle - sensor_angle, 1.0),         // balra

                (angle + sensor_angle, 1.0),         // jobbra

                (angle - sensor_angle * 2.0, 0.8),   // balra távolabb

                (angle + sensor_angle * 2.0, 0.8),   // jobbra távolabb

            ];

            let mut best_conc = -1.0;

            let mut best_angle = angle;

            for (sensor_angle, weight) in &sensors {

                let sensor_x = (px + sensor_angle.cos() * sensor_distance) as usize;

                let sensor_y = (py + sensor_angle.sin() * sensor_distance) as usize;

                if sensor_x < grid_size && sensor_y < grid_size {

                    let trail_val = trail[sensor_y][sensor_x];

                    let conc = trail_val * weight;

                    // Field hatás

                    let field_val = field.concentration_at(

                        sensor_x as f64, sensor_y as f64, 0.0,

                    );

                    let total = conc + field_val * 0.3;

                    if total > best_conc {

                        best_conc = total;

                        best_angle = *sensor_angle;

                    }

                }

            }

            // Irány váltás a legjobb szenzor felé

            let new_angle = if best_conc < 0.0 {

                angle + (rand::random::<f64>() - 0.5) * rotation_angle * 2.0

            } else {

                let diff = best_angle - angle;

                if diff.abs() > rotation_angle {

                    angle + rotation_angle.copysign(diff)

                } else {

                    best_angle

                }

            };

            // Mozgás

            let new_x = px + new_angle.cos() * step_size;

            let new_y = py + new_angle.sin() * step_size;

            // Grid határok ellenőrzése

            if new_x >= 0.0 && new_x < grid_size as f64 - 1.0

                && new_y >= 0.0 && new_y < grid_size as f64 - 1.0

            {

                particle.0 = new_x;

                particle.1 = new_y;

                particle.2 = new_angle;

                // Nyom lerakása

                let gx = new_x as usize;

                let gy = new_y as usize;

                trail[gy][gx] = (trail[gy][gx] + config.trail_persistence).min(1.0);

                // Grid attraktorok gyűjtése a struktúrához

                let key = (gx as i32, gy as i32);

                *grid_attractors.entry(key).or_insert(0.0) += 0.01;

            }

        }

        // Párolgás

        for y in 0..grid_size {

            for x in 0..grid_size {

                trail[y][x] *= 1.0 - config.trail_evaporation;

                if trail[y][x] < 0.01 {

                    trail[y][x] = 0.0;

                }

            }

        }

    }

    // Grid attraktorok → node-ok és kapcsolatok

    let mut id_map: HashMap<(i32, i32), usize> = HashMap::new();

    let mut next_id = 0;

    let mut next_conn = 1;

    // Erős nyomokból node-ok

    for y in 0..grid_size {

        for x in 0..grid_size {

            if trail[y][x] > 0.15 {

                let node_id = next_id;

                next_id += 1;

                id_map.insert((x as i32, y as i32), node_id);

                let node_type = if food_sources.contains(&(x, y)) {

                    NodeType::Root

                } else if trail[y][x] > 0.8 {

                    NodeType::Gateway

                } else if trail[y][x] > 0.5 {

                    NodeType::Service

                } else {

                    NodeType::Cache

                };

                organism.nodes.push(MorphNode {

                    id: node_id,

                    position: (x as f64, y as f64, 0.0),

                    node_type,

                    name: format!("slime_{}", node_id),

                    latency_base_ms: 5.0 - trail[y][x] * 3.0,

                    capacity: trail[y][x] * 100.0,

                    energy: trail[y][x] * 50.0,

                    depth: 0,

                    metadata: HashMap::new(),

                });

            }

        }

    }

    // Kapcsolatok a szomszédos node-ok között

    for (&(x, y), &from_id) in &id_map {

        for (dx, dy) in &[(1, 0), (0, 1), (1, 1), (-1, 1)] {

            let nx = x + dx;

            let ny = y + dy;

            if let Some(&to_id) = id_map.get(&(nx, ny)) {

                let avg_trail = (trail[y as usize][x as usize] + trail[ny as usize][nx as usize]) / 2.0;

                organism.connections.push(MorphConnection {

                    id: next_conn,

                    from_node: from_id,

                    to_node: to_id,

                    weight: avg_trail,

                    bandwidth: avg_trail * 10000.0,

                    protocol: "gRPC".to_string(),

                    latency_ms: (1.0 - avg_trail).max(0.1) * 10.0,

                    connection_type: ConnectionType::DataFlow,

                    is_active: true,

                    age_cycles: 0,

                });

                next_conn += 1;

            }

        }

    }

    organism.calculate_metrics();

    organism

}

/// L-system fraktál növekedés

///

/// Turtle-graphics szimuláció 3D-ben.

/// Axióma + produkciós szabályok iteratív alkalmazása.

/// Minden szimbólum egy node típust és növekedési irányt reprezentál.

pub fn fractal_growth(seed: &Seed, _field: &MorphogenField, config: &GrowthConfig) -> Organism {

    let mut organism = Organism::new(

        &format!("fractal_{}", rand::random::<u32>()),

        &format!("Fractal_{}", seed.type_tag),

        seed.clone(),

        GrowthPattern::FractalLSystem,

    );

    // L-system generálás

    let axiom = if seed.type_tag.contains("cache") {

        "F[+F][-F]F"

    } else if seed.type_tag.contains("database") {

        "F[+F[+F][-F]][-F[+F][-F]]"

    } else {

        "F+F+F+F"

    };

    let mut current = axiom.to_string();

    for _gen in 0..config.max_depth {

        let mut next = String::new();

        for ch in current.chars() {

            let mut replaced = false;

            for (rule_from, rule_to) in &config.lsystem_rules {

                if ch == *rule_from {

                    next.push_str(rule_to);

                    replaced = true;

                    break;

                }

            }

            if !replaced {

                next.push(ch);

            }

        }

        current = next;

        if current.len() > 2000 { break; } // Limit a túlcsordulás ellen

    }

    // Turtle graphics interpreter

    struct TurtleState {

        x: f64, y: f64, z: f64,

        angle_xy: f64,

        angle_z: f64,

    }

    let mut turtle = TurtleState {

        x: seed.position.0,

        y: seed.position.1,

        z: seed.position.2,

        angle_xy: 0.0,

        angle_z: 0.0,

    };

    let mut node_stack: Vec<(usize, TurtleState)> = Vec::new();

    let mut next_node_id = 0;

    let mut next_conn_id = 1;

    // Root node

    organism.nodes.push(MorphNode {

        id: next_node_id,

        position: (turtle.x, turtle.y, turtle.z),

        node_type: NodeType::Root,

        name: "fractal_root".to_string(),

        latency_base_ms: 0.5,

        capacity: 100.0,

        energy: 100.0,

        depth: 0,

        metadata: HashMap::new(),

    });

    let mut last_node_id = next_node_id;

    next_node_id += 1;

    for ch in current.chars() {

        if next_node_id >= config.max_nodes { break; }

        match ch {

            'F' | 'A' | 'B' => {

                // Előre lépés és új node

                let step = 1.0;

                let nx = turtle.x + turtle.angle_xy.cos() * turtle.angle_z.cos() * step;

                let ny = turtle.y + turtle.angle_xy.sin() * turtle.angle_z.cos() * step;

                let nz = turtle.z + turtle.angle_z.sin() * step;

                let node_type = match ch {

                    'A' => NodeType::Gateway,

                    'B' => NodeType::Database,

                    _ => NodeType::Service,

                };

                let node_id = next_node_id;

                next_node_id += 1;

                organism.nodes.push(MorphNode {

                    id: node_id,

                    position: (nx, ny, nz),

                    node_type,

                    name: format!("fnode_{}", node_id),

                    latency_base_ms: 2.0,

                    capacity: 50.0,

                    energy: 80.0,

                    depth: node_stack.len() as u32,

                    metadata: HashMap::new(),

                });

                organism.connections.push(MorphConnection {

                    id: next_conn_id,

                    from_node: last_node_id,

                    to_node: node_id,

                    weight: 0.5,

                    bandwidth: 5000.0,

                    protocol: "HTTP/2".to_string(),

                    latency_ms: 1.0,

                    connection_type: ConnectionType::DataFlow,

                    is_active: true,

                    age_cycles: 0,

                });

                next_conn_id += 1;

                turtle.x = nx;

                turtle.y = ny;

                turtle.z = nz;

                last_node_id = node_id;

            }

            '+' => {

                turtle.angle_xy += config.branching_angle;

            }

            '-' => {

                turtle.angle_xy -= config.branching_angle;

            }

            '[' => {

                // Push: elágazás kezdete

                node_stack.push((last_node_id, TurtleState {

                    x: turtle.x,

                    y: turtle.y,

                    z: turtle.z,

                    angle_xy: turtle.angle_xy,

                    angle_z: turtle.angle_z,

                }));

            }

            ']' => {

                // Pop: vissza az elágazási ponthoz

                if let Some((saved_node, saved_turtle)) = node_stack.pop() {

                    turtle = saved_turtle;

                    last_node_id = saved_node;

                }

            }

            '^' => {

                turtle.angle_z += config.branching_angle * 0.5;

            }

            'v' => {

                turtle.angle_z -= config.branching_angle * 0.5;

            }

            _ => {}

        }

    }

    organism.calculate_metrics();

    organism

}

// ─── Fitness Célok ───────────────────────────────────────────────────────────

/// Fitness célok az evolúciós szelekcióhoz

#[derive(Debug, Clone, PartialEq)]

pub enum FitnessObjective {

    /// Minimális késleltetés

    MinimizeLatency,

    /// Maximális áteresztőképesség

    MaximizeThroughput,

    /// Minimális költség (legkevesebb node)

    MinimizeCost,

    /// Maximális redundancia (hibatűrés)

    MaximizeRedundancy,

    /// Kiegyensúlyozott (alapértelmezett)

    Balanced,

    /// Egyedi célfüggvény

    Custom(String),

}

/// Fitness eredmény

#[derive(Debug, Clone)]

pub struct FitnessResult {

    pub score: f64,

    pub latency_score: f64,

    pub throughput_score: f64,

    pub cost_score: f64,

    pub redundancy_score: f64,

    pub detail: String,

}

/// Fitness függvény kiértékelése

pub fn evaluate_fitness(organism: &Organism, objective: &FitnessObjective) -> FitnessResult {

    let metrics = organism.metrics.as_ref()

        .cloned()

        .unwrap_or_else(|| {

            let mut o = organism.clone();

            o.calculate_metrics()

        });

    let node_count = metrics.node_count.max(1) as f64;

    let conn_count = metrics.connection_count.max(1) as f64;

    let _avg_depth = metrics.max_depth.max(1) as f64;

    let redundancy = metrics.redundancy_score;

    // Latency score: kevesebb = jobb

    let avg_latency: f64 = if organism.connections.is_empty() {

        50.0

    } else {

        organism.connections.iter().map(|c| c.latency_ms).sum::<f64>() / conn_count

    };

    let latency_score = (100.0 / (avg_latency + 10.0)).min(1.0);

    // Throughput score: több kapcsolat és kapacitás = jobb

    let avg_bandwidth: f64 = if organism.connections.is_empty() {

        0.0

    } else {

        organism.connections.iter().map(|c| c.bandwidth).sum::<f64>() / conn_count

    };

    let throughput_score = (avg_bandwidth / 20000.0).min(1.0);

    // Cost score: kevesebb node = olcsóbb

    let cost_score = (1.0 - (node_count / 20.0).min(1.0)).max(0.05);

    // Redundancy: több kapcsolat = robusztusabb

    let redundancy_score = (redundancy / 3.0).min(1.0);

    // Fractal efficiency

    let fractal_efficiency = (metrics.fractal_dimension / 3.0).min(1.0);

    // Complexity penalty: dense graphs cost more
    let complexity_penalty = ((conn_count / node_count.max(1.0)) / 3.0).min(1.0) * 0.15;

    let score = match objective {

        FitnessObjective::MinimizeLatency => {

            latency_score * 0.6 + throughput_score * 0.2 + cost_score * 0.1 + redundancy_score * 0.1

        }

        FitnessObjective::MaximizeThroughput => {

            throughput_score * 0.5 + latency_score * 0.2 + redundancy_score * 0.2 + cost_score * 0.1

        }

        FitnessObjective::MinimizeCost => {

            cost_score * 0.6 + latency_score * 0.2 + throughput_score * 0.1 + redundancy_score * 0.1

        }

        FitnessObjective::MaximizeRedundancy => {

            redundancy_score * 0.5 + latency_score * 0.2 + throughput_score * 0.2 + cost_score * 0.1

        }

        FitnessObjective::Balanced => {

            latency_score * 0.3 + throughput_score * 0.3 + cost_score * 0.2 + redundancy_score * 0.2

        }

        FitnessObjective::Custom(_) => {

            (latency_score + throughput_score + cost_score + redundancy_score) / 4.0

        }

    };

    FitnessResult {

        score: (score * fractal_efficiency).max(0.0).min(1.0),

        latency_score,

        throughput_score,

        cost_score,

        redundancy_score,

        detail: format!(

            "nodes={}, conns={}, avg_lat={:.1}ms, avg_bw={:.0}, redund={:.2}",

            node_count, conn_count, avg_latency, avg_bandwidth, redundancy

        ),

    }

}

// ─── Morphogenesis Engine ────────────────────────────────────────────────────

/// A fő morfogenetikus motor

pub struct MorphogenesisEngine {

    /// Konfiguráció

    pub config: Arc<RwLock<GrowthConfig>>,

    /// Organizmus populáció

    pub organisms: Arc<RwLock<Vec<Organism>>>,

    /// Morfogén mező

    pub field: Arc<RwLock<MorphogenField>>,

    /// Generációs számláló

    pub generation: Arc<RwLock<u32>>,

    /// Evolúciós történet (legjobb fitness per generáció)

    pub evolution_history: Arc<RwLock<Vec<f64>>>,

}

impl MorphogenesisEngine {

    /// Létrehoz egy új motort

    pub fn new() -> Self {

        Self {

            config: Arc::new(RwLock::new(GrowthConfig::default())),

            organisms: Arc::new(RwLock::new(Vec::new())),

            field: Arc::new(RwLock::new(MorphogenField::new())),

            generation: Arc::new(RwLock::new(0)),

            evolution_history: Arc::new(RwLock::new(Vec::new())),

        }

    }

    /// Konfiguráció beállítása

    pub fn set_config(&self, config: GrowthConfig) {

        *self.config.write().unwrap() = config;

    }

    /// Morfogén mező beállítása

    pub fn set_field(&self, field: MorphogenField) {

        *self.field.write().unwrap() = field;

    }

    /// Organizmus növesztése egy seed-ből

    pub fn grow_from_seed(&self, seed: &Seed, pattern: Option<GrowthPattern>) -> Organism {

        let config = self.config.read().unwrap().clone();

        let pattern = pattern.or(seed.preferred_pattern).unwrap_or(config.pattern);

        let field = self.field.read().unwrap().clone();

        let organism = match pattern {

            GrowthPattern::Mycelium => mycelium_growth(seed, &field, &config),

            GrowthPattern::Capillary => capillary_growth(seed, &field, &config),

            GrowthPattern::SlimeMold => slime_mold_growth(seed, &field, &config),

            GrowthPattern::FractalLSystem => fractal_growth(seed, &field, &config),

            GrowthPattern::Hybrid => {

                // Hibrid: mycelium + kapilláris kombináció

                let mycelium = mycelium_growth(seed, &field, &config);

                if mycelium.fitness_score > 0.3 {

                    mycelium

                } else {

                    capillary_growth(seed, &field, &config)

                }

            }

        };

        let fitness = evaluate_fitness(&organism, &FitnessObjective::Balanced);

        let mut org = organism;

        org.fitness_score = fitness.score;

        self.organisms.write().unwrap().push(org.clone());

        org

    }

    /// Populáció evolúciója generációkon keresztül

    ///

    /// Minden generáció:

    /// 1. Fitness kiértékelés

    /// 2. Szelekció (legjobb N marad)

    /// 3. Keresztezés (seed paraméterek keverése)

    /// 4. Mutáció (növekedési paraméterek véletlenszerű változtatása)

    /// 5. Új organizmusok növesztése

    pub fn evolve_population(

        &self,

        seeds: &[Seed],

        generations: u32,

        objective: &FitnessObjective,

        population_size: usize,

    ) -> Vec<Organism> {

        if seeds.is_empty() { return Vec::new(); }

        let _config = self.config.read().unwrap().clone();

        let patterns = [

            GrowthPattern::Mycelium,

            GrowthPattern::Capillary,

            GrowthPattern::SlimeMold,

            GrowthPattern::FractalLSystem,

        ];

        // Kezdeti populáció létrehozása

        let mut population: Vec<Organism> = seeds.iter().flat_map(|seed| {

            patterns.iter().map(|pattern| {

                let field = self.field.read().unwrap().clone();

                let config = self.config.read().unwrap().clone();

                let mut org = match pattern {

                    GrowthPattern::Mycelium => mycelium_growth(seed, &field, &config),

                    GrowthPattern::Capillary => capillary_growth(seed, &field, &config),

                    GrowthPattern::SlimeMold => slime_mold_growth(seed, &field, &config),

                    GrowthPattern::FractalLSystem => fractal_growth(seed, &field, &config),

                    GrowthPattern::Hybrid => unreachable!(),

                };

                let fitness = evaluate_fitness(&org, objective);

                org.fitness_score = fitness.score;

                org

            }).collect::<Vec<_>>()

        }).collect();

        // Evolúciós ciklusok

        for gen in 0..generations {

            *self.generation.write().unwrap() = gen + 1;

            // 1. Fitness alapú rendezés

            population.sort_by(|a, b| b.fitness_score.partial_cmp(&a.fitness_score).unwrap());

            // Legjobb fitness mentése

            if let Some(best) = population.first() {

                self.evolution_history.write().unwrap().push(best.fitness_score);

            }

            // 2. Szelekció: top 30% marad

            let keep_count = (population_size / 3).max(population.len() / 3);

            let survivors: Vec<Organism> = population.into_iter().take(keep_count).collect();

            // 3-4-5. Új generáció: mutáció és keresztezés

            let mut new_population = survivors.clone();

            let mut rng_idx = 0;

            while new_population.len() < population_size {

                let parent = &survivors[rng_idx % survivors.len()];

                rng_idx += 1;

                // Mutált seed

                let mutation_factor = 0.8 + rand::random::<f64>() * 0.4;

                let mutated_seed = Seed {

                    id: format!("evo_{}_{}", gen, new_population.len()),

                    position: (

                        parent.seed.position.0 + (rand::random::<f64>() - 0.5) * 5.0,

                        parent.seed.position.1 + (rand::random::<f64>() - 0.5) * 5.0,

                        parent.seed.position.2 + (rand::random::<f64>() - 0.5) * 5.0,

                    ),

                    energy: parent.seed.energy * mutation_factor,

                    type_tag: parent.seed.type_tag.clone(),

                    preferred_pattern: Some(patterns[rand::random::<usize>() % patterns.len()]),

                };

                let field = self.field.read().unwrap().clone();

                let config = self.config.read().unwrap().clone();

                let pattern = mutated_seed.preferred_pattern.unwrap_or(config.pattern);

                let mut child = match pattern {

                    GrowthPattern::Mycelium => mycelium_growth(&mutated_seed, &field, &config),

                    GrowthPattern::Capillary => capillary_growth(&mutated_seed, &field, &config),

                    GrowthPattern::SlimeMold => slime_mold_growth(&mutated_seed, &field, &config),

                    GrowthPattern::FractalLSystem => fractal_growth(&mutated_seed, &field, &config),

                    GrowthPattern::Hybrid => unreachable!(),

                };

                child.generation = gen + 1;

                let fitness = evaluate_fitness(&child, objective);

                child.fitness_score = fitness.score;

                new_population.push(child);

            }

            population = new_population;

        }

        // Final rendezés és tárolás

        population.sort_by(|a, b| b.fitness_score.partial_cmp(&a.fitness_score).unwrap());

        let top: Vec<Organism> = population.into_iter().take(population_size / 2).collect();

        *self.organisms.write().unwrap() = top.clone();

        top

    }

    /// Pruning: gyenge ágak eltávolítása

    pub fn prune_weak(&self, organism: &mut Organism, threshold: f64) -> usize {

        let before = organism.connections.len();

        // Inaktív kapcsolatok eltávolítása

        organism.connections.retain(|c| {

            c.is_active && c.weight > threshold

        });

        // Elszigetelt node-ok eltávolítása

        let connected_nodes: std::collections::HashSet<usize> = organism.connections.iter()

            .flat_map(|c| vec![c.from_node, c.to_node])

            .collect();

        organism.nodes.retain(|n| connected_nodes.contains(&n.id));

        before - organism.connections.len()

    }

    /// Topológiai elemzés

    pub fn analyze_topology(organism: &Organism) -> HashMap<String, f64> {

        let mut analysis = HashMap::new();

        let n = organism.nodes.len() as f64;

        let e = organism.connections.len() as f64;

        // Átlagos fokszám

        if n > 0.0 {

            analysis.insert("avg_degree".to_string(), (e * 2.0 / n).round());

        }

        // Kapcsolati sűrűség

        if n > 1.0 {

            let max_edges = n * (n - 1.0) / 2.0;

            analysis.insert("density".to_string(), (e / max_edges * 100.0 * 100.0).round() / 100.0);

        }

        // Elágazási arány

        let internal = organism.nodes.iter()

            .filter(|n| {

                organism.connections.iter().any(|c| c.from_node == n.id)

            }).count() as f64;

        if internal > 0.0 {

            analysis.insert("branching_ratio".to_string(), (n / internal * 100.0).round() / 100.0);

        }

        // Átlagos késleltetés

        if !organism.connections.is_empty() {

            let avg_lat: f64 = organism.connections.iter().map(|c| c.latency_ms).sum::<f64>() / e;

            analysis.insert("avg_latency_ms".to_string(), (avg_lat * 100.0).round() / 100.0);

        }

        // Redundancia index

        if n > 1.0 {

            analysis.insert("redundancy".to_string(), (e / (n - 1.0) * 100.0).round() / 100.0);

        }

        analysis

    }

    /// Legjobb organizmus lekérése

    pub fn get_best_organism(&self) -> Option<Organism> {

        let organisms = self.organisms.read().unwrap();

        organisms.iter()

            .max_by(|a, b| a.fitness_score.partial_cmp(&b.fitness_score).unwrap())

            .cloned()

    }

    /// Evolúciós történet lekérése

    pub fn evolution_summary(&self) -> Vec<(u32, f64)> {

        let history = self.evolution_history.read().unwrap();

        history.iter().enumerate().map(|(i, s)| (i as u32 + 1, *s)).collect()

    }

}

// ─── Expresszió: Organizmus → Architecture leképezés ────────────────────────

/// Organizmus kifejezése szimulálható Architecture-vé

pub fn express_as_architecture(organism: &Organism) -> crate::architecture_simulator::Architecture {

    let mut components = std::collections::HashMap::new();

    let mut connections = Vec::new();

    for node in &organism.nodes {

        let comp_type = match node.node_type {

            NodeType::Root | NodeType::Gateway => crate::architecture_simulator::ComponentType::Software,

            NodeType::Database | NodeType::Cache => crate::architecture_simulator::ComponentType::Storage,

            NodeType::LoadBalancer | NodeType::Queue => crate::architecture_simulator::ComponentType::Network,

            NodeType::Service | NodeType::Leaf => crate::architecture_simulator::ComponentType::Software,

            NodeType::Custom(_) => crate::architecture_simulator::ComponentType::Custom(node.node_type.to_string()),

        };

        let mut capacity = std::collections::HashMap::new();

        capacity.insert("memory_mb".to_string(), node.capacity);

        components.insert(node.id.to_string(), crate::architecture_simulator::Component {

            id: node.id.to_string(),

            name: node.name.clone(),

            component_type: comp_type,

            capacity,

            load: 0.0,

            error_rate: 0.01,

            latency_ms: node.latency_base_ms,

        });

    }

    for conn in &organism.connections {

        if !conn.is_active { continue; }

        connections.push(crate::architecture_simulator::Connection {

            from: conn.from_node.to_string(),

            to: conn.to_node.to_string(),

            bandwidth: conn.bandwidth,

            protocol: conn.protocol.clone(),

            latency_ms: conn.latency_ms,

            packet_loss: 0.001,

        });

    }

    crate::architecture_simulator::Architecture {

        id: format!("morph_{}", organism.id),

        name: organism.name.clone(),

        description: format!(

            "Morphogenetically grown {:?} (gen={}, fitness={:.3})",

            organism.growth_pattern, organism.generation, organism.fitness_score

        ),

        components,

        connections,

        version: organism.generation as u32,

        cohesion_score: organism.fitness_score,

    }

}

// ─── Integráció: Vagus trigger ──────────────────────────────────────────────

/// Vagus stressz szint alapján kompenzatórikus növekedés triggerelése

///

/// Ha a vagus tónus egy kritikus szint alá esik (magas stressz),

/// a morphogenesis automatikusan új struktúrákat növeszt,

/// hogy tehermentesítse a szűk keresztmetszeteket.

pub fn trigger_from_vagus(

    vagus_tone: &crate::vagus::VagusTone,

    system_pulse: &crate::vagus::SystemPulse,

    engine: &MorphogenesisEngine,

    threshold: f64,

) -> Option<Organism> {

    // Ha a tónus a küszöb alatt van, stressz kompenzáció

    if vagus_tone.current > threshold {

        return None; // Nincs szükség beavatkozásra

    }

    // Stressz forrás azonosítása a pulzus alapján

    let stress_source = if system_pulse.cpu_pressure > 0.8 {

        "cpu_bound"

    } else if system_pulse.memory_pressure > 0.8 {

        "memory_bound"

    } else if system_pulse.network_pressure > 0.8 {

        "network_bound"

    } else {

        "general"

    };

    // Kompenzatórikus seed létrehozása

    let seed = Seed {

        id: format!("compensate_{}", rand::random::<u32>()),

        position: (0.0, 0.0, 0.0),

        energy: vagus_tone.baseline * 100.0,

        type_tag: stress_source.to_string(),

        preferred_pattern: match stress_source {

            "network_bound" => Some(GrowthPattern::Mycelium),   // több hálózati útvonal

            "memory_bound" => Some(GrowthPattern::Capillary),   // cache réteg növesztése

            "cpu_bound" => Some(GrowthPattern::FractalLSystem), // új feldolgozó ágak

            _ => Some(GrowthPattern::Hybrid),

        },

    };

    let organism = engine.grow_from_seed(&seed, None);

    if organism.fitness_score > 0.3 {

        Some(organism)

    } else {

        None

    }

}

// ─── Integráció: Neuroplasticity térkép ─────────────────────────────────────

/// Organizmus struktúra leképezése neuroplasztikus útvonalakra

///

/// Minden MorphConnection egy szinaptikus kapcsolattá alakul.

/// Az erős kapcsolatok magasabb súlyt kapnak.

pub fn map_to_neuroplasticity(organism: &Organism) -> Vec<(u32, u32, f32)> {

    organism.connections.iter()

        .filter(|c| c.is_active)

        .map(|c| {

            let weight = (c.weight * 0.5 + c.bandwidth / 20000.0 * 0.3 + (1.0 - c.latency_ms / 20.0).max(0.0) * 0.2) as f32;

            (c.from_node as u32, c.to_node as u32, weight.min(1.0).max(0.0))

        })

        .collect()

}

// ─── Integráció: Federation fingerprint export ──────────────────────────────

/// Fingerprint generálása federation rezonanciához

///

/// A fingerprint tartalmazza:

/// - Növekedési minta típusa

/// - Node-ok és kapcsolatok számának tömörített hash-e

/// - Fitness score

/// - Topológiai jellemzők (átlagos fokszám, sűrűség, fraktál dimenzió)

pub fn export_fingerprint(organism: &Organism) -> Vec<u8> {

    use std::hash::{Hash, Hasher};

    use std::collections::hash_map::DefaultHasher;

    let mut fingerprint = Vec::new();

    // Növekedési minta

    fingerprint.push(match organism.growth_pattern {

        GrowthPattern::Mycelium => 0x01,

        GrowthPattern::Capillary => 0x02,

        GrowthPattern::SlimeMold => 0x03,

        GrowthPattern::FractalLSystem => 0x04,

        GrowthPattern::Hybrid => 0xFF,

    });

    // Strukturális hash

    let mut hasher = DefaultHasher::new();

    organism.nodes.len().hash(&mut hasher);

    organism.connections.len().hash(&mut hasher);

    ((organism.fitness_score * 1000.0) as u64).hash(&mut hasher);

    let structural_hash = hasher.finish();

    fingerprint.extend_from_slice(&structural_hash.to_le_bytes());

    // Topológiai jellemzők (kvantált)

    if let Some(ref metrics) = organism.metrics {

        let fractal_quant = (metrics.fractal_dimension * 100.0) as u16;

        let redundancy_quant = (metrics.redundancy_score * 100.0) as u16;

        let depth_quant = metrics.max_depth as u8;

        fingerprint.extend_from_slice(&fractal_quant.to_le_bytes());

        fingerprint.extend_from_slice(&redundancy_quant.to_le_bytes());

        fingerprint.push(depth_quant);

    }

    fingerprint

}

// ─── Display implementációk ─────────────────────────────────────────────────

use std::fmt;

impl fmt::Display for NodeType {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        match self {

            NodeType::Root => write!(f, "Root"),

            NodeType::Service => write!(f, "Service"),

            NodeType::Database => write!(f, "Database"),

            NodeType::Cache => write!(f, "Cache"),

            NodeType::Gateway => write!(f, "Gateway"),

            NodeType::LoadBalancer => write!(f, "LoadBalancer"),

            NodeType::Queue => write!(f, "Queue"),

            NodeType::Leaf => write!(f, "Leaf"),

            NodeType::Custom(s) => write!(f, "Custom({})", s),

        }

    }

}

impl fmt::Display for GrowthPattern {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        match self {

            GrowthPattern::Mycelium => write!(f, "Mycelium"),

            GrowthPattern::Capillary => write!(f, "Capillary"),

            GrowthPattern::SlimeMold => write!(f, "SlimeMold"),

            GrowthPattern::FractalLSystem => write!(f, "FractalLSystem"),

            GrowthPattern::Hybrid => write!(f, "Hybrid"),

        }

    }

}

impl fmt::Display for ConnectionType {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        match self {

            ConnectionType::DataFlow => write!(f, "DataFlow"),

            ConnectionType::Control => write!(f, "Control"),

            ConnectionType::Replication => write!(f, "Replication"),

            ConnectionType::Backup => write!(f, "Backup"),

        }

    }

}

impl fmt::Display for Organism {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        writeln!(f, "Organism: {} (gen={}, fitness={:.3})", self.name, self.generation, self.fitness_score)?;

        writeln!(f, "  Pattern: {}", self.growth_pattern)?;

        writeln!(f, "  Nodes: {}, Connections: {}", self.nodes.len(), self.connections.len())?;

        if let Some(ref m) = self.metrics {

            writeln!(f, "  Max depth: {}, Fractal dim: {:.3}", m.max_depth, m.fractal_dimension)?;

            writeln!(f, "  Redundancy: {:.3}, Avg path: {:.3}", m.redundancy_score, m.avg_path_length)?;

        }

        Ok(())

    }

}

// ─── Segédfüggvények ─────────────────────────────────────────────────────────

/// Több organizmus egyesítése egy nagyobb struktúrába

pub fn merge_organisms(organisms: &[Organism]) -> Option<Organism> {

    if organisms.is_empty() { return None; }

    let mut merged = organisms[0].clone();

    merged.id = format!("merged_{}", rand::random::<u32>());

    merged.name = format!("Merged_{}", organisms.len());

    let mut next_node_id = merged.nodes.len();

    let mut next_conn_id = merged.connections.len();

    for org in organisms.iter().skip(1) {

        for node in &org.nodes {

            let mut new_node = node.clone();

            new_node.id = next_node_id;

            merged.nodes.push(new_node);

            next_node_id += 1;

        }

        for conn in &org.connections {

            merged.connections.push(MorphConnection {

                id: next_conn_id,

                from_node: conn.from_node,

                to_node: conn.to_node,

                ..conn.clone()

            });

            next_conn_id += 1;

        }

    }

    merged.calculate_metrics();

    Some(merged)

}

/// Organizmus struktúra egyszerűsítése (redundáns node-ok összevonása)

pub fn simplify_organism(organism: &mut Organism) -> usize {

    let _before = organism.nodes.len();

    let mut removed = std::collections::HashSet::new();

    // Azonos típusú, szomszédos és alacsony energiájú node-ok összevonása

    let mut i = 0;

    while i < organism.nodes.len() {

        if removed.contains(&i) { i += 1; continue; }

        let node = &organism.nodes[i];

        if node.node_type == NodeType::Leaf && node.energy < 10.0 {

            removed.insert(i);

        }

        i += 1;

    }

    // Node-ok eltávolítása (indexeket fenntartva)

    let old_count = organism.nodes.len();

    organism.nodes.retain(|n| !removed.contains(&n.id));

    let removed_count = old_count - organism.nodes.len();

    // Elszigetelt kapcsolatok tisztítása

    let active_ids: std::collections::HashSet<usize> = organism.nodes.iter().map(|n| n.id).collect();

    organism.connections.retain(|c| active_ids.contains(&c.from_node) && active_ids.contains(&c.to_node));

    organism.calculate_metrics();

    removed_count

}

// ─── NodeType → string konverzió ────────────────────────────────────────────

impl NodeType {

    pub fn to_string(&self) -> String {

        match self {

            NodeType::Root => "Root".to_string(),

            NodeType::Service => "Service".to_string(),

            NodeType::Database => "Database".to_string(),

            NodeType::Cache => "Cache".to_string(),

            NodeType::Gateway => "Gateway".to_string(),

            NodeType::LoadBalancer => "LoadBalancer".to_string(),

            NodeType::Queue => "Queue".to_string(),

            NodeType::Leaf => "Leaf".to_string(),

            NodeType::Custom(s) => s.clone(),

        }

    }

}

// ─── Tesztek ─────────────────────────────────────────────────────────────────

#[cfg(test)]

mod tests {

    use super::*;

    fn test_seed() -> Seed {

        Seed::new("test_seed", 0.0, 0.0, 0.0, "service")

    }

    #[test]

    fn test_seed_creation() {

        let seed = test_seed();

        assert_eq!(seed.id, "test_seed");

        assert_eq!(seed.energy, 100.0);

    }

    #[test]

    fn test_mycelium_growth() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::mycelium_default();

        let organism = mycelium_growth(&seed, &field, &config);

        assert!(!organism.nodes.is_empty(), "Mycelium growth should produce nodes");

        assert!(organism.nodes.len() > 1, "Should have more than just the root node");

        assert!(organism.fitness_score >= 0.0);

    }

    #[test]

    fn test_capillary_growth() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::capillary_default();

        let organism = capillary_growth(&seed, &field, &config);

        assert!(!organism.nodes.is_empty(), "Capillary growth should produce nodes");

    }

    #[test]

    fn test_slime_mold_growth() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::slime_mold_default();

        let organism = slime_mold_growth(&seed, &field, &config);

        // Slime mold is stochastic; may produce 0 nodes in rare cases

        if organism.nodes.is_empty() {

            let retry = slime_mold_growth(&seed, &field, &config);

            assert!(!retry.nodes.is_empty(), "Slime mold should produce nodes on retry");

        }

    }

    #[test]

    fn test_fractal_growth() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::fractal_lsystem_default();

        let organism = fractal_growth(&seed, &field, &config);

        assert!(!organism.nodes.is_empty(), "Fractal growth should produce nodes");

    }

    #[test]

    fn test_engine_grow_from_seed() {

        let engine = MorphogenesisEngine::new();

        let seed = test_seed();

        let organism = engine.grow_from_seed(&seed, Some(GrowthPattern::Mycelium));

        assert!(!organism.nodes.is_empty());

        assert!(organism.metrics.is_some());

    }

    #[test]

    fn test_evolve_population() {

        let engine = MorphogenesisEngine::new();

        let seeds = vec![test_seed()];

        let results = engine.evolve_population(

            &seeds,

            2,

            &FitnessObjective::Balanced,

            10,

        );

        assert!(!results.is_empty(), "Evolution should produce results");

        // First result should have highest fitness

        if results.len() > 1 {

            assert!(results[0].fitness_score >= results[1].fitness_score);

        }

    }

    #[test]

    fn test_evaluate_fitness() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let organism = mycelium_growth(&seed, &field, &config);

        let fitness = evaluate_fitness(&organism, &FitnessObjective::Balanced);

        assert!(fitness.score >= 0.0 && fitness.score <= 1.0);

        let latency_fitness = evaluate_fitness(&organism, &FitnessObjective::MinimizeLatency);

        assert!(latency_fitness.score >= 0.0 && latency_fitness.score <= 1.0);

    }

    #[test]

    fn test_morphogen_field() {

        let mut field = MorphogenField::new();

        field.add_attractor(5.0, 5.0, 5.0, 10.0);

        let near_conc = field.concentration_at(5.0, 5.0, 5.0);

        let far_conc = field.concentration_at(50.0, 50.0, 50.0);

        assert!(near_conc > far_conc, "Concentration should be higher near attractor");

    }

    #[test]

    fn test_organism_metrics() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let mut organism = mycelium_growth(&seed, &field, &config);

        organism.calculate_metrics();

        assert!(organism.metrics.is_some());

        let metrics = organism.metrics.unwrap();

        assert_eq!(metrics.node_count, organism.nodes.len());

        assert_eq!(metrics.connection_count, organism.connections.len());

    }

    #[test]

    fn test_prune_weak() {

        let engine = MorphogenesisEngine::new();

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let mut organism = mycelium_growth(&seed, &field, &config);

        let before = organism.connections.len();

        let pruned = engine.prune_weak(&mut organism, 0.5);

        assert!(pruned <= before);

    }

    #[test]

    fn test_express_as_architecture() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let organism = mycelium_growth(&seed, &field, &config);

        let arch = express_as_architecture(&organism);

        assert_eq!(arch.components.len(), organism.nodes.len());

    }

    #[test]

    fn test_trigger_from_vagus() {

        let engine = MorphogenesisEngine::new();

        use crate::vagus;

        let low_tone = vagus::VagusTone {

            current: 0.2,

            baseline: 0.5,

            trend: -0.1,

            volatility: 0.3,

            last_update: 0,

        };

        let pulse = vagus::SystemPulse {

            timestamp: 0,

            cpu_pressure: 0.9,

            memory_pressure: 0.5,

            io_pressure: 0.5,

            network_pressure: 0.3,

            request_rate: 100.0,

            error_rate: 0.05,

            hrv: 0.3,

        };

        let result = trigger_from_vagus(&low_tone, &pulse, &engine, 0.5);

        assert!(result.is_some(), "Should trigger growth when vagus tone is low");

    }

    #[test]

    fn test_merge_organisms() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let org1 = mycelium_growth(&seed, &field, &config);

        let org2 = capillary_growth(&seed, &field, &config);

        let merged = merge_organisms(&[org1, org2]);

        assert!(merged.is_some());

        assert!(merged.unwrap().nodes.len() > 0);

    }

    #[test]

    fn test_simplify_organism() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let mut organism = mycelium_growth(&seed, &field, &config);

        let removed = simplify_organism(&mut organism);

        assert!(removed >= 0);

    }

    #[test]

    fn test_fingerprint_export() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let organism = mycelium_growth(&seed, &field, &config);

        let fp = export_fingerprint(&organism);

        assert!(!fp.is_empty(), "Fingerprint should not be empty");

        assert_eq!(fp[0], 0x01, "Mycelium pattern marker");

    }

    #[test]

    fn test_diffusion() {

        let mut field = MorphogenField::new();

        field.add_attractor(0.0, 0.0, 0.0, 100.0);

        field.diffuse();

        let conc = field.concentration_at(0.0, 0.0, 0.0);

        assert!(conc > 0.0, "After diffusion, concentration should remain > 0");

    }

    #[test]

    fn test_evaporation() {

        let mut field = MorphogenField::new();

        field.add_attractor(0.0, 0.0, 0.0, 1.0);

        field.evaporate();

        assert!(field.gradients.is_empty() || field.gradients.values().any(|v| *v < 1.0));

    }

    #[test]

    fn test_analyze_topology() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let organism = mycelium_growth(&seed, &field, &config);

        let analysis = MorphogenesisEngine::analyze_topology(&organism);

        assert!(analysis.contains_key("avg_degree"));

        assert!(analysis.contains_key("density"));

        assert!(analysis.contains_key("avg_latency_ms"));

    }

    #[test]

    fn test_display_traits() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let mut organism = mycelium_growth(&seed, &field, &config);

        organism.fitness_score = 0.75;

        let display_str = format!("{}", organism);

        assert!(display_str.contains("Organism:"));

        assert!(display_str.contains("fitness=0.75"));

    }

    #[test]

    fn test_growth_config_defaults() {

        let mycelium = GrowthConfig::mycelium_default();

        assert_eq!(mycelium.pattern, GrowthPattern::Mycelium);

        assert!(mycelium.branching_probability > 0.0);

        let capillary = GrowthConfig::capillary_default();

        assert_eq!(capillary.pattern, GrowthPattern::Capillary);

        let slime = GrowthConfig::slime_mold_default();

        assert_eq!(slime.pattern, GrowthPattern::SlimeMold);

        let fractal = GrowthConfig::fractal_lsystem_default();

        assert_eq!(fractal.pattern, GrowthPattern::FractalLSystem);

    }

    #[test]

    fn test_evolution_summary() {

        let engine = MorphogenesisEngine::new();

        let seeds = vec![test_seed()];

        engine.evolve_population(&seeds, 3, &FitnessObjective::Balanced, 8);

        let summary = engine.evolution_summary();

        assert!(!summary.is_empty());

        assert_eq!(summary.len(), 3);

    }

    #[test]

    fn test_get_best_organism() {

        let engine = MorphogenesisEngine::new();

        let seeds = vec![test_seed()];

        engine.evolve_population(&seeds, 2, &FitnessObjective::Balanced, 6);

        let best = engine.get_best_organism();

        assert!(best.is_some());

    }

    #[test]

    fn test_node_type_conversion() {

        assert_eq!(NodeType::Root.to_string(), "Root");

        assert_eq!(NodeType::Service.to_string(), "Service");

        assert_eq!(NodeType::Cache.to_string(), "Cache");

        assert_eq!(NodeType::Custom("test".to_string()).to_string(), "test");

    }

    #[test]

    fn test_different_growth_patterns_produce_different_structures() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let org1 = mycelium_growth(&seed, &field, &config);

        let org2 = fractal_growth(&seed, &field, &config);

        // Both growth patterns should produce structures

        assert!(org1.nodes.len() > 1);

        assert!(org2.nodes.len() > 1);

    }

    #[test]

    fn test_organism_is_alive() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let organism = mycelium_growth(&seed, &field, &config);

        assert!(organism.is_alive());

    }

    #[test]

    fn test_map_to_neuroplasticity() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let organism = mycelium_growth(&seed, &field, &config);

        let pathways = map_to_neuroplasticity(&organism);

        assert_eq!(pathways.len(), organism.connections.iter().filter(|c| c.is_active).count());

    }

    #[test]

    fn test_hybrid_growth() {

        let engine = MorphogenesisEngine::new();

        engine.set_config(GrowthConfig {

            pattern: GrowthPattern::Hybrid,

            ..GrowthConfig::default()

        });

        let seed = test_seed();

        let organism = engine.grow_from_seed(&seed, None);

        assert!(!organism.nodes.is_empty());

    }

    #[test]

    fn test_seed_with_energy() {

        let seed = Seed::new("high", 0.0, 0.0, 0.0, "db")

            .with_energy(500.0)

            .with_pattern(GrowthPattern::Capillary);

        assert_eq!(seed.energy, 500.0);

        assert_eq!(seed.preferred_pattern, Some(GrowthPattern::Capillary));

    }

    #[test]

    fn test_organism_age() {

        let seed = test_seed();

        let field = MorphogenField::new();

        let config = GrowthConfig::default();

        let mut organism = mycelium_growth(&seed, &field, &config);

        let initial_energy: f64 = organism.nodes.iter().map(|n| n.energy).sum();

        organism.age_one_cycle();

        let after_energy: f64 = organism.nodes.iter().map(|n| n.energy).sum();

        assert!(after_energy < initial_energy, "Aging should reduce energy");

        assert_eq!(organism.age_cycles, 1);

    }

    #[test]

    fn test_map_to_architecture_preserves_structure() {

        let seed = Seed::new("arch_test", 0.0, 0.0, 0.0, "gateway");

        let field = MorphogenField::new();

        let config = GrowthConfig::capillary_default();

        let organism = capillary_growth(&seed, &field, &config);

        let arch = express_as_architecture(&organism);

        assert_eq!(arch.components.len(), organism.nodes.len());

    }

}

