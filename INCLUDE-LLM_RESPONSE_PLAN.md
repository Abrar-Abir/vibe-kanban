# Plan: Include LLM Response in Telegram Notifications

## Problem

Currently, Telegram notifications only show "Task x completed" without the LLM response. The infrastructure for including LLM summaries exists but is incomplete:

- ✅ `include_llm_summary` field exists in config (v9)
- ✅ `TelegramService::send_task_notification()` checks the flag and includes summaries
- ✅ LLM summary is fetched from `CodingAgentTurn.summary`
- ❌ No API endpoint to update Telegram settings
- ❌ Frontend toggle is disabled (`disabled={true}`)
- ❌ Default value is `false`, so summaries are never included

## Solution Overview

Enable the existing `include_llm_summary` functionality by:
1. Adding a `PATCH /api/telegram/settings` endpoint
2. Creating a `TelegramService::update_settings()` method
3. Adding `telegramApi.updateSettings()` in frontend
4. Enabling the UI toggles and wiring them to the API

---

## Implementation Steps

### Step 1: Add TelegramService::update_settings() Method

**File:** `crates/services/src/services/telegram.rs`

Add a new method to update notification settings:

```rust
/// Settings that can be updated via API
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TelegramSettingsUpdate {
    pub notifications_enabled: Option<bool>,
    pub notify_on_task_done: Option<bool>,
    pub include_llm_summary: Option<bool>,
}

impl TelegramService {
    /// Update Telegram notification settings
    pub async fn update_settings(&self, update: TelegramSettingsUpdate) -> Result<TelegramConfig, TelegramError> {
        let mut config = self.config.write().await;

        if let Some(v) = update.notifications_enabled {
            config.telegram.notifications_enabled = v;
        }
        if let Some(v) = update.notify_on_task_done {
            config.telegram.notify_on_task_done = v;
        }
        if let Some(v) = update.include_llm_summary {
            config.telegram.include_llm_summary = v;
        }

        Ok(config.telegram.clone())
    }
}
```

### Step 2: Add PATCH /api/telegram/settings Endpoint

**File:** `crates/server/src/routes/telegram.rs`

Add request/response types and handler:

```rust
/// Request to update Telegram notification settings
#[derive(Debug, serde::Deserialize, TS)]
#[ts(export)]
pub struct UpdateTelegramSettingsRequest {
    pub notifications_enabled: Option<bool>,
    pub notify_on_task_done: Option<bool>,
    pub include_llm_summary: Option<bool>,
}

// Add to router:
.route("/telegram/settings", patch(update_settings))

/// PATCH /api/telegram/settings
async fn update_settings(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<UpdateTelegramSettingsRequest>,
) -> Result<ResponseJson<ApiResponse<TelegramStatusResponse>>, ApiError> {
    let service = get_telegram_service(&deployment)?;

    let update = TelegramSettingsUpdate {
        notifications_enabled: request.notifications_enabled,
        notify_on_task_done: request.notify_on_task_done,
        include_llm_summary: request.include_llm_summary,
    };

    let updated = service.update_settings(update).await?;

    // Save config to disk
    let config = deployment.config().read().await.clone();
    if let Err(e) = save_config_to_file(&config, &config_path()).await {
        tracing::error!("Failed to save Telegram settings: {}", e);
    }

    let mut response = TelegramStatusResponse::from(updated);
    response.bot_configured = true;

    Ok(ResponseJson(ApiResponse::success(response)))
}
```

### Step 3: Add Frontend API Method

**File:** `frontend/src/lib/api.ts`

Add `updateSettings` to `telegramApi`:

```typescript
export const telegramApi = {
  // ... existing methods ...

  /**
   * Update Telegram notification settings
   */
  updateSettings: async (settings: {
    notifications_enabled?: boolean;
    notify_on_task_done?: boolean;
    include_llm_summary?: boolean;
  }): Promise<TelegramStatusResponse> => {
    const response = await makeRequest('/api/telegram/settings', {
      method: 'PATCH',
      body: JSON.stringify(settings),
    });
    return handleApiResponse<TelegramStatusResponse>(response);
  },
};
```

### Step 4: Enable UI Toggles

**File:** `frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx`

Enable the checkboxes and add handlers:

```tsx
const handleSettingChange = async (
  setting: 'notifications_enabled' | 'notify_on_task_done' | 'include_llm_summary',
  value: boolean
) => {
  try {
    const updated = await telegramApi.updateSettings({ [setting]: value });
    setStatus(updated);
  } catch (err) {
    setError(t('integrations.telegram.updateError'));
    console.error('Failed to update setting:', err);
  }
};

// Change the checkboxes from disabled to enabled:
<SettingsCheckbox
  id="telegram-include-llm-summary"
  label={t('integrations.telegram.includeLlmSummary.label')}
  description={t('integrations.telegram.includeLlmSummary.description')}
  checked={status.include_llm_summary}
  onChange={(checked) => handleSettingChange('include_llm_summary', checked)}
  disabled={false}  // Was: disabled={true}
/>
```

### Step 5: Regenerate TypeScript Types

Run type generation to export the new request type:

```bash
pnpm run generate-types
```

### Step 6: Add Missing Translation (if needed)

**File:** `frontend/src/i18n/locales/en/settings.json`

```json
{
  "integrations": {
    "telegram": {
      "updateError": "Failed to update Telegram settings"
    }
  }
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/services/src/services/telegram.rs` | Add `TelegramSettingsUpdate` struct, `update_settings()` method |
| `crates/server/src/routes/telegram.rs` | Add `UpdateTelegramSettingsRequest`, `PATCH /settings` endpoint |
| `crates/server/src/bin/generate_types.rs` | Add `UpdateTelegramSettingsRequest` to exports |
| `frontend/src/lib/api.ts` | Add `telegramApi.updateSettings()` |
| `frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx` | Enable toggles, add handlers |
| `frontend/src/i18n/locales/en/settings.json` | Add `updateError` translation |
| `shared/types.ts` | (auto-generated) |

---

## Verification Steps

1. **Build verification:**
   ```bash
   cargo build
   pnpm run check
   ```

2. **API test:**
   ```bash
   # Get current settings
   curl http://localhost:3001/api/telegram/status

   # Enable LLM summaries
   curl -X PATCH http://localhost:3001/api/telegram/settings \
     -H "Content-Type: application/json" \
     -d '{"include_llm_summary": true}'
   ```

3. **UI test:**
   - Go to Settings → Integrations
   - Toggle "Include AI Summary" on
   - Complete a task
   - Verify Telegram notification includes the LLM response

4. **End-to-end test:**
   - With `include_llm_summary: true`, complete a task with coding agent
   - Verify Telegram message contains:
     - ✅ Task Completed header
     - Task title
     - **Summary:** section with LLM response

---

## Default Value Consideration

Currently `include_llm_summary` defaults to `false`. Options:

**Option A (Recommended):** Keep default as `false`, user enables via UI
- Safer: User explicitly opts in
- Less data sent by default

**Option B:** Change default to `true` in v9 migration
- Better UX: Summaries work immediately
- Would require updating existing configs

Recommendation: **Option A** - Keep default false, but ensure UI toggle works.

---

## Success Criteria

- [ ] `PATCH /api/telegram/settings` endpoint works
- [ ] UI toggles are enabled and functional
- [ ] Settings persist after restart (saved to config file)
- [ ] Task completion notification includes LLM summary when enabled
- [ ] TypeScript types are generated correctly
