use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct DetectResult {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResult {
    pub success: bool,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub error: Option<String>,
    pub error_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResult {
    pub running: bool,
    pub port: Option<u16>,
    pub url: Option<String>,
}

pub struct ServerManager {
    child: Option<Child>,
    running: bool,
    port: Option<u16>,
    url: Option<String>,
    intentional_stop: bool,
    log_tx: Option<mpsc::Sender<String>>,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            child: None,
            running: false,
            port: None,
            url: None,
            intentional_stop: false,
            log_tx: None,
        }
    }

    pub fn set_log_sender(&mut self, tx: mpsc::Sender<String>) {
        self.log_tx = Some(tx);
    }

    fn emit_log(&self, msg: String) {
        if let Some(tx) = &self.log_tx {
            let _ = tx.send(msg);
        }
    }

    pub fn detect_server(&self, saved_path: Option<&str>) -> DetectResult {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(saved) = saved_path {
            candidates.push(PathBuf::from(saved));
        }

        if let Ok(cwd) = std::env::current_dir() {
            if let Some(parent) = cwd.parent() {
                candidates.push(parent.join("Haven"));
                candidates.push(parent.join("haven"));
            }
        }

        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Haven"));
            candidates.push(home.join("Desktop").join("Haven"));
            candidates.push(home.join("Documents").join("Haven"));
            if let Some(config) = dirs::config_dir() {
                candidates.push(config.join("HavenServer"));
                candidates.push(config.join("Haven"));
                candidates.push(config.join("haven"));
            }
            if let Some(data) = dirs::data_local_dir() {
                candidates.push(data.join("Programs").join("Haven"));
                candidates.push(data.join("Programs").join("haven"));
                candidates.push(data.join("Programs").join("HavenServer"));
            }
        }

        for dir in candidates {
            if let Some(result) = inspect_haven_dir(&dir) {
                return result;
            }
        }

        DetectResult {
            found: false,
            path: None,
            version: None,
        }
    }

    pub fn start_server(&mut self, server_dir: &str) -> StartResult {
        if self.running {
            return StartResult {
                success: true,
                port: self.port,
                url: self.url.clone(),
                error: None,
                error_key: None,
            };
        }

        let sjs = Path::new(server_dir).join("server.js");
        if !sjs.exists() {
            return StartResult {
                success: false,
                port: None,
                url: None,
                error: None,
                error_key: Some("server.error.fileNotFound".into()),
            };
        }

        let _ = kill_process_on_port(3000);

        let port = match find_port(3000) {
            Some(p) => p,
            None => {
                return StartResult {
                    success: false,
                    port: None,
                    url: None,
                    error: None,
                    error_key: Some("server.error.noPort".into()),
                }
            }
        };

        let node = which::which("node").unwrap_or_else(|_| PathBuf::from("node"));
        let mut cmd = Command::new(node);
        cmd.args(["--max-old-space-size=512", "--expose-gc", "server.js"])
            .current_dir(server_dir)
            .env("PORT", port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return StartResult {
                    success: false,
                    port: None,
                    url: None,
                    error: Some(e.to_string()),
                    error_key: None,
                }
            }
        };

        let (ready_tx, ready_rx) = mpsc::channel::<bool>();
        let mut is_https = false;

        if let Some(stdout) = child.stdout.take() {
            let tx = self.log_tx.clone();
            let ready_tx = ready_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    if line.to_lowercase().contains("https enabled") {
                        let _ = ready_tx.send(true);
                    }
                    if line.to_lowercase().contains("listening")
                        || line.to_lowercase().contains("running")
                        || line.to_lowercase().contains("started")
                    {
                        let _ = ready_tx.send(false);
                    }
                    if let Some(tx) = &tx {
                        let _ = tx.send(format!("{line}\n"));
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let tx = self.log_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    if let Some(tx) = &tx {
                        let _ = tx.send(format!("[ERR] {line}\n"));
                    }
                }
            });
        }

        let start = Instant::now();
        loop {
            if let Ok(https_flag) = ready_rx.try_recv() {
                is_https = https_flag || is_https;
                break;
            }
            if start.elapsed() > Duration::from_secs(15) {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let protocol = if is_https { "https" } else { "http" };
        let url = format!("{protocol}://localhost:{port}");
        self.child = Some(child);
        self.running = true;
        self.port = Some(port);
        self.url = Some(url.clone());
        self.intentional_stop = false;

        StartResult {
            success: true,
            port: Some(port),
            url: Some(url),
            error: None,
            error_key: None,
        }
    }

    pub fn stop_server(&mut self) -> StatusResult {
        self.intentional_stop = true;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.running = false;
        self.port = None;
        self.url = None;
        StatusResult {
            running: false,
            port: None,
            url: None,
        }
    }

    pub fn status(&self) -> StatusResult {
        StatusResult {
            running: self.running,
            port: self.port,
            url: self.url.clone(),
        }
    }

    pub fn set_saved_path_hint(&mut self, _path: &str) {}
}

fn inspect_haven_dir(dir: &Path) -> Option<DetectResult> {
    let sjs = dir.join("server.js");
    let pkg = dir.join("package.json");
    if !(sjs.exists() && pkg.exists()) {
        return None;
    }
    let raw = std::fs::read_to_string(&pkg).ok()?;
    let cleaned = raw.trim_start_matches('\u{feff}');
    let json: Value = serde_json::from_str(cleaned).ok()?;
    if json.get("name").and_then(|v| v.as_str()) != Some("haven") {
        return None;
    }
    Some(DetectResult {
        found: true,
        path: Some(dir.display().to_string()),
        version: json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

fn find_port(start: u16) -> Option<u16> {
    for p in start..start.saturating_add(100) {
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return Some(p);
        }
    }
    None
}

fn kill_process_on_port(port: u16) {
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("lsof").args(["-ti", &format!(":{port}")]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for pid in text.split_whitespace() {
                let _ = Command::new("kill").args(["-9", pid]).status();
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = port;
    }
}
