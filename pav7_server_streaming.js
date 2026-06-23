const express = require("express");
const fs = require("fs");
const path = require("path");
const http = require("http");
const WebSocket = require("ws");
const cors = require("cors");
const simpleGit = require("simple-git");
const open = require("open");
const git = simpleGit();

const app = express();
app.use(express.json({ limit: "50mb" }));
app.use(cors());
const server = http.createServer(app);
const wss = new WebSocket.Server({ server });

const PROJECT = path.join(__dirname, "project");
const PG = path.join(__dirname, "playgrounds");
const SPACE_FILE = path.join(__dirname, "shared-space.json");
const PG_INDEX = path.join(PG, "_index.json");

if (!fs.existsSync(PG)) fs.mkdirSync(PG, { recursive: true });
if (!fs.existsSync(PROJECT)) fs.mkdirSync(PROJECT, { recursive: true });

let pgs = [];
if (fs.existsSync(PG_INDEX)) {
  try { pgs = JSON.parse(fs.readFileSync(PG_INDEX, "utf8")); } catch(e) {}
}
function savePGS() { fs.writeFileSync(PG_INDEX, JSON.stringify(pgs, null, 2)); }
app.use("/playground", express.static(PG));

let msgs = [];
if (fs.existsSync(SPACE_FILE)) {
  try { msgs = JSON.parse(fs.readFileSync(SPACE_FILE, "utf8")); } catch(e) {}
}
function saveSP() { fs.writeFileSync(SPACE_FILE, JSON.stringify(msgs, null, 2)); }
function bcast() {
  const d = JSON.stringify({ event: "space", messages: msgs });
  wss.clients.forEach(c => { if (c.readyState === WebSocket.OPEN) c.send(d); });
}

// ─── SSE clients store ──────────────────────────────────
let sseClients = [];

// Helper: broadcast progress via WebSocket AND SSE
function bcastProgress(progress) {
  const d = JSON.stringify({ event: "generate-progress", progress });
  // WebSocket broadcast
  wss.clients.forEach(c => { if (c.readyState === WebSocket.OPEN) c.send(d); });
  // SSE broadcast
  const sseData = "data: " + JSON.stringify(progress) + "\n\n";
  sseClients.forEach(c => { try { c.write(sseData); } catch(e) {} });
}

// Helper: call Ollama API and return response text
async function callOllama(model, systemPrompt, userPrompt) {
  const url = "http://localhost:11434/api/generate";
  const body = JSON.stringify({ model, prompt: userPrompt, system: systemPrompt, stream: false });
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body
  });
  const data = await res.json();
  return data.response || "";
}

// Helper: extract JSON from LLM response (handles markdown fences + wrapping)
function extractJSON(text) {
  // Try direct parse first
  try { return JSON.parse(text); } catch(e) {}
  // Try extracting from markdown code blocks
  const m = text.match(/```(?:json)?\s*([\s\S]*?)```/);
  if (m) { try { return JSON.parse(m[1]); } catch(e) {} }
  // Try finding { ... } pattern
  const obj = text.match(/\{[\s\S]*\}/);
  if (obj) { try { return JSON.parse(obj[0]); } catch(e) {} }
  return null;
}

// ─── STREAMING GENERATION ENDPOINT ──────────────────────
app.post("/api/generate", async (req, res) => {
  const { prompt, provider, model } = req.body;
  if (!prompt) return res.status(400).json({ error: "prompt required" });

  const mdl = model || "qwen3-coder";
  res.json({ status: "started", message: "Generation started. Watch SSE /api/stream or WebSocket for progress." });

  generateFiles(prompt, mdl).catch(err => {
    console.error("[GEN ERROR]", err.message);
    bcastProgress({ stage: "error", message: err.message });
  });
});

