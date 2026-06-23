// ═══════════════════════════════════════════════════════════════
// modules/skills.js — 129 Skill Index
// Riboszoma modul: kategóriákra szervezett tudásbázis
// ═══════════════════════════════════════════════════════════════

export const SKILL_CATEGORIES = {
  'agent-development': {
    label: 'Agent Development',
    icon: 'AG',
    count: 8,
    skills: ['create_agent', 'agent_prompt', 'agent_tools', 'agent_examples', 'agent_frontmatter', 'description_trigger', 'autonomous_agent', 'agent_color']
  },
  'command-development': {
    label: 'Command Development',
    icon: 'CM',
    count: 6,
    skills: ['create_command', 'command_args', 'command_frontmatter', 'dynamic_args', 'slash_command', 'yaml_metadata']
  },
  'hook-development': {
    label: 'Hook Development',
    icon: 'HK',
    count: 7,
    skills: ['create_hook', 'pre_tool_use', 'post_tool_use', 'stop_hook', 'validate_tool', 'prompt_based_hooks', 'hookify_rules']
  },
  'memory': {
    label: 'Memory & Persistence',
    icon: 'MM',
    count: 6,
    skills: ['memory_core', 'long_term_memory', 'episodic_memory', 'semantic_memory', 'working_memory', 'memory_consolidation']
  },
  'bio-core': {
    label: 'Bio-Inspired',
    icon: 'BI',
    count: 9,
    skills: ['macrophage', 'synaptic_pruning', 'crispr_hotfix', 'bio_core', 'homeostasis', 'immune_system', 'apoptosis', 'mitosis', 'dna_repair']
  },
  'consciousness': {
    label: 'Consciousness',
    icon: 'CN',
    count: 5,
    skills: ['hebbian', 'mirror', 'resonance', 'archetype', 'emoti_mem']
  },
  'voice': {
    label: 'Voice Systems',
    icon: 'VC',
    count: 7,
    skills: ['sherpa_onnx_tts', 'openai_whisper', 'openai_whisper_api', 'edge_tts', 'elevenlabs', 'web_speech_api', 'voice_loop']
  },
  'code-surgery': {
    label: 'Code Surgery',
    icon: 'CS',
    count: 6,
    skills: ['rust_surgeon', 'omni_surgeon', 'crispr_mutate', 'code_reader', 'code_writer', 'file_surgeon']
  },
  'pipeline': {
    label: 'Pipeline Architect',
    icon: 'PL',
    count: 8,
    skills: ['pipeline_architect_v7', 'planner', 'executor', 'writer', 'providers', 'hybrid_mode', 'plan_validate', 'step_run']
  },
  'ai-integration': {
    label: 'AI Integrations',
    icon: 'AI',
    count: 9,
    skills: ['claude_api', 'openai_api', 'gemini_cli', 'ollama', 'nvidia_nim', 'anthropic_proxy', 'opencode_zen', 'model_router', 'llm_bridge']
  },
  'multimodal': {
    label: 'Multimodal',
    icon: 'MM',
    count: 5,
    skills: ['nano_banana_pro', 'openai_image_gen', 'songsee', 'video_frames', 'camsnap']
  },
  'document': {
    label: 'Document & Office',
    icon: 'DC',
    count: 8,
    skills: ['pdf', 'docx', 'pptx', 'spreadsheets', 'nano_pdf', 'documents', 'presentations', 'theme_factory']
  },
  'web-research': {
    label: 'Web & Research',
    icon: 'WR',
    count: 7,
    skills: ['web_research', 'web_extractor', 'summarize', 'playwright', 'webapp_testing', 'browser_control', 'goplaces']
  },
  'github': {
    label: 'GitHub & Code',
    icon: 'GH',
    count: 7,
    skills: ['github', 'gh_issues', 'merge_pr', 'review_pr', 'merge_pr_v1', 'coding_agent', 'git_worktree']
  },
  'orchestration': {
    label: 'Orchestration',
    icon: 'OR',
    count: 6,
    skills: ['orchestration', 'colony_swarm', 'colony_swarm_mode', 'lobster', 'prose', 'multi_agent']
  },
  'design': {
    label: 'Design & UI',
    icon: 'DS',
    count: 8,
    skills: ['frontend_design', 'ui_design_system', 'ui_ux_pro_max', 'theme_factory', 'canvas_design', 'playground', 'brand_voice', 'senior_prompt_engineer']
  },
  'system': {
    label: 'System & OS',
    icon: 'SY',
    count: 8,
    skills: ['tmux', 'computer_use', 'healthcheck', 'session_logs', 'skill_creator', 'plugin_creator', 'skill_installer', 'find_skills']
  },
  'productivity': {
    label: 'Productivity',
    icon: 'PD',
    count: 5,
    skills: ['obsidian', 'notion', 'himalaya', 'discord', 'wacli']
  },
  'data': {
    label: 'Data & Analysis',
    icon: 'DT',
    count: 5,
    skills: ['data_master', 'gog', 'weather', 'local_places', 'gifgrep']
  },
  'archive': {
    label: 'Archive & Knowledge',
    icon: 'AR',
    count: 3,
    skills: ['still_archive', 'working_memory_plan', 'emoti_mem_v3']
  }
};

export const SKILL_KEYWORDS = {
  // category -> trigger keywords
  'code-surgery': ['mutate', 'patch', 'crispr', 'inject', 'replace', 'insert', 'anchor'],
  'pipeline': ['plan', 'execute', 'pipeline', 'workflow', 'multi-step', 'planner'],
  'voice': ['speak', 'tts', 'stt', 'whisper', 'voice', 'hang', 'beszél', 'mond'],
  'memory': ['remember', 'recall', 'store', 'emlékez', 'tárol', 'lookup'],
  'consciousness': ['hebbian', 'mirror', 'resonance', 'archetype', 'pattern', 'reminiscence'],
  'agent-development': ['agent', 'subagent', 'description', 'when to use'],
  'multimodal': ['image', 'kép', 'photo', 'drawing', 'generate', 'edit'],
  'web-research': ['search', 'web', 'research', 'find online', 'weboldal'],
  'github': ['pr', 'pull request', 'commit', 'merge', 'review', 'issue'],
  'document': ['pdf', 'docx', 'pptx', 'excel', 'document', 'presentation'],
  'system': ['install', 'config', 'system', 'os', 'permission', 'setup']
};

export const TOTAL_SKILLS = 129;

export function detectCategory(text) {
  const t = text.toLowerCase();
  let best = null;
  let bestScore = 0;
  for (const [cat, keywords] of Object.entries(SKILL_KEYWORDS)) {
    let score = 0;
    for (const kw of keywords) {
      if (t.includes(kw)) score += kw.length;
    }
    if (score > bestScore) {
      bestScore = score;
      best = cat;
    }
  }
  return bestScore > 2 ? best : null;
}

export function getCategoryInfo(cat) {
  return SKILL_CATEGORIES[cat] || null;
}

export function listAllCategories() {
  return Object.entries(SKILL_CATEGORIES).map(([id, info]) => ({
    id,
    label: info.label,
    icon: info.icon,
    count: info.count
  }));
}

export function getSkillsInCategory(cat) {
  return SKILL_CATEGORIES[cat]?.skills || [];
}

export const MODULE_INFO = {
  name: 'skills',
  version: '1.0.0',
  dependencies: [],
  exports: ['SKILL_CATEGORIES', 'SKILL_KEYWORDS', 'TOTAL_SKILLS', 'detectCategory', 'getCategoryInfo', 'listAllCategories', 'getSkillsInCategory']
};
