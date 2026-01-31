# Plan: Add Integrations Tab to Settings Navigation

## Problem Summary

The Telegram integration is fully implemented in the backend and has a complete UI component (`IntegrationsSettingsSectionContent`) in the new design system. However, users accessing settings via the main navigation (`/settings`) cannot see the Integrations tab because it only exists in the new design dialog system, not in the legacy page-based settings system.

## Current State

**Two Coexisting Settings Systems:**

1. **Legacy Settings** (page-based) at `/frontend/src/pages/settings/`
   - Accessed via Navbar → `/settings` routes
   - Has 6 tabs: General, Projects, Repos, Organizations, Agents, MCP
   - **Missing:** Integrations tab

2. **New Design Settings** (dialog-based) at `/frontend/src/components/ui-new/dialogs/settings/`
   - Modal dialog accessed via Actions.Settings in workspaces UI
   - Has 7 tabs **including** Integrations
   - `IntegrationsSettingsSectionContent` fully implements Telegram integration

**Telegram Integration Status:**
- ✅ Backend API complete (`/api/telegram/status`, `/api/telegram/link`, `/api/telegram/unlink`)
- ✅ TelegramService implemented
- ✅ TypeScript types generated in `shared/types.ts`
- ✅ Frontend API methods in `lib/api.ts`
- ✅ UI component exists (`IntegrationsSettingsSectionContent`)
- ✅ i18n translations in `settings.json`
- ❌ Not accessible from legacy settings navigation

## Recommended Approach

**Add Integrations page to the legacy settings system** by creating a wrapper page that reuses the existing `IntegrationsSettingsSectionContent` component.

**Rationale:**
- Both systems coexist by design during the transition period
- Maintains consistency - all other sections exist in both systems
- Minimal code changes - reuses existing, tested component
- No backend changes needed
- Meets user expectations - they navigate to `/settings`

## Implementation Steps

### Step 1: Create IntegrationsSettings.tsx

**File:** `/frontend/src/pages/settings/IntegrationsSettings.tsx`

**Pattern:** Similar to other legacy settings pages, but wraps the new design component in `NewDesignScope` for proper styling.

**Implementation:**
```typescript
import { useTranslation } from 'react-i18next';
import { NewDesignScope } from '@/components/ui-new/scope/NewDesignScope';
import { IntegrationsSettingsSectionContent } from '@/components/ui-new/dialogs/settings/IntegrationsSettingsSection';

export function IntegrationsSettings() {
  const { t } = useTranslation('settings');

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-semibold mb-2">
          {t('settings.layout.nav.integrations')}
        </h2>
        <p className="text-muted-foreground">
          {t('settings.layout.nav.integrationsDesc')}
        </p>
      </div>

      <NewDesignScope>
        <IntegrationsSettingsSectionContent />
      </NewDesignScope>
    </div>
  );
}
```

**Why NewDesignScope?**
- `IntegrationsSettingsSectionContent` uses new design components (`SettingsCard`, `PrimaryButton`)
- Uses custom text classes (`text-low`, `text-high`, `text-normal`) that require new design CSS variables
- Wrapping in `NewDesignScope` ensures proper styling without breaking the component

### Step 2: Add Route in App.tsx

**File:** `/frontend/src/App.tsx`

**Changes:**

1. **Add import** (around line 14-22):
```typescript
import {
  AgentSettings,
  GeneralSettings,
  IntegrationsSettings,  // ← Add this
  McpSettings,
  OrganizationSettings,
  ProjectSettings,
  ReposSettings,
  SettingsLayout,
} from '@/pages/settings/';
```

2. **Add route** (around line 168, after `<Route path="mcp" element={<McpSettings />} />`):
```typescript
<Route path="integrations" element={<IntegrationsSettings />} />
```

### Step 3: Update Settings Navigation

**File:** `/frontend/src/pages/settings/SettingsLayout.tsx`

**Changes:**

1. **Add icon import** (around line 3-11):
```typescript
import {
  Settings,
  Cpu,
  Server,
  X,
  FolderOpen,
  Building2,
  GitBranch,
  PlugZap,  // ← Add this for integrations icon
} from 'lucide-react';
```

2. **Add navigation item** to `settingsNavigation` array (around line 20-45, after the `mcp` entry):
```typescript
const settingsNavigation = [
  {
    path: 'general',
    icon: Settings,
  },
  {
    path: 'projects',
    icon: FolderOpen,
  },
  {
    path: 'repos',
    icon: GitBranch,
  },
  {
    path: 'organizations',
    icon: Building2,
  },
  {
    path: 'agents',
    icon: Cpu,
  },
  {
    path: 'mcp',
    icon: Server,
  },
  {
    path: 'integrations',  // ← Add this
    icon: PlugZap,
  },
];
```

