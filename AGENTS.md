# AKCloud – Agent Instructions

## Overview
**AKCloud** is a self‑hosted file‑sync and tagging server written in Rust.  
It provides:

* A REST API for uploading, downloading, and searching files.  
* Automatic hashing (SHA‑256) and optional MD5 for duplicate detection.  
* A “graveyard” for soft‑deleted objects with compression (zstd).  
* Peer‑to‑peer sync via a lightweight discovery service.  
* Configurable storage, logging, and networking.

All builds are performed in CI (GitHub Actions) using the Rust toolchain.  
Local developers should **not** install Rust globally; rely on the CI for compilation and binary distribution.

---

## Build System

| Item                | Value |
|---------------------|-------|
| Build system        | Rust (cargo) |
| Build command       | `cargo build --release` |
| Binary name         | `akcloud` |
| Binary location     | `target/release/akcloud` |
| Version source       | `Cargo.toml` |
| Homebrew tap        | *(none configured)* |
| Webhook endpoint    | `https://webhook.akinus21.com/webhook/akcloud-build` |

**Important:** The CI workflow automatically runs the above command, packages the binary, and pushes it to the webhook URL. Do **not** run `cargo install` locally.

If you need to verify a change without waiting for CI:

1. Run `cargo check` to catch syntax errors.  
2. Run `cargo test` (if tests exist).  
3. Push the commit and let CI report the final build status.

---

## Git Push Workflow

The `gh` CLI is not authenticated on the host, so push via SSH directly.

```bash
cd /home/akinus/dockge-stacks/dev-stack/projects/AKCloud
git add -A
git commit -m "<description of changes>"
GIT_SSH_COMMAND="ssh -i /home/akinus/.ssh/github -o StrictHostKeyChecking=no" \
git push origin main
```

- **SSH key:** `/home/akinus/.ssh/github`  
- **Repository:** `git@github.com:Akinus21/AKCloud.git`

> **Always push** after making and reviewing changes. CI will build the binary and report any failures.

---

## Secrets & Configuration

| Secret / File                              | Location |
|--------------------------------------------|----------|
| Git SSH key                                 | `/home/akinus/.ssh/github` |
| Project‑specific secrets (DB paths, tokens) | `/home/akinus/dockge-stacks/dev-stack/.secrets` |
| GitHub webhook secret (not set)             | – (run `gh secret set WEBHOOK_SECRET` if needed) |
| GitHub webhook URL                         | `https://webhook.akinus21.com/webhook/akcloud-build` |

When adding new environment variables or secret values, store them in the `.secrets` file and reference them via the `Config` struct (`src/config.rs`).

---

## Project Structure

```
AKCloud/
├── Cargo.toml                     # Crate manifest, version, dependencies
├── Cargo.lock
├── AGENTS.md                     # ← This file
├── README.md
├── src/
│   ├── main.rs                   # Entry point, CLI parsing (clap)
│   ├── config.rs                 # Configuration structs & loader
│   ├── db.rs                     # SQLite wrapper, FileRecord, TagRecord, Stats
│   ├── tagger.rs                 # Hashing utilities (SHA‑256, MD5) & type guessing
│   ├── server.rs                 # Axum HTTP server, routes, multipart handling
│   ├── web.rs                    # Embedded UI (index.html)
│   ├── graveyard.rs              # Soft‑delete handling, compression (zstd)
│   └── sync/
│       ├── mod.rs                # Sync orchestrator
│       ├── identity.rs           # Ed25519 key management
│       └── discovery.rs          # Peer discovery service
├── tests/                        # Integration tests (if any)
└── .github/
    └── workflows/
        └── ci.yml               # CI: cargo build, test, publish binary
```

### Key Files Explained

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI definition (`clap`), logging init (`tracing_subscriber`), starts HTTP server and optional sync service. |
| `src/config.rs` | Loads configuration from `$HOME/.config/akcloud/*.toml` (or defaults). Defines `Config`, `ServerConfig`, `StorageConfig`, `SyncConfig`, `GraveyardConfig`, `LoggingConfig`. |
| `src/db.rs` | Thin wrapper around `rusqlite`. Provides `Database` struct with CRUD for files, tags, and statistics. |
| `src/tagger.rs` | Async hash computation (`compute_file_hash`, `compute_file_md5`) and simple MIME‑type guessing. |
| `src/server.rs` | Axum router: upload (`POST /files`), download (`GET /files/:id`), delete, tag operations, health checks. |
| `src/graveyard.rs` | Handles “deleted” objects: stores original hash, compresses data with `zstd`, and tracks retention. |
| `src/sync/identity.rs` | Generates/loads Ed25519 key pair for signed peer communication. |
| `src/sync/discovery.rs` | In‑memory peer list with periodic broadcast/heartbeat. |
| `src/sync/mod.rs` | Entry point for the P2P sync daemon; spawns discovery and future transport layers. |

---

## Coding Conventions

1. **Formatting** – Run `cargo fmt` before committing.  
2. **Linting** – Run `cargo clippy -- -D warnings` to keep the codebase warning‑free.  
3. **Error handling** – Use `anyhow::Result` for fallible functions; add context with `.context("...")`.  
4. **Async** – All I/O (file, network) should be async (`tokio`). Avoid blocking calls inside async contexts.  
5. **Logging** – Use `tracing` macros (`info!`, `debug!`, `error!`). Initialize subscriber in `main.rs`.  
6. **Database** – All DB interactions go through the `Database` wrapper; use prepared statements (`params!`).  
7. **Security** – Never log raw file contents or secret keys. Store private keys (`identity.key`) with permissions `600`.  
8. **Testing** – Prefer unit tests in the same module (`#[cfg(test)]`). Add integration tests under `tests/` for API endpoints.  

---

## Deployment Checklist

- [ ] Increment version in `Cargo.toml` (`cargo release` can automate).  
- [ ] Verify `config.toml` (or environment overrides) are correct for the target environment.  
- [ ] Ensure the `.secrets` file contains any new keys/tokens and is **not** committed.  
- [ ] Push to `main`; CI will build `target/release/akcloud` and POST to the webhook URL.  
- [ ] After CI succeeds, restart the service on the host (e.g., `systemctl restart akcloud`).  

---

## FAQ

**Q: I need to add a new environment variable for the sync service.**  
A: Add it to `Config::SyncConfig` in `src/config.rs`, update the TOML schema, and reference it via `config.sync.<var>`.

**Q: The webhook secret is missing.**  
A: Run `gh secret set WEBHOOK_SECRET -b"<your-secret>"` in the repository context, then re‑run the CI pipeline.

**Q: How do I generate a new identity key?**  
A: Delete `identity.key` from the config directory and restart the server; `Identity::load_or_generate` will create a fresh Ed25519 key pair.

--- 

*End of AGENTS.md*