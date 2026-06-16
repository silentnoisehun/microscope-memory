import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { ListToolsRequestSchema, CallToolRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { execSync } from "child_process";
import { readFileSync, existsSync } from "fs";

const BIN = "E:\\microscope-memory\\target\\release\\microscope-mem.exe";
const CWD = "E:\\microscope-memory\\server-data\\microscope-server";
const SERVER_URL = "http://100.76.113.83:6060";

function sh(cmd) {
    return execSync(`cd /d ${CWD} && ${cmd}`, { encoding: "utf-8", timeout: 30000, maxBuffer: 10 * 1024 * 1024 }).trim();
}

async function recall(q, k = 10) {
    const safe = q.replace(/"/g, '""');
    return sh(`"${BIN}" recall "${safe}" ${k}`);
}

async function remember(text, layer = "long_term", importance = 5) {
    const safe = text.replace(/"/g, '""');
    const local = sh(`"${BIN}" store "${safe}" -l ${layer} -i ${importance}`);
    let remote = "";
    try {
        const r = await fetch(`${SERVER_URL}/remember`, {
            method: "POST", headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ text, layer, importance }),
            signal: AbortSignal.timeout(5000)
        });
        const j = await r.json();
        remote = `\n[mirror] Server: ${j.status}`;
    } catch {
        remote = "\n[mirror] Server unreachable (stored locally only)";
    }
    return local + remote;
}

async function status() {
    return sh(`"${BIN}" stats`);
}

async function find(query, k = 10) {
    const safe = query.replace(/"/g, '""');
    return sh(`"${BIN}" find "${safe}" ${k}`);
}

async function look(x, y, z, zoom, k = 10) {
    return sh(`"${BIN}" look ${x} ${y} ${z} ${zoom} ${k}`);
}

async function mqlQuery(mql) {
    const safe = mql.replace(/"/g, '""');
    return sh(`"${BIN}" query "${safe}"`);
}

async function build(force = false) {
    const flag = force ? "--force" : "";
    const local = sh(`"${BIN}" build ${flag}`.trim());
    let remote = "";
    try {
        const r = await fetch(`${SERVER_URL}/build`, {
            method: "POST", headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ force }),
            signal: AbortSignal.timeout(5000)
        });
        const j = await r.json();
        remote = `\n[mirror] Server: ${j.status}`;
    } catch {
        remote = "\n[mirror] Server unreachable";
    }
    return local + remote;
}

async function sessionLog(n = 50) {
    const path = `${CWD}\\layers\\session.txt`;
    if (!existsSync(path)) return "Session memory is empty.";
    const content = readFileSync(path, "utf-8").trim();
    if (!content) return "Session memory is empty.";
    const lines = content.split("\n").filter(l => l.trim());
    const recent = lines.slice(-n);
    return `Session Memory — ${recent.length} interactions:\n\n` + recent.map((e, i) => `${i + 1}. ${e.slice(0, 300)}`).join("\n");
}

async function consolidate() {
    try {
        const r = await fetch(`${SERVER_URL}/consolidate`, {
            method: "POST", headers: { "Content-Type": "application/json" },
            signal: AbortSignal.timeout(10000)
        });
        const j = await r.json();
        return `Consolidated ${j.consolidated} session groups:\n` + (j.groups || []).map((g) => `  ${g}`).join("\n");
    } catch {
        return "Consolidation failed: server unreachable";
    }
}

async function dream() {
    const local = sh(`"${BIN}" dream`);
    let remote = "";
    try {
        const r = await fetch(`${SERVER_URL}/dream`, {
            method: "POST", headers: { "Content-Type": "application/json" },
            signal: AbortSignal.timeout(10000)
        });
        const j = await r.json();
        remote = `\n[mirror] Server: ${j.status}`;
    } catch {
        remote = "\n[mirror] Server unreachable";
    }
    return local + remote;
}

const server = new Server({ name: "microscope-proxy", version: "2.1" }, { capabilities: { tools: {} } });

server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: [
        {
            name: "microscope_memory_recall",
            description: "Natural language recall with auto-zoom — searches both main index and append log",
            inputSchema: { type: "object", properties: { query: { type: "string", description: "Natural language query" }, k: { type: "integer", description: "Max results to return", default: 10 } }, required: ["query"] }
        },
        {
            name: "microscope_memory_store",
            description: "Store a new memory into the microscope append log",
            inputSchema: { type: "object", properties: { text: { type: "string", description: "Memory text to store" }, layer: { type: "string", description: "Memory layer (long_term, short_term, session, associative, emotional, relational, reflections, echo_cache)", default: "long_term" }, importance: { type: "integer", description: "Importance level 1-10", default: 5 } }, required: ["text"] }
        },
        {
            name: "microscope_memory_status",
            description: "Get microscope memory index status: block count, depths, append log size",
            inputSchema: { type: "object", properties: {} }
        },
        {
            name: "microscope_memory_find",
            description: "Brute-force text search across all depths",
            inputSchema: { type: "object", properties: { query: { type: "string", description: "Text to search for" }, k: { type: "integer", description: "Max results", default: 10 } }, required: ["query"] }
        },
        {
            name: "microscope_memory_look",
            description: "Manual spatial look at specific 3D coordinates and zoom level",
            inputSchema: { type: "object", properties: { x: { type: "number", description: "X coordinate (0.0-1.0)" }, y: { type: "number", description: "Y coordinate (0.0-1.0)" }, z: { type: "number", description: "Z coordinate (0.0-1.0)" }, zoom: { type: "integer", description: "Depth level (0-8)" }, k: { type: "integer", description: "Max results", default: 10 } }, required: ["x", "y", "z", "zoom"] }
        },
        {
            name: "microscope_memory_mql_query",
            description: "Execute an MQL (Microscope Query Language) query with filters: layer, depth, spatial, boolean",
            inputSchema: { type: "object", properties: { mql: { type: "string", description: "MQL expression, e.g. 'layer:long_term depth:2..5 \"memory\"'" } }, required: ["mql"] }
        },
        {
            name: "microscope_memory_build",
            description: "Rebuild the microscope index from layer source files (merges append log)",
            inputSchema: { type: "object", properties: { force: { type: "boolean", description: "Force rebuild even if unchanged", default: false } } }
        },
        {
            name: "microscope_memory_session_log",
            description: "Read last N interactions from the session memory layer (reads layers/session.txt directly)",
            inputSchema: { type: "object", properties: { n: { type: "integer", description: "Number of recent interactions to return", default: 50 } } }
        },
        {
            name: "microscope_memory_consolidate",
            description: "Consolidate recent session entries into long-term memory summaries. Groups entries by session ID and creates short summaries.",
            inputSchema: { type: "object", properties: {} }
        },
        {
            name: "microscope_memory_dream",
            description: "Dream consolidation — offline memory replay that strengthens important pathways and prunes weak ones (biological sleep analog).",
            inputSchema: { type: "object", properties: {} }
        }
    ]
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;
    try {
        let text;
        if (name === "microscope_memory_recall") text = await recall(args.query, args.k ?? 10);
        else if (name === "microscope_memory_store") text = await remember(args.text, args.layer ?? "long_term", args.importance ?? 5);
        else if (name === "microscope_memory_status") text = await status();
        else if (name === "microscope_memory_find") text = await find(args.query, args.k ?? 10);
        else if (name === "microscope_memory_look") text = await look(args.x, args.y, args.z, args.zoom, args.k ?? 10);
        else if (name === "microscope_memory_mql_query") text = await mqlQuery(args.mql);
        else if (name === "microscope_memory_build") text = await build(args.force ?? false);
        else if (name === "microscope_memory_session_log") text = await sessionLog(args.n ?? 50);
        else if (name === "microscope_memory_consolidate") text = await consolidate();
        else if (name === "microscope_memory_dream") text = await dream();
        else throw new Error(`Unknown tool: ${name}`);
        return { content: [{ type: "text", text }] };
    } catch (e) {
        return { content: [{ type: "text", text: `Error: ${e.message}` }], isError: true };
    }
});

const transport = new StdioServerTransport();
await server.connect(transport);
