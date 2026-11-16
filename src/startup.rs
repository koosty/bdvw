//! Startup argument parsing. Kept out of `main` so it is testable.

use std::path::PathBuf;

pub const USAGE: &str = "\
bdvw - Beads viewer

Usage:
  bdvw [--dir <path>]

Options:
  --dir <path>   Beads workspace directory (default: current directory)
  -h, --help     Show this help
  -V, --version  Show version
";

/// Package version, from the manifest at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the process should do after reading its arguments.
#[derive(Debug, PartialEq, Eq)]
pub enum Startup {
    /// Open the viewer, optionally pointed at a workspace directory.
    Run { dir: Option<PathBuf> },
    /// Print usage and exit successfully.
    Help,
    /// Print the version and exit successfully.
    Version,
}

/// Parse arguments (without the program name). Unknown or incomplete
/// arguments are an error carrying the usage text.
pub fn parse(args: &[String]) -> Result<Startup, String> {
    let mut dir = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Startup::Help),
            "-V" | "--version" => return Ok(Startup::Version),
            "--dir" => {
                let value = rest
                    .next()
                    .ok_or_else(|| format!("--dir needs a path\n\n{USAGE}"))?;
                dir = Some(PathBuf::from(value));
            }
            other => {
                if let Some(value) = other.strip_prefix("--dir=") {
                    if value.is_empty() {
                        return Err(format!("--dir needs a path\n\n{USAGE}"));
                    }
                    dir = Some(PathBuf::from(value));
                } else {
                    return Err(format!("unknown argument '{other}'\n\n{USAGE}"));
                }
            }
        }
    }
    Ok(Startup::Run { dir })
}

/// Canonical workspace directory, rejecting paths that are missing or are
/// not directories.
pub fn resolve_dir(dir: &std::path::Path) -> Result<PathBuf, String> {
    let path = dir
        .canonicalize()
        .map_err(|e| format!("--dir {}: {e}", dir.display()))?;
    if !path.is_dir() {
        return Err(format!("--dir {}: not a directory", dir.display()));
    }
    Ok(path)
}
