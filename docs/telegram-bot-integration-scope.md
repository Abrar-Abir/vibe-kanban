# Telegram Bot Integration - Scope of Changes

## Overview

This document outlines the scope of changes required to integrate vibe-kanban with a Telegram bot. The integration will enable:
1. **Webhook notifications** when tasks are completed (with LLM response)
2. **Slash command interactions** to create/manage tasks or send/queue messages

**Assumptions:**
- Single user system (projects are not shared across users)
- User links their Telegram account once via the web UI

---

## Current Architecture Summary

### Technology Stack
- **Backend:** Rust + Axum (async web framework)
- **Database:** SQLite with SQLx (compile-time verified queries)
- **Frontend:** React + TypeScript (Vite)
- **Real-time:** WebSocket/SSE for event streaming
- **Types:** ts-rs generates TypeScript from Rust structs

### Key Data Models
```
Project → Task → Workspace → Session → ExecutionProcess
                                    └→ CodingAgentTurn (LLM responses)
```

### Existing Notification Infrastructure
- `EventService` (`crates/services/src/services/events.rs`) - SQLite hooks for database change events
- `NotificationService` (`crates/services/src/services/notification.rs`) - Sound/push notifications
- `QueuedMessageService` (`crates/services/src/services/queued_message.rs`) - Background message handling
- Event streaming via `/api/events` (SSE) and WebSocket endpoints

---

## Proposed Changes

### 1. Configuration (No New Database Tables)

Since this is a single-user system, Telegram linkage data can be stored in the existing user config file alongside other settings. No database migration needed.

**File to modify:** `crates/utils/src/config/versions/v8.rs` (or current version)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TelegramConfig {
    // Linkage info (populated after user links their Telegram account)
    pub telegram_chat_id: Option<i64>,        // Chat ID for sending messages
    pub telegram_user_id: Option<i64>,        // Telegram user ID
    pub telegram_username: Option<String>,    // @username (optional)

    // Notification preferences
    pub notifications_enabled: bool,
    pub notify_on_task_done: bool,
}
```

**Rationale:**
- Single user = single config file = no need for a database table
- Follows existing pattern for user preferences (notifications, editor, etc.)
- Simpler to implement and maintain

---

### 2. Rust Services

#### 2.1 TelegramBotService
**File to create:** `crates/services/src/services/telegram_bot.rs`

Responsibilities:
- Initialize Telegram Bot API client (using `teloxide` or `frankenstein` crate)
- Send messages to the linked user
- Format notifications (markdown/HTML)

Key methods:
```rust
impl TelegramBotService {
    pub async fn send_task_completed_notification(&self, task: &Task, llm_response: &str) -> Result<()>;
    pub async fn send_message(&self, message: &str) -> Result<()>;
}
```

#### 2.2 TelegramWebhookService
**File to create:** `crates/services/src/services/telegram_webhook.rs`

Responsibilities:
- Parse incoming Telegram webhook updates
- Validate webhook authenticity
- Route commands to appropriate handlers
- Handle user linking flow

**Slash Commands:**

| Command | Description |
|---------|-------------|
| `/start` | Begin account linking flow |
| `/help` | Show available commands |
| `/projects` | List all projects |
| `/project <id>` | Set active project context for subsequent commands |
| `/tasks` | List tasks in active project |
| `/tasks <project_id>` | List tasks in specified project |
| `/task <id>` | Get task details |
| `/newtask <title>` | Create task in active project |
| `/newtask <project_id> <title>` | Create task in specified project |
| `/message <task_id> <text>` | Send/queue a message for a task |

**Project Context:** The bot maintains an "active project" per user (in-memory or simple key-value). Commands like `/tasks` and `/newtask` use this context when project ID is not explicitly provided.

#### 2.3 EventService Integration
**File to modify:** `crates/services/src/services/events.rs`

Add hook to trigger Telegram notification when:
- Task status changes to "done"
- Include LLM response summary from `coding_agent_turns`

---

### 3. API Endpoints

**File to create:** `crates/server/src/routes/telegram.rs`

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/telegram/webhook` | Telegram webhook receiver (from Telegram servers) |
| GET | `/api/telegram/link` | Get bot deep link for account linking |
| DELETE | `/api/telegram/unlink` | Unlink Telegram account |
| GET | `/api/telegram/status` | Check if Telegram is linked |

**File to modify:** `crates/server/src/routes/mod.rs`
- Register telegram router

---

### 3. Environment Variables

**Files to modify:**
- `.env.example` (create or update)

New environment variables:
```env
TELEGRAM_BOT_TOKEN=<bot_token_from_botfather>
TELEGRAM_WEBHOOK_SECRET=<random_secret_for_validation>
TELEGRAM_WEBHOOK_URL=<public_url>/api/telegram/webhook
```

