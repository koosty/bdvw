//! Entry point: open the Beads Viewer window.

use bdvw::app::AppShell;
use bdvw::client::{BeadsClient as _, CliClient};
use bdvw::drawer::{DetailClient, Drawer};
use bdvw::store::Store;
use gpui_kit::component::TitleBar;
use gpui_kit::AppContext as _;
use gpui_kit::{WindowDecorations, WindowOptions};
use std::rc::Rc;
use std::sync::Arc;

struct CliShim {
    /// Shared with the poll loop; the client serializes invocations.
    client: Arc<CliClient>,
}

impl bdvw::app::BeadsClientShim for CliShim {
    fn refresh(&self, store: &mut Store, cx: &mut gpui_kit::Context<Store>) {
        store.refresh(self.client.as_ref(), cx);
    }

    fn graph(&self) -> Result<bdvw::graph::GraphData, String> {
        self.client.graph().map_err(|e| e.to_string())
    }

    fn poll_client(&self) -> std::sync::Arc<CliClient> {
        self.client.clone()
    }
}

impl DetailClient for CliShim {
    fn detail(&self, id: &str) -> Result<bdvw::model::BeadDetail, String> {
        bdvw::client::BeadsClient::detail(self.client.as_ref(), id).map_err(|e| e.to_string())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match bdvw::startup::parse(&args) {
        Ok(bdvw::startup::Startup::Help) => {
            println!("{}", bdvw::startup::USAGE);
            return;
        }
        Ok(bdvw::startup::Startup::Version) => {
            println!("bdvw {}", bdvw::startup::VERSION);
            return;
        }
        Ok(bdvw::startup::Startup::Run { dir: Some(dir) }) => {
            let dir = match bdvw::startup::resolve_dir(&dir) {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            };
            // bd, the client, and the sidebar's workspace label all read the
            // process directory, so point it at the requested workspace.
            if let Err(e) = std::env::set_current_dir(&dir) {
                eprintln!("--dir {}: {e}", dir.display());
                std::process::exit(2);
            }
        }
        Ok(bdvw::startup::Startup::Run { dir: None }) => {}
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_decorations: Some(WindowDecorations::Client),
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    window.set_window_title("Beads Viewer");
                    let cli = Arc::new(CliClient::new());
                    let client = Rc::new(CliShim {
                        client: cli.clone(),
                    });
                    let store = cx.new(|_| Store::new());
                    let drawer = cx.new(|_| {
                        Drawer::new(Arc::new(CliShim {
                            client: cli.clone(),
                        }))
                    });
                    cx.new(|cx| AppShell::new(store, client, drawer, window, cx))
                },
            )
            .expect("failed to open window");
        });
}
