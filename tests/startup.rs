//! S1: startup argument parsing (--dir workspace selection).

use bdui::startup::{parse, resolve_dir, Startup};
use std::path::PathBuf;

fn args(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| s.to_string()).collect()
}

#[test]
fn no_arguments_runs_in_current_directory() {
    assert_eq!(parse(&[]), Ok(Startup::Run { dir: None }));
}

#[test]
fn dir_flag_accepts_separate_and_inline_forms() {
    assert_eq!(
        parse(&args(&["--dir", "/tmp/ws"])),
        Ok(Startup::Run { dir: Some(PathBuf::from("/tmp/ws")) })
    );
    assert_eq!(
        parse(&args(&["--dir=/tmp/ws"])),
        Ok(Startup::Run { dir: Some(PathBuf::from("/tmp/ws")) })
    );
}

#[test]
fn help_wins_and_short_circuits() {
    assert_eq!(parse(&args(&["--help"])), Ok(Startup::Help));
    assert_eq!(parse(&args(&["-h"])), Ok(Startup::Help));
}

#[test]
fn bad_arguments_are_errors_with_usage() {
    let missing = parse(&args(&["--dir"])).unwrap_err();
    assert!(missing.contains("--dir needs a path"), "{missing}");
    let unknown = parse(&args(&["--verbose"])).unwrap_err();
    assert!(unknown.contains("unknown argument '--verbose'"), "{unknown}");
    assert!(unknown.contains("Usage:"), "{unknown}");
    let empty = parse(&args(&["--dir="])).unwrap_err();
    assert!(empty.contains("--dir needs a path"), "{empty}");
}

#[test]
fn resolve_rejects_missing_paths_and_files() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(resolve_dir(dir.path()).unwrap(), dir.path().canonicalize().unwrap());

    let file = dir.path().join("file.txt");
    std::fs::write(&file, b"x").unwrap();
    assert!(resolve_dir(&file).unwrap_err().contains("not a directory"));

    let missing = dir.path().join("nope");
    assert!(resolve_dir(&missing).is_err());
}
