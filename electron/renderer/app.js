// ─── DOM refs ──────────────────────────────────────────
const daemonBadge = document.getElementById("daemonBadge");
const btnStart = document.getElementById("btnStart");
const btnStop = document.getElementById("btnStop");
const ttsToggle = document.getElementById("ttsToggle");
const statBlocks = document.getElementById("statBlocks");
const statAppend = document.getElementById("statAppend");
const statData = document.getElementById("statData");
const statDaemon = document.getElementById("statDaemon");
const timelineList = document.getElementById("timelineList");

// ─── State ──────────────────────────────────────────────
let running = false;

// ─── Update UI ───────────────────────────────────────────
function updateUI(data) {
  const { stats, timeline, running: isRunning, tts } = data;

  running = isRunning;

  // Badge
  daemonBadge.textContent = isRunning ? "▶ Fut" : "⏹ Leállítva";
  daemonBadge.className = "status-badge" + (isRunning ? " running" : "");

  // Buttons
  btnStart.disabled = isRunning;
  btnStop.disabled = !isRunning;

  // TTS
  if (tts !== undefined) ttsToggle.checked = tts;

  // Stats
  if (stats) {
    statBlocks.textContent = stats.blocks || "?";
    statAppend.textContent = stats.append || "0";

    const dataMatch = stats.raw.match(/Total:\s+([\d.]+\s*\w+)/);
    statData.textContent = dataMatch ? dataMatch[1] : "?";
  }

  statDaemon.textContent = isRunning ? "▶" : "⏹";
  statDaemon.style.color = isRunning ? "#3fb950" : "#da3633";

  // Timeline
  if (timeline && timeline.length > 0) {
    timelineList.innerHTML = timeline
      .map((line) => {
        const clean = line.replace(/^.{0,2}/, "").trim();
        if (!clean) return "";
        const layerMatch = clean.match(/\[(\w+)\]/);
        const layer = layerMatch ? layerMatch[1] : "";
        const timeMatch = clean.match(/^\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}/);
        const time = timeMatch ? timeMatch[0] : "";
        const rest = clean.replace(time, "").trim();
        return `<div class="timeline-item">
          <span class="time">${time}</span>
          ${layer ? `<span class="layer">${layer}</span>` : ""}
          ${rest}
        </div>`;
      })
      .join("");
  }
}

// ─── Initial load ──────────────────────────────────────
async function init() {
  const [stats, timeline, daemonStatus, tts] = await Promise.all([
    window.microscope.getStats(),
    window.microscope.getTimeline(),
    window.microscope.getDaemonStatus(),
    window.microscope.getTts(),
  ]);
  updateUI({ stats, timeline, running: daemonStatus, tts });
}

// ─── Event listeners ────────────────────────────────────
btnStart.addEventListener("click", () => {
  window.microscope.startDaemon();
});

btnStop.addEventListener("click", () => {
  window.microscope.stopDaemon();
});

ttsToggle.addEventListener("change", () => {
  window.microscope.setTts(ttsToggle.checked);
});

// ─── Live updates ───────────────────────────────────────
window.microscope.onUpdate((data) => updateUI(data));
window.microscope.onDaemonStatus((status) => {
  updateUI({
    stats: null,
    timeline: null,
    running: status,
    tts: ttsToggle.checked,
  });
});

// ─── Go! ────────────────────────────────────────────────
init();
