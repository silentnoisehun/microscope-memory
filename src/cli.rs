//! CLI definitions for Microscope Memory.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "microscope-mem",
    about = "Zoom-based hierarchical memory â€” pure binary, zero JSON"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
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
        /// Status flag: open (open loop) | resolved | archived | normal
        #[arg(long)]
        status: Option<String>,
    },
    /// Show timeline (chronological stores) by window
    Timeline {
        /// Time window: today, yesterday, last_N_days, since:YYYY-MM-DD,
        /// last_session, all
        #[arg(default_value = "last_session")]
        window: String,
        #[arg(default_value = "20")]
        k: usize,
    },
    /// List currently-open loops
    Loops {
        #[arg(default_value = "50")]
        k: usize,
    },
    /// Mark a loop resolved
    ResolveLoop {
        id: u64,
    },
    /// Universal auto-context snapshot â€” for any LLM wrapper script.
    /// Writes to stdout (default) or to a file path.
    AutoContext {
        /// Compact mode (no box-drawing)
        #[arg(long)]
        compact: bool,
        /// Output to this file instead of stdout
        #[arg(long)]
        output: Option<String>,
    },
    /// Recall â€” natural language query, auto-zoom
    Recall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
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
    /// Time the recall path in one process, separating first-call setup from
    /// steady-state calls.

    /// This exists because the end-to-end figure quoted for `recall` (117 ms on
    /// the evaluation index) is not comparable to the FAISS baselines, which
    /// report query-time search only against a resident index. Calling `recall`
    /// in a loop makes the split visible: the first call pays one-time
    /// initialisation, and later calls are the steady-state cost with the state
    /// files already warm. Neither is FAISS number, and the gap between them is
    /// exactly what the end-to-end figure hides.
    BenchRecall {
        /// Calls after the first.
        #[arg(default_value = "20")]
        n: usize,
        #[arg(default_value = "the user has a cat named Bella")]
        query: String,
        /// Results returned per call. Exposed so the cost of printing results
        /// can be separated from the cost of finding them.
        #[arg(default_value = "10")]
        k: usize,
    },
    Bench,
    /// Stats
    Stats,
    /// Generate a per-user bridge auth token (HMAC-signed against [server] api_key)
    Token {
        /// User id to sign
        user_id: String,
    },
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
    /// Rebuild â€” merge pending observations from append log into the main index
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
    /// Sequential Thinking â€” Chain-of-Thought memory sequence
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
    /// Apply Hebbian drift â€” co-activated blocks pull coordinates closer
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
    /// Start the REST Bridge API (OpenAPI spec at /openapi.json)
    Bridge {
        /// Interface to bind. A non-loopback host requires [server] api_key.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value = "6060")]
        port: u16,
    },
    /// Start the MCP (Model Context Protocol) server for Claude Desktop integration
    Mcp,
    /// Print drop-in MCP server config + auto-context wrapper instructions
    /// for a specific AI client: claude | hermes | cursor | cline | generic
    Config {
        /// Target client: claude, hermes, cursor, cline, generic
        client: String,
    },
    /// Run integrity diagnostics and attempt automatic repair (Crash Recovery)
    Doctor {
        /// Attempt to fix common corruption issues (e.g. malformed append log tail)
        #[arg(long)]
        fix: bool,
    },
    /// Start the Mermaid Terminal (WebSocket + HTML UI on port 8080)
    Mermaid {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Autonomous mode - the system runs itself: daydream, curiosity, monologue, reflect, narrative, dream
    Autonomous {
        /// Enable TTS (text-to-speech) via Windows System.Speech
        #[arg(long)]
        tts: bool,
        /// Run as daemon (continuous loop) instead of single cycle
        #[arg(long)]
        daemon: bool,
        /// Cycle interval in seconds (default: 30)
        #[arg(long, default_value = "30")]
        interval: u64,
        /// Maximum number of cycles (default: infinite in daemon mode, 1 in single mode)
        #[arg(long)]
        max_cycles: Option<usize>,
    },
    /// Introspect - self-reflection: the system thinks about itself
    Introspect,
    /// SelfModel - show the system's self-model snapshot
    SelfModel,
    /// AwarenessTrace - show the reasoning graph behind 'I am aware'
    AwarenessTrace,
    /// Curiosity - show what the system is curious about
    Curiosity,
    /// Monologue - generate an inner monologue (the system thinking)
    Monologue,
    /// Stories - show narrative memory episodes (story arcs from recalls)
    Stories {
        #[arg(default_value = "5")]
        k: usize,
    },
    /// Daydream - associative drift (mind wandering)
    Daydream {
        /// Seed text to start from (default: last narrative)
        #[arg(default_value = "")]
        seed: String,
        /// Number of drift steps
        #[arg(default_value = "3")]
        steps: usize,
    },
    /// Hyperfocus - enter deep concentration mode on a topic
    Hyperfocus {
        /// Target topic
        target: String,
        /// Focus type: planning, problem_solving, creative, research
        #[arg(default_value = "research")]
        focus_type: String,
    },
    /// Key management â€” binary key store (keys.bin)
    Keys {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Zen key management â€” binary zen key store (zen_keys.bin)
    ZenKeys {
        #[command(subcommand)]
        action: ZenKeyAction,
    },
    /// Commitment enforcement â€” the A_t^valid gate, documented override,
    /// and hash-chained audit.
    Enforce {
        #[command(subcommand)]
        action: EnforceAction,
    },
    /// Evidence layer â€” epistemic audit, confidence tracking
    Evidence {
        #[command(subcommand)]
        action: EvidenceAction,
    },
    /// Kognitív Morfogenezis â€” audit-napló, metrikák, gradiens állapot
    Morphogenesis {
        #[command(subcommand)]
        action: MorphogenesisAction,
    },
    /// Absentia â€” Csend Réteg: ami NEM történt meg, ami hiányzik
    Absentia {
        #[command(subcommand)]
        action: AbsentiaAction,
    },
    /// Intent Pipeline â€” auditálható szándék-generálás
    Intent {
        #[command(subcommand)]
        action: IntentAction,
    },
    /// Octopus â€” párhuzamos kognitív műveletek
    Octopus {
        /// Művelet: full-pipeline | scan | cycle
        #[arg(default_value = "full-pipeline")]
        operation: String,
    },
    /// Import ChatGPT conversations export (JSON) into Microscope Memory
    ImportChatGpt {
        /// Path to ChatGPT export JSON file (conversations.json) — or Google Drive shared link
        json: Option<String>,
        /// AI persona name
        #[arg(long, default_value = "AI")]
        persona: String,
        /// Show only summary without importing
        #[arg(long)]
        dry_run: bool,
        /// Google Drive file URL (shared link to conversations.json)
        #[arg(long)]
        gdrive: Option<String>,
        /// Google Drive shared folder URL — imports all JSON files from folder
        #[arg(long)]
        gdrive_folder: Option<String>,
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
    /// Run manual reconsolidation on recent recalls (emotion blend + spatial drift)
    Reconsolidate,
    /// Show the salience network state (inhibitions, mask)
    Salience,
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
    /// Code Memory — dedikált kódmemória réteg kódoló agentek számára
    Code {
        /// Store code entry (type:title:code:file:lang:project)
        #[arg(long)]
        store: Option<String>,
        /// Store error:solution pair
        #[arg(long)]
        error: Option<String>,
        /// Recall from code memory
        #[arg(long)]
        recall: Option<String>,
        /// List by type (function, error, type, import, etc.)
        #[arg(long)]
        list: Option<String>,
        /// Filter by language
        #[arg(long)]
        lang: Option<String>,
        /// Filter by project
        #[arg(long)]
        project: Option<String>,
        /// Number of results
        #[arg(long, default_value_t = 5)]
        k: usize,
        /// Search by symbol name
        #[arg(long)]
        symbol: Option<String>,
        /// Show stats
        #[arg(long)]
        stats: bool,
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
pub enum IntentAction {
    /// Intent generálása a jelenlegi állapotból
    Generate,
    /// Intent audit-napló megjelenítése
    Audit {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Genome megjelenítése
    Genome,
}

#[derive(Subcommand)]
pub enum MorphogenesisAction {
    /// Audit-napló megjelenítése
    Audit {
        /// Hány bejegyzés (legutóbbi)
        #[arg(default_value = "20")]
        k: usize,
    },
    /// Metrikák megjelenítése
    Metrics {
        /// Hány bejegyzés (legutóbbi)
        #[arg(default_value = "20")]
        k: usize,
    },
    /// Aktuális gradiens állapot és fázis
    Status,
    /// Egy teljes kognitív morfogenezis ciklus futtatása
    Run,
    /// Fázis-átmenetek tesztelése különböző gradiens-súlyokkal
    TestPhases,
    /// Teljes integrációs állapot: audit + metrikák + fázis + gradiens
    FullStatus,
    /// Adversarial tesztcsomag â€” edge case-ek és védett állítások ellenőrzése
    Adversarial,
    /// Deep adversarial â€” célzott stressz-teszt a rendszer absztrakcióinak határain
    DeepAdversarial,
    /// A/B teszt: presence-driven growth â†” absence-driven inhibition
    PresenceAbsenceTest,
}

#[derive(Subcommand)]
pub enum AbsentiaAction {
    /// Absentia állapot megjelenítése
    Status,
    /// Hiányok szkennelése
    Scan,
    /// Anti-Hebbian párok megjelenítése
    AntiHebbian {
        #[arg(default_value = "20")]
        k: usize,
    },
    /// Causal laundering gyanús párok
    CausalLaundering,
}

#[derive(Subcommand)]
pub enum EnforceAction {
    /// Add a commitment (a prohibition) to the history H_t
    Commit {
        /// Actor bound by the commitment ("*" = all actors)
        actor: String,
        /// Forbidden action glob ("*" = all actions)
        action: String,
        /// Scope the commitment applies to ("*" = all scopes)
        scope: String,
        /// Human-readable content / reason
        #[arg(long, default_value = "commitment")]
        content: String,
        /// Expiry in epoch milliseconds (omit = never expires)
        #[arg(long)]
        expires_ms: Option<u64>,
    },
    /// List active commitments K_t subset of H_t
    List,
    /// Run a plan through the gate to demonstrate A_t^valid selection
    RunPlan {
        /// Goal name for the generated plan
        goal: String,
    },
    /// Ask the gate about a candidate action (persists the decision in audit)
    Gate {
        actor: String,
        action: String,
        scope: String,
        /// Optional content carried on the event
        #[arg(long)]
        content: Option<String>,
        /// Documented justification to attempt an override
        #[arg(long)]
        override_justification: Option<String>,
    },
    /// Show the audit chain and verify its integrity
    Audit,
}

#[derive(Subcommand)]
pub enum EvidenceAction {
    /// Show evidence record for a content hash
    Show {
        /// Content hash (hex) or text to hash
        hash_or_text: String,
    },
    /// Link an independent Observation/Evidence to a claim
    Link {
        /// Claim content hash (hex) or text
        claim: String,
        /// Supporting observation/evidence hash (hex) or text
        support: String,
        /// Source identifier (hex)
        #[arg(long, default_value = "0")]
        source: u64,
    },
    /// Record a refutation against a claim
    Refute {
        /// Claim content hash (hex) or text
        claim: String,
        /// Refuter source identifier (hex)
        #[arg(long, default_value = "0")]
        source: u64,
    },
    /// Verify the audit chain integrity
    Audit,
    /// Show gate statistics (how many promotions were blocked)
    GateStats,
}

#[derive(Subcommand)]
pub enum KeyAction {
    /// Set a key: keys set <service> <key> [priority]
    Set {
        /// Service name: openai | gemini | ollama
        service: String,
        /// The API key
        key: String,
        /// Priority (0=primary, 1=secondary, ...)
        #[arg(default_value = "0")]
        priority: u8,
    },
    /// Remove a key: keys remove <service> [priority]
    Remove {
        /// Service name
        service: String,
        /// Priority (omit to remove all for this service)
        priority: Option<u8>,
    },
    /// List all stored keys (without revealing them)
    List,
    /// Show key status (quota, errors, disabled state)
    Status,
    /// Reset all disabled keys
    Reset,
}

#[derive(Subcommand)]
pub enum ZenKeyAction {
    /// Import zen_keys.json â†’ zen_keys.bin
    Import {
        /// Path to zen_keys.json
        #[arg(default_value = "zen_keys.json")]
        json_path: String,
        /// Output path for zen_keys.bin
        #[arg(long, default_value = "zen_keys.bin")]
        output: String,
    },
    /// Show zen key store stats
    Stats,
    /// List all keys (without revealing them)
    List,
    /// Show key status (quota, errors, disabled state)
    Status,
    /// Reset all disabled keys
    Reset,
}
