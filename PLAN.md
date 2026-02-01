# Telegram Bot Integration Plan

## Overview

Integrate a Telegram bot into vibe-kanban to enable:
- Webhook notifications when tasks complete (with LLM response summary)
- Slash command interactions to create/manage tasks and send messages

Based on scope document: [docs/telegram-bot-integration-scope.md](docs/telegram-bot-integration-scope.md)

---

## Progress Summary

**Status: Phase 5 In Progress (Webhook & Auto-linking)**

| Phase | Status | Key Deliverables |
|-------|--------|------------------|
| Phase 1: Foundation | ✅ Complete | frankenstein crate, config v9, TelegramService |
| Phase 2: Backend Integration | ✅ Complete | API routes, task completion notifications |
| Phase 3: Frontend Integration | ✅ Complete | TypeScript types, API methods, Settings UI |
| Phase 4: Documentation | ✅ Complete | Environment variables documented, tests added |
| Phase 5: Webhook & Auto-linking | 🔄 In Progress | Webhook handler, `/start` command, manual link UI |

### What's Working
- TelegramService can send messages to linked Telegram chats
- Config v9 stores Telegram settings (chat ID, user ID, notification preferences)
- API endpoints for status, linking, and unlinking
- Settings UI with Integrations tab for managing Telegram connection
- Automatic notifications when tasks are marked as "done"
- **NEW:** Webhook handler receives Telegram updates
- **NEW:** `/start TOKEN` command links accounts automatically
- **NEW:** Deep link generation with 15-min expiring tokens
- **NEW:** Manual link UI with copy button (fallback for Telegram Desktop)
- **NEW:** Auto-polling UI to detect when account is linked

### What's Deferred (Future Work)
- Additional slash commands (`/help`, `/projects`, `/tasks`, `/newtask`, `/message`)
- Project context management for conversational bot interactions

---

## Current Dev Setup (ngrok)

The webhook requires a public URL. For local development:

```bash
# Start ngrok tunnel to backend port
ngrok http 3001

# Get the public URL
curl -s http://localhost:4040/api/tunnels | grep -o '"public_url":"[^"]*"'

# Register webhook with Telegram (replace URL)
curl "https://api.telegram.org/bot$TELEGRAM_BOT_TOKEN/setWebhook?url=https://XXXX.ngrok-free.app/api/telegram/webhook"

# Verify webhook
curl "https://api.telegram.org/bot$TELEGRAM_BOT_TOKEN/getWebhookInfo"
```

**Current ngrok URL:** `https://57fb9c29d77a.ngrok-free.app`

**Note:** ngrok URL changes on restart - must update `.env` and re-register webhook.

---

## Next Steps

### Immediate: Verify End-to-End Flow
1. Go to Settings → Integrations
2. Copy `/start TOKEN` command
3. Send to @kanban_vibe_bot in Telegram
4. Verify UI auto-updates to "Connected"

### Future Directions

### Phase 6: Additional Slash Commands
1. **Command Parser** - Route incoming slash commands to handlers
2. **`/help`** - Show available commands
3. **`/projects`** - List user's projects
4. **`/tasks`** - List tasks in active project

### Phase 3: Enhanced Notifications
1. **Rich Notifications** - Include LLM summary in task completion messages
2. **Notification Filters** - Per-project notification settings
3. **Message Threading** - Link notifications to original task context

### Phase 4: Advanced Features
1. **Two-way Messaging** - Send messages to active tasks via Telegram
2. **Task Creation** - Create tasks from Telegram conversations
3. **Status Updates** - Query task status via bot

---

## Manual Testing Guide

### Prerequisites

