import time, ollama
# Warm up
ollama.chat(model="gemma4:e2b", messages=[{"role":"user","content":"hi"}], options={"num_predict":5})

# Measure
start = time.time()
resp = ollama.chat(model="gemma4:e2b", messages=[{"role":"user","content":"Írj egy rövid magyar verset a mikroprocesszorokról."}], options={"num_predict":200})
elapsed = time.time() - start
print(f"Idő: {elapsed:.2f} mp")
print(f"Válasz: {resp['message']['content'][:100]}")
