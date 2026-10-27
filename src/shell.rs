use crate::vfs::{Vfs, VfsError};
use crate::commands;
use crate::net;
use std::collections::HashMap;
use wasm_bindgen::JsValue;

pub struct Shell {
    pub vfs: Vfs,
    pub cwd: String,
    pub env: HashMap<String, String>,
}

impl Shell {
    pub fn new() -> Self {
        let mut env = HashMap::new();
        env.insert("HOME".into(), "/root".into());
        env.insert("PATH".into(), "/bin:/usr/bin".into());
        env.insert("USER".into(), "root".into());
        env.insert("TERM".into(), "xterm-256color".into());
        Self {
            vfs: Vfs::new(),
            cwd: "/root".into(),
            env,
        }
    }

    pub fn init(&mut self) -> Result<(), JsValue> {
        // fire-and-forget async init
        let vfs = &mut self.vfs;
        // We cannot easily await here; the first command will trigger load if needed.
        // For simplicity the VFS starts with the seeded tree and persists on every mutation.
        Ok(())
    }

    pub async fn execute(&mut self, line: &str) -> Result<String, JsValue> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(String::new());
        }

        // very simple pipeline / redirect support omitted for clarity;
        // the core commands below are fully functional.

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        let result = match cmd {
            "ls"     => commands::ls(self, &args).await,
            "cd"     => commands::cd(self, &args).await,
            "pwd"    => Ok(self.cwd.clone() + "\n"),
            "cat"    => commands::cat(self, &args).await,
            "mkdir"  => commands::mkdir(self, &args).await,
            "rm"     => commands::rm(self, &args).await,
            "cp"     => commands::cp(self, &args).await,
            "mv"     => commands::mv(self, &args).await,
            "echo"   => Ok(args.join(" ") + "\n"),
            "clear"  => Ok("\x1b[2J\x1b[H".into()),
            "env"    => {
                let mut out = String::new();
                for (k, v) in &self.env {
                    out.push_str(&format!("{}={}\n", k, v));
                }
                Ok(out)
            }
            "export" => {
                if let Some(kv) = args.get(0) {
                    if let Some((k, v)) = kv.split_once('=') {
                        self.env.insert(k.to_string(), v.to_string());
                    }
                }
                Ok(String::new())
            }
            "grep"   => commands::grep(self, &args).await,
            "curl"   => net::curl(&args).await,
            "wget"   => net::wget(self, &args).await,
            "ping"   => net::ping(&args).await,
            "help"   => Ok(HELP.into()),
            _ => {
                // try to run a WASI binary from /bin if present
                let bin_path = format!("/bin/{}", cmd);
                match self.vfs.read_file(&bin_path).await {
                    Ok(wasm_bytes) => {
                        // In a full implementation we would instantiate a WASI runtime
                        // (wasmer / wasmtime with browser polyfills). For this deliverable
                        // we return a clear message; the download path is already present.
                        Ok(format!("[WASI] {} loaded ({} bytes) – runtime integration point\n", cmd, wasm_bytes.len()))
                    }
                    Err(_) => Ok(format!("{}: command not found\n", cmd)),
                }
            }
        };

        result.map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

const HELP: &str = r#"Mobile Linux Shell – built-in commands:
  ls, cd, pwd, cat, mkdir, rm, cp, mv, echo, clear, env, export, grep
  curl, wget, ping
  help
Extra keys (Termux style) are provided by the UI layer.
"#;
