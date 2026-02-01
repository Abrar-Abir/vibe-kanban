# Plan: Real-Time Telegram Streaming

## Status: ✅ Complete

The real-time streaming feature is fully implemented and working.

---

## Bug Fix (Completed)

The initial implementation had a bug where streaming showed raw JSON instead of human-readable content. This has been fixed.

### Problem (Fixed)

Telegram receives raw executor JSON instead of human-readable messages:

**Current (broken):**
```json
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me create this file"}]}}
```

**Expected:**
```
🚀 Task Name

💬 Let me create this file
📝 Editing: src/file.rs
💭 Analyzing the codebase...
```

### Root Cause

In [telegram.rs:398](crates/services/src/services/telegram.rs#L398), the streaming listens to `LogMsg::Stdout`:

```rust
match msg {
    LogMsg::Stdout(content) | LogMsg::Stderr(content) => {  // ← WRONG
        buffer.push_str(&content);  // Raw JSON!
```

But `LogMsg::Stdout` contains raw executor JSON. The **human-readable content** is in `LogMsg::JsonPatch` with `NormalizedEntry`.

### Data Flow

```
Child stdout → LogMsg::Stdout(raw_json) → MsgStore
                                              ↓
                                    ClaudeLogProcessor
                                              ↓
                             LogMsg::JsonPatch(NormalizedEntry) → MsgStore
                                              ↓
                                    Frontend (human-readable)
```

The Telegram streaming should tap into `JsonPatch`, not `Stdout`.

---

## Fix (1 file)

**File:** [crates/services/src/services/telegram.rs](crates/services/src/services/telegram.rs#L388-L435)

### 1. Update imports

```rust
use executors::logs::{
    NormalizedEntryType, ActionType,
    utils::patch::extract_normalized_entry_from_patch,
};
```

### 2. Change stream handler to use JsonPatch

Replace the stream handling logic:

```rust
let mut stream = store.history_plus_stream();
while let Some(Ok(msg)) = stream.next().await {
    match msg {
        LogMsg::JsonPatch(patch) => {
            // Extract NormalizedEntry from the patch
            if let Some((_, entry)) = extract_normalized_entry_from_patch(&patch) {
                if let Some(formatted) = format_entry(&entry) {
                    if !buffer.is_empty() {
                        buffer.push_str("\n\n");
                    }
                    buffer.push_str(&formatted);

                    // Debounce updates
                    if last_update.elapsed() >= flush_interval {
                        let text = format_stream_message(&task_name, &buffer);
                        let edit_params = EditMessageTextParams::builder()
                            .chat_id(ChatId::Integer(chat_id))
                            .message_id(msg_id)
                            .text(&text)
                            .parse_mode(ParseMode::Html)
                            .build();
                        let _ = api.edit_message_text(&edit_params).await;
                        last_update = Instant::now();
                    }
                }
            }
        }
        LogMsg::Finished => {
            // Final update
            let text = format_stream_message(&task_name, &buffer) + "\n\n✅ Done";
            // ... edit message ...
            break;
        }
        _ => {} // Ignore Stdout, Stderr, etc.
    }
}
```

### 3. Add entry formatting helper

```rust
/// Format a NormalizedEntry for Telegram display
fn format_entry(entry: &NormalizedEntry) -> Option<String> {
    match &entry.entry_type {
        NormalizedEntryType::AssistantMessage => {
            let content = entry.content.trim();
            if content.is_empty() {
                None
            } else {
                Some(format!("💬 {}", content))
            }
        }
        NormalizedEntryType::Thinking => {
            let content = entry.content.trim();
            if content.is_empty() {
                None
            } else {
                // Truncate long thinking content
                let truncated = if content.len() > 200 {
                    format!("{}...", &content[..200])
                } else {
                    content.to_string()
                };
                Some(format!("💭 {}", truncated))
            }
        }
        NormalizedEntryType::ToolUse { tool_name, action_type, .. } => {
            let icon = match action_type {
                ActionType::FileRead { path } => format!("📖 Reading: {}", path),
                ActionType::FileEdit { path, .. } => format!("📝 Editing: {}", path),
                ActionType::CommandRun { command, .. } => format!("⚡ Running: {}", command),
                ActionType::Search { query } => format!("🔍 Searching: {}", query),
                ActionType::WebFetch { url } => format!("🌐 Fetching: {}", url),
                _ => format!("🔧 {}", tool_name),
            };
            Some(icon)
        }
        NormalizedEntryType::ErrorMessage { .. } => {
            Some(format!("❌ {}", entry.content.trim()))
        }
        // Skip user messages, system messages, loading states
        _ => None,
    }
}
```

---

## Verification

1. **Build:** `cargo build --workspace`
2. **Test scenarios:**
   - Chat question (e.g., "What is 2+2?") → Should show "💬 4"
   - File creation task → Should show:
     - "💭 Let me create..." (thinking)
     - "📝 Editing: path/to/file.rs" (tool use)
     - "💬 I've created the file..." (assistant message)
   - Error case → Should show "❌ Error message"
3. **Edge cases:**
   - Long thinking content → Truncated to 200 chars
   - Rapid updates → Debounced every 500ms
   - Very long output → Telegram message truncated to 4096 chars

---

## Implementation Summary

All changes are complete and working:

| File | Change | Status |
|------|--------|--------|
| `crates/services/src/services/config/versions/v9.rs` | Add `stream_enabled: bool` | ✅ |
| `crates/services/src/services/telegram.rs` | Add `spawn_stream_to_telegram()` + `format_entry()` | ✅ |
| `crates/services/src/services/container.rs` | Call streaming at execution start | ✅ |
| `frontend/.../IntegrationsSettingsSection.tsx` | Add stream toggle UI | ✅ |
| `crates/server/src/routes/telegram.rs` | Add `stream_enabled` to types | ✅ |
| `frontend/.../locales/en/settings.json` | Add translation keys | ✅ |

**Bug Fix Applied:** Changed stream handler to use `LogMsg::JsonPatch` instead of `LogMsg::Stdout` for human-readable content.
