//! Entry point: open the Beads Viewer window.

use bdui::app::AppShell;
use bdui::client::{BeadsClient as _, CliClient};
use bdui::drawer::{DetailClient, Drawer};
use bdui::store::Store;
use gpui_kit::AppContext as _;
use gpui_kit::{WindowDecorations, WindowOptions};
use std::rc::Rc;
use std::sync::Arc;

struct CliShim {
    /// Shared with the poll loop; the client serializes invocations.
    client: Arc<CliClient>,
}

impl bdui::app::BeadsClientShim for CliShim {
    fn refresh(&self, store: &mut Store, cx: &mut gpui_kit::Context<Store>) {
        store.refresh(self.client.as_ref(), cx);
    }

    fn graph(&self) -> Result<bdui::graph::GraphData, String> {
        self.client.graph().map_err(|e| e.to_string())
    }

    fn poll_client(&self) -> std::sync::Arc<CliClient> {
        self.client.clone()
    }
}

impl DetailClient for CliShim {
    fn detail(&self, id: &str) -> Result<bdui::model::BeadDetail, String> {
        bdui::client::BeadsClient::detail(self.client.as_ref(), id)
            .map_err(|e| e.to_string())
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_decorations: Some(WindowDecorations::Server),
                    ..WindowOptions::default()
                },
                cx,
                |window, cx| {
                let cli = Arc::new(CliClient::new());
                let client = Rc::new(CliShim { client: cli.clone() });
                let store = cx.new(|_| Store::new());
                let drawer = cx.new(|_| Drawer::new(Arc::new(CliShim { client: cli.clone() })));
                cx.new(|cx| AppShell::new(store, client, drawer, window, cx))
            })
            .expect("failed to open window");
        });
}