### Step 4: Export from Index

**File:** `/frontend/src/pages/settings/index.ts`

**Add export:**
```typescript
export { IntegrationsSettings } from './IntegrationsSettings';
```

### Step 5: Add i18n Translations (if missing)

**File:** `/frontend/src/i18n/locales/en/settings.json`

**Verify these keys exist** under `settings.layout.nav`:
```json
{
  "settings": {
    "layout": {
      "nav": {
        "integrations": "Integrations",
        "integrationsDesc": "Configure third-party integrations and notifications"
      }
    }
  }
}
```

*(These likely already exist since they're used in the new design dialog)*

## Critical Files

### Files to Create
- `/frontend/src/pages/settings/IntegrationsSettings.tsx` - New page component (20-30 lines)

### Files to Modify
- `/frontend/src/App.tsx` - Add import and route (~2 line changes)
- `/frontend/src/pages/settings/SettingsLayout.tsx` - Add icon import and navigation item (~3 line changes)
- `/frontend/src/pages/settings/index.ts` - Add export (1 line)
- `/frontend/src/i18n/locales/en/settings.json` - Add nav labels if missing (optional, may already exist)

### Reference Files (no changes)
- `/frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx` - Component being reused
- `/frontend/src/pages/settings/McpSettings.tsx` - Pattern reference for creating the wrapper page

## Verification Steps

### 1. Visual Verification
1. Start dev server: `pnpm run dev`
2. Navigate to Settings via navbar
3. **Expected:** See "Integrations" tab in the left sidebar with PlugZap icon
4. Click Integrations tab
5. **Expected:** See Telegram integration UI

### 2. Bot Not Configured State
1. Ensure `TELEGRAM_BOT_TOKEN` is NOT set in `.env`
2. Open Settings → Integrations
3. **Expected:** "Bot not configured" message displayed

### 3. Bot Configured, Not Linked State
1. Set `TELEGRAM_BOT_TOKEN` in `.env`
2. Restart backend
3. Open Settings → Integrations
4. **Expected:**
   - "Connect Telegram" button visible
   - Telegram icon and "Not Linked" status shown

### 4. Account Linking Flow
1. Click "Connect Telegram" button
2. **Expected:** Opens Telegram deep link in new tab
3. Send `/start` command to bot in Telegram
4. Return to Settings → Integrations
5. **Expected:**
   - Shows "Connected" status
   - Displays Telegram username if available
   - Shows notification settings (read-only)
   - "Unlink" button available

### 5. Account Unlinking
1. While linked, click "Unlink" button
2. **Expected:**
   - Status changes back to "Not Linked"
   - Shows "Connect Telegram" button again

### 6. Styling Consistency
1. Compare Integrations page with other settings pages (MCP, Agents)
2. **Expected:**
   - Page header matches style
   - Content area is well-formatted
   - No CSS conflicts or broken layouts

### 7. Translation Check
1. Change language in General Settings
2. Navigate to Integrations
3. **Expected:** All text properly translated (if translations exist for that language)

### 8. Navigation Flow
1. Navigate between different settings tabs
2. Refresh page while on Integrations tab
3. **Expected:**
   - Integrations tab stays selected after refresh
   - URL shows `/settings/integrations`

## Risk Assessment

**Low Risk:**
- No backend changes required
- No database migrations
- No breaking changes to existing code
- Component already tested in new design system
- Simple wrapper pattern used elsewhere

**Potential Issues:**
- Styling differences due to NewDesignScope wrapper → **Mitigated:** This is an expected trade-off for code reuse
- Missing translations in some languages → **Mitigated:** Falls back to English keys
- Icon availability → **Mitigated:** PlugZap is a standard lucide-react icon

## Success Criteria

✅ Integrations tab appears in legacy settings navigation
✅ Clicking tab shows Telegram integration UI
✅ All three states work correctly (not configured, not linked, linked)
✅ Link/unlink operations function properly
✅ No console errors or warnings
✅ Styling looks reasonable (doesn't need to match perfectly)
✅ Translations display correctly
✅ No regressions in other settings pages

## Future Enhancements (Out of Scope)

- Migrate all legacy settings pages to new design system
- Add more integrations (Slack, Discord, etc.) to the Integrations page
- Make notification settings editable in the UI (currently configured via Telegram bot)
- Add webhook handler for incoming Telegram commands
