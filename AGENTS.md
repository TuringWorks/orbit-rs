# AI Agent Instructions for Orbit-RS

This document provides instructions for AI coding assistants (Gemini, Copilot, Warp, Antigravity, and others) working with the Orbit-RS codebase.

## Required Reading

Before making any changes, read these files:
1. **`docs/PRD.md`** - Single source of truth for architecture and modules
2. **`CLAUDE.md`** - Quick development reference and commands

## Architecture Reference

**`docs/PRD.md` is the authoritative reference** for:
- Workspace structure and all 15 crates
- Module directory layouts and file purposes
- Protocol implementations (PostgreSQL, MySQL, CQL, Redis, REST, gRPC)
- Storage architecture (RocksDB, Memory, Iceberg)
- AI-native subsystems
- Feature status matrix with test counts
- Development guidelines

## PRD.md Maintenance (MANDATORY)

**When making architectural changes, you MUST update `docs/PRD.md`.**

### Triggers for PRD.md Updates
Update PRD.md when you:
- Add new modules, crates, or significant source files
- Change directory structures or file organization
- Add or modify protocol implementations or commands
- Update feature flags or cargo features
- Change API interfaces or add new endpoints
- Modify storage or compute backends
- Add new AI subsystems or capabilities
- Change test coverage significantly

### Sections to Update
1. **Module Reference** - Directory trees, file descriptions
2. **Feature Status Matrix** - Implementation status, test counts
3. **Protocol Commands** - New commands, updated syntax
4. **Architecture Sections** - Structural changes

### Client SDK & Extension Updates (Breaking Changes)

**When making breaking changes, also update external clients:**

| Change Type | Update Required |
|-------------|-----------------|
| New Redis/RESP commands | `orbit-python-client/orbit_client/client.py` |
| New PostgreSQL features | `orbit-python-client/orbit_client/protocols.py` |
| Protocol wire format | Both Python client and VS Code extension |
| New query languages | `orbit-vscode-extension/syntaxes/` |
| New connection types | `orbit-vscode-extension/src/connections/` |

**Files to check:**
- `orbit-python-client/orbit_client/client.py` - Client methods
- `orbit-python-client/examples/*.py` - Usage examples
- `orbit-vscode-extension/src/connections/*.ts` - Connections
- `orbit-vscode-extension/syntaxes/*.tmLanguage.json` - Syntax

### Change Management Workflow
```
1. Read docs/PRD.md to understand current architecture
2. Make code changes
3. Update docs/PRD.md to reflect changes
4. Run: cargo fmt --all
5. Run: cargo clippy --workspace -- -D warnings
6. Run: cargo test --workspace
7. Commit code AND PRD.md changes together
8. Use descriptive commit messages
```

## Code Standards

### Build and Test
```bash
cargo build --workspace              # Build all crates
cargo test --workspace               # Run all tests
cargo test --workspace -- --ignored  # Run slow integration tests
cargo fmt --all                      # Format code
cargo clippy --workspace -- -D warnings  # Lint
```

### Quality Requirements
- Zero compiler warnings
- Clippy must pass with `-D warnings`
- All tests must pass
- Code must be formatted with rustfmt

### Conventions
- **Crates**: `orbit-{name}` (orbit-server, orbit-engine, etc.)
- **Modules**: snake_case
- **Types**: PascalCase
- **Functions**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

### Error Handling
- `anyhow::Result` for application errors
- `thiserror` for library error types
- Custom errors in `orbit-shared/src/error.rs`

### Async Runtime
- Tokio with `#[tokio::main]` for binaries
- `#[tokio::test]` for async tests

## Key Directories

```
orbit-rs/
├── orbit/                    # Source code (15 workspace crates)
│   ├── server/              # Main binary - all protocols
│   ├── protocols/           # Protocol implementations
│   ├── engine/              # Storage engine
│   ├── compute/             # Hardware acceleration
│   ├── ml/                  # ML inference
│   ├── shared/              # Shared types and traits
│   └── client/              # Client library
├── config/                   # Configuration files
├── docs/                     # Documentation
│   └── PRD.md               # ARCHITECTURE REFERENCE (KEEP UPDATED)
└── tests/                    # Integration tests
```

## Protocol Ports

| Protocol   | Port  |
|------------|-------|
| PostgreSQL | 5432  |
| MySQL      | 3306  |
| CQL        | 9042  |
| Redis      | 6379  |
| REST       | 8080  |
| gRPC       | 50051 |

## Commit Message Format

```
type(scope): description

[body - optional]

🤖 Generated with [AI Assistant Name]
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

## Important Reminders

1. **Always read PRD.md first** before making architectural decisions
2. **Always update PRD.md** when architecture changes
3. **Never commit without running tests**
4. **Keep PRD.md synchronized** with actual codebase
5. **Commit PRD.md changes together** with code changes

## Agent-Specific Files

- `CLAUDE.md` - Claude Code / Anthropic Claude instructions
- `.cursorrules` - Cursor AI instructions
- `AGENTS.md` - This file (generic AI agents)
