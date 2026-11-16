//! Seam S2: CliClient::graph against a real bd workspace.

use bdvw::client::{BeadsClient, CliClient};
use bdvw::graph::{layout_graph, EdgeStyle};
use std::process::Command;

fn workspace() -> (tempfile::TempDir, String, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let init = Command::new("bd")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("run bd init");
    assert!(init.status.success(), "bd init failed");
    let q = |title: &str| -> String {
        let out = Command::new("bd")
            .args(["q", title])
            .current_dir(dir.path())
            .output()
            .expect("run bd");
        assert!(
            out.status.success(),
            "bd q {title} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout)
            .expect("utf8")
            .trim()
            .to_string()
    };
    let alpha = q("Alpha root");
    let beta = q("Beta middle");
    let gamma = q("Gamma tip");
    // alpha blocks beta, beta blocks gamma.
    for (blocked, blocker) in [(&beta, &alpha), (&gamma, &beta)] {
        let out = Command::new("bd")
            .args(["link", blocked, blocker])
            .current_dir(dir.path())
            .output()
            .expect("run bd link");
        assert!(
            out.status.success(),
            "bd link failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    (dir, alpha, beta, gamma)
}

#[test]
fn graph_lists_all_nodes_with_blocker_first_edges() {
    let (dir, alpha, beta, _gamma) = workspace();
    let client = CliClient::in_dir(dir.path());
    let data = client.graph().expect("graph fetch");
    assert_eq!(data.nodes.len(), 3, "every bead appears once");
    assert_eq!(data.edges.len(), 2, "both links");
    // Edge direction: blocker -> blocked.
    assert!(data
        .edges
        .iter()
        .any(|e| e.from == alpha && e.to == beta && e.kind == "blocks"));
}

#[test]
fn graph_layout_layers_the_chain() {
    let (dir, alpha, beta, gamma) = workspace();
    let client = CliClient::in_dir(dir.path());
    let laid = layout_graph(&client.graph().expect("graph fetch"));
    let layer = |id: &str| {
        laid.nodes
            .iter()
            .find(|n| n.bead.id == id)
            .expect("node present")
            .layer
    };
    assert_eq!(layer(&alpha), 0);
    assert_eq!(layer(&beta), 1);
    assert_eq!(layer(&gamma), 2);
    assert!(laid
        .edges
        .iter()
        .any(|e| e.from == beta && e.to == gamma && e.style == EdgeStyle::Solid));
}
