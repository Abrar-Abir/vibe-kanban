# Telegram Bot Integration - Progress

## Status: Core Features Complete | Next: Real-Time Streaming

| Phase | Status |
|-------|--------|
| Foundation (config v9, TelegramService, frankenstein crate) | ✅ |
| Backend (API routes, task notifications) | ✅ |
| Frontend (Settings UI, TypeScript types) | ✅ |
| Webhook & Auto-linking (/start command, deep links) | ✅ |
| Settings API & LLM Summaries | ✅ |
| Real-Time Streaming | 🔜 Next |

## Next Up: Real-Time Streaming

> **Design**: [STREAM_FRONTEND.md](STREAM_FRONTEND.md)

Replace end-of-task LLM summaries with real-time streaming of all LLM output (thinking, file edits, tool usage) to Telegram.

**Key Components:**
- `TelegramStreamService` - Subscribe to execution process broadcasts
- Message Buffer + Rate Limiter - Debounce Telegram API calls
- Content Parser - Format raw executor output
- Settings UI - Stream mode selector

## What's Working

- **Account Linking**: Deep link + manual `/start TOKEN` fallback
- **Notifications**: Task completion with optional LLM summary
- **Settings UI**: All toggles functional (notifications, task done, LLM summary)
- **Webhook**: Receives Telegram updates, handles `/start` command
- **Polling**: UI auto-detects when account is linked

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/telegram/status` | Link status |
| GET | `/api/telegram/link` | Get deep link |
| DELETE | `/api/telegram/unlink` | Unlink account |
| PATCH | `/api/telegram/settings` | Update settings |
| POST | `/api/telegram/webhook` | Receive updates |

## Files Created/Modified

**New:**
- `crates/utils/src/config/versions/v9.rs` - TelegramConfig
- `crates/services/src/services/telegram.rs` - TelegramService
- `crates/server/src/routes/telegram.rs` - API routes
- `frontend/src/components/ui-new/dialogs/settings/IntegrationsSettingsSection.tsx`

**Modified:**
- `crates/services/src/services/task.rs` - Notification trigger
- `frontend/src/lib/api.ts` - API methods
- `shared/types.ts` - Generated types

## Dev Setup (ngrok)

```bash
ngrok http 3001
curl "https://api.telegram.org/bot$TELEGRAM_BOT_TOKEN/setWebhook?url=https://XXXX.ngrok-free.app/api/telegram/webhook"
```
