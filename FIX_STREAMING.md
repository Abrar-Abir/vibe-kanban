# Fix Telegram Streaming Issues

## Problems

### Problem 1: Multiple Partial Line Updates
Streaming currently shows the same line being built incrementally:
```
💬 I'll research
💬 I'll research frameworks
💬 I'll research frameworks and
💬 I'll research frameworks and tools...
```

**Cause**: Each JsonPatch update to a NormalizedEntry triggers a Telegram message edit, showing partial content.

### Problem 2: Message Overflow Loses Content
When accumulated content exceeds Telegram's 4096 char limit, only the LAST 3996 characters are kept, losing earlier context.

**Cause**: `format_stream_message()` truncates from the beginning: `&content[content.len() - max_content..]`

## Solution Design

### Fix 1: Track Entry Completion State

Add a `HashMap<usize, String>` to track which entry indices we've seen and their last content. Only send updates when an entry is "complete":

**Completion criteria**:
- Entry ends with newline (`\n`)
- Entry ends with sentence punctuation (`.`, `!`, `?`)
- A different entry index arrives (previous entry must be done)
- Entry type is ToolUse (always one-shot complete)

This ensures each line is streamed atomically at natural boundaries.

### Fix 2: Multi-Message Support

Maintain `Vec<i32>` of message IDs. When buffer exceeds ~3900 chars:
1. Split at the last complete entry before the limit
2. Edit current message with content up to split point
3. Send new message for remaining content
4. Track new message ID

This preserves all content chronologically across multiple messages.

## Implementation

### Critical File
- [crates/services/src/services/telegram.rs](crates/services/src/services/telegram.rs)

### Changes to `spawn_stream_to_telegram()` (lines 337-457)

#### 1. Add State Tracking (after line 397)

```rust
// Track seen entries to avoid partial updates
let mut seen_entries: HashMap<usize, String> = HashMap::new();
let mut last_entry_index: Option<usize> = None;

// Track message IDs for multi-message support
let mut message_ids: Vec<i32> = vec![msg_id];
let mut accumulated_content = String::new();
```

#### 2. Modify Patch Processing (lines 404-432)

Replace entry processing logic with:

```rust
LogMsg::JsonPatch(patch) => {
    if let Some((entry_index, entry)) = extract_normalized_entry_from_patch(&patch) {
        if let Some(formatted) = format_entry(&entry) {
            // Check if we've seen this entry before
            let previous_content = seen_entries.get(&entry_index);

            // Determine if entry is complete
            let is_complete = formatted.ends_with('\n')
                || formatted.ends_with('.')
                || formatted.ends_with('!')
                || formatted.ends_with('?')
                || matches!(entry, NormalizedEntry::ToolUse { .. });

            // Also emit if a NEW entry arrives (previous must be done)
            let new_entry_arrived = last_entry_index.is_some()
                && last_entry_index != Some(entry_index);

            // Only add to buffer if content changed AND is complete
            let content_changed = previous_content != Some(&formatted);

            if content_changed && (is_complete || new_entry_arrived) {
                if !accumulated_content.is_empty() {
                    accumulated_content.push('\n');
                }
                accumulated_content.push_str(&formatted);
                seen_entries.insert(entry_index, formatted);
                last_entry_index = Some(entry_index);

                // Debounce updates
                if last_update.elapsed() >= flush_interval {
                    send_or_split_message(
                        &api,
                        chat_id,
                        &task_name,
                        &accumulated_content,
                        &mut message_ids,
                    ).await;
                    last_update = Instant::now();
                }
            }
        }
    }
}
```

#### 3. Add Helper Function for Multi-Message Logic (after line 457)

