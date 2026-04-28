```markdown
# AKCloud Rust Refactor - Agent Instructions

## Overview

This is the Rust rewrite of AKCloud - a self-hosted file sync and tagging server.

## Building

**IMPORTANT: Do NOT install Rust locally.** GitHub Actions handles all building automatically. The CI workflow builds the binary and reports any errors back to you.

If you need to verify code changes without building:
1. Review the code logic manually
2. Check for syntax errors by reading the files
3. Push to GitHub and wait for the build results

## Git Push Workflow

Since gh CLI is not authenticated, use SSH directly:

```bash
cd /home/akinus/dockge-stacks/dev-stack/projects/AKCloud
git add -A
git commit -m "<description>"
GIT_SSH_COMMAND="ssh -i /home/akinus/.ssh/github -o StrictHostKeyChecking=no" git push origin main
```

**IMPORTANT: Always push to GitHub after making and verifying changes.**

## Documentation Updates

**IMPORTANT: Update README.md when adding new features or changing existing features.**

The README should reflect:
- New commands added
- Changed command behavior
- Updated installation instructions
- New use cases or examples

## Project Structure

```
AKCloud/
├── Cargo.toml
├── README.md
├── AGENTS.md
└── src/
    ├── db.rs
    ├── graveyard.rs
    ├── server.rs
    ├── tagger.rs
    ├── web.rs
    ├── config.rs
    ├── sync/
    │   ├── mod.rs
    │   ├── identity.rs
    │   ├── discovery.rs
    │   └── ...
```

## Module Structure

Modules are stored in `~/.aktools/modules/`:
- Each module is a folder
- Contains `manifest.xml` with metadata
- May contain scripts or resources

### manifest.xml Format

```xml
<?xml version="1.0"?>
<module>
    <name>modulename</name>
    <alias>alias</alias>
    <executable>./script.sh</executable>
    <option>
        <flag>flagname</flag>
        <command>command to run</command>
    </option>
</module>
```

- `<executable>`: Path to script (empty for command-only modules)
- `<flag>`: Command-line flag to match
- `<command>`: Command(s) to execute when flag is used

## Key Files

*   `src/db.rs`: Database interaction logic.
*   `src/graveyard.rs`: Handles deleted files.
*   `src/server.rs`:  Handles HTTP requests.
*   `src/tagger.rs`: File hashing and tagging logic.
*   `src/web.rs`:  Serves the web UI.
*   `src/config.rs`: Configuration management.
*   `src/sync/mod.rs`:  P2P sync logic.
*   `src/sync/identity.rs`: Manages node identities.
*   `src/sync/discovery.rs`: Handles peer discovery.

## Build Commands

```bash
cargo build --release
```

## Conventions

*   Use Rust's standard library and common crates.
*   Follow the Rust style guide (https://doc.rust-lang.org/rust-by-example/).
*   Write clear and concise code.
*   Use comprehensive tests.
*   Document code thoroughly.
```