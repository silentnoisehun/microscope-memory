//! Microscope Hooks - thin orchestration layer for LLM lifecycle events.
//!
//! The model is only the motor.
//! Hooks are the nervous system.
//! Microscope Memory is the memory.
//!
//! Six lifecycle hooks:
//! - on_session_start  - load identity, project context, constraints
//! - before_prompt     - inspect request, search memory, build context, inject contract
//! - before_tool_call  - retrieve task memory, check constraints, add project context
//! - after_tool_call   - summarize result, detect facts, prepare memory candidate
//! - after_response   - extract durable memory, assign layer/importance, write if enabled
//! - on_error         - store error trace, associate with task, expose recovery context

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("Hook '{0}' execution failed: {1}")]
    ExecutionFailed(String, String),
    #[error("Hook '{0}' not registered")]
    NotRegistered(String),
    #[error("Memory store disabled by configuration")]
    StoreDisabled,
    #[error("Write operations disabled by configuration")]
    WriteDisabled,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type HookResult<T> = std::result::Result<T, HookError>;


// ── HookEvent ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    SessionStart,
    BeforePrompt,
    BeforeToolCall,
    AfterToolCall,
    AfterResponse,
    Error,
}

impl HookEvent {
    pub fn label(&self) -> &'static str {
        match self {
            HookEvent::SessionStart => "on_session_start",
            HookEvent::BeforePrompt => "before_prompt",
            HookEvent::BeforeToolCall => "before_tool_call",
            HookEvent::AfterToolCall => "after_tool_call",
            HookEvent::AfterResponse => "after_response",
            HookEvent::Error => "on_error",
        }
    }

    pub fn all() -> [HookEvent; 6] {
        [
            HookEvent::SessionStart,
            HookEvent::BeforePrompt,
            HookEvent::BeforeToolCall,
            HookEvent::AfterToolCall,
            HookEvent::AfterResponse,
            HookEvent::Error,
        ]
    }
}

// ── HookContext ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub chain_id: String,
    pub event: HookEvent,
    pub timestamp_ms: u64,
    pub query: Option<String>,
    pub response: Option<String>,
    pub tool_name: Option<String>,
    pub tool_args: Option<serde_json::Value>,
    pub tool_result: Option<String>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub project_context: Option<String>,
    pub constraints: Vec<String>,
    pub memory_contract: Option<String>,
    pub retrieved_memories: Vec<MemoryBlockRef>,
    pub memory_candidates: Vec<MemoryCandidate>,
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    pub fn new(event: HookEvent) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            chain_id: uuid::Uuid::new_v4().to_string(),
            event,
            timestamp_ms: now,
            query: None,
            response: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            error_message: None,
            error_code: None,
            project_context: None,
            constraints: Vec::new(),
            memory_contract: None,
            retrieved_memories: Vec::new(),
            memory_candidates: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_query(mut self, query: &str) -> Self {
        self.query = Some(query.to_string());
        self
    }

    pub fn with_response(mut self, response: &str) -> Self {
        self.response = Some(response.to_string());
        self
    }

    pub fn with_tool(mut self, name: &str, args: serde_json::Value) -> Self {
        self.tool_name = Some(name.to_string());
        self.tool_args = Some(args);
        self
    }

    pub fn with_error(mut self, message: &str, code: &str) -> Self {
        self.error_message = Some(message.to_string());
        self.error_code = Some(code.to_string());
        self
    }
}

// ── MemoryBlockRef ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBlockRef {
    pub id: String,
    pub layer: String,
    pub depth: u8,
    pub importance: u8,
    pub snippet: String,
    pub distance: f32,
}

// ── MemoryCandidate ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub text: String,
    pub layer: String,
    pub importance: u8,
    pub source_event: HookEvent,
    pub source_tool: Option<String>,
    pub confidence: f32,
    pub is_error: bool,
    pub is_task: bool,
    pub association_id: Option<String>,
}