1. **Create a Telegram Bot**
   - Open Telegram and message [@BotFather](https://t.me/BotFather)
   - Send `/newbot` and follow the prompts
   - Save the bot token (looks like `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)

2. **Get Your Telegram Chat ID**
   - Message your new bot (any message)
   - Visit: `https://api.telegram.org/bot<YOUR_BOT_TOKEN>/getUpdates`
   - Find `"chat":{"id":<YOUR_CHAT_ID>}` in the response
   - Note: Chat ID is typically a positive number for private chats

3. **Configure Environment**
   ```bash
   # Add to your .env file
   TELEGRAM_BOT_TOKEN=your_bot_token_here
   ```

### Test Scenarios

#### Test 1: Verify Bot Configuration
1. Start vibe-kanban: `pnpm run dev`
2. Open Settings -> Integrations
3. **Expected**: Shows "Telegram bot is configured" if `TELEGRAM_BOT_TOKEN` is set
4. **Expected**: Shows warning if token is not set

#### Test 2: Link Telegram Account
1. Open Settings -> Integrations
2. Enter your Chat ID and optionally User ID
3. Click "Link Account"
4. **Expected**: Status changes to "Connected" with green indicator
5. **Expected**: Your chat ID is displayed

#### Test 3: Send Test Notification (Manual)
1. With account linked, open browser console
2. Create a task and change its status to "Done"
3. **Expected**: Telegram notification received with task title

#### Test 4: Toggle Notifications
1. Open Settings -> Integrations
2. Toggle "Notify on task completion" off
3. Complete another task
4. **Expected**: No Telegram notification received
5. Toggle back on and complete a task
6. **Expected**: Notification received

#### Test 5: Unlink Account
1. Open Settings -> Integrations
2. Click "Unlink" or "Disconnect"
3. Confirm the action
4. **Expected**: Status changes to "Not Connected"
5. Complete a task
6. **Expected**: No notification (gracefully handles unlinked state)

### API Testing (curl)

```bash
# Check Telegram status
curl http://localhost:3000/api/telegram/status

# Get link info (returns bot username for deep link)
curl http://localhost:3000/api/telegram/link

# Link account manually
curl -X POST http://localhost:3000/api/telegram/link \
  -H "Content-Type: application/json" \
  -d '{"chat_id": 123456789, "user_id": 987654321}'

# Unlink account
curl -X DELETE http://localhost:3000/api/telegram/unlink
```

### Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| "Bot not configured" | Missing `TELEGRAM_BOT_TOKEN` | Add token to `.env` and restart |
| No notification received | Chat ID incorrect | Verify chat ID via getUpdates API |
| "Failed to send message" | Bot blocked by user | Start conversation with bot first |
| Link fails silently | Invalid chat ID format | Chat ID should be a number (no quotes) |
| Webhook returns 403 | ngrok tunneling to wrong port | Ensure ngrok targets port 3001 (backend), not 3000 (frontend) |
| Deep link doesn't work | Telegram Desktop quirk | Use manual copy/paste method instead |
| Token not found | Tokens are in-memory only | Restart generates new tokens; previous ones are lost |

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Telegram crate | `frankenstein` | Lightweight, async-friendly, simpler API than teloxide |
| Config version | Create v9 | Clean migration path, significant structural change |
| Settings UI | New "Integrations" section | Leaves room for future integrations, cleaner separation |
| Service architecture | Single TelegramService | Simpler state management, follows existing patterns |

---

## Implementation Tasks (All Complete)

### Phase 1: Foundation
| Task ID | Title | Status |
|---------|-------|--------|
| P1-T1 | Add frankenstein crate dependency | Complete |
| P1-T2 | Create config v9 with TelegramConfig | Complete |
| P1-T3 | Create TelegramService | Complete |

### Phase 2: Backend Integration
| Task ID | Title | Status |
|---------|-------|--------|
| P2-T1 | Create Telegram API routes | Complete |
| P2-T2 | Integrate notifications into task completion | Complete |

### Phase 3: Frontend Integration
| Task ID | Title | Status |
|---------|-------|--------|
| P3-T1 | Add Telegram types to TypeScript generator | Complete |
| P3-T2 | Add Telegram API methods to frontend | Complete |
| P3-T3 | Create Integrations settings section | Complete |

### Phase 4: Documentation & Testing
| Task ID | Title | Status |
|---------|-------|--------|
| P4-T1 | Document environment variables | Complete |
| P4-T2 | Write tests and run verification | Complete |

---

## Files Summary

### Files Created
| File | Description |
|------|-------------|
| `crates/utils/src/config/versions/v9.rs` | Config v9 with TelegramConfig |
| `crates/services/src/services/telegram.rs` | TelegramService implementation |
| `crates/server/src/routes/telegram.rs` | API endpoints |
| `frontend/src/components/settings/IntegrationsSettings.tsx` | Settings UI |

### Files Modified
| File | Changes |
|------|---------|
| `crates/services/Cargo.toml` | Added frankenstein dependency |
| `crates/utils/src/config/versions/mod.rs` | Added v9 module export |
| `crates/utils/src/config/mod.rs` | Updated to use v9 as current config |
| `crates/services/src/services/mod.rs` | Exported TelegramService |
| `crates/services/src/services/task.rs` | Added notification on task completion |
| `crates/server/src/routes/mod.rs` | Registered telegram routes |
| `crates/server/src/bin/generate_types.rs` | Added Telegram types |
| `frontend/src/lib/api.ts` | Added Telegram API methods |
| `frontend/src/components/dialogs/settings-dialog.tsx` | Added Integrations tab |
| `shared/types.ts` | Generated Telegram types |
| `.env.example` | Added Telegram environment variables |

---

## Key Data Structures

### TelegramConfig (Rust)
```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
pub struct TelegramConfig {
    pub chat_id: Option<i64>,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub notifications_enabled: bool,
    pub notify_on_task_done: bool,
}
```

### API Endpoints
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/telegram/status` | Check link status |
| GET | `/api/telegram/link` | Get deep link for account linking |
| POST | `/api/telegram/link` | Link account with chat ID |
| DELETE | `/api/telegram/unlink` | Unlink Telegram account |

---

## Environment Variables

```bash
TELEGRAM_BOT_TOKEN=           # Bot token from @BotFather (required)
TELEGRAM_WEBHOOK_SECRET=      # Random secret for webhook validation (future)
TELEGRAM_WEBHOOK_URL=         # Public URL for webhook (future)
```

---

## Security Considerations

- Validate webhook secret in URL parameter (when webhooks implemented)
- Keep bot token in env var only (never expose to frontend)
- Escape user input in Telegram messages
- Chat IDs should be validated as numbers before storage
