```markdown
# AKCloud - Agent Instructions

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
GIT_SSH_COMMAND="ssh -i /config/.ssh/github -o StrictHostKeyChecking=no" git push origin main
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
    ├── db.rs          # Database interactions
    ├── graveyard.rs   # Graveyard management
    ├── server.rs      # API server implementation
    ├── web.rs         # Web UI serving
    ├── sync/
    │   ├── mod.rs      # Sync module
    │   ├── identity.rs # Identity management
    │   └── discovery.rs # Peer discovery
    ├── config.rs      # Configuration loading
    ├── tagger.rs      # File hashing and tagging
```

## Module Structure

Modules are stored in `~/.akcloud/modules/`:
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
```