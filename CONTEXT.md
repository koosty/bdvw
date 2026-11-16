# Beads Viewer

A desktop UI for supervising bd (beads) issue-tracker work driven by agents. Built in Rust with gpui-kit; ported from the HTML prototype at `docs/ui/prototype.html`.

## Language

### Core objects

**Bead**:
A single tracked issue in the beads database. Has status, priority (P0–P4), type, assignee, and labels.
_Avoid_: ticket, work item, task (task is a bead type, not a synonym for bead)

**Epic**:
A bead of type `epic` that parents other beads. In the Graph it collapses into a single node; in the Tree it shows a progress bar.

**Dependency**:
A typed link from one bead to another. The five real types: `blocks`, `tracks`, `related`, `parent-child`, `discovered-from`.
_Avoid_: duplicates, supersedes, replies-to (do not exist in bd)

**Memory**:
A persistent project fact stored via `bd remember`, injected by `bd prime`. Listed on the Memory view.

**Message**:
A threadable bead type for agent-to-agent conversation, shown with `bd show --thread`. Rendered on the Messages view, never as a graph edge.

### Derived state

**State**:
The viewer's derived five-value classification of a bead: ready, in progress, blocked, deferred, or closed. Computed from status plus open blockers — a bead with status `open` and open blockers is shown as blocked.
_Avoid_: mixing "state" (derived, visual) with "status" (stored field)

**Ready**:
A bead whose status is `open` with zero open blockers and no parent epic collapsed over it. The home view lists these ranked by priority.

**Unblocks**:
Count of open beads this bead directly blocks. Shown as a badge in list and board views.

### Architecture

**Store**:
The single gpui Entity holding the normalized bead cache plus derived state. Views read only the Store; the Store alone talks to a BeadsClient.

**BeadsClient**:
The transport trait for all bd access (list, ready, detail, history, memories, claim, unclaim, close). Two implementations: CliClient (bd subprocess + JSON) and a future HttpClient (bd serve REST).

**CliClient**:
The v1 BeadsClient that shells out to `bd <cmd> --json`, one invocation at a time, with a 2-second poll on `bd list --json`.

**Workspace**:
One beads database (.beads directory) and its mode: embedded (Dolt in-process; the norm here) or server (Dolt SQL server required for `bd serve`).
