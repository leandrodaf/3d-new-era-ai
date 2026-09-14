//! Plugins: external programs that extend the editor through its public
//! HTTP API, in any language.
//!
//! A plugin is a directory with a `plugin.json`:
//!
//! ```json
//! {
//!   "name": "quadro-areas",
//!   "title": "Quadro de áreas",
//!   "description": "Adds a text with the area of every room.",
//!   "command": ["python3", "main.py"]
//! }
//! ```
//!
//! Running it starts `command` inside that directory with
//! - `NEWERA_URL` — base URL of the API (`GET /api/home`, `POST /api/commands`, `/mcp`…)
//! - `NEWERA_TOKEN` — bearer token, when the server has one
//! - `NEWERA_SESSION` — a session id, so edits show up as the plugin's
//! - arguments as JSON on standard input.
//!
//! Standard output is returned to whoever ran it (menu, REST or MCP).
//!
//! Plugins are looked up in every directory of `NEWERA_PLUGINS` and in
//! `<config>/3d-new-era-ai/plugins`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Manifest file inside a plugin directory.
pub const MANIFEST: &str = "plugin.json";
/// Longest time a plugin may run.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// Output kept from each stream.
const MAX_OUTPUT: usize = 64 * 1024;

/// A plugin found on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    /// Identifier: lowercase letters, digits, `-` and `_`.
    pub name: String,
    /// Name shown in menus.
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Program and arguments, run inside the plugin directory.
    pub command: Vec<String>,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Plugin {
    /// Menu title, falling back to the name.
    pub fn label(&self) -> &str {
        if self.title.is_empty() {
            &self.name
        } else {
            &self.title
        }
    }
}

/// Where the plugin reaches the editor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Host {
    pub url: String,
    pub token: Option<String>,
    pub session: Option<String>,
}

/// What a finished run produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunOutput {
    /// Exit code; `None` when killed (timeout) or ended by a signal.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl RunOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
}

/// Directories searched for plugins, in order.
pub fn plugin_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("NEWERA_PLUGINS")
        .map(|v| std::env::split_paths(&v).collect())
        .unwrap_or_default();
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    if let Some(config) = config {
        dirs.push(config.join("3d-new-era-ai").join("plugins"));
    }
    dirs
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Reads one plugin directory.
///
/// # Errors
/// When the manifest is missing or invalid.
pub fn load(dir: &Path) -> Result<Plugin, String> {
    let file = dir.join(MANIFEST);
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut plugin: Plugin =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    if !valid_name(&plugin.name) {
        return Err(format!(
            "{}: invalid name `{}`",
            file.display(),
            plugin.name
        ));
    }
    if plugin.command.is_empty() {
        return Err(format!("{}: empty command", file.display()));
    }
    plugin.dir = dir.to_path_buf();
    Ok(plugin)
}

/// Every valid plugin under `dirs` (first one wins on duplicate names),
/// sorted by name. Invalid manifests are skipped.
pub fn discover(dirs: &[PathBuf]) -> Vec<Plugin> {
    let mut found: Vec<Plugin> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths.into_iter().filter(|p| p.join(MANIFEST).is_file()) {
            if let Ok(plugin) = load(&path)
                && !found.iter().any(|p| p.name == plugin.name)
            {
                found.push(plugin);
            }
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Finds a plugin by name.
pub fn find(name: &str) -> Option<Plugin> {
    discover(&plugin_dirs())
        .into_iter()
        .find(|p| p.name == name)
}

fn read_capped(mut stream: impl Read) -> String {
    let mut buf = Vec::new();
    let _ = stream
        .by_ref()
        .take(MAX_OUTPUT as u64)
        .read_to_end(&mut buf);
    // Drain the rest so the child never blocks on a full pipe.
    let _ = std::io::copy(&mut stream, &mut std::io::sink());
    String::from_utf8_lossy(&buf).into_owned()
}

/// Runs `plugin` with `args` on standard input and waits up to `timeout`.
///
/// # Errors
/// When the program can't be started.
pub fn run(
    plugin: &Plugin,
    host: &Host,
    args: &serde_json::Value,
    timeout: Duration,
) -> Result<RunOutput, String> {
    let (program, rest) = plugin.command.split_first().ok_or("empty command")?;
    // A program given relative to the plugin (`./run.sh`) is found there.
    let local = plugin.dir.join(program);
    let program = if program.contains('/') && local.exists() {
        local.into_os_string()
    } else {
        program.into()
    };
    let mut command = Command::new(program);
    command
        .args(rest)
        .current_dir(&plugin.dir)
        .env("NEWERA_URL", &host.url)
        .env("NEWERA_PLUGIN_DIR", &plugin.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match &host.token {
        Some(token) => command.env("NEWERA_TOKEN", token),
        None => command.env_remove("NEWERA_TOKEN"),
    };
    match &host.session {
        Some(session) => command.env("NEWERA_SESSION", session),
        None => command.env_remove("NEWERA_SESSION"),
    };
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start `{}`: {e}", plugin.command.join(" ")))?;
    if let Some(mut stdin) = child.stdin.take() {
        let input = args.to_string();
        std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    let stdout = child
        .stdout
        .take()
        .map(|s| std::thread::spawn(move || read_capped(s)));
    let stderr = child
        .stderr
        .take()
        .map(|s| std::thread::spawn(move || read_capped(s)));

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() >= timeout => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(err) => return Err(err.to_string()),
        }
    };
    let join = |handle: Option<std::thread::JoinHandle<String>>| {
        handle.and_then(|h| h.join().ok()).unwrap_or_default()
    };
    Ok(RunOutput {
        code: if timed_out {
            None
        } else {
            status.and_then(|s| s.code())
        },
        stdout: join(stdout),
        stderr: join(stderr),
        timed_out,
    })
}

/// Why a plugin could not run for a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    NotFound(String),
    /// The document has no HTTP server to call back into.
    NoServer,
    Start(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "no plugin {name}"),
            Self::NoServer => {
                f.write_str("plugins need the HTTP server (start the editor or `newera serve`)")
            }
            Self::Start(err) => f.write_str(err),
        }
    }
}

