//! Sequential Thinking — Chain-of-Thought layer for Microscope Memory.
//! This module organizes memory retrievals into logical sequences (Steps).

use crate::config::Config;
use crate::reader::MicroscopeReader;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ThoughtStep {
    pub thought_number: usize,
    pub total_thoughts: usize,
    pub content: String,
    pub is_revision: bool,
    pub revises_thought: Option<usize>,
    pub timestamp: Instant,
}

pub struct ThinkingChain {
    pub steps: Vec<ThoughtStep>,
    pub max_steps: usize,
    pub next_thought_needed: bool,
}

impl ThinkingChain {
    pub fn new(max_steps: usize) -> Self {
        Self {
            steps: Vec::new(),
            max_steps,
            next_thought_needed: true,
        }
    }

    /// Appends a new thought to the sequence.
    pub fn add_step(&mut self, content: String, is_revision: bool, revises: Option<usize>) {
        let step_num = self.steps.len() + 1;
        self.steps.push(ThoughtStep {
            thought_number: step_num,
            total_thoughts: self.max_steps,
            content,
            is_revision,
            revises_thought: revises,
            timestamp: Instant::now(),
        });

        if step_num >= self.max_steps {
            self.next_thought_needed = false;
        }
    }

    /// Evaluates the Hebbian field to determine the next logical memory block.
    pub fn brainstorm(&mut self, reader: &MicroscopeReader, config: &Config, initial_query: &str) {
        println!("🧠 Sequential Thinking: Processing '{}'...", initial_query);

        // Step 1: Initial search (text-based fallback for the seed)
        let results = reader.find_text(initial_query, 5);
        if let Some(&(_depth, idx)) = results.first() {
            self.add_step(reader.text(idx).to_string(), false, None);
        }

        // Step 2-N: Iterative refinement based on previous step using spatial radial search
        while self.next_thought_needed && self.steps.len() < self.max_steps {
            let last_idx = if let Some(last) = self.steps.last() {
                // We'd ideally need the index, but let's re-search for now
                let text = &last.content;
                reader
                    .find_text(text, 1)
                    .first()
                    .map(|&(_, i)| i)
                    .unwrap_or(0)
            } else {
                break;
            };

            let h = reader.header(last_idx);

            // Radial search to find similar/nearby memories in the Hebbian field (spatial mmap)
            let result_set = reader.radial_search(
                config,
                h.x,
                h.y,
                h.z,
                h.depth,
                config.search.zoom_weight * 0.5, // radius
                10,                              // k
            );

            let retrieved: Vec<String> = result_set
                .all()
                .iter()
                .map(|r| reader.text(r.block_idx).to_string())
                .collect();

            if let Some(next_text) = retrieved.get(1) {
                // 2nd best as a "new branch/association"
                self.add_step(next_text.clone(), false, None);
            } else {
                self.next_thought_needed = false;
            }
        }
    }

    pub fn display(&self) {
        for step in &self.steps {
            let prefix = if step.is_revision {
                "🔄 REVISION"
            } else {
                "💭 THOUGHT"
            };
            println!(
                "[{}/{}] {} ({}): {}",
                step.thought_number,
                step.total_thoughts,
                prefix,
                step.revises_thought
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "new".to_string()),
                step.content
            );
        }
    }
}
