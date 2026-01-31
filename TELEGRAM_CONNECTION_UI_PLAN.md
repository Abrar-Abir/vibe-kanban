# Plan: Fix Telegram Desktop Deep Link Token Issue

## Problem Summary

When clicking "Connect Telegram" in Settings → Integrations, the deep link (`https://t.me/kanban_vibe_bot?start=TOKEN`) opens Telegram Desktop correctly, but **Telegram Desktop only sends `/start` without the token parameter**. This causes the linking to fail because the backend requires the token to validate and complete the link.

**Root Cause:** Known Telegram Desktop quirk - the `?start=PARAM` query parameter from deep links is not always passed through to the bot conversation.

## Current Behavior

1. User clicks "Connect Telegram" → Opens `https://t.me/bot?start=TOKEN`
2. Telegram Desktop opens and starts a chat with the bot
3. Telegram Desktop sends just `/start` (without the token)
4. Backend receives empty args, shows welcome message instead of linking
5. User is confused - account not linked

## Solution: Show Token with Copy Button + Manual Instructions

Add a fallback UI that displays the token with a copy button and clear instructions for users to manually send `/start TOKEN` to the bot.

**Why this approach:**
- Simplest fix with minimal code changes
- Works for all Telegram clients (Desktop, Web, Mobile)
- No backend changes needed
- Provides clear user guidance
- Deep link remains as primary action for clients that support it

## Implementation Steps

### Step 1: Update IntegrationsSettingsSection.tsx

**File:** `/frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx`

**Changes:**

1. Add a "Copy token" button that copies `/start TOKEN` to clipboard
2. Show manual instructions below the "Connect Telegram" button
3. Add visual feedback for copy action (toast or button state)

**UI Layout (after changes):**
```
┌─────────────────────────────────────────────────────────┐
│  Telegram                                    Not Linked │
│                                                         │
│  [Connect Telegram Button]                              │
│                                                         │
│  ─────────────── Or link manually ───────────────       │
│                                                         │
│  1. Open Telegram and search for @kanban_vibe_bot       │
│  2. Send this command to the bot:                       │
│                                                         │
│  ┌────────────────────────────────────────────┐        │
│  │ /start abc123-def456-...                 📋 │        │
│  └────────────────────────────────────────────┘        │
│                                                         │
│  Token expires in 15 minutes                            │
└─────────────────────────────────────────────────────────┘
```

**Code changes (around line 200-220):**

```typescript
// Add state for copy feedback
const [copied, setCopied] = useState(false);

// Add copy handler
const handleCopyCommand = async () => {
  if (linkInfo?.token) {
    await navigator.clipboard.writeText(`/start ${linkInfo.token}`);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }
};
```

**JSX additions (after the Connect Telegram button):**
```tsx
{linkInfo?.token && (
  <div className="mt-4 pt-4 border-t border-stroke-low">
    <p className="text-xs text-low mb-2">
      {t('integrations.telegram.manualLinkTitle')}
    </p>
    <ol className="text-xs text-low list-decimal list-inside space-y-1 mb-3">
      <li>{t('integrations.telegram.manualStep1', { botUsername: 'kanban_vibe_bot' })}</li>
      <li>{t('integrations.telegram.manualStep2')}</li>
    </ol>
    <div className="flex items-center gap-2 bg-fill-secondary rounded-md p-2">
      <code className="text-xs text-normal flex-1 font-mono truncate">
        /start {linkInfo.token}
      </code>
      <button
        onClick={handleCopyCommand}
        className="p-1 hover:bg-fill-tertiary rounded"
        title={t('common.copy')}
      >
        {copied ? <CheckIcon className="w-4 h-4 text-green-500" /> : <ClipboardIcon className="w-4 h-4" />}
      </button>
    </div>
    <p className="text-xs text-low mt-2">
      {t('integrations.telegram.tokenExpiry')}
    </p>
  </div>
)}
```

### Step 2: Add i18n Translations

**File:** `/frontend/src/i18n/locales/en/settings.json`

**Add under `integrations.telegram`:**
```json
{
  "integrations": {
    "telegram": {
      "manualLinkTitle": "Or link manually:",
      "manualStep1": "Open Telegram and search for @{{botUsername}}",
      "manualStep2": "Send this command to the bot:",
      "tokenExpiry": "Token expires in 15 minutes"
    }
  }
}
```

### Step 3: Add Icon Import (if needed)

**File:** `/frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx`

Check if `ClipboardIcon` and `CheckIcon` are already imported, if not add:
```typescript
import { ClipboardIcon, CheckIcon } from '@heroicons/react/24/outline';
// or from lucide-react:
import { Clipboard, Check } from 'lucide-react';
```

## Critical Files

### Files to Modify
- `/frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx` - Add manual link UI (~30 lines)
- `/frontend/src/i18n/locales/en/settings.json` - Add translations (~4 keys)

### Reference Files (no changes)
- `/frontend/src/lib/api.ts` - API already returns token in `TelegramLinkInfo`
- `shared/types.ts` - `TelegramLinkInfo` type already has `token` field

## Verification Steps

### 1. UI Verification
1. Start dev server: `pnpm run dev`
2. Navigate to Settings → Integrations
3. **Expected:**
   - "Connect Telegram" button visible
   - Manual linking section visible below with token
   - Copy button next to token command

### 2. Copy Functionality
1. Click the copy button next to `/start TOKEN`
2. **Expected:**
   - Button shows checkmark briefly
   - Clipboard contains `/start <actual-token>`

### 3. Manual Linking Flow (Telegram Desktop)
1. Click "Connect Telegram" (deep link opens Telegram Desktop)
2. If Telegram only sends `/start`, copy the command from the UI
3. Paste and send `/start TOKEN` manually in Telegram
4. **Expected:** Bot responds with "Account linked successfully!"
5. Refresh Integrations page
6. **Expected:** Shows "Connected" status

### 4. Deep Link Flow (Telegram Mobile/Web)
1. Click "Connect Telegram" on mobile or web
2. **Expected:** Deep link works normally, account links without manual step

### 5. Token Expiry Display
1. Verify "Token expires in 15 minutes" text is visible
2. Wait 15+ minutes
3. Generate new link
4. **Expected:** New token is displayed

## Risk Assessment

**Low Risk:**
- Frontend-only changes
- No backend modifications
- Fallback mechanism - doesn't break existing flow
- Token already available in API response

**Potential Issues:**
- Clipboard API requires HTTPS or localhost → **Mitigated:** Dev runs on localhost
- Copy button styling might need adjustment → **Mitigated:** Simple icon button

## Success Criteria

✅ Manual link section appears when not linked
✅ Token command is displayed correctly
✅ Copy button copies full command to clipboard
✅ Visual feedback on copy (checkmark)
✅ Deep link still works as primary action
✅ Manual flow successfully links account
✅ Translations display correctly
✅ No console errors

## Alternative Approaches (Not Recommended)

1. **QR Code** - More complex, requires QR library, overkill for this use case
2. **Reverse linking (bot sends code)** - Requires backend changes, more complex flow
3. **Session-based linking** - Would require significant backend refactoring
