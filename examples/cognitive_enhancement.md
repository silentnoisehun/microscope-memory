# Cognitive Enhancement Modules for Microscope Memory

Three new modules have been added to enhance the cognitive capabilities of the system:

## 1. Mental Sandbox (`mental_sandbox.rs`)

**Purpose**: Simulate scenarios before taking real action (like a mental playground).

**Features**:
- Create and evaluate multiple action scenarios
- Calculate risk/reward ratios
- Score scenarios based on alignment with long-term goals
- Parallel simulation support

**Usage**: none from the command line. The `sandbox` subcommand was removed in
`a962ad1` and has not been replaced; the module is still compiled into the crate
and reachable through `lib.rs`. The removed invocations were `sandbox --simulate`,
`sandbox --best` and `sandbox --clear`.

## 2. Impulse Control (`impulse_control.rs`)

**Purpose**: Filter incoming stimuli and suppress irrelevant thoughts for better focus.

**Features**:
- Content filtering based on relevance scoring
- Suppression pattern system (keywords to automatically block)
- Attention budget management
- Long-term goal alignment checking

**Usage**: none from the command line. The `impulse` subcommand was removed in
`a962ad1` and has not been replaced; the module is still compiled into the crate.
The removed invocations were `impulse --filter`, `impulse --suppress`,
`impulse --stats` and `impulse --clear`.

## 3. Meta-Supervision (`meta_supervision.rs`)

**Purpose**: Continuously monitor system performance and apply corrections.

**Features**:
- Performance metrics tracking and scoring
- Trend analysis and volatility calculation
- Automatic correction strategies
- Performance threshold monitoring (warning/alert/critical)

**Usage**: none from the command line. The `meta` subcommand was removed in
`a962ad1` and has not been replaced; the module is still compiled into the crate.
The removed invocations were `meta --record`, `meta --evaluate`, `meta --trends`,
`meta --report` and `meta --add-strategy`.

## Integration Examples

### Scenario Planning with Meta-Supervision:
```rust
use microscope_memory::mental_sandbox::MentalSandbox;
use microscope_memory::meta_supervision::MetaSupervisor;

let mut sandbox = MentalSandbox::new();
sandbox.add_goal("efficient_workflow");
sandbox.add_goal("high_quality_output");

let mut supervisor = MetaSupervisor::new();

// Simulate different approaches
let scenario1 = sandbox.simulate_scenario(
    "Implement with tests first",
    vec!["write_tests", "implement", "refactor"]
);

let scenario2 = sandbox.simulate_scenario(
    "Quick prototype then refine",
    vec!["prototype", "test", "refine", "document"]
);

// Use meta-supervision to evaluate which approach is better
supervisor.record_metrics(100.0, 150.0, 0.9, 0.6, 0.05);
```

### Impulse Control for Focus Management:
```rust
use microscope_memory::impulse_control::ImpulseControl;

let mut control = ImpulseControl::new();
control.add_goal("complete_feature_x");
control.add_suppression_pattern("social_media");
control.add_suppression_pattern("unimportant_notifications");

// Filter incoming stimuli
let stimulus1 = control.filter_stimulus(
    "Reminder: meeting in 30 minutes",
    "calendar",
    0.8
);

let stimulus2 = control.filter_stimulus(
    "New tweet from followed account",
    "twitter",
    0.3
);

// Stimulus2 will be suppressed due to pattern matching
```

## Benefits

1. **Better Decision Making**: Mental sandbox allows testing scenarios before commitment
2. **Improved Focus**: Impulse control filters distractions automatically
3. **Self-Optimization**: Meta-supervision ensures the system learns and improves over time
4. **Goal Alignment**: All modules work together toward long-term objectives

These modules work alongside the existing 13 cognitive layers to create a more complete artificial cognitive system.