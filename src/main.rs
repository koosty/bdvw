//! Smoke binary: print the ready list ranked by priority against the
//! current workspace. This is ticket bdui-rs-z6a's deliverable.

use bdui::client::{BeadsClient, CliClient};
use std::process::ExitCode;

fn main() -> ExitCode {
    let client = CliClient::new();
    let mut ready = match client.ready() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bdui: {e}");
            return ExitCode::FAILURE;
        }
    };
    ready.sort_by(bdui::model::Bead::ready_rank);
    println!("Ready: {} bead(s)", ready.len());
    for bead in &ready {
        println!("{} P{} [{}] {}", bead.id, bead.priority, bead.issue_type, bead.title);
    }
    ExitCode::SUCCESS
}
