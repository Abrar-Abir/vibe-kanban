//! Telegram API routes for account linking and webhook handling.
//!
//! Endpoints:
//! - POST /api/telegram/webhook - Receive Telegram updates (bypasses origin validation)
//! - GET /api/telegram/link - Get deep link for account linking
//! - DELETE /api/telegram/unlink - Unlink Telegram account
//! - GET /api/telegram/status - Check link status

use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{delete, get, post},
};
use deployment::Deployment;
use frankenstein::objects::Update;
use serde::Serialize;
use services::services::{
    config::{TelegramConfig, save_config_to_file},
    telegram::{TelegramError, TelegramService, UpdateResult},
};
use ts_rs::TS;
use utils::{assets::config_path, response::ApiResponse};

use crate::{DeploymentImpl, error::ApiError};

/// Response containing the deep link URL for Telegram account linking
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct TelegramLinkInfo {
    /// The token used for linking (for reference)
    pub token: String,
    /// The deep link URL to open in Telegram
    pub deep_link: String,
    /// Whether the bot is configured (has a token)
    pub bot_configured: bool,
}

/// Response containing the current Telegram link status
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct TelegramStatusResponse {
    /// Whether an account is currently linked
    pub linked: bool,
    /// The linked Telegram username (if available)
    pub username: Option<String>,
    /// Whether notifications are enabled
    pub notifications_enabled: bool,
    /// Whether to notify on task completion
    pub notify_on_task_done: bool,
    /// Whether to include LLM summaries in notifications
    pub include_llm_summary: bool,
    /// Whether the bot is configured (has a token)
    pub bot_configured: bool,
}

impl From<TelegramConfig> for TelegramStatusResponse {
    fn from(config: TelegramConfig) -> Self {
        Self {
            linked: config.chat_id.is_some(),
            username: config.username,
            notifications_enabled: config.notifications_enabled,
            notify_on_task_done: config.notify_on_task_done,
            include_llm_summary: config.include_llm_summary,
            bot_configured: false, // Set by the handler
        }
    }
}

/// Create the Telegram router.
///
/// Note: The webhook endpoint should be registered separately without origin validation.
pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/telegram/link", get(get_link))
        .route("/telegram/unlink", delete(unlink))
        .route("/telegram/status", get(get_status))
}

/// Create a router for the webhook endpoint that bypasses origin validation.
///
/// This should be merged before the origin validation layer is applied.
pub fn webhook_router() -> Router<DeploymentImpl> {
    Router::new().route("/telegram/webhook", post(webhook))
}

/// Create a TelegramService instance from the deployment.
fn create_telegram_service(deployment: &DeploymentImpl) -> TelegramService {
    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
    let bot_username = std::env::var("TELEGRAM_BOT_USERNAME").ok();

    let service = TelegramService::new(bot_token, deployment.config().clone(), deployment.db().pool.clone());

    if let Some(username) = bot_username {
        service.with_bot_username(username)
    } else {
        service
    }
}

/// POST /api/telegram/webhook
///
/// Receive and process Telegram updates from the bot.
/// This endpoint bypasses origin validation since Telegram sends webhooks.
async fn webhook(
    State(deployment): State<DeploymentImpl>,
    Json(update): Json<Update>,
) -> Result<StatusCode, ApiError> {
    let service = create_telegram_service(&deployment);

    if !service.is_configured() {
        tracing::warn!("Telegram webhook received but bot is not configured");
        return Ok(StatusCode::OK);
    }

    match service.handle_update(update).await {
        Ok(UpdateResult::Response(text)) => {
            // Get chat_id from the config to send response
            let config = deployment.config().read().await;
            if let Some(chat_id) = config.telegram.chat_id {
                drop(config);
                if let Err(e) = service.send_message(chat_id, &text).await {
                    tracing::error!("Failed to send Telegram response: {}", e);
                }
            }
        }
        Ok(UpdateResult::LinkCompleted {
            chat_id,
            user_id: _,
            username,
        }) => {
            // Save the updated config to disk
            let config = deployment.config().read().await.clone();
            if let Err(e) = save_config_to_file(&config, &config_path()).await {
                tracing::error!("Failed to save config after Telegram link: {}", e);
            }

            // Send confirmation message
            let message = format!(
                "✅ <b>Account linked successfully!</b>\n\nWelcome{}! You will now receive notifications for task completions.",
                username.as_ref().map(|u| format!(", @{}", u)).unwrap_or_default()
            );
            if let Err(e) = service.send_message(chat_id, &message).await {
                tracing::error!("Failed to send link confirmation: {}", e);
            }
        }
        Ok(UpdateResult::NoResponse) => {
            // No response needed
        }
        Err(e) => {
            tracing::error!("Error handling Telegram update: {}", e);
            // Don't return error to Telegram - just log it
        }
    }

    Ok(StatusCode::OK)
}

/// GET /api/telegram/link
///
/// Generate a deep link for Telegram account linking.
async fn get_link(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TelegramLinkInfo>>, ApiError> {
    let service = create_telegram_service(&deployment);

    if !service.is_configured() {
        return Ok(ResponseJson(ApiResponse::success(TelegramLinkInfo {
            token: String::new(),
            deep_link: String::new(),
            bot_configured: false,
        })));
    }

    let (token, deep_link) = service
        .generate_link_token()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(ResponseJson(ApiResponse::success(TelegramLinkInfo {
        token,
        deep_link,
        bot_configured: true,
    })))
}

/// DELETE /api/telegram/unlink
///
/// Unlink the Telegram account.
async fn unlink(State(deployment): State<DeploymentImpl>) -> Result<StatusCode, ApiError> {
    let service = create_telegram_service(&deployment);

    service
        .unlink()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Save the updated config to disk
    let config = deployment.config().read().await.clone();
    if let Err(e) = save_config_to_file(&config, &config_path()).await {
        tracing::error!("Failed to save config after Telegram unlink: {}", e);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/telegram/status
///
/// Get the current Telegram link status.
async fn get_status(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TelegramStatusResponse>>, ApiError> {
    let service = create_telegram_service(&deployment);
    let status = service.get_link_status().await;

    let mut response = TelegramStatusResponse::from(status);
    response.bot_configured = service.is_configured();

    Ok(ResponseJson(ApiResponse::success(response)))
}

// Convert TelegramError to ApiError
impl From<TelegramError> for ApiError {
    fn from(err: TelegramError) -> Self {
        match err {
            TelegramError::NotConfigured => {
                ApiError::BadRequest("Telegram bot is not configured".to_string())
            }
            TelegramError::NotLinked => {
                ApiError::BadRequest("Telegram account is not linked".to_string())
            }
            TelegramError::InvalidLinkToken => {
                ApiError::BadRequest("Invalid link token".to_string())
            }
            TelegramError::LinkTokenExpired => {
                ApiError::BadRequest("Link token has expired".to_string())
            }
            TelegramError::Database(e) => ApiError::Database(e),
            TelegramError::Api(msg) => ApiError::BadRequest(format!("Telegram API error: {}", msg)),
            TelegramError::ProjectNotFound(id) => {
                ApiError::BadRequest(format!("Project not found: {}", id))
            }
            TelegramError::TaskNotFound(id) => {
                ApiError::BadRequest(format!("Task not found: {}", id))
            }
            TelegramError::NoActiveProject => {
                ApiError::BadRequest("No active project set".to_string())
            }
            TelegramError::InvalidCommand(msg) => ApiError::BadRequest(msg),
        }
    }
}
