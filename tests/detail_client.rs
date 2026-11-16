//! Seam S2: CliClient::detail against the real `bd` CLI in a throwaway
//! workspace. Companion to the list/ready integration tests.

use bdui::client::{BeadsClient, CliClient};
use std::process::Command;

/// Chain: alpha blocks beta; beta has a comment.
fn workspace() -> (tempfile::TempDir, String, String) {
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
    let alpha = String::from_utf8(run(&["q", "Alpha"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    let beta = String::from_utf8(run(&["q", "Beta"]).stdout)
        .expect("utf8")
        .trim()
        .to_string();
    run(&["link", &beta, &alpha]); // alpha blocks beta
    run(&["comment", &beta, "noise from the cheap seats"]);
    (dir, alpha, beta)
}

#[test]
fn detail_reports_dependencies_and_comments() {
    let (dir, _alpha, beta) = workspace();
    let client = CliClient::in_dir(dir.path());

    let detail = client.detail(&beta).expect("detail for blocked bead");
    assert_eq!(detail.id, beta);
    assert_eq!(detail.dependencies.len(), 1);
    assert_eq!(detail.dependencies[0].title, "Alpha");
    assert_eq!(detail.dependencies[0].dependency_type, "blocks");
    assert_eq!(detail.comments.len(), 1);
    assert_eq!(detail.comments[0].text, "noise from the cheap seats");
}

#[test]
fn detail_reports_dependents_on_the_blocker_side() {
    let (dir, alpha, _beta) = workspace();
    let client = CliClient::in_dir(dir.path());

    let detail = client.detail(&alpha).expect("detail for blocker");
    assert_eq!(detail.id, alpha);
    assert!(detail.dependencies.is_empty());
    assert_eq!(detail.dependents.len(), 1);
    assert_eq!(detail.dependents[0].title, "Beta");
}