```rust
async fn send_or_split_message(
    api: &Api,
    chat_id: i64,
    task_name: &str,
    content: &str,
    message_ids: &mut Vec<i32>,
) {
    const MAX_CONTENT_LEN: usize = 3900; // Leave room for header/footer

    if content.len() <= MAX_CONTENT_LEN {
        // Single message - edit the last one
        let text = format_stream_message(task_name, content);
        let last_msg_id = *message_ids.last().unwrap();

        let edit_params = EditMessageTextParams::builder()
            .chat_id(ChatId::Integer(chat_id))
            .message_id(last_msg_id)
            .text(&text)
            .parse_mode(ParseMode::Html)
            .build();

        if let Err(e) = api.edit_message_text(&edit_params).await {
            tracing::debug!("Failed to edit message: {}", e);
        }
    } else {
        // Need to split - find last newline before limit
        let split_pos = content[..MAX_CONTENT_LEN]
            .rfind('\n')
            .unwrap_or(MAX_CONTENT_LEN);

        let first_part = &content[..split_pos];
        let remaining = &content[split_pos..].trim_start();

        // Edit last message with first part
        let last_msg_id = *message_ids.last().unwrap();
        let text = format_stream_message(task_name, first_part);
        let edit_params = EditMessageTextParams::builder()
            .chat_id(ChatId::Integer(chat_id))
            .message_id(last_msg_id)
            .text(&text)
            .parse_mode(ParseMode::Html)
            .build();

        if let Err(e) = api.edit_message_text(&edit_params).await {
            tracing::debug!("Failed to edit message: {}", e);
            return;
        }

        // Send new message for remaining content
        let new_text = format!("🚀 <b>{}</b> (continued)\n\n<pre>{}</pre>",
            escape_html(task_name),
            escape_html(remaining)
        );

        let send_params = SendMessageParams::builder()
            .chat_id(ChatId::Integer(chat_id))
            .text(&new_text)
            .parse_mode(ParseMode::Html)
            .build();

        match api.send_message(&send_params).await {
            Ok(msg) => {
                if let Some(new_msg_id) = msg.message_id {
                    message_ids.push(new_msg_id);
                }
            }
            Err(e) => tracing::debug!("Failed to send continuation message: {}", e),
        }
    }
}
```

#### 4. Update Finished Handler (lines 434-448)

Modify to append "✅ Done" only to the last message:

```rust
LogMsg::Finished => {
    // Final flush
    send_or_split_message(
        &api,
        chat_id,
        &task_name,
        &accumulated_content,
        &mut message_ids,
    ).await;

    // Append done indicator to LAST message only
    let last_msg_id = *message_ids.last().unwrap();
    let final_text = format_stream_message(
        &task_name,
        &format!("{}\n\n✅ Done", accumulated_content)
    );

    let edit_params = EditMessageTextParams::builder()
        .chat_id(ChatId::Integer(chat_id))
        .message_id(last_msg_id)
        .text(&final_text)
        .parse_mode(ParseMode::Html)
        .build();

    if let Err(e) = api.edit_message_text(&edit_params).await {
        tracing::debug!("Failed to edit final message: {}", e);
    }
    break;
}
```

#### 5. Update `format_stream_message()` (lines 902-918)

Remove truncation logic since multi-message handles overflow:

```rust
fn format_stream_message(task_name: &str, content: &str) -> String {
    // No truncation needed - multi-message support handles overflow
    format!(
        "🚀 <b>{}</b>\n\n<pre>{}</pre>",
        escape_html(task_name),
        escape_html(content)
    )
}
```

## Edge Cases Handled

1. **Entry with no newlines/punctuation**: Falls back to emitting when next entry arrives
2. **Very long entries**: Multi-message splits at newline boundaries
3. **Message edit failures**: Logged and gracefully continue
4. **Empty content**: Checks prevent empty message updates

## Testing & Verification

### Manual Test
1. Start Vibe Kanban: `pnpm run dev`
2. Configure Telegram bot in settings
3. Create a task with a long prompt that generates multi-paragraph responses
4. Observe Telegram messages:
   - **Problem 1 fix**: Lines should appear complete, not incrementally
   - **Problem 2 fix**: If output exceeds ~4000 chars, should see multiple messages

### Expected Behavior
- Each line appears once when complete (at newline/punctuation boundaries)
- Long outputs split into multiple messages preserving chronological order
- Final "✅ Done" only on last message

### Code Verification
- Run: `cargo check -p services`
- Run: `cargo test -p services`
- Check Telegram API types match (EditMessageTextParams, SendMessageParams)
