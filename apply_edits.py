import os
# reader.rs add allows
path = r'D:\codex\microscope-memory\src\reader.rs'
with open(path, 'r', encoding='utf-8') as f:
    text = f.read()
for func in ['look_with_options', 'look_soft_with_options', 'radial_search_with_options']:
    marker = f'    pub fn {func}(\n'
    repl = f'    #[allow(clippy::too_many_arguments)]\n{marker}'
    text = text.replace(marker, repl, 1)
marker2 = 'fn write_append_entry(\n'
repl2 = '#[allow(clippy::too_many_arguments)]\nfn write_append_entry(\n'
text = text.replace(marker2, repl2, 1)
with open(path, 'w', encoding='utf-8') as f:
    f.write(text)
print('added allows reader.rs')

# mcp.rs underscore config
path2 = r'D:\codex\microscope-memory\src\mcp.rs'
with open(path2, 'r', encoding='utf-8') as f:
    text2 = f.read()
for line in [
    '        let config = test_config();\n        let hook_config = HookConfig::read_only();',
    '        let config = test_config();\n        let hook_config = HookConfig::full();',
    '        let config = test_config();\n        let ctx = HookContext::new(HookEvent::BeforeToolCall)',
    '        let config = test_config();\n        let mut ctx = HookContext::new(HookEvent::AfterToolCall)'
]:
    text2 = text2.replace(line, line.replace('let config =', 'let _config ='))
with open(path2, 'w', encoding='utf-8') as f:
    f.write(text2)
print('fixed mcp unused config')

# auto_context.rs sort_by_key
path3 = r'D:\codex\microscope-memory\src\auto_context.rs'
with open(path3, 'r', encoding='utf-8') as f:
    text3 = f.read()
text3 = text3.replace('candidates.sort_by(|a, b| b.importance.cmp(&a.importance));', 'candidates.sort_by_key(|a| std::cmp::Reverse(a.importance));')
with open(path3, 'w', encoding='utf-8') as f:
    f.write(text3)
print('fixed auto_context sort')
