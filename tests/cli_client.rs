//! Seam S2: CliClient against the real `bd` CLI in a throwaway workspace.
//! Verifies the transport end to end: spawn, JSON parse, ready semantics.

use bdvw::client::{BeadsClient, CliClient};
use bdvw::model::State;
use std::process::Command;

/// Create a temp workspace with a two-bead dependency chain:
/// blocker bd-a blocks bd-b, so bd-b is not ready until bd-a closes.
/// Returns the dir (kept alive) and the two bead IDs.
fn workspace_with_chain() -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = |args: &[&str]| {
        let out = Command::new("bd")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("run bd");
        assert!(
            out.status.success(),
            "bd {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    run(&["init"]);
    let blocker = String::from_utf8(run(&["q", "Blocker"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    let blocked = String::from_utf8(run(&["q", "Blocked work"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    run(&["link", &blocked, &blocker]);
    (dir, blocker, blocked)
}

#[test]
fn ready_excludes_beads_with_open_blockers() {
    let (dir, _blocker, _blocked) = workspace_with_chain();
    let client = CliClient::in_dir(dir.path());
    let list = client.list().expect("list works");
    let ready = client.ready().expect("ready works");

    assert_eq!(list.len(), 2, "both beads listed");
    assert_eq!(ready.len(), 1, "only the blocker is ready");
    assert_eq!(ready[0].title, "Blocker");
    assert_eq!(ready[0].state, State::Ready);
    assert!(list.iter().any(|b| b.title == "Blocked work"));
}

#[test]
fn closing_blocker_frees_blocked_bead() {
    let (dir, blocker, _blocked) = workspace_with_chain();
    let run = |args: &[&str]| {
        let out = Command::new("bd")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("run bd");
        assert!(out.status.success(), "bd {:?} failed", args);
        out
    };
    run(&["close", &blocker]);
    let client = CliClient::in_dir(dir.path());
    let ready = client.ready().expect("ready works");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].title, "Blocked work");
}

#[test]
fn list_includes_closed_beads_with_closed_state() {
    let (dir, blocker, _blocked) = workspace_with_chain();
    let out = Command::new("bd")
        .args(["close", &blocker])
        .current_dir(dir.path())
        .output()
        .expect("close");
    assert!(out.status.success());

    let client = CliClient::in_dir(dir.path());
    let all = client.list().expect("list works");
    let closed = all
        .iter()
        .find(|b| b.id == blocker)
        .expect("closed bead is still listed");
    assert_eq!(closed.state, State::Closed);
}
