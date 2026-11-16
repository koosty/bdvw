//! Store: the single Entity holding the bead cache (CONTEXT.md).
//! Views read only the Store; the Store alone talks to a BeadsClient.

use crate::client::{BeadsClient, ClientError};
use crate::model::{Bead, State};
use gpui_kit::Context;

pub struct Store {
    beads: Vec<Bead>,
    /// Error from the last refresh, if any; drives the sync indicator.
    last_error: Option<String>,
    /// True once at least one successful refresh has completed.
    loaded: bool,
}

impl Store {
    pub fn new() -> Self {
        Self { beads: Vec::new(), last_error: None, loaded: false }
    }

    /// Fetch the full bead list from the client. One call, then derive.
    pub fn refresh(&mut self, client: &dyn BeadsClient, cx: &mut Context<Self>) {
        match client.list() {
            Ok(mut beads) => {
                beads.sort_by(Bead::ready_rank);
                self.beads = beads;
                self.last_error = None;
                self.loaded = true;
            }
            Err(ClientError::Spawn(e)) => {
                self.last_error = Some(format!("bd unavailable: {e}"));
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
            }
        }
        cx.notify();
    }

    pub fn beads(&self) -> &[Bead] {
        &self.beads
    }

    /// Ready work: the home view. bd's own `ready` semantics are the
    /// authority; at list level that means open beads shown as Ready.
    pub fn ready(&self) -> Vec<Bead> {
        self.beads
            .iter()
            .filter(|b| b.state == State::Ready)
            .cloned()
            .collect()
    }

    pub fn sync_label(&self) -> &str {
        if let Some(err) = &self.last_error {
            return err.as_str();
        }
        if self.loaded {
            "synced"
        } else {
            "loading…"
        }
    }

    pub fn is_synced(&self) -> bool {
        self.loaded && self.last_error.is_none()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

