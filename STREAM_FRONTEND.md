# Real-Time Telegram Streaming - Feature Design

## Overview

Mirror the frontend's real-time LLM output stream to Telegram. Every piece of content that streams to the frontend (thinking indicators, text responses, file edits, tool usage) should also be pushed to the linked Telegram account.

---

## Current Architecture

### Frontend Streaming Flow

```
ExecutionProcess (running LLM)
    ↓
stdout/stderr → ConversationPatch (JSON Patch ops)
    ↓
MsgStore.push_patch() → broadcast::Sender<LogMsg>
    ↓
WebSocket endpoint: /api/execution-processes/{id}/raw-logs/ws
    ↓
Frontend: useLogStream() / useJsonPatchWsStream()
    ↓
React components render streaming content
```

### Key Data Structures

**LogMsg** (`crates/utils/src/log_msg.rs`):
```rust
pub enum LogMsg {
    Stdout(String),
    Stderr(String),
    JsonPatch(Patch),
    SessionId(String),
    MessageId(String),
    Ready,
    Finished,
}
```

**MsgStore** (`crates/utils/src/msg_store.rs`):
- Ring buffer (100MB) for history
- `broadcast::Sender<LogMsg>` for live subscribers
- Methods: `push_patch()`, `subscribe()`, `stream()`

---

## Proposed Design

### Architecture: Broadcast Subscriber Pattern

```
MsgStore (existing)
    ↓
broadcast::Sender<LogMsg>
    ↓
    ├── WebSocket handlers (existing) → Frontend
    │
    └── TelegramStreamService (NEW) → Telegram API
            ↓
        Message Buffer + Rate Limiter
            ↓
        Telegram Bot API
```

### New Component: TelegramStreamService

Lives alongside existing `TelegramService`. Subscribes to execution process streams and forwards to Telegram.

```rust
pub struct TelegramStreamService {
    config: Arc<RwLock<Config>>,
    api: Api,

    // Active subscriptions: exec_process_id → task handle
    active_streams: DashMap<Uuid, JoinHandle<()>>,

    // Message batching
    buffer: Arc<Mutex<MessageBuffer>>,
    flush_interval: Duration,  // e.g., 500ms
}

struct MessageBuffer {
    content: String,
    last_flush: Instant,
    message_id: Option<i32>,  // For editing existing message
}
```

### Streaming Modes

User can configure streaming granularity in settings:

| Mode | Description | Telegram Behavior |
|------|-------------|-------------------|
| `off` | No streaming | Only task completion notification |
| `summary` | Current behavior | Single message with LLM summary at end |
| `realtime` | Full streaming | Edit single message with growing content |
| `chunked` | Batched updates | New message every N seconds or N chars |

### Message Formatting

Telegram has a 4096 character limit per message. Design handles this:

```
┌─────────────────────────────────────┐
│ 🤖 Task: Fix login bug              │
│ ─────────────────────────────────── │
│ 💭 Thinking...                      │
│                                     │
│ Let me analyze the authentication   │
│ flow in the codebase...             │
│                                     │
│ 📝 Editing: src/auth/login.rs       │
│ ───────────────────                 │
│ + fn validate_token(token: &str)    │
│ +     -> Result<Claims, AuthError>  │
│                                     │
│ ⏳ In progress...                   │
└─────────────────────────────────────┘
```

When content exceeds limit:
1. Truncate oldest content (keep most recent)
2. Add "..." indicator at top
3. Or: Start new message, reply-chain to previous

### Rate Limiting Strategy

Telegram API limits:
- 30 messages/second to same chat (burst)
- 1 message/second sustained for edits
- 20 messages/minute to same chat (long-term)

**Strategy: Single-Message Editing**

```rust
impl TelegramStreamService {
    async fn handle_stream_message(&self, chat_id: i64, content: &str) {
        let mut buffer = self.buffer.lock().await;
        buffer.content.push_str(content);

        // Debounce: only flush every 500ms
        if buffer.last_flush.elapsed() < self.flush_interval {
            return;
        }

        let formatted = self.format_message(&buffer.content);

        if let Some(msg_id) = buffer.message_id {
            // Edit existing message
            self.api.edit_message_text(chat_id, msg_id, &formatted).await;
        } else {
            // Send new message, store ID for future edits
            let msg = self.api.send_message(chat_id, &formatted).await;
            buffer.message_id = Some(msg.message_id);
        }

        buffer.last_flush = Instant::now();
    }
}
```

### Content Parsing

The stream contains raw executor output. Need to parse and format:

| Raw Content | Telegram Format |
|-------------|-----------------|
| `[THINKING] ...` | `💭 Thinking...\n{content}` |
| `[TOOL_USE] Read file.rs` | `📖 Reading: file.rs` |
| `[TOOL_USE] Edit file.rs` | `📝 Editing: file.rs` |
| `[TOOL_USE] Bash: npm test` | `⚡ Running: npm test` |
| `[ASSISTANT] ...` | `💬 {content}` |
| `[ERROR] ...` | `❌ Error: {content}` |

Content parser extracts structured data from executor output format.

---

## Integration Points

### 1. Subscribe to Execution Process

When a new execution process starts for a task:

```rust
// In LocalDeploymentContainer or ExecutionProcess handler
async fn on_execution_start(&self, exec_id: Uuid, task_id: Uuid) {
    if let Some(telegram) = self.telegram_stream_service.as_ref() {
        if telegram.should_stream_for_task(task_id).await {
            telegram.subscribe_to_process(exec_id, task_id).await;
        }
    }
}
```

### 2. Stream Subscription

