# Cognitive Enhancement Modules for Microscope Memory

Three new modules have been added to enhance the cognitive capabilities of the system:

## 1. Mental Sandbox (`mental_sandbox.rs`)

**Purpose**: Simulate scenarios before taking real action (like a mental playground).

**Features**:
- Create and evaluate multiple action scenarios
- Calculate risk/reward ratios
- Score scenarios based on alignment with long-term goals
- Parallel simulation support

**Usage**:
```bash
# Simulate a scenario
microscope-mem sandbox --simulate "Implement new feature" --actions "design,code,test,deploy"

# Show best scenario
microscope-mem sandbox --best

# Clear all scenarios
microscope-mem sandbox --clear
```

## 2. Impulse Control (`impulse_control.rs`)

**Purpose**: Filter incoming stimuli and suppress irrelevant thoughts for better focus.

**Features**:
- Content filtering based on relevance scoring
- Suppression pattern system (keywords to automatically block)
- Attention budget management
- Long-term goal alignment checking

**Usage**:
```bash
# Filter a stimulus
microscope-mem impulse --filter "New email notification" --urgency 0.7

# Add suppression pattern
microscope-mem impulse --suppress "spam"

# Show stats
microscope-mem impulse --stats

# Clear patterns
microscope-mem impulse --clear
```

## 3. Meta-Supervision (`meta_supervision.rs`)

**Purpose**: Continuously monitor system performance and apply corrections.

**Features**:
- Performance metrics tracking and scoring
- Trend analysis and volatility calculation
- Automatic correction strategies
- Performance threshold monitoring (warning/alert/critical)

**Usage**:
```bash
# Record performance metrics
microscope-mem meta --record "50,100,0.8,0.5,0.1"

# Evaluate and get correction suggestions
microscope-mem meta --evaluate

# Show performance trends
microscope-mem meta --trends

# Generate full report
microscope-mem meta --report

# Add custom correction strategy
microscope-mem meta --add-strategy "optimize_workflow"
```

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