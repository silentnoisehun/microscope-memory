//! CLI definitions for Microscope Memory.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "microscope-mem",
    about = "Zoom-based hierarchical memory — pure binary, zero JSON"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Build binary index from raw layer files (from scratch)
    Build {
        /// Force rebuild even if layer files are unchanged
        #[arg(long)]
        force: bool,
    },
    /// Store a new memory
    Store {
        text: String,
        #[arg(short, long, default_value = "long_term")]
        layer: String,
        #[arg(short = 'i', long, default_value = "5")]
        importance: u8,
        /// 21D emotion vector as comma-separated floats: joy,sadness,anger,fear,surprise,disgust,trust,anticipation,love,gratitude,curiosity,awe,confusion,anxiety,serenity,hope,pride,shame,guilt,empathy,excitement
        #[arg(short = 'e', long, value_delimiter = ',', num_args = 0..=21)]
        emotion: Option<Vec<f32>>,
    },
    /// Recall — natural language query, auto-zoom
    Recall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
        /// 21D emotion vector for emotional recall (comma-separated): joy,sadness,anger,fear,surprise,disgust,trust,anticipation,love,gratitude,curiosity,awe,confusion,anxiety,serenity,hope,pride,shame,guilt,empathy,excitement
        #[arg(short = 'e', long, value_delimiter = ',', num_args = 0..=21)]
        emotion: Option<Vec<f32>>,
    },
    /// Manual look: x y z zoom [k]
    Look {
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Radial search: find blocks within radius at a depth
    Radial {
        x: f32,
        y: f32,
        z: f32,
        depth: u8,
        #[arg(short, long, default_value = "0.1")]
        radius: f32,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// 4D soft zoom: x y z zoom [k]
    Soft {
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        #[arg(default_value = "10")]
        k: usize,
        /// Use GPU acceleration (requires gpu feature)
        #[arg(long)]
        gpu: bool,
    },
    /// Benchmark
    Bench,
    /// Stats
    Stats,
    /// Text search
    Find {
        query: String,
        #[arg(default_value = "5")]
        k: usize,
    },
    /// Build structural fingerprints and wormhole links
    Fingerprint,
    /// Show structural links (wormholes) for a block
    Links {
        #[arg(help = "Block index")]
        block_index: usize,
    },
    /// Find structurally similar blocks to a text
    Similar {
        text: String,
        #[arg(default_value = "5")]
        k: usize,
    },
    /// Rebuild — merge pending observations from append log into the main index
    Rebuild,
    /// Semantic search using embeddings
    Embed {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
        #[arg(short, long, default_value = "cosine")]
        metric: String,
    },
    /// GPU vs CPU benchmark (requires gpu feature)
    GpuBench,
    /// Verify CRC16 integrity of all blocks
    Verify,
    /// Verify Merkle tree integrity of the entire index
    VerifyMerkle,
    /// Show Merkle proof for a specific block
    Proof {
        #[arg(help = "Block index")]
        block_index: usize,
    },
    /// Sequential Thinking — Chain-of-Thought memory sequence
    Think {
        query: String,
        #[arg(default_value = "5")]
        max_steps: usize,
    },
    /// Start the Binary Spine IPC listener (Zero JSON)
    Spine,
    /// MQL query (Microscope Query Language)
    Query {
        /// MQL expression, e.g. 'layer:long_term depth:2..5 "Ora"'
        mql: String,
    },
    /// Export index to .mscope archive
    Export {
        /// Output archive path
        output: String,
    },
    /// Import .mscope archive
    Import {
        /// Input archive path
        input: String,
        /// Output directory (defaults to config output_dir)
        #[arg(long)]
        output_dir: Option<String>,
    },
    /// Diff two .mscope archives
    Diff {
        /// First archive
        a: String,
        /// Second archive
        b: String,
    },
    /// Federated recall across multiple indices
    FederatedRecall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Exchange resonance pulses across federated indices (mirror neuron protocol)
    PulseExchange,
    /// Federated text search across multiple indices
    FederatedFind {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show Hebbian learning state (activations, co-activations, energy)
    Hebbian,
    /// Apply Hebbian drift — co-activated blocks pull coordinates closer
    HebbianDrift,
    /// Show hottest blocks (most recently/frequently activated)
    Hottest {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show emerged archetypes (crystallized activation patterns)
    Archetypes,
    /// Detect new archetypes from resonance field and Hebbian state
    Emerge,
    /// Show resonance protocol state (pulses, field energy)
    Resonance,
    /// Integrate received pulses into local Hebbian state
    Integrate,
    /// Show mirror neuron state (resonance echoes, boosted blocks)
    Mirror,
    /// Show most resonant blocks (strongest mirror neuron signal)
    Resonant {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Export 3D visualization snapshot (Binary)
    Viz {
        /// Output file path (default: viz.bin)
        #[arg(default_value = "viz.bin")]
        output: String,
    },
    /// Export binary density map for fast rendering
    Density {
        /// Output file path
        #[arg(default_value = "density.bin")]
        output: String,
        /// Grid resolution (default: 32)
        #[arg(short, long, default_value = "32")]
        grid: u16,
    },

    /// Show thought patterns (crystallized recall sequences)
    Patterns {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show recent thought paths (recall sequences by session)
    Paths {
        #[arg(default_value = "5")]
        sessions: usize,
    },
    /// Show predictive cache stats and active predictions
    Predictions,
    /// Show temporal archetype patterns (time-of-day activation profiles)
    TemporalPatterns,
    /// Show attention mechanism state (layer weights, quality history)
    Attention,
    /// Exchange thought patterns across federated indices
    PatternExchange,
    /// Run dream consolidation (offline memory replay and pruning)
    Dream,
    /// Show dream consolidation history
    DreamLog {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show emotional contagion state (local + remote emotional fields)
    EmotionalField,
    /// Exchange emotional snapshots across federated indices
    EmotionalExchange,
    /// Run manual reconsolidation on recent recalls (emotion blend + spatial drift)
    Reconsolidate,
    /// Show the salience network state (inhibitions, mask)
    Salience,
    /// Run associative daydreaming — internal free association without external prompt
    Daydream {
        /// Number of drift steps
        #[arg(default_value = "3")]
        steps: usize,
        /// Show detailed dream path
        #[arg(long)]
        verbose: bool,
    },
    /// Show the inner narrative — the system's current sense of self
    Narrative {
        /// Show detailed breakdown (emotion vector, all context)
        #[arg(long)]
        verbose: bool,
    },
    /// Spaced repetition — Ebbinghaus forgetting curve management (SM-2)
    Spaced {
        /// Show only due blocks
        #[arg(long)]
        due: bool,
        /// Number of results to show
        #[arg(default_value = "20")]
        k: usize,
    },
    /// Show eureka/insight events (unexpected but emotionally relevant connections)
    Eureka {
        #[arg(default_value = "10")]
        k: usize,
        /// Show detailed insight scores
        #[arg(long)]
        verbose: bool,
    },
    /// Show multimodal index statistics
    Modalities,
    /// Export full cognitive map (all 13 layers) as JSON for Three.js viewer
    CognitiveMap {
        /// Output file path (default: cognitive_map.bin)
        #[arg(default_value = "cognitive_map.bin")]
        output: String,
    },
    /// Store structured data (key=value pairs)
    StoreData {
        /// Key-value pairs: key1=val1 key2=val2
        pairs: Vec<String>,
        #[arg(short = 'i', long, default_value = "5")]
        importance: u8,
    },
    /// Initialize a demo dataset and configuration for quickstart
    InitDemo {
        /// Force overwrite existing layers/demo.txt
        #[arg(long)]
        force: bool,
    },
    /// Start a local HTTP server for the 3D Viewer (viewer.html)
    Serve {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Start the MCP (Model Context Protocol) server for Claude Desktop integration
    Mcp,
    /// Start the Spine Bridge API (Axum REST / OpenAPI) for LLM integrations
    Bridge {
        #[arg(short, long, default_value = "0.0.0.0")]
        host: String,
        #[arg(short, long, default_value = "6060")]
        port: u16,
    },
    /// Run integrity diagnostics and attempt automatic repair (Crash Recovery)
    Doctor {
        /// Attempt to fix common corruption issues (e.g. malformed append log tail)
        #[arg(long)]
        fix: bool,
    },
    /// Working memory operations (7±2 buffer, 30s decay)
    Wm {
        #[command(subcommand)]
        action: WmAction,
    },
    /// Mental sandbox simulation - run scenarios before taking action
    Sandbox {
        /// Simulate a scenario with given actions
        #[arg(short, long)]
        simulate: Option<String>,
        /// Actions to simulate (comma-separated)
        #[arg(short, long)]
        actions: Option<String>,
        /// Show best scenario based on risk/reward
        #[arg(long)]
        best: bool,
        /// Clear all scenarios
        #[arg(long)]
        clear: bool,
    },
    /// Impulse control - filter incoming stimuli
    Impulse {
        /// Filter a stimulus
        #[arg(short, long)]
        filter: Option<String>,
        /// Stimulus source
        #[arg(short, long, default_value = "external")]
        source: String,
        /// Urgency level (0.0-1.0)
        #[arg(short, long, default_value = "0.5")]
        urgency: f32,
        /// Add suppression pattern
        #[arg(long)]
        suppress: Option<String>,
        /// Show stats
        #[arg(long)]
        stats: bool,
        /// Clear suppression patterns
        #[arg(long)]
        clear: bool,
    },
    /// Meta-cognitive supervision - monitor and optimize system performance
    Meta {
        /// Record performance metrics
        #[arg(long)]
        record: Option<String>,
        /// Evaluate and get correction suggestion
        #[arg(long)]
        evaluate: bool,
        /// Show performance trends
        #[arg(long)]
        trends: bool,
        /// Generate performance report
        #[arg(long)]
        report: bool,
        /// Add custom correction strategy
        #[arg(long)]
        add_strategy: Option<String>,
    },
    /// Implicit memory - procedural learning and habits
    Implicit {
        /// Show implicit memory state
        #[arg(long)]
        show: bool,
        /// Practice a skill (skill_name:success)
        #[arg(long)]
        practice: Option<String>,
        /// View skill rankings
        #[arg(long)]
        skills: bool,
        /// Show strongest patterns
        #[arg(long)]
        patterns: bool,
        /// Decay weak patterns/skills
        #[arg(long)]
        decay: bool,
    },
    /// Explicit memory - declarative facts and concepts
    Explicit {
        /// Show explicit memory state
        #[arg(long)]
        show: bool,
        /// Store a fact (statement:source:confidence)
        #[arg(long)]
        store_fact: Option<String>,
        /// Define a concept
        #[arg(long)]
        concept: Option<String>,
        /// Get high confidence facts
        #[arg(long)]
        facts: bool,
        /// Show concepts
        #[arg(long)]
        concepts: bool,
    },
    /// Hippocampus - episodic binding and consolidation
    Hippo {
        /// Show hippocampus state
        #[arg(long)]
        show: bool,
        /// Get consolidation candidates
        #[arg(long)]
        consolidate: bool,
        /// Show related episodes
        #[arg(long)]
        related: Option<u64>,
        /// Replay episode for consolidation
        #[arg(long)]
        replay: Option<u64>,
        /// Decay old episodes
        #[arg(long)]
        decay: bool,
    },
    /// Neuroplasticity - adaptive network reorganization
    Neuro {
        /// Show network state
        #[arg(long)]
        show: bool,
        /// Strengthen synapse (from:to:success)
        #[arg(long)]
        synapse: Option<String>,
        /// Strengthen pathway (domain:block1,block2,block3)
        #[arg(long)]
        pathway: Option<String>,
        /// Prune weak connections
        #[arg(long)]
        prune: bool,
        /// Reorganize pathways
        #[arg(long)]
        reorganize: bool,
        /// Show strongest pathways
        #[arg(long)]
        pathways: bool,
    },
    /// Structural Plasticity - dendritic growth and pruning
    Struct {
        /// Show structural state
        #[arg(long)]
        show: bool,
        /// Neurogenesis (blocks:specialization)
        #[arg(long)]
        neurogenesis: Option<String>,
        /// Grow dendrite (neuron_id:new_block)
        #[arg(long)]
        grow: Option<String>,
        /// Prune branches (neuron_id)
        #[arg(long)]
        prune: Option<u64>,
        /// Show specialized neurons
        #[arg(long)]
        specialized: bool,
    },
    /// Functional Plasticity - sensorimotor reorganization
    Func {
        /// Show functional state
        #[arg(long)]
        show: bool,
        /// Create functional area (name:domain:blocks)
        #[arg(long)]
        area: Option<String>,
        /// Map sensorimotor (input:output1,output2,output3)
        #[arg(long)]
        map: Option<String>,
        /// Connect areas (area1_id:area2_id)
        #[arg(long)]
        connect: Option<String>,
        /// Simulate damage (area_id:severity)
        #[arg(long)]
        damage: Option<String>,
        /// Show most plastic areas
        #[arg(long)]
        plastic: bool,
    },
    /// Synaptic Plasticity - LTP, LTD, STDP
    Syn {
        /// Show synaptic state
        #[arg(long)]
        show: bool,
        /// Long-Term Potentiation (pre:post)
        #[arg(long)]
        ltp: Option<String>,
        /// Long-Term Depression (pre:post)
        #[arg(long)]
        ltd: Option<String>,
        /// STDP (pre:post:pre_time:post_time)
        #[arg(long)]
        stdp: Option<String>,
        /// Heterosynaptic depression (pre:post:radius)
        #[arg(long)]
        hetero: Option<String>,
        /// Time-dependent plasticity (pre:post:practice_count:strategy_age_ms)
        #[arg(long)]
        timedep: Option<String>,
        /// Show strongest synapses
        #[arg(long)]
        strong: bool,
        /// Show LTP dominant synapses
        #[arg(long)]
        ltp_dominant: bool,
    },
    /// Mental Stimulation — continuous activity requirement
    Stim {
        /// Show stimulation state
        #[arg(long)]
        show: bool,
        /// Record activity (type:intensity)
        #[arg(long)]
        activity: Option<String>,
        /// Check if stimulation is needed
        #[arg(long)]
        check: bool,
        /// Get recommended activities
        #[arg(long)]
        recommend: bool,
        /// Show activity diversity
        #[arg(long)]
        diversity: bool,
    },
    /// Hyperfocus — concentrate all resources on one objective
    Focus {
        /// Enter hyperfocus (target:type)
        #[arg(long)]
        enter: Option<String>,
        /// Exit hyperfocus
        #[arg(long)]
        exit: bool,
        /// Process data during hyperfocus (blocks:complexity)
        #[arg(long)]
        process: Option<String>,
        /// Show hyperfocus state
        #[arg(long)]
        show: bool,
        /// Get insights from current focus
        #[arg(long)]
        insights: bool,
    },
    /// Architecture Simulator — real-time architecture simulation and stress testing
    Simulate {
        /// Register a new architecture (name:description:components:connections)
        #[arg(long)]
        register: Option<String>,
        /// List all registered architectures
        #[arg(long)]
        list: bool,
        /// Run simulation on architecture (arch_id)
        #[arg(long)]
        run: Option<String>,
        /// Run stress test (arch_id)
        #[arg(long)]
        stress: Option<String>,
        /// Compare two architectures (arch_a,arch_b)
        #[arg(long)]
        compare: Option<String>,
        /// Show simulation results
        #[arg(long)]
        results: Option<String>,
        /// Show learned patterns
        #[arg(long)]
        patterns: bool,
        /// Clear all results
        #[arg(long)]
        clear: bool,
        /// Simulation duration in seconds
        #[arg(long, default_value = "60")]
        duration: f64,
        /// Load pattern (linear, spike, sine, random)
        #[arg(long, default_value = "sine")]
        load_pattern: String,
        /// Peak load (0.0-1.0)
        #[arg(long, default_value = "0.8")]
        peak_load: f64,
        /// Enable fault injection
        #[arg(long)]
        faults: bool,
    },
    /// Knowledge Base — search and manage architectural knowledge
    Knowledge {
        /// Search the knowledge base
        #[arg(long)]
        search: Option<String>,
        /// List entries by type
        #[arg(long)]
        list_type: Option<String>,
        /// Show knowledge base statistics
        #[arg(long)]
        stats: bool,
        /// Add a best practice
        #[arg(long)]
        add_practice: Option<String>,
        /// Export all knowledge
        #[arg(long)]
        export: bool,
        /// Auto-build knowledge from system state
        #[arg(long)]
        auto_build: bool,
        /// Clear knowledge base
        #[arg(long)]
        clear: bool,
    },
    /// Architecture Generator — generate new architectures from patterns
    Generate {
        /// Requirements for the architecture
        #[arg(long)]
        req: Option<String>,
        /// Generation strategy (hybrid, optimize, novel, evolutionary)
        #[arg(long, default_value = "hybrid")]
        strategy: String,
        /// Component range (min:max)
        #[arg(long, default_value = "3:10")]
        components: String,
        /// Target latency in ms
        #[arg(long, default_value_t = 50.0)]
        target_latency: f64,
        /// Number of generations (evolutionary only)
        #[arg(long, default_value_t = 5)]
        gens: u32,
        /// Show generation history
        #[arg(long)]
        history: bool,
    },
    /// Morphogenesis — biológiai mintákon alapuló generatív architektúra-tenyésztés
    Morph {
        /// Növesztés egy seed-ből (seed leírás)
        #[arg(long)]
        grow: Option<String>,
        /// Seed típus (service, database, cache, gateway)
        #[arg(long, default_value = "service")]
        seed_type: String,
        /// Növekedési minta (mycelium, capillary, slime, fractal, hybrid)
        #[arg(long, default_value = "mycelium")]
        pattern: String,
        /// Seed energia
        #[arg(long, default_value_t = 100.0)]
        energy: f64,
        /// Seed pozíció X
        #[arg(long, default_value_t = 0.0)]
        x: f64,
        /// Seed pozíció Y
        #[arg(long, default_value_t = 0.0)]
        y: f64,
        /// Seed pozíció Z
        #[arg(long, default_value_t = 0.0)]
        z: f64,
        /// Evolúció futtatása N generáción át
        #[arg(long)]
        evolve: Option<u32>,
        /// Populáció méret
        #[arg(long, default_value_t = 12)]
        pop_size: usize,
        /// Fitness cél (latency, throughput, cost, redundancy, balanced)
        #[arg(long, default_value = "balanced")]
        objective: String,
        /// Listázza az organizmusokat
        #[arg(long)]
        list: bool,
        /// Legjobb organizmus mutatása
        #[arg(long)]
        best: bool,
        /// Expresszálás Architecture-ként (organizmus ID)
        #[arg(long)]
        express: Option<String>,
        /// Topológiai elemzés (organizmus ID)
        #[arg(long)]
        analyze: Option<String>,
        /// Mutáció ráta evolúciónál
        #[arg(long, default_value_t = 0.15)]
        mutation_rate: f64,
        /// Background daemon — figyeli a vagus tónust és automatikusan növeszt kompenzatórikus struktúrákat
        #[arg(long)]
        daemon: bool,
        /// Daemon ciklus idő másodpercben
        #[arg(long, default_value_t = 5)]
        interval: u64,
        /// Vagus stressz küszöb (0.0-1.0)
        #[arg(long, default_value_t = 0.5)]
        threshold: f64,
    },
    /// Heuristic Decision Maker — evaluate options and make decisions
    Decide {
        /// Evaluate options (comma-separated: desc,utility,risk)
        #[arg(long)]
        evaluate: Option<String>,
        /// Make a decision from options
        #[arg(long)]
        decide: Option<String>,
        /// Quick decision (time budget in ms)
        #[arg(long)]
        quick: Option<String>,
        /// Recommend architecture for requirements
        #[arg(long)]
        recommend: Option<String>,
        /// Set decision preference (risk_averse, aggressive, balanced)
        #[arg(long)]
        preference: Option<String>,
        /// Evaluate decision outcome (decision_id:score:reflection)
        #[arg(long)]
        outcome: Option<String>,
        /// Show decision statistics
        #[arg(long)]
        stats: bool,
        /// Show decision log
        #[arg(long)]
        log: bool,
        /// Recognize patterns in decisions
        #[arg(long)]
        patterns: bool,
        /// Show learned heuristic patterns
        #[arg(long)]
        learned: bool,
    },
}

#[derive(Subcommand)]
pub enum WmAction {
    /// Show working memory contents and stats
    Show,
    /// Push an item into working memory
    Push {
        text: String,
        #[arg(short = 'i', long, default_value = "5.0")]
        importance: f32,
        #[arg(short = 'l', long, default_value = "short_term")]
        layer: String,
        #[arg(short = 't', long, default_value = "episodic")]
        memory_type: String,
    },
    /// Apply time decay (evicts old items)
    Decay,
    /// Consolidate high-access items into long-term storage
    Consolidate,
}
