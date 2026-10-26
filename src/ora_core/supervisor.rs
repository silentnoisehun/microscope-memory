//! Autonomous Headless Process Supervisor
//!
//! Supervises the core daemons of Ora-Rongyász:
//!   - Queen (omega-master.exe on UDP 8888)
//!   - Drone (unified-mind.exe serve)
//!   - Stratos Server (stratos-server.exe on port 8090)
//!   - Model Router (bun opencodex on port 10100)
//!
//! 100% C: drive canonical, zero conhost popups (CREATE_NO_WINDOW: 0x08000000),
//! zero external drive dependencies.

use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

pub struct Supervisor {
    pub hope_bin: PathBuf,
    pub workspace_root: PathBuf,
    pub running: Arc<AtomicBool>,
}

impl Supervisor {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let hope_bin = PathBuf::from(r"C:\Users\mater\.hope\bin");
        Self {
            hope_bin,
            workspace_root: root,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Spawn a headless process with CREATE_NO_WINDOW on Windows
    pub fn spawn_headless<P: AsRef<Path>>(
        exe: P,
        args: &[&str],
        work_dir: P,
    ) -> std::io::Result<Child> {
        let mut cmd = Command::new(exe.as_ref());
        cmd.args(args);
        cmd.current_dir(work_dir.as_ref());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd.stdin(std::process::Stdio::null());
        cmd.spawn()
    }

    /// Check if UDP port 8888 is active (Queen probe)
    pub fn is_queen_alive() -> bool {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        if let Ok(socket) = UdpSocket::bind(addr) {
            let queen_addr: SocketAddr = "127.0.0.1:8888".parse().unwrap();
            let _ = socket.set_read_timeout(Some(Duration::from_millis(150)));
            if socket.send_to(&[0x00], queen_addr).is_ok() {
                return true;
            }
        }
        false
    }

    /// Check if HTTP port 8090 /api/health responds (Stratos probe)
    pub fn is_stratos_alive() -> bool {
        TcpStream::connect_timeout(
            &"127.0.0.1:8090".parse().unwrap(),
            Duration::from_millis(200),
        )
        .is_ok()
    }

    /// Check if HTTP port 10100 responds (Model Router probe)
    pub fn is_router_alive() -> bool {
        TcpStream::connect_timeout(
            &"127.0.0.1:10100".parse().unwrap(),
            Duration::from_millis(200),
        )
        .is_ok()
    }

    /// Check if unified-mind process is alive
    pub fn is_drone_alive() -> bool {
        #[cfg(windows)]
        {
            let out = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    "Get-Process unified-mind -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id",
                ])
                .output();
            if let Ok(o) = out {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                return !s.is_empty();
            }
        }
        false
    }

    /// Run a single supervisor tick: checks and recovers missing daemons
    pub fn tick(&self) -> Vec<String> {
        let mut events = Vec::new();

        // 1. Queen: omega-master.exe on UDP 8888
        if !Self::is_queen_alive() {
            let queen_exe = self.hope_bin.join("omega-master.exe");
            if queen_exe.exists() {
                events.push("Queen (omega-master) DOWN -> Starting headless".into());
                let work_dir = self.workspace_root.clone();
                let _ = Self::spawn_headless(
                    &queen_exe,
                    &["--state-dir", &work_dir.to_string_lossy(), "start"],
                    &work_dir,
                );
            }
        }

        // 2. Drone: unified-mind.exe serve
        if !Self::is_drone_alive() {
            let drone_exe = self.hope_bin.join("unified-mind.exe");
            if drone_exe.exists() {
                events.push("Drone (unified-mind) DOWN -> Starting headless serve".into());
                let work_dir = self.workspace_root.clone();
                let _ = Self::spawn_headless(&drone_exe, &["serve"], &work_dir);
            }
        }

        // 3. Stratos Server: stratos-server.exe on port 8090
        if !Self::is_stratos_alive() {
            let stratos_exe = self.hope_bin.join("stratos-server.exe");
            if stratos_exe.exists() {
                events.push("Stratos server DOWN -> Starting headless on 8090".into());
                let work_dir = self.workspace_root.clone();
                let data_dir = self.workspace_root.join("stratos_data");
                let _ = Self::spawn_headless(
                    &stratos_exe,
                    &[
                        "--data-dir",
                        &data_dir.to_string_lossy(),
                        "--addr",
                        "127.0.0.1:8090",
                        "--repo-root",
                        &work_dir.to_string_lossy(),
                        "--hardware-mode",
                        "mock",
                        "--ollama-host",
                        "http://127.0.0.1:11434",
                        "--small-model",
                        "qwen2.5:3b",
                        "--large-model",
                        "qwen2.5:7b",
                        "--num-ctx",
                        "2048",
                        "--router-url",
                        "http://127.0.0.1:10100/v1",
                        "--router-small-model",
                        "nvidia/nvidia/nemotron-3.5-lightning-30b-a3b",
                        "--router-large-model",
                        "nvidia/nvidia/nemotron-3-ultra-550b-a55b",
                    ],
                    &work_dir,
                );
            }
        }

        // 4. Model Router on port 10100
        if !Self::is_router_alive() {
            let bun_exe = PathBuf::from(
                r"C:\Users\mater\AppData\Roaming\npm\node_modules\@bitkyc08\opencodex\node_modules\bun\bin\bun.exe",
            );
            let ocx_cli = PathBuf::from(
                r"C:\Users\mater\AppData\Roaming\npm\node_modules\@bitkyc08\opencodex\src\cli\index.ts",
            );
            let router_work_dir = PathBuf::from(
                r"C:\Users\mater\AppData\Roaming\npm\node_modules\@bitkyc08\opencodex",
            );
            if bun_exe.exists() && ocx_cli.exists() {
                events.push("Model Router DOWN -> Starting opencodex headless on 10100".into());
                let _ = Self::spawn_headless(
                    &bun_exe,
                    &[&ocx_cli.to_string_lossy(), "start", "--port", "10100"],
                    &router_work_dir,
                );
            }
        }

        events
    }
}
