Telegram Bot Integration Plan
Overview
Integrate a Telegram bot into vibe-kanban to enable:
Webhook notifications when tasks complete (with LLM response summary)
Slash command interactions to create/manage tasks and send messages
Based on scope document: docs/telegram-bot-integration-scope.md
---
Design Decisions
Decision
Choice
Rationale
----------
--------
-----------
Telegram crate
frankenstein
Lightweight, async-friendly, simpler API than teloxide
Config version
Create v9
Clean migration path, significant structural change
Settings UI
New "Integrations" section
Leaves room for future integrations, cleaner separation
Service architecture
Single TelegramService
Simpler state management, follows existing patterns
---
Implementation Tasks (Created in VibeKanban)
Phase 1: Backend Core
Task ID
Title
Parallel With
---------
-------
---------------
P1-T1
Add frankenstein crate dependency
P1-T2
P1-T2
Create config v9 with TelegramConfig
P1-T1
P1-T3
Create TelegramService
— (after P1-T1, P1-T2)
Phase 2: API Layer
Task ID
Title
Parallel With
---------
-------
---------------
P2-T1
Create Telegram API routes
P2-T2
P2-T2
Integrate notifications into task completion
P2-T1
Phase 3: Frontend
Task ID
Title
Parallel With
---------
-------
---------------
P3-T1
Add Telegram types to TypeScript generator
— (must complete first)
P3-T2
Add Telegram API methods to frontend
P3-T3
P3-T3
Create Integrations settings section
P3-T2
Phase 4: Documentation & Testing
Task ID
Title
Parallel With
---------
-------
---------------
P4-T1
Document environment variables
P4-T2
P4-T2
Write tests and run verification
P4-T1
Execution Order Summary

[P1-T1] ──┬──> [P1-T3] ──┬──> [P2-T1] ──┬──> [P3-T1] ──┬──> [P3-T2] ──┬──> [P4-T1] ──┐
[P1-T2] ──┘              │              │              │              │              │
                         └──> [P2-T2] ──┘              └──> [P3-T3] ──┘              │
                                                                       └──> [P4-T2] ──┘

---
Files to Create
File
Description
------
-------------
crates/services/src/services/config/versions/v9.rs
Config v9 with TelegramConfig
crates/services/src/services/telegram.rs
TelegramService implementation
crates/server/src/routes/telegram.rs
API endpoints
frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx
Settings UI
Files to Modify
File
Changes
------
---------
crates/services/Cargo.toml
Add frankenstein dependency
crates/services/src/services/mod.rs
Export telegram module
crates/services/src/services/config/mod.rs
Update to v9 exports
crates/services/src/services/container.rs
Add notification in finalize_task
crates/deployment/src/lib.rs
Add telegram_service() to trait
crates/server/src/routes/mod.rs
Register telegram router
crates/server/src/bin/generate_types.rs
Add Telegram types
frontend/src/lib/api.ts
Add telegramApi methods
frontend/src/components/ui-new/dialogs/settings/SettingsSection.tsx
Add integrations case
frontend/src/components/ui-new/dialogs/SettingsDialog.tsx
Add integrations to nav
.env.example
Document env vars
---
Key Data Structures
TelegramConfig (Rust)

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
pub struct TelegramConfig {
    pub chat_id: Option<i64>,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub notifications_enabled: bool,
    pub notify_on_task_done: bool,
    pub include_llm_summary: bool,
}

API Endpoints
Method
Endpoint
Description
--------
----------
-------------
POST
/api/telegram/webhook
Receive Telegram updates
GET
/api/telegram/link
Get deep link for account linking
DELETE
/api/telegram/unlink
Unlink Telegram account
GET
/api/telegram/status
Check link status
Slash Commands
Command
Description
---------
-------------
/start
Begin account linking
/help
Show available commands
/projects
List all projects
/project <id>
Set active project context
/tasks
List tasks in active project
/task <id>
Get task details
/newtask <title>
Create task in active project
/message <task_id> <text>
Send/queue message for task
---
Environment Variables

TELEGRAM_BOT_TOKEN=           # Bot token from @BotFather
TELEGRAM_WEBHOOK_SECRET=      # Random secret for webhook validation
TELEGRAM_WEBHOOK_URL=         # Public URL for webhook

---
Verification Steps
Build check: pnpm run backend:check and pnpm run check
Type generation: pnpm run generate-types
Tests: cargo test --workspace
Manual testing:
Create bot via @BotFather
Set up webhook (ngrok for local dev)
Link account via Settings UI
Test slash commands
Complete a task and verify notification
Unlink account
---
Security Considerations
Validate webhook secret in URL parameter
Use time-limited (15 min), single-use link tokens
Keep bot token in env var only (never expose to frontend)
Escape user input in Telegram messages

I've completed the task management updates:

**Tasks created in VibeKanban (10 total):**

| Phase | Task | Can Run In Parallel With |
|-------|------|--------------------------|
| **Phase 1: Backend Core** |||
| P1-T1 | Add frankenstein crate dependency | P1-T2 |
| P1-T2 | Create config v9 with TelegramConfig | P1-T1 |
| P1-T3 | Create TelegramService | — (sequential) |
| **Phase 2: API Layer** |||
| P2-T1 | Create Telegram API routes | P2-T2 |
| P2-T2 | Integrate notifications into task completion | P2-T1 |
| **Phase 3: Frontend** |||
| P3-T1 | Add Telegram types to TypeScript generator | — (must complete first) |
| P3-T2 | Add Telegram API methods to frontend | P3-T3 |
| P3-T3 | Create Integrations settings section | P3-T2 |
| **Phase 4: Docs & Testing** |||
| P4-T1 | Document environment variables | P4-T2 |
| P4-T2 | Write tests and run verification | P4-T1 |

The plan file has been updated with the task tables and a dependency diagram showing which tasks can be parallelized.