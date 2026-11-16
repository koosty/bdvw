---
status: accepted
date: 2026-11-16
---

# ADR-0001: Transport via bd subprocess, not direct DB access

The viewer talks to bd only through the `bd` CLI with `--json` (CliClient), behind a `BeadsClient` trait. Rejected alternatives:

- **Direct .beads file access**: embedded Dolt stores data as proprietary noms chunks under `.beads/embeddeddolt/`, not queryable by any Rust library, and the workspace gate lock makes concurrent reads unsafe for a live-sync view.
- **`bd serve` HTTP API now**: the current workspace is embedded mode and this machine lacks the `dolt` binary; `bd serve` refuses to start in embedded mode. Re-initializing the workspace to server mode before the UI exists inverts the dependency.

A future `HttpClient` implementing the same trait can replace CliClient without UI changes if the workspace moves to server mode.
