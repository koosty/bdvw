//! Entry point: open the Beads Viewer window.

use bdui::app::AppShell;
use bdui::client::CliClient;
use bdui::store::Store;
use gpui_kit::AppContext as _;
use gpui_kit::WindowOptions;
use std::rc::Rc;

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
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                let client = Rc::new(CliShim { client: CliClient::new() });
                let store = cx.new(|_| Store::new());
                cx.new(|cx| AppShell::new(store, client, cx))
            })
            .expect("failed to open window");
        });
}
