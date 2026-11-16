//! Entry point: open the Beads Viewer window.

use bdui::app::AppShell;
use bdui::client::{BeadsClient as _, CliClient};
use bdui::drawer::{DetailClient, Drawer};
use bdui::store::Store;
use gpui_kit::AppContext as _;
use gpui_kit::WindowOptions;
use std::rc::Rc;

struct CliShim {
    client: CliClient,
}

impl bdui::app::BeadsClientShim for CliShim {
    fn refresh(&self, store: &mut Store, cx: &mut gpui_kit::Context<Store>) {
        store.refresh(&self.client, cx);
    }

    fn graph(&self) -> Result<bdui::graph::GraphData, String> {
        self.client.graph().map_err(|e| e.to_string())
    }
}

impl DetailClient for CliShim {
    fn detail(&self, id: &str) -> Result<bdui::model::BeadDetail, String> {
        bdui::client::BeadsClient::detail(&self.client, id).map_err(|e| e.to_string())
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                let client = Rc::new(CliShim { client: CliClient::new() });
                let store = cx.new(|_| Store::new());
                let drawer = cx.new(|_| Drawer::new(client.clone()));
                cx.new(|cx| AppShell::new(store, client, drawer, window, cx))
            })
            .expect("failed to open window");
        });
}