// ── HookConfig ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub enabled_hooks: HashMap<HookEvent, bool>,
    pub write: WriteConfig,
    pub read_only: bool,
    pub max_candidates: usize,
    pub min_importance: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteConfig {
    pub enabled: bool,
    pub require_confirmation: bool,
    pub filter_secrets: bool,
    pub filter_api_keys: bool,
}

impl Default for HookConfig {
    fn default() -> Self {
        let mut enabled = HashMap::new();
        enabled.insert(HookEvent::SessionStart, true);
        enabled.insert(HookEvent::BeforePrompt, true);
        enabled.insert(HookEvent::BeforeToolCall, true);
        enabled.insert(HookEvent::AfterToolCall, true);
        enabled.insert(HookEvent::AfterResponse, false);
        enabled.insert(HookEvent::Error, true);
        Self {
            enabled_hooks: enabled,
            write: WriteConfig {
                enabled: false,
                require_confirmation: true,
                filter_secrets: true,
                filter_api_keys: true,
            },
            read_only: false,
            max_candidates: 5,
            min_importance: 3,
        }
    }
}

impl HookConfig {
    pub fn read_only() -> Self {
        let mut config = Self::default();
        config.read_only = true;
        config.write.enabled = false;
        config.enabled_hooks.insert(HookEvent::AfterResponse, false);
        config
    }

    pub fn full() -> Self {
        let mut config = Self::default();
        config.write.enabled = true;
        config.enabled_hooks.insert(HookEvent::AfterResponse, true);
        config
    }

    pub fn is_enabled(&self, event: &HookEvent) -> bool {
        *self.enabled_hooks.get(event).unwrap_or(&false)
    }

    pub fn can_write(&self) -> bool {
        !self.read_only && self.write.enabled
    }
}

// ── HookHandler trait ────────────────────────────────────────────────────────

pub trait HookHandler: Send + Sync {
    fn event(&self) -> HookEvent;
    fn execute(&self, ctx: HookContext) -> HookContext;
}

// ── Default Handlers ─────────────────────────────────────────────────────────

pub fn default_on_session_start(mut ctx: HookContext) -> HookContext {
    ctx.memory_contract = Some(include_str!("contract.txt").to_string());
    ctx.constraints.push("Never store secrets or API keys".to_string());
    ctx.constraints.push("Memory is authoritative - never invent memories".to_string());
    ctx
}

pub fn default_before_prompt(mut ctx: HookContext) -> HookContext {
    if ctx.memory_contract.is_none() {
        ctx.memory_contract = Some(include_str!("contract.txt").to_string());
    }
    ctx
}

pub fn default_before_tool_call(mut ctx: HookContext) -> HookContext {
    if let Some(project) = &ctx.project_context {
        ctx.metadata.insert("project_context_active".to_string(), "true".to_string());
    }
    ctx
}

pub fn default_after_tool_call(mut ctx: HookContext) -> HookContext {
    if let Some(result) = &ctx.tool_result {
        if !result.is_empty() && result.len() > 20 {
            let tool = ctx.tool_name.clone().unwrap_or_default();
            ctx.memory_candidates.push(MemoryCandidate {
                text: format!("[Tool: {}] {}", tool, &result.chars().take(500).collect::<String>()),
                layer: "session".to_string(),
                importance: 4,
                source_event: HookEvent::AfterToolCall,
                source_tool: ctx.tool_name.clone(),
                confidence: 0.6,
                is_error: false,
                is_task: true,
                association_id: None,
            });
        }
    }
    ctx
}

pub fn default_after_response(mut ctx: HookContext) -> HookContext {
    if let (Some(query), Some(response)) = (&ctx.query, &ctx.response) {
        if response.len() > 30 {
            ctx.memory_candidates.push(MemoryCandidate {
                text: format!("Q: {} | A: {}", query, &response.chars().take(300).collect::<String>()),
                layer: "session".to_string(),
                importance: 3,
                source_event: HookEvent::AfterResponse,
                source_tool: None,
                confidence: 0.5,
                is_error: false,
                is_task: false,
                association_id: None,
            });
        }
    }
    ctx
}