---

### 4. Frontend Changes

#### 5.1 Settings UI
**File to create:** `frontend/src/components/settings/TelegramSettings.tsx`

Features:
- Display Telegram linking status (linked/unlinked)
- Show deep link (`t.me/YourBot?start=<token>`) for linking
- Toggle notification preferences
- Unlink button

#### 5.2 API Client
**File to modify:** `frontend/src/lib/api.ts`

Add methods:
```typescript
getTelegramLinkInfo(): Promise<{ linked: boolean; linkUrl?: string }>
unlinkTelegram(): Promise<void>
```

#### 5.3 Type Generation
**File to modify:** `crates/server/src/bin/generate_types.rs`

Add new types for TypeScript generation:
- `TelegramLinkInfo`
- `TelegramConfig`

---

### 5. Dependencies

**File to modify:** `crates/services/Cargo.toml` (or workspace Cargo.toml)

```toml
[dependencies]
teloxide = { version = "0.12", features = ["webhooks-axum"] }
# OR
frankenstein = "0.30"  # Lighter alternative if teloxide is too heavy
```

---

## Integration Flows

### Account Linking Flow
```
1. User opens Settings in web UI
2. UI calls GET /api/telegram/link → returns deep link with unique token
3. User clicks link → opens Telegram → sends /start to bot
4. Bot receives /start with token
5. TelegramWebhookService:
   a. Validates token
   b. Stores telegram_user_id + telegram_chat_id in database
   c. Responds "Account linked successfully!"
6. UI polls /api/telegram/status or refreshes to show linked state
```

### Task Completion Notification Flow
```
1. Task status updated to "done" (via API, UI, or coding agent)
2. EventService detects update via SQLite hook
3. EventService triggers TelegramBotService.send_task_completed_notification()
4. TelegramBotService:
   a. Checks if Telegram is linked (query telegram_users table)
   b. Checks if notifications enabled (user config)
   c. Fetches latest CodingAgentTurn summary for the task
   d. Formats message with task title and LLM summary
   e. Sends via Telegram API
```

### Slash Command Flow (Create Task)
```
1. User sends "/newtask Fix login bug" to bot
2. Telegram POSTs update to /api/telegram/webhook
3. TelegramWebhookService:
   a. Validates webhook signature
   b. Parses command and arguments
   c. Gets active project context (or prompts user to set one)
   d. Creates task via existing TaskService
   e. Responds with confirmation: "Created task #42: Fix login bug"
```

### Message to Task Flow
```
1. User sends "/message 42 Please also add unit tests" to bot
2. TelegramWebhookService:
   a. Parses task_id and message text
   b. Validates task exists
   c. If task has active workspace/session:
      - Deliver message immediately via existing QueuedMessageService
   d. If task is not active:
      - Queue message to be delivered when task starts
   e. Responds with confirmation
```

---

## Files Summary

### New Files to Create
| Path | Description |
|------|-------------|
| `crates/services/src/services/telegram_bot.rs` | Bot API client service |
| `crates/services/src/services/telegram_webhook.rs` | Webhook handler service |
| `crates/server/src/routes/telegram.rs` | API endpoints |
| `frontend/src/components/settings/TelegramSettings.tsx` | Settings UI |

### Files to Modify
| Path | Changes |
|------|---------|
| `crates/services/src/services/mod.rs` | Export telegram services |
| `crates/services/src/services/events.rs` | Add notification hook on task completion |
| `crates/server/src/routes/mod.rs` | Register telegram router |
| `crates/utils/src/config/versions/v8.rs` | Add TelegramConfig struct |
| `crates/server/src/bin/generate_types.rs` | Add new TS types |
| `frontend/src/lib/api.ts` | Add telegram API methods |
| `Cargo.toml` (workspace or services) | Add teloxide dependency |
| `.env.example` | Document new env vars |

---

## Security Considerations

1. **Webhook Validation**: Verify Telegram webhook requests using secret token in URL
2. **Link Token**: Use time-limited, single-use tokens for account linking
3. **Token Storage**: Store bot token in environment variable only
4. **Message Sanitization**: Escape user content in Telegram messages to prevent formatting injection

---

## Estimated Scope

| Component | Complexity | Notes |
|-----------|------------|-------|
| Telegram Bot Service | Medium | API integration, formatting |
| Webhook Handler | Medium | Command parsing, project context |
| Event Integration | Low | Hook into existing EventService |
| API Endpoints | Low | 4 simple endpoints |
| Configuration | Low | Env vars + config struct |
| Frontend Settings | Low | Simple status + link UI |

**Total:** 4 new files, 8 files to modify. No database changes needed.