impl std::error::Error for RunError {}

/// Runs plugin `name` (searched in `dirs`) against the server of `document`
/// under a session of its own, and reports its output, how many edits it
/// made and the revision it left.
///
/// The document lock is not held while the plugin runs, so it can call back.
///
/// # Errors
/// See [`RunError`].
pub fn run_for_document(
    document: &newera_core::SharedDocument,
    dirs: &[PathBuf],
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, RunError> {
    use newera_core::collab::now_ms;
    let plugin = discover(dirs)
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| RunError::NotFound(name.to_owned()))?;
    let (server, session) = {
        let mut doc = document.write();
        let server = doc.server().cloned().ok_or(RunError::NoServer)?;
        let session = doc.sessions_mut().join(plugin.label(), now_ms());
        (server, session.id)
    };
    let host = Host {
        url: server.url,
        token: server.token,
        session: Some(session.clone()),
    };
    let result = run(&plugin, &host, args, DEFAULT_TIMEOUT);
    let mut doc = document.write();
    let edits = doc.sessions().get(&session).map_or(0, |s| s.edits);
    doc.sessions_mut().leave(&session);
    let output = result.map_err(RunError::Start)?;
    Ok(serde_json::json!({
        "ok": output.success(),
        "code": output.code,
        "timed_out": output.timed_out,
        "stdout": output.stdout,
        "stderr": output.stderr,
        "edits": edits,
        "revision": doc.revision(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("newera-plugins-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_plugin(root: &Path, dir: &str, manifest: &str) -> PathBuf {
        let path = root.join(dir);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(MANIFEST), manifest).unwrap();
        path
    }

    #[test]
    fn discovers_valid_plugins_first_directory_wins() {
        let a = temp_dir("a");
        let b = temp_dir("b");
        write_plugin(
            &a,
            "one",
            r#"{"name":"one","title":"Um","command":["true"]}"#,
        );
        write_plugin(&a, "bad", r#"{"name":"Bad Name","command":["true"]}"#);
        write_plugin(&a, "empty", r#"{"name":"empty","command":[]}"#);
        write_plugin(
            &b,
            "one",
            r#"{"name":"one","title":"Outro","command":["true"]}"#,
        );
        write_plugin(&b, "two", r#"{"name":"two","command":["true"]}"#);
        let found = discover(&[a.clone(), b.clone(), a.join("missing")]);
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["one", "two"]);
        assert_eq!(found[0].label(), "Um");
        assert_eq!(found[1].label(), "two");
        std::fs::remove_dir_all(a).ok();
        std::fs::remove_dir_all(b).ok();
    }

    #[cfg(unix)]
    #[test]
    fn runs_with_the_host_environment_and_stdin() {
        let root = temp_dir("run");
        let dir = write_plugin(
            &root,
            "echo",
            r#"{"name":"echo","command":["sh","-c","printf '%s|%s|%s|' \"$NEWERA_URL\" \"$NEWERA_TOKEN\" \"$NEWERA_SESSION\"; cat; echo oops >&2; exit 3"]}"#,
        );
        let plugin = load(&dir).unwrap();
        let host = Host {
            url: "http://127.0.0.1:7878".into(),
            token: Some("t0k".into()),
            session: Some("s4".into()),
        };
        let out = run(
            &plugin,
            &host,
            &serde_json::json!({"n": 2}),
            DEFAULT_TIMEOUT,
        )
        .unwrap();
        assert_eq!(out.stdout, r#"http://127.0.0.1:7878|t0k|s4|{"n":2}"#);
        assert_eq!(out.stderr.trim(), "oops");
        assert_eq!(out.code, Some(3));
        assert!(!out.success());

        let slow = write_plugin(&root, "slow", r#"{"name":"slow","command":["sleep","5"]}"#);
        let out = run(
            &load(&slow).unwrap(),
            &host,
            &serde_json::Value::Null,
            Duration::from_millis(200),
        )
        .unwrap();
        assert!(out.timed_out && out.code.is_none());

        let missing = write_plugin(
            &root,
            "missing",
            r#"{"name":"missing","command":["./nope"]}"#,
        );
        assert!(
            run(
                &load(&missing).unwrap(),
                &host,
                &serde_json::Value::Null,
                DEFAULT_TIMEOUT
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).ok();
    }
}
