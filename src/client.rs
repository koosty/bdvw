//! BeadsClient: transport trait for all bd access (CONTEXT.md).
//! v1 implementation is the CLI transport: `bd <cmd> --json` subprocess.

use crate::graph::GraphData;
use crate::model::{Bead, BeadDetail};

/// Transport boundary between the Store and bd. One method per CLI-shaped
/// read; ADR-0001: no direct DB access, no HTTP in v1.
pub trait BeadsClient {
    /// `bd list --json` — all beads, summary rows.
    fn list(&self) -> Result<Vec<Bead>, ClientError>;
    /// `bd ready --json` — ready work per bd's blocker-aware semantics.
    fn ready(&self) -> Result<Vec<Bead>, ClientError>;
    /// `bd show <id> --json --long --include-comments --include-dependents`
    /// — full record for the detail drawer.
    fn detail(&self, id: &str) -> Result<BeadDetail, ClientError>;
    /// `bd graph --all --json` — connected components with full records
    /// and dependency edges for the DAG view.
    fn graph(&self) -> Result<GraphData, ClientError>;
    /// `bd history <id> --events --json` — audit events for one bead
    /// (activity feed, bdui-rs-2u6).
    fn history(&self, id: &str) -> Result<Vec<crate::activity::ActivityEvent>, ClientError>;
}

#[derive(Debug)]
pub enum ClientError {
    Spawn(String),
    Output(String),
    Parse(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(s) => write!(f, "failed to run bd: {s}"),
            Self::Output(s) => write!(f, "bd exited with error: {s}"),
            Self::Parse(s) => write!(f, "could not parse bd output: {s}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// CLI transport: runs `bd` as a subprocess with `--json`.
///
/// A process-wide lock serializes every invocation (bdui-rs-7lr): polls,
/// detail fetches, and (later) writes never interleave. `bd` itself is safe
/// to run concurrently, but one-at-a-time keeps ordering obvious and the
/// workspace lock uncontended.
pub struct CliClient {
    cwd: Option<std::path::PathBuf>,
    /// Held for the duration of each subprocess run.
    lock: std::sync::Mutex<()>,
}

impl CliClient {
    pub fn new() -> Self {
        Self {
            cwd: None,
            lock: std::sync::Mutex::new(()),
        }
    }

    /// Point at a specific beads workspace directory.
    pub fn in_dir(cwd: impl Into<std::path::PathBuf>) -> Self {
        Self {
            cwd: Some(cwd.into()),
            lock: std::sync::Mutex::new(()),
        }
    }

    fn run_json(&self, args: &[&str]) -> Result<String, ClientError> {
        // One bd invocation at a time, process-wide.
        let _guard = self.lock.lock().expect("bd client lock poisoned");
        let mut cmd = std::process::Command::new("bd");
        cmd.args(args).arg("--json");
        if let Some(dir) = &self.cwd {
            cmd.current_dir(dir);
        }
        let out = cmd
            .output()
            .map_err(|e| ClientError::Spawn(e.to_string()))?;
        if !out.status.success() {
            return Err(ClientError::Output(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        String::from_utf8(out.stdout).map_err(|e| ClientError::Parse(e.to_string()))
    }

    fn parse(&self, json: &str) -> Result<Vec<Bead>, ClientError> {
        crate::model::parse_summary(json).map_err(ClientError::Parse)
    }
}

impl Default for CliClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BeadsClient for CliClient {
    fn list(&self) -> Result<Vec<Bead>, ClientError> {
        let out = self.run_json(&["list", "--all"])?;
        self.parse(&out)
    }

    fn ready(&self) -> Result<Vec<Bead>, ClientError> {
        let out = self.run_json(&["ready"])?;
        self.parse(&out)
    }

    fn history(&self, id: &str) -> Result<Vec<crate::activity::ActivityEvent>, ClientError> {
        let json = self.run_json(&["history", id, "--events"])?;
        crate::activity::parse_events(&json).map_err(ClientError::Parse)
    }

    fn detail(&self, id: &str) -> Result<BeadDetail, ClientError> {
        let out = self.run_json(&[
            "show",
            id,
            "--long",
            "--include-comments",
            "--include-dependents",
        ])?;
        crate::model::parse_detail(&out).map_err(ClientError::Parse)
    }

    fn graph(&self) -> Result<GraphData, ClientError> {
        let out = self.run_json(&["graph", "--all"])?;
        crate::graph::parse_components(&out).map_err(ClientError::Parse)
    }
}
