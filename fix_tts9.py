import sys
content = open('src/autonomous.rs', 'r', encoding='utf-8').read()
start = content.find('fn speak(&self, text: &str) {')
end = content.find('fn store_result', start)
old_func = content[start:end]
new_func = open('new_speak.txt', 'r', encoding='utf-8').read()
content = content[:start] + new_func + content[end:]
open('src/autonomous.rs', 'w', encoding='utf-8').write(content)
print('OK')