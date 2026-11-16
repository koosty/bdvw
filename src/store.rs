//! Store: the single Entity holding the bead cache (CONTEXT.md).
//! Views read only the Store; the Store alone talks to a BeadsClient.

use crate::client::{BeadsClient, ClientError};
use crate::model::{Bead, State};
use gpui_kit::Context;

pub struct Store {
    beads: Vec<Bead>,
    /// Shared filter surface (bdui-rs-8db); applies to every list view.
    filters: crate::tree::Filters,
    /// Error from the last refresh, if any; drives the sync indicator.
    last_error: Option<String>,
    /// True once at least one successful refresh has completed.
    loaded: bool,
    /// Wall clock of the last successful fetch; drives the live indicator.
    last_success: Option<std::time::Instant>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            beads: Vec::new(),
            filters: crate::tree::Filters::default(),
            last_error: None,
            loaded: false,
            last_success: None,
        }
    }

    /// Apply a fetched bead list directly (poll path). Returns true when
    /// anything changed.
    pub fn apply_beads(&mut self, mut beads: Vec<Bead>, cx: &mut Context<Self>) -> bool {
        beads.sort_by(Bead::ready_rank);
        let changed = self.differs_from(&beads);
        self.beads = beads;
        self.last_error = None;
        self.loaded = true;
        self.last_success = Some(std::time::Instant::now());
        if changed {
            cx.notify();
        }
        changed
    }

    /// Fetch the full bead list from the client. One call, then derive.
    pub fn refresh(&mut self, client: &dyn BeadsClient, cx: &mut Context<Self>) {
        match client.list() {
            Ok(mut beads) => {
                beads.sort_by(Bead::ready_rank);
                self.beads = beads;
                self.last_error = None;
                self.loaded = true;
                self.last_success = Some(std::time::Instant::now());
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

    /// Current shared filters (type/priority/assignee/closed/query).
    pub fn filters(&self) -> &crate::tree::Filters {
        &self.filters
    }

    /// Replace the filters; notifies so every view re-renders.
    pub fn set_filters(
        &mut self,
        filters: crate::tree::Filters,
        cx: &mut Context<Self>,
    ) {
        self.filters = filters;
        cx.notify();
    }

    /// All beads passing the shared filters, ready_rank sorted.
    pub fn filtered(&self) -> Vec<Bead> {
        let mut v: Vec<Bead> = self
            .beads
            .iter()
            .filter(|b| self.filters.matches(b))
            .cloned()
            .collect();
        v.sort_by(Bead::ready_rank);
        v
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

    /// Sync indicator: error, loading, or "live · Ns" since last success.
    pub fn sync_label(&self) -> String {
        Self::sync_label_from(
            self.loaded,
            self.last_error.as_deref(),
            self.last_success,
        )
    }

    /// Pure derivation of the sync-indicator label (S1 seam).
    pub fn sync_label_from(
        loaded: bool,
        error: Option<&str>,
        last_success: Option<std::time::Instant>,
    ) -> String {
        if let Some(err) = error {
            return format!("error: {err}");
        }
        match last_success {
            Some(t) if loaded => format!("live · {}s", t.elapsed().as_secs()),
            _ => "loading…".to_string(),
        }
    }

    /// Record a failed poll/refresh (drives the sync indicator).
    pub fn set_sync_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.last_error = Some(message);
        cx.notify();
    }

    /// True when the bead list differs from a fresh fetch.
    pub fn differs_from(&self, fresh: &[Bead]) -> bool {
        beads_differ(&self.beads, fresh)
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

/// Poll diff (S1 seam): order-insensitive comparison over identity,
/// status, and priority. Title/state-only edits intentionally do not
/// trigger a refresh beyond the list itself.
pub fn beads_differ(a: &[Bead], b: &[Bead]) -> bool {
    fn rank(b: &Bead) -> (&str, &str, u8) {
        (b.id.as_str(), b.status.as_str(), b.priority)
    }
    let a: Vec<_> = a.iter().map(rank).collect();
    let b: Vec<_> = b.iter().map(rank).collect();
    a != b
}
