with open("src/cli.rs","r",encoding="utf-8") as f:
    c = f.read()
old = "    /// Monologue - generate an inner monologue (the system thinking)\n    Monologue,\n}"
new = """    /// Monologue - generate an inner monologue (the system thinking)
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
}"""
c = c.replace(old, new, 1)
with open("src/cli.rs","w",encoding="utf-8") as f:
    f.write(c)
print("CLI commands added")
