content = open("src/autonomous.rs", "r", encoding="utf-8").read()

replacements = [
    ('"Daydream complete. {} steps with emotional shift of {:.2}"',
     '"Daydream kész. {} lépés, érzelmi eltolódás: {:.2}"'),
    ('"I am curious about: {}"',
     '"Kíváncsi vagyok: {}"'),
    ('"Inner monologue: {}"',
     '"Belső monológ: {}"'),
    ('"Self reflection: {}"',
     '"Önreflexió: {}"'),
    ('"New story: {}. {}"',
     '"Új történet: {}. {}"'),
    ('"Dream consolidation complete. Energy changed from {:.2} to {:.2}"',
     '"Álom konszolidáció kész. Energia: {:.2} -> {:.2}"'),
    ('"Append log rebuilt and cleared."',
     '"Append log újraépítve és törölve."'),
    ('"Self model updated. {} blocks, {} patterns."',
     '"Önkép frissítve. {} blokk, {} minta."'),
    ('"Cycle {} complete. {} activities."',
     '"Ciklus {} kész. {} aktivitás."'),
]

for old, new in replacements:
    content = content.replace(old, new)

open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK - magyar TTS szövegek")