pub fn default_on_error(mut ctx: HookContext) -> HookContext {
    if let (Some(msg), Some(code)) = (&ctx.error_message, &ctx.error_code) {
        ctx.memory_candidates.push(MemoryCandidate {
            text: format!("ERROR [{}]: {}", code, msg),
            layer: "session".to_string(),
            importance: 7,
            source_event: HookEvent::Error,
            source_tool: ctx.tool_name.clone(),
            confidence: 0.9,
            is_error: true,
            is_task: true,
            association_id: None,
        });
    }
    ctx
}

// ── HookManager ───────────────────────────────────────────────────────────────

pub struct HookManager {
    config: HookConfig,
    handlers: HashMap<HookEvent, Vec<Box<dyn HookHandler>>>,
    use_defaults: bool,
}

impl HookManager {
    pub fn new(config: HookConfig) -> Self {
        Self {
            config,
            handlers: HashMap::new(),
            use_defaults: true,
        }
    }

    pub fn default() -> Self {
        Self::new(HookConfig::default())
    }

    pub fn read_only() -> Self {
        Self::new(HookConfig::read_only())
    }

    pub fn register(&mut self, handler: Box<dyn HookHandler>) {
        let event = handler.event();
        self.handlers.entry(event).or_default().push(handler);
    }

    pub fn set_use_defaults(&mut self, use_defaults: bool) {
        self.use_defaults = use_defaults;
    }

    pub fn config(&self) -> &HookConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: HookConfig) {
        self.config = config;
    }

    pub fn execute(&self, event: HookEvent, ctx: HookContext) -> HookContext {
        if !self.config.is_enabled(&event) {
            tracing::debug!("Hook '{}' is disabled, skipping", event.label());
            return ctx;
        }

        tracing::debug!("Executing hook '{}' (chain: {})", event.label(), ctx.chain_id);
        let mut current_ctx = ctx;

        if self.use_defaults {
            current_ctx = match event {
                HookEvent::SessionStart => default_on_session_start(current_ctx),
                HookEvent::BeforePrompt => default_before_prompt(current_ctx),
                HookEvent::BeforeToolCall => default_before_tool_call(current_ctx),
                HookEvent::AfterToolCall => default_after_tool_call(current_ctx),
                HookEvent::AfterResponse => default_after_response(current_ctx),
                HookEvent::Error => default_on_error(current_ctx),
            };
        }

        if let Some(handlers) = self.handlers.get(&event) {
            for handler in handlers {
                current_ctx = handler.execute(current_ctx);
            }
        }

        if !self.config.can_write() {
            current_ctx.memory_candidates.clear();
        } else {
            if self.config.write.filter_secrets {
                current_ctx.memory_candidates.retain(|c| !contains_secrets(&c.text));
            }
            if self.config.write.filter_api_keys {
                current_ctx.memory_candidates.retain(|c| !contains_api_key(&c.text));
            }
            current_ctx.memory_candidates.retain(|c| c.importance >= self.config.min_importance);
            current_ctx.memory_candidates.truncate(self.config.max_candidates);
        }

        current_ctx
    }

    pub fn execute_chain(&self, events: &[HookEvent], initial_ctx: HookContext) -> HookContext {
        let mut ctx = initial_ctx;
        for event in events {
            ctx = self.execute(*event, ctx);
        }
        ctx
    }

    pub fn extract_candidates(&self, ctx: &HookContext) -> Vec<MemoryCandidate> {
        ctx.memory_candidates.clone()
    }

    pub fn is_enabled(&self, event: &HookEvent) -> bool {
        self.config.is_enabled(event)
    }

    pub fn can_write(&self) -> bool {
        self.config.can_write()
    }
}