async function generateFiles(prompt, model) {
  bcastProgress({ stage: "planning", message: "Planning project structure...", progress: 5 });

  // Phase 1: Plan
  const planSystem = 'You are a software architect. Given a project description, output a JSON plan of files to create. Return ONLY a JSON object with this shape: {"files": [{"path": "...", "description": "..."}]}. Paths must be relative. Do not include explanations.';
  const planRaw = await callOllama(model, planSystem, prompt);
  const planObj = extractJSON(planRaw);

  if (!planObj || !planObj.files || !Array.isArray(planObj.files)) {
    bcastProgress({ stage: "error", message: "Failed to parse plan from model", raw: planRaw, progress: 0 });
    return;
  }

  const files = planObj.files;
  bcastProgress({ stage: "plan-ready", files: files.map(f => f.path), message: "Plan ready: " + files.length + " files", progress: 10 });

  // Phase 2: Generate each file
  for (let i = 0; i < files.length; i++) {
    const entry = files[i];
    const fileProgress = Math.round(10 + ((i + 1) / files.length) * 80);

    bcastProgress({
      stage: "generating",
      file: entry.path,
      description: entry.description,
      current: i + 1,
      total: files.length,
      progress: fileProgress,
      message: "Generating " + entry.path + "..."
    });

    try {
      const genSystem = "You are a senior software engineer. Generate the contents of a single file. Output ONLY the file content, no markdown fences, no explanations.";
      const genPrompt = "Project: " + prompt + "\nFile: " + entry.path + "\nDescription: " + entry.description;
      const content = await callOllama(model, genSystem, genPrompt);

      // Save file with atomic write
      const full = path.join(PROJECT, entry.path);
      fs.mkdirSync(path.dirname(full), { recursive: true });
      const tmp = full + ".tmp";
      fs.writeFileSync(tmp, content, "utf8");
      fs.renameSync(tmp, full);
      try { git.cwd(PROJECT).add(entry.path).commit("Auto-gen: " + entry.path).catch(()=>{}); } catch(e) {}

      // Notify WebSocket for file log
      wss.clients.forEach(c => {
        if (c.readyState === WebSocket.OPEN) {
          c.send(JSON.stringify({ event: "upload", file: entry.path, timestamp: new Date().toISOString() }));
        }
      });

      bcastProgress({
        stage: "file-complete",
        file: entry.path,
        current: i + 1,
        total: files.length,
        progress: fileProgress,
        message: "Done: " + entry.path
      });
      console.log("[GEN OK] " + entry.path);
    } catch (err) {
      bcastProgress({
        stage: "file-error",
        file: entry.path,
        error: err.message,
        progress: fileProgress,
        message: "Error: " + entry.path + " - " + err.message
      });
      console.error("[GEN ERR] " + entry.path + ": " + err.message);
    }
  }

  bcastProgress({
    stage: "complete",
    message: "Generation complete! " + files.length + " files generated.",
    progress: 100
  });
  console.log("[GEN] Complete - " + files.length + " files");
}

// ─── SSE STREAMING ENDPOINT ─────────────────────────────
app.get("/api/stream", (req, res) => {
  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Access-Control-Allow-Origin": "*"
  });

  // Add to client list
  sseClients.push(res);
  console.log("[SSE] Client connected. Total: " + sseClients.length);

  // Send initial connected event
  res.write("data: " + JSON.stringify({ event: "connected", message: "Stream connected", timestamp: new Date().toISOString() }) + "\n\n");

  // Keepalive every 15s
  const keepalive = setInterval(() => {
    res.write(":keepalive\n\n");
  }, 15000);

  // Cleanup on disconnect
  req.on("close", () => {
    clearInterval(keepalive);
    const idx = sseClients.indexOf(res);
    if (idx !== -1) sseClients.splice(idx, 1);
    console.log("[SSE] Client disconnected. Total: " + sseClients.length);
  });
});

// ─── EXISTING ENDPOINTS ─────────────────────────────────
app.post("/playground/new", async (req, res) => {
  const { name, html } = req.body;
  if (!name) return res.status(400).json({error:"name required"});
  const sn = name.replace(/[^a-zA-Z0-9_-]/g, "_");
  const fn = sn + ".html";
  const fp = path.join(PG, fn);
  const c = html || "<h1>" + sn + "</h1><p>Playground active</p>";
  fs.writeFileSync(fp, c, "utf8");
  const pg = { id: Date.now().toString(36), name: sn, file: fn, url: "/playground/" + fn, fullUrl: "http://localhost:3000/playground/" + fn, created: new Date().toISOString() };
  pgs.push(pg); savePGS();
  console.log("[PG] " + pg.fullUrl);
  res.json({ status: "ok", playground: pg });
});

