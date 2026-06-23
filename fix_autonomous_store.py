content = open("src/autonomous.rs", "r", encoding="utf-8").read()

# Update import
content = content.replace(
    "use crate::reader::{store_memory, MicroscopeReader};",
    "use crate::reader::{store_memory, store_memory_temporary, MicroscopeReader};"
)

# Update store_result function
old = """    fn store_result(&self, config: &Config, text: &str, layer: &str, importance: u8) {
        if let Err(e) = store_memory(config, text, layer, importance) {
            eprintln!("  {} Store error: {}", "ERROR:".red(), e);
        }
    }"""

new = """    fn store_result(&self, config: &Config, text: &str, layer: &str, importance: u8) {
        // Internal thoughts: only to append log + timeline, NOT to layer files
        // (they would accumulate and never be forgotten)
        if let Err(e) = store_memory_temporary(config, text, layer, importance) {
            eprintln!("  {} Store error: {}", "ERROR:".red(), e);
        }
    }"""

content = content.replace(old, new)

open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK")
