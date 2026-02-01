# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Vibe Kanban is a task management and orchestration tool for AI coding agents. It enables developers to:
- Switch between different AI coding agents (Claude, Cursor, Copilot, Gemini, etc.)
- Orchestrate multiple agents in parallel or sequence
- Track task status and review agent work
- Manage MCP (Model Context Protocol) configurations
- Open projects remotely via SSH

## Architecture

### High-Level Structure

This is a full-stack monorepo with:
- **Backend**: Rust-based Axum server with SQLite database
- **Frontend**: React + TypeScript (Vite, Tailwind, Zustand for state)
- **Type Sharing**: ts-rs generates TypeScript types from Rust structs
- **Deployment**: Multiple deployment modes (local, remote) abstracted via the `Deployment` trait
- **Distribution**: Published as an npm package (`npx vibe-kanban`)

### Rust Workspace Structure

The backend is organized as a Cargo workspace with these crates:

- **server**: Main API server, routes, error handling, MCP server
  - Routes in `crates/server/src/routes/` (tasks, projects, executors, sessions, etc.)
  - Binary targets: `server` (main), `generate_types` (type generation), `mcp_task_server` (MCP server)
- **db**: SQLx models and migrations
  - Models in `crates/db/src/models/` match database tables
  - Migrations in `crates/db/migrations/` (timestamped SQL files)
- **executors**: Implementations for each AI coding agent
  - Each executor in `crates/executors/src/executors/` (claude.rs, cursor.rs, copilot.rs, etc.)
  - Shared executor logic and command building
- **services**: Business logic layer
  - Services in `crates/services/src/services/` (container, workspace_manager, git_host, analytics, etc.)
  - Container service manages execution processes and orchestration
- **deployment**: Deployment trait and shared logic
  - Abstracts differences between local and remote deployments
- **local-deployment**: Local deployment implementation
- **remote**: Remote deployment with Postgres backend
- **git**: Git operations and worktree management
- **utils**: Shared utilities (assets, browser, ports, sentry)
- **review**: Code review CLI tool

### Frontend Structure

