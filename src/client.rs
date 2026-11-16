//! BeadsClient: transport trait for all bd access (CONTEXT.md).
//! v1 implementation is the CLI transport: `bd <cmd> --json` subprocess.

use crate::model::Bead;

/// Transport boundary between the Store and bd. One method per CLI-shaped
/// read; ADR-0001: no direct DB access, no HTTP in v1.
pub trait BeadsClient {
    /// `bd list --json` — all beads, summary rows.
    fn list(&self) -> Result<Vec<Bead>, ClientError>;
    /// `bd ready --json` — ready work per bd's blocker-aware semantics.
    fn ready(&self) -> Result<Vec<Bead>, ClientError>;
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
/// Serialized by the caller (one invocation at a time, per the design).
pub struct CliClient {
    cwd: Option<std::path::PathBuf>,
}

impl CliClient {
    pub fn new() -> Self {
        Self { cwd: None }
    }

    /// Point at a specific beads workspace directory.
    pub fn in_dir(cwd: impl Into<std::path::PathBuf>) -> Self {
        Self { cwd: Some(cwd.into()) }
    }

    fn run_json(&self, args: &[&str]) -> Result<String, ClientError> {
        let mut cmd = std::process::Command::new("bd");
        cmd.args(args).arg("--json");
        if let Some(dir) = &self.cwd {
            cmd.current_dir(dir);
        }
        let out = cmd.output().map_err(|e| ClientError::Spawn(e.to_string()))?;
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
        let out = self.run_json(&["list"])?;
        self.parse(&out)
    }

    fn ready(&self) -> Result<Vec<Bead>, ClientError> {
        let out = self.run_json(&["ready"])?;
        self.parse(&out)
    }
}
