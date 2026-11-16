# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0-alpha.1] - 2026-11-16

First public alpha. Read-only: the viewer never writes to a bead database.

### Added

- **Ready** view ranked by priority, with a suggested next bead and copyable
  claim commands.
- **Tree** view with epic progress and ancestor retention under filters.
- **Board** view as a kanban over derived state; the closed column is capped
  at the 30 most recent beads.
- **Graph** view: layered DAG with pan, zoom, fit, epic collapse, and
  hover focus.
- **Activity** view: day-grouped event timeline across all beads.
- **Detail drawer** with the full record, dependency links, and copyable
  commands.
- **Live sync** polling `bd` every 2 s, refreshing the drawer and graph only
  on change.
- Shared filter surface across list views: text query, type, priority,
  assignee, and a closed toggle.
- Command palette (`Ctrl/Cmd+K`, `/`, and `g`-prefixed view chords).
- `--dir` to point the viewer at a workspace other than the working
  directory, plus `--help`.
- Statuses outside bd's built-in set (`pinned`, `hooked`,
  `bd config status.custom`) display their raw label with a neutral color.

### Known limitations

- **Memory and Messages views** are not implemented; `g m` shows a
  placeholder.
- **In-app writes** are not supported. Claim and close are copyable
  commands, not buttons.
- **Workspace switching** requires restarting with `--dir`.
- **Linux only.** Developed and verified on Wayland; X11 is untested.
- Activity events are structural only. Comments produce no `bd history`
  events, so they never appear in the feed.

[0.1.0-alpha.1]: https://github.com/koosty/bdvw/releases/tag/v0.1.0-alpha.1
