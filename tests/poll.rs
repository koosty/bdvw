//! S2: live-sync poll path against real `bd` (bdui-rs-7lr).
//! The shell loop itself is UI-bound; these exercise the pieces beneath
//! it: fetching lists repeatedly, observing external mutations, and
//! serialization of concurrent client invocations.

use bdui::client::{BeadsClient, CliClient};
use std::process::Command;

fn workspace_with_bead() -> (tempfile::TempDir, String) {
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
    let id = String::from_utf8(run(&["q", "Poll target"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    (dir, id)
}

#[test]
fn poll_observes_external_claim() {
    let (dir, id) = workspace_with_bead();
    let client = CliClient::in_dir(dir.path());

    let before = client.list().expect("first list");
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].status, "open");

    // "External" mutation: a terminal user claims the bead. `bd list`
    // keeps it visible with status in_progress.
    let out = Command::new("bd")
        .args(["update", &id, "--claim"])
        .current_dir(dir.path())
        .output()
        .expect("claim");
    assert!(out.status.success());

    let after = client.list().expect("second list");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].status, "in_progress");
    // The diff seam sees it.
    assert!(bdui::store::beads_differ(&before, &after));
}

#[test]
fn concurrent_invocations_serialize() {
    let (dir, _id) = workspace_with_bead();
    let client = std::sync::Arc::new(CliClient::in_dir(dir.path()));

    // Two threads issuing overlapping fetches through one shared client:
    // the client's lock must keep every response intact.
    let a = std::thread::spawn({
        let c = client.clone();
        move || c.list()
    });
    let b = std::thread::spawn({
        let c = client.clone();
        move || c.list()
    });
    for r in [a.join().unwrap(), b.join().unwrap()] {
        let beads = r.expect("list under contention");
        assert_eq!(beads.len(), 1);
    }
}

#[test]
fn poll_observes_external_close_as_closed_state() {
    let (dir, id) = workspace_with_bead();
    let client = CliClient::in_dir(dir.path());

    let before = client.list().expect("first list");
    assert_eq!(before.len(), 1);

    // `bd list --all` keeps closed beads, so a close is a state change for
    // the diff seam, not a removal.
    let out = Command::new("bd")
        .args(["close", &id])
        .current_dir(dir.path())
        .output()
        .expect("close");
    assert!(out.status.success());

    let after = client.list().expect("second list");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].state, bdui::model::State::Closed);
    assert!(bdui::store::beads_differ(&before, &after));
}
