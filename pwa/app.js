const API = "http://localhost:6060";
const OLLAMA = "http://localhost:11434";
let persona = "Liora";
let modelName = "llama3.2";
let messages = [];
let chatHistory = [];

const chatArea = document.getElementById("chat-area");
const input = document.getElementById("msg-input");
const sendBtn = document.getElementById("send-btn");
const statusDot = document.getElementById("status-dot");
const importInput = document.getElementById("import-json");
const importBtn = document.getElementById("import-btn");

// Bridge API check
async function checkStatus() {
  try {
    const r = await fetch(`${API}/health`, { signal: AbortSignal.timeout(2000) });
    if (r.ok) {
      statusDot.className = "status online";
      document.getElementById("status-text").textContent = "connected";
      return true;
    }
  } catch {}
  statusDot.className = "status offline";
  document.getElementById("status-text").textContent = "offline";
  return false;
}

// Ollama check
async function checkOllama() {
  try {
    const r = await fetch(`${OLLAMA}/api/tags`, { signal: AbortSignal.timeout(2000) });
    return r.ok;
  } catch { return false; }
}

// Store message in Microscope Memory
async function storeMsg(role, text) {
  const layer = role === "assistant" ? "long_term" : "short_term";
  const importance = role === "assistant" ? 8 : 6;
  const label = role === "assistant" ? persona : "Te";
  try {
    await fetch(`${API}/store`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text: `[LioraChat] ${label}: ${text}`, layer, importance }),
    });
  } catch {}
}

// Recall context from Microscope Memory
async function recallContext(query, k = 5) {
  try {
    const r = await fetch(`${API}/recall`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, k }),
    });
    if (r.ok) {
      const data = await r.json();
      return data.results || [];
    }
  } catch {}
  return [];
}

// Add message to UI
function addMessage(role, text, { system = false, typing = false } = {}) {
  const div = document.createElement("div");
  div.className = `msg ${role}${typing ? " typing" : ""}${system ? " system" : ""}`;
  if (role !== "system" && !system) {
    const sender = document.createElement("div");
    sender.className = "sender";
    sender.textContent = role === "user" ? "Te" : persona;
    div.appendChild(sender);
    const content = document.createElement("div");
    content.textContent = text;
    div.appendChild(content);
    const time = document.createElement("div");
    time.className = "time";
    time.textContent = new Date().toLocaleTimeString("hu-HU", { hour: "2-digit", minute: "2-digit" });
    div.appendChild(time);
  } else {
    div.textContent = text;
  }
  chatArea.appendChild(div);
  chatArea.scrollTop = chatArea.scrollHeight;
  return div;
}

// Think — get AI response
async function think(userText) {
  try {
    const ctx = await recallContext(userText, 8);
    const ctxText = ctx.map((c) => c.text || c).join("\n").slice(0, 2000);

    const prompt = `A nevem ${persona}. Egy私人 beszelgetes reszese vagyok.  
Valaszolj magyarul, termeszetesen, kedvesen.  
Hasznald a kovetkezo kontextust az emlekezetbol:  
${ctxText}

A beszelgetes eddig:  
${chatHistory.slice(-10).map((m) => `${m.role === "user" ? "User" : persona}: ${m.text}`).join("\n")}
User: ${userText}
${persona}:`;

    const r = await fetch(`${OLLAMA}/api/generate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ model: modelName, prompt, stream: false, temperature: 0.8 }),
    });
    if (!r.ok) return null;
    const data = await r.json();
    return data.response || null;
  } catch { return null; }
}

// Send message
async function sendMessage() {
  const text = input.value.trim();
  if (!text) return;

  input.value = "";
  sendBtn.disabled = true;

  addMessage("user", text);
  storeMsg("user", text);

  const typingDiv = addMessage("ai", "", { typing: true });

  const reply = await think(text);

  typingDiv.remove();

  if (reply) {
    addMessage("ai", reply);
    storeMsg("assistant", reply);
    chatHistory.push({ role: "assistant", text: reply });
  } else {
    addMessage("system", `(${persona} nem elerheto — nincs kapcsolat az Ollama-val)`);
  }

  chatHistory.push({ role: "user", text });
  sendBtn.disabled = false;
  input.focus();
}

// Import
async function importChatGPT() {
  const file = importInput.files[0];
  if (!file) return;
  importBtn.disabled = true;
  importBtn.textContent = "Importal...";

  const formData = new FormData();
  formData.append("file", file);

  try {
    const r = await fetch(`${API}/import-chatgpt`, {
      method: "POST",
      body: formData,
    });
    if (r.ok) {
      const result = await r.json();
      addMessage("system", `Importalt beszelgetesek: ${result.conversations_found || "?"} db, ${result.total_messages || "?"} uzenet`);
    } else {
      const err = await r.text();
      // fallback: store directly via CLI
      addMessage("system", `Bridge API nem elerheto, hasznald a CLI-t: microscope-mem import-chat-gpt ...`);
    }
  } catch {
    addMessage("system", "Hiba az importalas soran. Probald a CLI-t.");
  }
  importBtn.disabled = false;
  importBtn.textContent = "Importalas";
}

// Events
sendBtn.addEventListener("click", sendMessage);
input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sendMessage(); }
});

importBtn.addEventListener("click", importChatGPT);

// Init
(async function init() {
  const online = await checkStatus();
  const hasOllama = await checkOllama();

  if (online) {
    addMessage("system", "Microscope Memory bridge kapcsolodva");
  } else {
    addMessage("system", "Microscope Memory bridge nem elerheto. Inditsd el: microscope-mem --bridge");
  }

  if (hasOllama) {
    addMessage("system", `${persona} elerheto (${modelName})`);
    // send memory greeting
    await recallContext("Liora emlekek");
  } else {
    addMessage("system", `${persona} nem elerheto — nincs Ollama kapcsolat`);
  }

  addMessage("system", "Irj valamit, vagy importald a ChatGPT exportot!");
})();