// ── Security Filters ─────────────────────────────────────────────────────────

fn contains_secrets(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "password", "passwd", "pwd", "secret", "token", "credential",
        "auth_token", "bearer", "private_key", "-----begin",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

fn contains_api_key(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "api_key", "apikey", "api-key", "sk-", "pk-", "openai_key",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

// ── Helper: Build context package ────────────────────────────────────────────

pub fn build_context_package(ctx: &HookContext) -> String {
    let mut package = String::new();

    if let Some(contract) = &ctx.memory_contract {
        package.push_str(contract);
        package.push_str("\n\n");
    }

    if let Some(project) = &ctx.project_context {
        package.push_str(&format!("## Project Context\n{}\n\n", project));
    }

    if !ctx.constraints.is_empty() {
        package.push_str("## Active Constraints\n");
        for c in &ctx.constraints {
            package.push_str(&format!("- {}\n", c));
        }
        package.push_str("\n");
    }

    if !ctx.retrieved_memories.is_empty() {
        package.push_str(&format!("## Retrieved Memories ({} blocks)\n", ctx.retrieved_memories.len()));
        for m in &ctx.retrieved_memories {
            package.push_str(&format!(
                "- [{}] D{} imp={} dist={:.3}: {}\n",
                m.layer, m.depth, m.importance, m.distance, m.snippet
            ));
        }
        package.push_str("\n");
    }

    package
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_labels() {
        assert_eq!(HookEvent::SessionStart.label(), "on_session_start");
        assert_eq!(HookEvent::BeforePrompt.label(), "before_prompt");
        assert_eq!(HookEvent::BeforeToolCall.label(), "before_tool_call");
        assert_eq!(HookEvent::AfterToolCall.label(), "after_tool_call");
        assert_eq!(HookEvent::AfterResponse.label(), "after_response");
        assert_eq!(HookEvent::Error.label(), "on_error");
    }

    #[test]
    fn test_hook_config_default() {
        let config = HookConfig::default();
        assert!(config.is_enabled(&HookEvent::SessionStart));
        assert!(config.is_enabled(&HookEvent::BeforePrompt));
        assert!(!config.is_enabled(&HookEvent::AfterResponse));
        assert!(!config.can_write());
    }

    #[test]
    fn test_hook_config_read_only() {
        let config = HookConfig::read_only();
        assert!(config.read_only);
        assert!(!config.can_write());
    }

    #[test]
    fn test_hook_config_full() {
        let config = HookConfig::full();
        assert!(config.can_write());
        assert!(config.is_enabled(&HookEvent::AfterResponse));
    }

    #[test]
    fn test_hook_context_creation() {
        let ctx = HookContext::new(HookEvent::SessionStart);
        assert_eq!(ctx.event, HookEvent::SessionStart);
        assert!(!ctx.chain_id.is_empty());
        assert!(ctx.timestamp_ms > 0);
    }

    #[test]
    fn test_hook_context_builder() {
        let ctx = HookContext::new(HookEvent::BeforePrompt)
            .with_query("test query")
            .with_response("test response");
        assert_eq!(ctx.query, Some("test query".to_string()));
        assert_eq!(ctx.response, Some("test response".to_string()));
    }

    #[test]
    fn test_default_on_session_start() {
        let ctx = HookContext::new(HookEvent::SessionStart);
        let result = default_on_session_start(ctx);
        assert!(result.memory_contract.is_some());
        assert!(!result.constraints.is_empty());
    }

    #[test]
    fn test_default_after_tool_call() {
        let ctx = HookContext::new(HookEvent::AfterToolCall)
            .with_tool("test_tool", serde_json::json!({}));
        let mut ctx_with_result = ctx.clone();
        ctx_with_result.tool_result = Some("This is a significant tool result with useful information.".to_string());
        let result = default_after_tool_call(ctx_with_result);
        assert!(!result.memory_candidates.is_empty());
        assert_eq!(result.memory_candidates[0].source_tool, Some("test_tool".to_string()));
    }

    #[test]
    fn test_default_on_error() {
        let ctx = HookContext::new(HookEvent::Error)
            .with_error("Connection timeout", "E1001");
        let result = default_on_error(ctx);
        assert!(!result.memory_candidates.is_empty());
        assert!(result.memory_candidates[0].is_error);
        assert_eq!(result.memory_candidates[0].importance, 7);
    }

    #[test]
    fn test_hook_manager_execute_disabled() {
        let mut config = HookConfig::default();
        config.enabled_hooks.insert(HookEvent::AfterResponse, false);
        let manager = HookManager::new(config);
        let ctx = HookContext::new(HookEvent::AfterResponse);
        let result = manager.execute(HookEvent::AfterResponse, ctx);
        assert!(result.memory_candidates.is_empty());
    }

    #[test]
    fn test_hook_manager_execute_enabled() {
        let config = HookConfig::full();
        let manager = HookManager::new(config);
        let ctx = HookContext::new(HookEvent::SessionStart);
        let result = manager.execute(HookEvent::SessionStart, ctx);
        assert!(result.memory_contract.is_some());
    }

    #[test]
    fn test_security_filters() {
        assert!(contains_secrets("my password is 1234"));
        assert!(contains_api_key("api_key=sk-test123"));
        assert!(!contains_secrets("hello world"));
        assert!(!contains_api_key("normal text"));
    }

    #[test]
    fn test_write_disabled_filters_candidates() {
        let config = HookConfig::default();
        let manager = HookManager::new(config);
        let mut ctx = HookContext::new(HookEvent::AfterToolCall);
        ctx.tool_result = Some("Important result".to_string());
        ctx = default_after_tool_call(ctx);
        let result = manager.execute(HookEvent::AfterToolCall, ctx);
        assert!(result.memory_candidates.is_empty());
    }

    #[test]
    fn test_build_context_package() {
        let mut ctx = HookContext::new(HookEvent::BeforePrompt);
        ctx.memory_contract = Some("Memory Contract".to_string());
        ctx.project_context = Some("Project X".to_string());
        ctx.constraints.push("Test constraint".to_string());
        ctx.retrieved_memories.push(MemoryBlockRef {
            id: "1".to_string(),
            layer: "long_term".to_string(),
            depth: 3,
            importance: 8,
            snippet: "Important memory".to_string(),
            distance: 0.1,
        });

        let package = build_context_package(&ctx);
        assert!(package.contains("Memory Contract"));
        assert!(package.contains("Project X"));
        assert!(package.contains("Test constraint"));
        assert!(package.contains("Important memory"));
    }

    #[test]
    fn test_execute_chain() {
        let config = HookConfig::full();
        let manager = HookManager::new(config);
        let ctx = HookContext::new(HookEvent::SessionStart);
        let events = [HookEvent::SessionStart, HookEvent::BeforePrompt];
        let result = manager.execute_chain(&events, ctx);
        assert!(result.memory_contract.is_some());
    }

    #[test]
    fn test_custom_handler() {
        struct TestHandler;
        impl HookHandler for TestHandler {
            fn event(&self) -> HookEvent { HookEvent::BeforePrompt }
            fn execute(&self, mut ctx: HookContext) -> HookContext {
                ctx.metadata.insert("custom".to_string(), "ran".to_string());
                ctx
            }
        }

        let mut manager = HookManager::new(HookConfig::full());
        manager.register(Box::new(TestHandler));
        let ctx = HookContext::new(HookEvent::BeforePrompt);
        let result = manager.execute(HookEvent::BeforePrompt, ctx);
        assert_eq!(result.metadata.get("custom").unwrap(), "ran");
    }
}
