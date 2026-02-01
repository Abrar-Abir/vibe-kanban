# Telegram Bot Integration - Plan

Based on scope: [SCOPE.md](SCOPE.md)

## Progress Summary

**All Core Features: ✅ Complete**
- Account linking via deep link + `/start TOKEN`
- Task completion notifications with LLM summary toggle
- Settings UI with all toggles functional
- Webhook handler for Telegram updates
- Real-time streaming with human-readable content

---

## Future Features

### Phase 1: Real-Time Streaming ✅ COMPLETE

Streams LLM output to Telegram in real-time as tasks run.

**What's Implemented:**
- `spawn_stream_to_telegram()` in `telegram.rs` - Subscribes to MsgStore broadcasts
- Debounced updates (500ms) to respect Telegram rate limits
- `format_entry()` helper - Formats NormalizedEntry with icons
- Settings toggle (`stream_enabled`) in UI

**Icons:**
- 💬 Assistant messages
- 💭 Thinking (truncated to 200 chars)
- 📖 File reads / 📝 File edits
- ⚡ Command execution
- 🔍 Searches / 🌐 Web fetches
- ❌ Errors

---

### Phase 2: Slash Commands

| Command | Description | Priority |
|---------|-------------|----------|
| `/help` | Show available commands | High |
| `/projects` | List all projects | High |
| `/tasks` | List tasks in active project | High |
| `/task <id>` | Get task details | Medium |
| `/newtask <title>` | Create task in active project | Medium |
| `/message <task_id> <text>` | Send message to task | Low |

**Implementation:**
1. Add command parser in `TelegramService::handle_update()`
2. Route commands to handlers (`cmd_help`, `cmd_projects`, etc.)
3. Format responses with Telegram HTML/Markdown

### Phase 3: Project Context

Enable commands like `/tasks` without specifying project ID.

**Data:**
```rust
// In-memory per-user context
active_projects: Arc<DashMap<i64, ProjectId>>  // chat_id → project_id
```

**Commands:**
- `/project <id>` - Set active project
- `/project` - Show current active project

### Phase 4: Two-Way Messaging

Send messages to active tasks via Telegram.

**Flow:**
1. User sends `/message 42 Add unit tests`
2. Bot validates task exists
3. If task has active session → deliver immediately
4. If task inactive → queue for later delivery
5. Respond with confirmation

**Files:**
- Integrate with `QueuedMessageService`
- Add `TelegramService::cmd_message()`

### Phase 5: Enhanced Notifications

- **Per-project settings** - Choose which projects send notifications
- **Message threading** - Link notifications to task context
- **Rich formatting** - Task metadata, links, progress indicators

---

## Command Implementation Template

```rust
// In telegram.rs
async fn cmd_help(&self, chat_id: i64) -> Result<String, TelegramError> {
    Ok(r#"
<b>Available Commands:</b>

/help - Show this message
/projects - List all projects
/project &lt;id&gt; - Set active project
/tasks - List tasks in active project
/task &lt;id&gt; - Get task details
/newtask &lt;title&gt; - Create new task
/message &lt;task_id&gt; &lt;text&gt; - Send message to task
"#.to_string())
}
```

---

## Testing Commands

```bash
# Verify webhook
curl "https://api.telegram.org/bot$TELEGRAM_BOT_TOKEN/getWebhookInfo"

# Check status
curl http://localhost:3001/api/telegram/status

# Update settings
curl -X PATCH http://localhost:3001/api/telegram/settings \
  -H "Content-Type: application/json" \
  -d '{"include_llm_summary": true}'
```

---

## Security Notes

- Webhook secret validation (TODO)
- Bot token in env only
- Escape user input in messages
- Validate chat IDs as numbers