- **src/pages/**: Main application pages (Projects, ProjectTasks, settings)
- **src/components/**: Reusable UI components, dialogs in `components/dialogs/`
- **src/hooks/**: Custom React hooks (extensive collection)
- **src/contexts/**: React contexts for shared state
- **src/stores/**: Zustand stores for UI state (diff view, expandable, preferences)
- **src/lib/**: API client and utilities
- **src/types/**: TypeScript type definitions

### Key Patterns

1. **Type Generation**: Rust structs annotated with `#[derive(TS)]` generate TypeScript types. Run `pnpm run generate-types` after Rust type changes. Edit [crates/server/src/bin/generate_types.rs](crates/server/src/bin/generate_types.rs) to control type generation, never edit [shared/types.ts](shared/types.ts) directly.

2. **Database Migrations**: Use SQLx migrations. Add new migrations with timestamps. After schema changes, run `pnpm run prepare-db` to update SQLx's compile-time checked queries.

3. **Executors**: Each AI agent has its own executor module implementing a common interface for spawning processes, handling stdio, and managing sessions.

4. **Deployment Abstraction**: The `Deployment` trait in [crates/deployment](crates/deployment) allows different storage backends (SQLite for local, Postgres for remote) without changing business logic.

5. **MCP Integration**: Model Context Protocol server runs alongside the main server, exposing Vibe Kanban capabilities as MCP tools.

## Project Structure & Module Organization

- `crates/`: Rust workspace (see Architecture section)
- `frontend/`: React + TypeScript app (Vite, Tailwind). Source in `frontend/src`
- `remote-frontend/`: Frontend for remote deployment
- `shared/`: Generated TypeScript types (`shared/types.ts`). **Do not edit directly**
- `assets/`, `dev_assets_seed/`, `dev_assets/`: Packaged and development assets
- `npx-cli/`: Files published to npm CLI package
- `scripts/`: Development helpers (ports, DB preparation)
- `docs/`: Documentation files (Mintlify-based)

## Development Commands

### Setup
```bash
pnpm i                           # Install dependencies
```

### Running Development Servers
```bash
pnpm run dev                     # Run both frontend + backend (auto-assigns ports)
pnpm run frontend:dev            # Run frontend only
pnpm run backend:dev:watch       # Run backend only (with cargo watch)
```

### Building
```bash
pnpm run build:npx               # Build full distribution for current platform
./local-build.sh                 # Same as build:npx (builds frontend, Rust binaries, packages)
cd npx-cli && node bin/cli.js    # Test local build
```

### Type Checking & Linting
```bash
pnpm run check                   # Frontend: TypeScript type check
pnpm run backend:check           # Backend: cargo check
pnpm run lint                    # Both: frontend + backend linting
pnpm run format                  # Format all code (Prettier + rustfmt)
```

### Testing
```bash
cargo test --workspace           # Run all Rust tests
cargo test -p <crate>            # Run tests for specific crate (e.g., cargo test -p db)
cargo test <test_name>           # Run specific test by name
```

### Type Generation
```bash
pnpm run generate-types          # Generate TypeScript types from Rust
pnpm run generate-types:check    # Check types are up to date (CI mode)
pnpm run remote:generate-types   # Generate types for remote deployment
```

### Database Operations
```bash
pnpm run prepare-db              # Prepare SQLx for offline compilation (local/SQLite)
pnpm run remote:prepare-db       # Prepare SQLx for remote deployment (Postgres)
```

### Remote Development
```bash
pnpm run remote:dev              # Run remote deployment with Docker Compose
pnpm run remote:dev:clean        # Clean up Docker volumes
```

## Coding Style & Naming Conventions

### Rust
- `rustfmt` enforced via `rustfmt.toml`
- Group imports by crate (std, external, workspace, local)
- snake_case for modules and functions, PascalCase for types
- Add `#[derive(Debug, Serialize, Deserialize)]` where useful
- Keep functions small and focused

### TypeScript/React
- ESLint + Prettier (2 spaces, single quotes, 80 char line length)
- PascalCase for components, camelCase for variables/functions
- kebab-case for file names where practical
- Prefer functional components with hooks

## Testing Guidelines

### Rust
- Write unit tests alongside code using `#[cfg(test)]` modules
- Add tests for new logic and edge cases
- Run `cargo test --workspace` to test everything
- Run `cargo test -p <crate>` for specific crate

### Frontend
- Ensure `pnpm run check` and `pnpm run lint` pass
- Include lightweight tests (Vitest) for runtime logic in the same directory

## Important Notes

### Environment Variables

**Build-time variables** (set during `pnpm run build`):
- `POSTHOG_API_KEY`, `POSTHOG_API_ENDPOINT`: Analytics configuration

**Runtime variables** (set when running the app):
- `PORT`: Server port (auto-assign if not set)
- `BACKEND_PORT`: Backend port (dev mode, defaults to PORT+1)
- `FRONTEND_PORT`: Frontend port (dev mode, defaults to 3000)
- `HOST`: Backend host (default: 127.0.0.1)
- `MCP_HOST`, `MCP_PORT`: MCP server connection settings
- `VK_ALLOWED_ORIGINS`: Comma-separated allowed origins for CORS (required for reverse proxies)
- `DISABLE_WORKTREE_CLEANUP`: Disable git worktree cleanup (debugging)

### SQLx and Database

- After schema changes, always run `pnpm run prepare-db` to regenerate SQLx's offline query data
- Migrations are in [crates/db/migrations/](crates/db/migrations) with timestamp prefixes
- The dev server copies a blank DB from `dev_assets_seed/` on startup

### Type Generation

- **Never** manually edit [shared/types.ts](shared/types.ts)
- Instead, modify Rust types and run `pnpm run generate-types`
- Edit [crates/server/src/bin/generate_types.rs](crates/server/src/bin/generate_types.rs) to control which types are generated

### Remote Deployment

- Remote deployment uses Postgres instead of SQLite
- Requires setting `VK_ALLOWED_ORIGINS` when behind a reverse proxy
- SSH remote configuration allows local VSCode to open remote projects
- See README for detailed remote setup instructions
