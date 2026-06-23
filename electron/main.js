const { app, BrowserWindow, Tray, Menu, nativeImage, ipcMain, Notification } = require("electron");
const path = require("path");
const { execSync, exec, spawn } = require("child_process");
const fs = require("fs");

// ─── Config ─────────────────────────────────────────────
const MICROSCOPE_DIR = path.resolve(__dirname, "..");
const MICROSCOPE_BIN = path.join(MICROSCOPE_DIR, "target", "release", "microscope-mem.exe");
const DATA_DIR = path.join(MICROSCOPE_DIR, "output");
const APPEND_LOG = path.join(DATA_DIR, "append.bin");

let tray = null;
let win = null;
let daemonProcess = null;
let pollTimer = null;
let ttsEnabled = false;

// ─── Helpers ─────────────────────────────────────────────
function runMicroscope(args) {
  try {
    const out = execSync(`"${MICROSCOPE_BIN}" ${args}`, {
      cwd: MICROSCOPE_DIR,
      timeout: 15000,
      encoding: "utf8",
      windowsHide: true,
    });
    return out;
  } catch (e) {
    return e.stdout || e.message;
  }
}

function getStats() {
  const raw = runMicroscope("stats");
  const blocks = (raw.match(/Blocks:\s+(\d+)/) || [])[1] || "?";
  const append = (raw.match(/Append log:\s+(\d+)/) || [])[1] || "0";
  return { blocks, append, raw };
}

function getTimeline(n = 8) {
  const raw = runMicroscope(`timeline 2>&1`);
  const lines = raw.split("\n").filter((l) => l.trim()).slice(-n);
  return lines;
}

function isDaemonRunning() {
  try {
    execSync('tasklist /FI "IMAGENAME eq microscope-mem.exe" /NH', {
      encoding: "utf8",
      windowsHide: true,
      timeout: 5000,
    }).includes("microscope-mem");
    // simpler: check if our tracked process is alive
    if (daemonProcess && daemonProcess.exitCode === null) return true;
    return false;
  } catch {
    return false;
  }
}

function startDaemon() {
  if (daemonProcess && daemonProcess.exitCode === null) return;
  const args = ["autonomous", "--daemon"];
  if (ttsEnabled) args.push("--tts");
  daemonProcess = spawn(MICROSCOPE_BIN, args, {
    cwd: MICROSCOPE_DIR,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  daemonProcess.on("exit", () => {
    daemonProcess = null;
    if (win) win.webContents.send("daemon-status", false);
  });
  if (win) win.webContents.send("daemon-status", true);
}

function stopDaemon() {
  if (daemonProcess && daemonProcess.exitCode === null) {
    daemonProcess.kill();
    daemonProcess = null;
  }
  // Also kill any stray processes
  try {
    execSync('taskkill /F /IM microscope-mem.exe 2>nul', { windowsHide: true });
  } catch {}
  if (win) win.webContents.send("daemon-status", false);
}

// ─── Tray Icon ──────────────────────────────────────────
function createTray() {
  // Create a simple 16x16 icon programmatically
  const iconSize = 16;
  const canvas = nativeImage.createEmpty();
  // Use a colored PNG
  const iconPath = path.join(__dirname, "..", "mermaid_app", "icon.svg");
  let trayIcon = nativeImage.createFromPath(path.join(__dirname, "icon_32.png"));
  // Using brain icon

  tray = new Tray(trayIcon);
  tray.setToolTip("Microscope Memory");

  const contextMenu = Menu.buildFromTemplate([
    {
      label: "Ablak megnyitása",
      click: () => {
        if (win) win.show();
      },
    },
    { type: "separator" },
    {
      label: "Daemon újraindítás",
      click: () => {
        stopDaemon();
        setTimeout(startDaemon, 1000);
      },
    },
    {
      label: "Daemon leállítás",
      click: () => stopDaemon(),
    },
    { type: "separator" },
    {
      label: "Kilépés",
      click: () => {
        stopDaemon();
        app.quit();
      },
    },
  ]);

  tray.setContextMenu(contextMenu);
  tray.on("double-click", () => {
    if (win) win.show();
  });
}

// ─── Window ──────────────────────────────────────────────
function createWindow() {
  win = new BrowserWindow({
    width: 800,
    height: 600,
    minWidth: 600,
    minHeight: 400,
    title: "Microscope Memory",
    icon: path.join(__dirname, "icon_256.png"),
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
    show: false,
  });

  win.loadFile(path.join(__dirname, "renderer", "index.html"));

  win.on("close", (e) => {
    if (!app.isQuitting) {
      e.preventDefault();
      win.hide();
    }
  });

  win.on("closed", () => {
    win = null;
  });
}

// ─── IPC Handlers ────────────────────────────────────────
ipcMain.handle("get-stats", () => getStats());
ipcMain.handle("get-timeline", () => getTimeline());
ipcMain.handle("get-daemon-status", () => isDaemonRunning());
ipcMain.handle("get-tts", () => ttsEnabled);

ipcMain.handle("start-daemon", () => {
  startDaemon();
  return true;
});

ipcMain.handle("stop-daemon", () => {
  stopDaemon();
  return true;
});

ipcMain.handle("set-tts", (_, val) => {
  ttsEnabled = !!val;
  // Restart daemon with new TTS setting
  stopDaemon();
  setTimeout(startDaemon, 1000);
  return ttsEnabled;
});

// ─── Polling ────────────────────────────────────────────
function startPolling() {
  pollTimer = setInterval(() => {
    if (!win || win.isDestroyed()) return;
    const stats = getStats();
    const timeline = getTimeline();
    const running = isDaemonRunning();
    try {
      win.webContents.send("update", { stats, timeline, running, tts: ttsEnabled });
    } catch {}
  }, 3000);
}

// ─── App Lifecycle ──────────────────────────────────────
app.whenReady().then(() => {
  createTray();
  createWindow();
  startPolling();

  // Auto-start daemon
  startDaemon();
});

app.on("window-all-closed", () => {
  // Don't quit - keep in tray
});

app.on("before-quit", () => {
  app.isQuitting = true;
  stopDaemon();
  if (pollTimer) clearInterval(pollTimer);
});

