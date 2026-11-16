# Issue Tracker: bd (beads)

Issues live in a local **bd (beads)** database, not a hosted tracker. The `bd` CLI is the only interface; there is no web UI and no remote to sync to.

## Creating issues

- Create one issue: `bd create "Title" --desc "..."`
- Quick capture, prints only the ID: `bd q "Title"`
- Batch from markdown: `bd create --batch <file.md>`

## Picking up work

- `bd ready` — list issues whose dependencies are all closed
- `bd update <id> --claim` — atomically claim an issue and move it to in_progress
- `bd heartbeat` — refresh the lease on an issue you hold in_progress
- `bd reclaim` — revert stale-lease in_progress issues back to ready (dead-worker recovery)

## Finishing work

- `bd close <id>` — close an issue
- `bd update <id> --unclaim` — release a claimed issue without closing it

## Notes for skills

- Skills like `to-tickets`, `triage`, and `to-spec` should create issues with `bd create` / `bd q` and query with `bd ready`, `bd list`, `bd query`, and `bd search`.
- `bd init` initializes the database if it doesn't exist; run it before the first create in a fresh repo. Set `BEADS_DIR` to use a non-default location.
- Use `bd link` to record dependencies between issues.
- Use `bd remember "insight"` for durable project facts (per AGENTS.md).
- Do not use markdown TODO lists for task tracking (per AGENTS.md).