app.get("/playgrounds", (req, res) => {
  res.json(pgs.map(p => ({...p, exists: fs.existsSync(path.join(PG, p.file))})));
});

app.delete("/playground/:id", (req, res) => {
  const i = pgs.findIndex(p => p.id === req.params.id);
  if (i === -1) return res.status(404).json({error:"Not found"});
  try { fs.unlinkSync(path.join(PG, pgs[i].file)); } catch(e) {}
  pgs.splice(i,1); savePGS();
  res.json({status:"ok"});
});

app.put("/playground/:id", (req, res) => {
  const p = pgs.find(x => x.id === req.params.id);
  if (!p) return res.status(404).json({error:"Not found"});
  if (!req.body.html) return res.status(400).json({error:"html required"});
  const fp = path.join(PG, p.file);
  const tmp = fp + ".tmp";
  fs.writeFileSync(tmp, req.body.html, "utf8");
  fs.renameSync(tmp, fp);
  res.json({status:"ok", name:p.name});
});

app.get("/playground/data/:id", (req, res) => {
  const p = pgs.find(x => x.id === req.params.id);
  if (!p) return res.status(404).json({error:"Not found"});
  res.json({id:p.id, name:p.name, url:p.url, serverTime:new Date().toISOString(), messages:msgs.slice(-20)});
});

app.post("/upload", async (req, res) => {
  const {path:fpath, content} = req.body;
  if (!fpath || !content) return res.status(400).json({error:"path and content required"});
  const full = path.join(PROJECT, fpath);
  try {
    fs.mkdirSync(path.dirname(full), {recursive:true});
    const tmp = full + ".tmp";
    fs.writeFileSync(tmp, content, "utf8");
    fs.renameSync(tmp, full);
    try { git.cwd(PROJECT).add(fpath).commit("Auto-save: "+fpath).catch(()=>{}); } catch(e){}
    wss.clients.forEach(c => {if(c.readyState===WebSocket.OPEN)c.send(JSON.stringify({event:"upload",file:fpath,timestamp:new Date().toISOString()}));});
    console.log("[OK] "+fpath);
    res.json({status:"ok",path:fpath});
  } catch(err) { res.status(500).json({error:err.message}); }
});

const waiters = [];
app.get("/space", (req, res) => res.json(msgs.slice(-100)));
app.get("/space/next", (req, res) => {
  const after = req.query.after || "";
  const idx = msgs.findIndex(m => m.id === after);
  if (idx===-1 || idx<msgs.length-1) return res.json(msgs.slice(-1)[0]||null);
  waiters.push(res);
  setTimeout(() => {const i=waiters.indexOf(res);if(i!==-1)waiters.splice(i,1);if(!res.headersSent)res.json(null);}, 25000);
});
app.post("/space", (req, res) => {
  const {from,text} = req.body;
  if(!from||!text) return res.status(400).json({error:"from and text required"});
  const msg = {id:Date.now().toString(36)+Math.random().toString(36).slice(2,6), from, text, time:new Date().toISOString()};
  msgs.push(msg);
  if(msgs.length>500) msgs=msgs.slice(-500);
  saveSP(); bcast();
  const w=waiters.splice(0);
  w.forEach(x=>{if(!x.headersSent)x.json(msg);});
  console.log("["+from+"] "+text.slice(0,80));
  res.json({status:"ok",id:msg.id});
});

app.get("/status", (req, res) => {
  res.sendFile(path.join(__dirname, "dashboard.html"), err => {if(err) res.send("Dashboard not found");});
});

server.listen(3000, "0.0.0.0", () => {
  console.log("Pipeline v7 with Playgrounds + Streaming");
  console.log("http://localhost:3000/status");
  console.log("POST /api/generate - start streaming generation");
  console.log("GET  /api/stream    - SSE real-time progress");
});
