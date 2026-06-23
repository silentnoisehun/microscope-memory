const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("microscope", {
  getStats: () => ipcRenderer.invoke("get-stats"),
  getTimeline: () => ipcRenderer.invoke("get-timeline"),
  getDaemonStatus: () => ipcRenderer.invoke("get-daemon-status"),
  getTts: () => ipcRenderer.invoke("get-tts"),
  startDaemon: () => ipcRenderer.invoke("start-daemon"),
  stopDaemon: () => ipcRenderer.invoke("stop-daemon"),
  setTts: (val) => ipcRenderer.invoke("set-tts", val),
  onUpdate: (cb) => ipcRenderer.on("update", (_, data) => cb(data)),
  onDaemonStatus: (cb) => ipcRenderer.on("daemon-status", (_, status) => cb(status)),
});
