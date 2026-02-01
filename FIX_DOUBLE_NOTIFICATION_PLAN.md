# Plan: Fix Duplicate Telegram Notifications on Task Completion

## Problem Summary

When a task completes, the user receives **two identical Telegram notifications** instead of one.

## Root Cause

In `crates/local-deployment/src/container.rs` (lines 520-588), the `finalize_task()` method can be called **twice** in the same execution flow:

```rust
// Block 1: Called when no changes made (line 532)
if should_start_next {
    // start next action...
} else {
    container.finalize_task(&ctx).await;  // ← CALL #1
}

// Block 2: Called when should_finalize() is true (line 586)
if container.should_finalize(&ctx) {
    if let Some(queued_msg) = ... {
        // handle queued messages...
    } else {
        container.finalize_task(&ctx).await;  // ← CALL #2
    }
}
```

**Both blocks execute** when:
1. `should_start_next == false` (no changes made) → triggers line 532
2. `should_finalize(&ctx) == true` AND no queued message → triggers line 586

Each `finalize_task()` call sends a Telegram notification, causing duplicates.

## Solution

Add a flag to track whether finalization already occurred, skipping the second call.

### Implementation

**File:** `crates/local-deployment/src/container.rs` (lines 520-588)

**Change:** Add a boolean flag to prevent double finalization

```rust
// Around line 520-533
let mut already_finalized = false;  // NEW: Track finalization state

if should_start_next {
    if let Err(e) = container.try_start_next_action(&ctx).await {
        tracing::error!("Failed to start next action after completion: {}", e);
    }
} else {
    tracing::info!(
        "Skipping cleanup script for workspace {} - no changes made by coding agent",
        ctx.workspace.id
    );
    container.finalize_task(&ctx).await;
    already_finalized = true;  // NEW: Mark as finalized
}

// Around line 536
if !already_finalized && container.should_finalize(&ctx) {  // NEW: Check flag
    // ... rest unchanged ...
}
```

## Files to Modify

| File | Change |
|------|--------|
| `crates/local-deployment/src/container.rs` | Add `already_finalized` flag (~3 lines) |

## Verification Steps

1. **Build:** `cargo build --workspace`
2. **Test scenario (no changes):**
   - Start a task
   - Let coding agent complete with no changes
   - Verify only ONE Telegram notification received
3. **Test normal completion:**
   - Start a task
   - Let coding agent make changes and complete normally
   - Verify still receives exactly one notification

## Alternative Approaches (Not Recommended)

1. **Guard in `finalize_task()`** - Would require tracking state per-task, more complex
2. **Restructure with else-if** - Would require deeper refactoring of the control flow

The flag approach is minimal and localized to the problem area.