```rust
impl TelegramStreamService {
    pub async fn subscribe_to_process(&self, exec_id: Uuid, task_id: Uuid) {
        let stream = self.container.stream_raw_logs(&exec_id).await?;
        let chat_id = self.get_linked_chat_id().await?;

        let handle = tokio::spawn(async move {
            // Send initial "Task started" message
            let msg = self.api.send_message(chat_id, "🚀 Task started...").await;
            let msg_id = msg.message_id;

            // Stream content
            pin_mut!(stream);
            while let Some(log_msg) = stream.next().await {
                match log_msg {
                    LogMsg::Stdout(content) => {
                        self.handle_stream_content(chat_id, msg_id, &content).await;
                    }
                    LogMsg::Finished => {
                        self.handle_stream_finished(chat_id, msg_id).await;
                        break;
                    }
                    _ => {}
                }
            }
        });

        self.active_streams.insert(exec_id, handle);
    }
}
```

### 3. Settings Integration

Extend `TelegramConfig`:

```rust
pub struct TelegramConfig {
    // ... existing fields ...

    /// Streaming mode: "off", "summary", "realtime", "chunked"
    pub stream_mode: String,

    /// Flush interval for realtime mode (milliseconds)
    pub stream_flush_interval_ms: u64,

    /// Max message length before truncation
    pub stream_max_message_length: usize,
}
```

---

## Frontend Settings UI

Add streaming options to Telegram settings:

```
┌─────────────────────────────────────────────┐
│ Telegram Integration                        │
│ ─────────────────────────────────────────── │
│                                             │
│ ☑ Enable notifications                      │
│ ☑ Notify on task completion                 │
│                                             │
│ Streaming Mode:                             │
│ ○ Off - No live updates                     │
│ ○ Summary only - End-of-task summary        │
│ ● Real-time - Live streaming (edits msg)    │
│ ○ Chunked - New message every 30s           │
│                                             │
│ Update interval: [500ms ▼]                  │
│                                             │
└─────────────────────────────────────────────┘
```

---

## Message Flow Diagram

```
┌──────────────────┐
│ ExecutionProcess │
│ (LLM running)    │
└────────┬─────────┘
         │ stdout/stderr
         ▼
┌──────────────────┐
│    MsgStore      │
│ (broadcast chan) │
└────────┬─────────┘
         │ LogMsg
         ├─────────────────────────────────┐
         ▼                                 ▼
┌──────────────────┐              ┌───────────────────────┐
│ WebSocket Handler│              │ TelegramStreamService │
│ (existing)       │              │ (new)                 │
└────────┬─────────┘              └───────────┬───────────┘
         │                                    │
         ▼                                    ▼
┌──────────────────┐              ┌───────────────────────┐
│    Frontend      │              │   Message Buffer      │
│ (React/TS)       │              │ + Rate Limiter        │
└──────────────────┘              └───────────┬───────────┘
                                              │ debounced
                                              ▼
                                  ┌───────────────────────┐
                                  │   Telegram Bot API    │
                                  │ (edit_message_text)   │
                                  └───────────┬───────────┘
                                              │
                                              ▼
                                  ┌───────────────────────┐
                                  │   User's Telegram     │
                                  │   (live updates)      │
                                  └───────────────────────┘
```

---

## Edge Cases

### 1. Long-Running Tasks

For tasks running >10 minutes:
- Consider starting a new message after threshold
- Use reply-chain to link messages
- Track total messages sent per task

### 2. Multiple Concurrent Tasks

Each task gets its own message:
- Track `task_id → message_id` mapping
- Buffer per-task, not globally
- Consider limiting concurrent streams (e.g., max 3)

### 3. Connection Interruption

If Telegram API fails:
- Log error, continue buffering
- Retry with exponential backoff
- Don't block execution process

### 4. User Disables Mid-Stream

When user disables streaming:
- Gracefully close subscription
- Send final "Streaming disabled" message
- Clean up resources

### 5. Binary/Non-Text Content

For file diffs with binary content:
- Detect and skip binary diffs
- Show placeholder: "📦 Binary file changed: image.png"

---

## Performance Considerations

1. **Memory**: Buffer size limit per stream (e.g., 64KB)
2. **CPU**: Parse content lazily, only when flushing
3. **Network**: Batch edits, respect rate limits
4. **Concurrency**: Use `DashMap` for lock-free stream tracking

---

## Security Considerations

1. **Content Sanitization**: Escape HTML entities for Telegram
2. **Sensitive Data**: Don't stream env vars or secrets
3. **Rate Abuse**: Per-user rate limiting to prevent spam
4. **Token Security**: Bot token never exposed in messages

---

## Files to Create/Modify

| File | Changes |
|------|---------|
| `crates/services/src/services/telegram_stream.rs` | NEW: TelegramStreamService |
| `crates/services/src/services/telegram.rs` | Add stream mode config |
| `crates/services/src/services/config/v10.rs` | Add streaming settings |
| `crates/local-deployment/src/container.rs` | Hook stream subscription on exec start |
| `crates/server/src/routes/telegram.rs` | Add streaming settings endpoints |
| `frontend/src/.../IntegrationsSettingsSection.tsx` | Add streaming UI options |

---

## API Changes

### Update Settings Request

```typescript
interface UpdateTelegramSettingsRequest {
  notifications_enabled?: boolean;
  notify_on_task_done?: boolean;
  include_llm_summary?: boolean;
  // NEW
  stream_mode?: 'off' | 'summary' | 'realtime' | 'chunked';
  stream_flush_interval_ms?: number;
}
```

### Status Response

```typescript
interface TelegramStatusResponse {
  linked: boolean;
  username?: string;
  notifications_enabled: boolean;
  notify_on_task_done: boolean;
  include_llm_summary: boolean;
  // NEW
  stream_mode: string;
  stream_flush_interval_ms: number;
  active_streams: number;  // Currently streaming tasks
}
```
