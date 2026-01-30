//! Telegram bot service for vibe-kanban.
//!
//! Provides Telegram bot integration including:
//! - Sending messages and task notifications
//! - Account linking/unlinking
//! - Webhook handling for bot commands
//! - Slash command handling (/start, /help, /projects, etc.)

use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use db::models::{
    project::Project,
    task::{CreateTask, Task, TaskStatus},
};
use frankenstein::{
    AsyncApi, AsyncTelegramApi, ChatId, ParseMode, SendMessageParams, Update, UpdateContent,
};
use sqlx::SqlitePool;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::services::config::{Config, TelegramConfig};

/// Errors that can occur in the Telegram service
#[derive(Debug, Error)]
pub enum TelegramError {
    #[error("Telegram API error: {0}")]
    Api(String),

    #[error("Bot token not configured")]
    NotConfigured,

    #[error("Account not linked")]
    NotLinked,

    #[error("Invalid link token")]
    InvalidLinkToken,

    #[error("Link token expired")]
    LinkTokenExpired,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Project not found: {0}")]
    ProjectNotFound(Uuid),

    #[error("Task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("No active project set. Use /project <id> to set one.")]
    NoActiveProject,

    #[error("Invalid command: {0}")]
    InvalidCommand(String),
}

/// Information about a pending link token
#[derive(Debug, Clone)]
pub struct LinkToken {
    pub token: String,
    pub created_at: DateTime<Utc>,
}

impl LinkToken {
    /// Check if this token has expired (15 minute lifetime)
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let expiry = self.created_at + chrono::Duration::minutes(15);
        now > expiry
    }
}

/// Result of processing a Telegram update
#[derive(Debug)]
pub enum UpdateResult {
    /// Command was processed successfully with a response message
    Response(String),
    /// Command was processed but requires no response
    NoResponse,
    /// Link completed successfully
    LinkCompleted { chat_id: i64, user_id: i64, username: Option<String> },
}

/// Service for Telegram bot integration
#[derive(Clone)]
pub struct TelegramService {
    /// Bot API client (None if token not configured)
    api: Option<AsyncApi>,
    /// User config (contains TelegramConfig for link status)
    config: Arc<RwLock<Config>>,
    /// Database pool for queries
    pool: SqlitePool,
    /// Pending link tokens (token -> LinkToken)
    pending_links: Arc<DashMap<String, LinkToken>>,
    /// Active project context per chat_id
    active_projects: Arc<DashMap<i64, Uuid>>,
    /// Bot username (for deep links)
    bot_username: Option<String>,
}

impl TelegramService {
    /// Create a new TelegramService
    ///
    /// If `bot_token` is None, the service will be in a "not configured" state
    /// and most operations will return `TelegramError::NotConfigured`.
    pub fn new(
        bot_token: Option<String>,
        config: Arc<RwLock<Config>>,
        pool: SqlitePool,
    ) -> Self {
        let api = bot_token.map(|token| AsyncApi::new(&token));

        Self {
            api,
            config,
            pool,
            pending_links: Arc::new(DashMap::new()),
            active_projects: Arc::new(DashMap::new()),
            bot_username: None,
        }
    }

    /// Set the bot username (used for generating deep links)
    pub fn with_bot_username(mut self, username: String) -> Self {
        self.bot_username = Some(username);
        self
    }

    /// Check if the bot is configured (has a token)
    pub fn is_configured(&self) -> bool {
        self.api.is_some()
    }

    /// Get the API client, or return NotConfigured error
    fn api(&self) -> Result<&AsyncApi, TelegramError> {
        self.api.as_ref().ok_or(TelegramError::NotConfigured)
    }

    // ========================================================================
    // Bot API Methods
    // ========================================================================

    /// Send a text message to a chat
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), TelegramError> {
        let api = self.api()?;

        let params = SendMessageParams::builder()
            .chat_id(ChatId::Integer(chat_id))
            .text(text)
            .parse_mode(ParseMode::Html)
            .build();

        api.send_message(&params)
            .await
            .map_err(|e| TelegramError::Api(e.to_string()))?;

        Ok(())
    }

    /// Send a task completion notification
    ///
    /// If `include_llm_summary` is true and a summary is provided, it will be included.
    pub async fn send_task_notification(
        &self,
        task: &Task,
        llm_summary: Option<&str>,
    ) -> Result<(), TelegramError> {
        let config = self.config.read().await;
        let telegram_config = &config.telegram;

        // Check if notifications are enabled and user is linked
        if !telegram_config.notifications_enabled || !telegram_config.notify_on_task_done {
            tracing::debug!("Telegram notifications disabled, skipping");
            return Ok(());
        }

        let chat_id = telegram_config.chat_id.ok_or(TelegramError::NotLinked)?;

        // Format the notification message
        let mut message = format!(
            "✅ <b>Task Completed</b>\n\n<b>{}</b>",
            escape_html(&task.title)
        );

        if telegram_config.include_llm_summary
            && let Some(summary) = llm_summary
        {
            message.push_str("\n\n<b>Summary:</b>\n");
            message.push_str(&escape_html(summary));
        }

        self.send_message(chat_id, &message).await
    }

    // ========================================================================
    // Link Management
    // ========================================================================

    /// Generate a new link token for account linking
    ///
    /// Returns a tuple of (token, deep_link_url)
    pub fn generate_link_token(&self) -> Result<(String, String), TelegramError> {
        let token = Uuid::new_v4().to_string();
        let link_token = LinkToken {
            token: token.clone(),
            created_at: Utc::now(),
        };

        // Clean up expired tokens first
        self.cleanup_expired_tokens();

        // Store the new token
        self.pending_links.insert(token.clone(), link_token);

        // Generate the deep link URL
        let deep_link = if let Some(username) = &self.bot_username {
            format!("https://t.me/{}?start={}", username, token)
        } else {
            // If we don't know the bot username, just return the token
            format!("start={}", token)
        };

        Ok((token, deep_link))
    }

    /// Validate a link token (check if it exists and is not expired)
    pub fn validate_link_token(&self, token: &str) -> Result<(), TelegramError> {
        let link_token = self
            .pending_links
            .get(token)
            .ok_or(TelegramError::InvalidLinkToken)?;

        if link_token.is_expired() {
            // Remove expired token
            drop(link_token);
            self.pending_links.remove(token);
            return Err(TelegramError::LinkTokenExpired);
        }

        Ok(())
    }

    /// Complete the account linking process
    ///
    /// This is called when a user sends /start <token> to the bot.
    /// Returns the updated TelegramConfig.
    pub async fn complete_link(
        &self,
        token: &str,
        chat_id: i64,
        user_id: i64,
        username: Option<String>,
    ) -> Result<TelegramConfig, TelegramError> {
        // Validate the token first
        self.validate_link_token(token)?;

        // Remove the token (single-use)
        self.pending_links.remove(token);

        // Update the config
        let mut config = self.config.write().await;
        config.telegram.chat_id = Some(chat_id);
        config.telegram.user_id = Some(user_id);
        config.telegram.username = username;
        config.telegram.notifications_enabled = true;
        config.telegram.notify_on_task_done = true;

        Ok(config.telegram.clone())
    }

    /// Unlink the Telegram account
    pub async fn unlink(&self) -> Result<(), TelegramError> {
        let mut config = self.config.write().await;
        config.telegram = TelegramConfig::default();
        Ok(())
    }

    /// Check if an account is currently linked
    pub async fn is_linked(&self) -> bool {
        let config = self.config.read().await;
        config.telegram.chat_id.is_some()
    }

    /// Get the current link status
    pub async fn get_link_status(&self) -> TelegramConfig {
        let config = self.config.read().await;
        config.telegram.clone()
    }

    /// Clean up expired link tokens
    fn cleanup_expired_tokens(&self) {
        self.pending_links.retain(|_, token| !token.is_expired());
    }

    // ========================================================================
    // Webhook Handling
    // ========================================================================

    /// Handle an incoming Telegram update (webhook payload)
    pub async fn handle_update(&self, update: Update) -> Result<UpdateResult, TelegramError> {
        // Only handle message updates
        let message = match update.content {
            UpdateContent::Message(msg) => msg,
            _ => return Ok(UpdateResult::NoResponse),
        };

        // Get text content
        let text = match &message.text {
            Some(t) => t.as_str(),
            None => return Ok(UpdateResult::NoResponse),
        };

        let chat_id = message.chat.id;
        // Telegram user IDs are u64, but we store as i64 (safe for all practical user IDs)
        let user_id = message.from.as_ref().map(|u| u.id as i64).unwrap_or(0);
        let username = message.from.as_ref().and_then(|u| u.username.clone());

        // Parse command
        if text.starts_with('/') {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            let command = parts[0].trim_start_matches('/');
            // Remove @botname suffix if present
            let command = command.split('@').next().unwrap_or(command);
            let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            return self
                .handle_command(command, args, chat_id, user_id, username)
                .await;
        }

        // Non-command messages are ignored for now
        Ok(UpdateResult::NoResponse)
    }

    // ========================================================================
    // Command Handlers
    // ========================================================================

    /// Handle a bot command
    async fn handle_command(
        &self,
        command: &str,
        args: &str,
        chat_id: i64,
        user_id: i64,
        username: Option<String>,
    ) -> Result<UpdateResult, TelegramError> {
        match command {
            "start" => self.cmd_start(args, chat_id, user_id, username).await,
            "help" => self.cmd_help().await,
            "projects" => self.cmd_projects().await,
            "project" => self.cmd_project(args, chat_id).await,
            "tasks" => self.cmd_tasks(args, chat_id).await,
            "task" => self.cmd_task(args).await,
            "newtask" => self.cmd_newtask(args, chat_id).await,
            "message" => self.cmd_message(args).await,
            _ => Ok(UpdateResult::Response(format!(
                "Unknown command: /{}. Use /help to see available commands.",
                command
            ))),
        }
    }

    /// Handle /start command (with optional link token)
    async fn cmd_start(
        &self,
        args: &str,
        chat_id: i64,
        user_id: i64,
        username: Option<String>,
    ) -> Result<UpdateResult, TelegramError> {
        // Check if this is a link request
        if !args.is_empty() {
            // Validate and complete the link
            match self.complete_link(args, chat_id, user_id, username.clone()).await {
                Ok(_) => {
                    return Ok(UpdateResult::LinkCompleted {
                        chat_id,
                        user_id,
                        username,
                    });
                }
                Err(TelegramError::InvalidLinkToken) => {
                    return Ok(UpdateResult::Response(
                        "❌ Invalid or expired link token. Please generate a new link from the web interface.".to_string()
                    ));
                }
                Err(TelegramError::LinkTokenExpired) => {
                    return Ok(UpdateResult::Response(
                        "❌ This link has expired. Please generate a new link from the web interface.".to_string()
                    ));
                }
                Err(e) => return Err(e),
            }
        }

        // Regular /start - show welcome message
        let welcome = r#"👋 <b>Welcome to VibeKanban Bot!</b>

I can help you manage your tasks and receive notifications.

<b>Available commands:</b>
/help - Show all commands
/projects - List your projects
/project &lt;id&gt; - Set active project
/tasks - List tasks in active project
/task &lt;id&gt; - Get task details
/newtask &lt;title&gt; - Create a new task
/message &lt;task_id&gt; &lt;text&gt; - Send message to a task

To link your account, use the link from the web interface."#;

        Ok(UpdateResult::Response(welcome.to_string()))
    }

    /// Handle /help command
    async fn cmd_help(&self) -> Result<UpdateResult, TelegramError> {
        let help = r#"<b>VibeKanban Bot Commands</b>

<b>Account:</b>
/start - Welcome message & account linking

<b>Projects:</b>
/projects - List all projects
/project &lt;id&gt; - Set active project for subsequent commands

<b>Tasks:</b>
/tasks - List tasks in active project
/tasks &lt;project_id&gt; - List tasks in specific project
/task &lt;id&gt; - Get task details
/newtask &lt;title&gt; - Create task in active project
/newtask &lt;project_id&gt; &lt;title&gt; - Create task in specific project

<b>Messages:</b>
/message &lt;task_id&gt; &lt;text&gt; - Send/queue a message for a task

<b>Notes:</b>
- Task and project IDs are UUIDs (can use short prefix)
- Set an active project with /project to avoid typing IDs"#;

        Ok(UpdateResult::Response(help.to_string()))
    }

    /// Handle /projects command
    async fn cmd_projects(&self) -> Result<UpdateResult, TelegramError> {
        let projects = Project::find_all(&self.pool).await?;

        if projects.is_empty() {
            return Ok(UpdateResult::Response(
                "No projects found. Create a project in the web interface first.".to_string(),
            ));
        }

        let mut message = String::from("<b>Your Projects:</b>\n\n");
        for project in projects {
            message.push_str(&format!(
                "• <b>{}</b>\n  <code>{}</code>\n\n",
                escape_html(&project.name),
                project.id
            ));
        }
        message.push_str("Use /project <id> to set the active project.");

        Ok(UpdateResult::Response(message))
    }

    /// Handle /project command - set active project
    async fn cmd_project(&self, args: &str, chat_id: i64) -> Result<UpdateResult, TelegramError> {
        if args.is_empty() {
            // Show current active project
            if let Some(project_id) = self.active_projects.get(&chat_id).map(|r| *r)
                && let Some(project) = Project::find_by_id(&self.pool, project_id).await?
            {
                return Ok(UpdateResult::Response(format!(
                    "Active project: <b>{}</b>\n<code>{}</code>",
                    escape_html(&project.name),
                    project.id
                )));
            }
            return Ok(UpdateResult::Response(
                "No active project set. Use /project <id> to set one.".to_string(),
            ));
        }

        // Parse project ID
        let project_id = parse_uuid(args)?;

        // Verify project exists
        let project = Project::find_by_id(&self.pool, project_id)
            .await?
            .ok_or(TelegramError::ProjectNotFound(project_id))?;

        // Set active project
        self.active_projects.insert(chat_id, project_id);

        Ok(UpdateResult::Response(format!(
            "✅ Active project set to: <b>{}</b>",
            escape_html(&project.name)
        )))
    }

    /// Handle /tasks command
    async fn cmd_tasks(&self, args: &str, chat_id: i64) -> Result<UpdateResult, TelegramError> {
        // Determine project ID
        let project_id = if args.is_empty() {
            // Use active project
            self.active_projects
                .get(&chat_id)
                .map(|r| *r)
                .ok_or(TelegramError::NoActiveProject)?
        } else {
            parse_uuid(args)?
        };

        // Get project name
        let project = Project::find_by_id(&self.pool, project_id)
            .await?
            .ok_or(TelegramError::ProjectNotFound(project_id))?;

        // Get tasks
        let tasks = Task::find_by_project_id_with_attempt_status(&self.pool, project_id).await?;

        if tasks.is_empty() {
            return Ok(UpdateResult::Response(format!(
                "No tasks in project <b>{}</b>.",
                escape_html(&project.name)
            )));
        }

        let mut message = format!("<b>Tasks in {}</b>\n\n", escape_html(&project.name));
        for task in tasks.iter().take(20) {
            let status_emoji = match task.task.status {
                TaskStatus::Todo => "📋",
                TaskStatus::InProgress => "🔄",
                TaskStatus::InReview => "👀",
                TaskStatus::Done => "✅",
                TaskStatus::Cancelled => "❌",
            };
            message.push_str(&format!(
                "{} <b>{}</b>\n  <code>{}</code>\n\n",
                status_emoji,
                escape_html(&task.task.title),
                task.task.id
            ));
        }

        if tasks.len() > 20 {
            message.push_str(&format!("... and {} more tasks", tasks.len() - 20));
        }

        Ok(UpdateResult::Response(message))
    }

    /// Handle /task command - get task details
    async fn cmd_task(&self, args: &str) -> Result<UpdateResult, TelegramError> {
        if args.is_empty() {
            return Ok(UpdateResult::Response(
                "Usage: /task <task_id>".to_string(),
            ));
        }

        let task_id = parse_uuid(args)?;
        let task = Task::find_by_id(&self.pool, task_id)
            .await?
            .ok_or(TelegramError::TaskNotFound(task_id))?;

        let status_emoji = match task.status {
            TaskStatus::Todo => "📋 Todo",
            TaskStatus::InProgress => "🔄 In Progress",
            TaskStatus::InReview => "👀 In Review",
            TaskStatus::Done => "✅ Done",
            TaskStatus::Cancelled => "❌ Cancelled",
        };

        let mut message = format!(
            "<b>{}</b>\n\nStatus: {}\nID: <code>{}</code>",
            escape_html(&task.title),
            status_emoji,
            task.id
        );

        if let Some(desc) = &task.description
            && !desc.is_empty()
        {
            message.push_str(&format!("\n\n<b>Description:</b>\n{}", escape_html(desc)));
        }

        Ok(UpdateResult::Response(message))
    }

    /// Handle /newtask command - create a new task
    async fn cmd_newtask(&self, args: &str, chat_id: i64) -> Result<UpdateResult, TelegramError> {
        if args.is_empty() {
            return Ok(UpdateResult::Response(
                "Usage: /newtask <title> or /newtask <project_id> <title>".to_string(),
            ));
        }

        // Try to parse first word as UUID (project_id)
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let (project_id, title) = if parts.len() == 2 {
            if let Ok(id) = Uuid::parse_str(parts[0]) {
                (id, parts[1].to_string())
            } else {
                // First word is not a UUID, use active project
                let pid = self
                    .active_projects
                    .get(&chat_id)
                    .map(|r| *r)
                    .ok_or(TelegramError::NoActiveProject)?;
                (pid, args.to_string())
            }
        } else {
            // Single argument = title, use active project
            let pid = self
                .active_projects
                .get(&chat_id)
                .map(|r| *r)
                .ok_or(TelegramError::NoActiveProject)?;
            (pid, args.to_string())
        };

        // Verify project exists
        let project = Project::find_by_id(&self.pool, project_id)
            .await?
            .ok_or(TelegramError::ProjectNotFound(project_id))?;

        // Create the task
        let create_task = CreateTask::from_title_description(project_id, title.clone(), None);
        let task_id = Uuid::new_v4();
        let task = Task::create(&self.pool, &create_task, task_id).await?;

        Ok(UpdateResult::Response(format!(
            "✅ Created task in <b>{}</b>:\n\n<b>{}</b>\n<code>{}</code>",
            escape_html(&project.name),
            escape_html(&task.title),
            task.id
        )))
    }

    /// Handle /message command - send/queue a message for a task
    async fn cmd_message(&self, args: &str) -> Result<UpdateResult, TelegramError> {
        if args.is_empty() {
            return Ok(UpdateResult::Response(
                "Usage: /message <task_id> <text>".to_string(),
            ));
        }

        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 {
            return Ok(UpdateResult::Response(
                "Usage: /message <task_id> <text>".to_string(),
            ));
        }

        let task_id = parse_uuid(parts[0])?;
        let message_text = parts[1];

        // Verify task exists
        let task = Task::find_by_id(&self.pool, task_id)
            .await?
            .ok_or(TelegramError::TaskNotFound(task_id))?;

        // For now, just acknowledge the message
        // The actual message queuing will be implemented when integrating with QueuedMessageService
        Ok(UpdateResult::Response(format!(
            "📨 Message queued for task <b>{}</b>:\n\n{}",
            escape_html(&task.title),
            escape_html(message_text)
        )))
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Escape HTML special characters for Telegram HTML parse mode
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Parse a UUID from a string, supporting short prefixes
fn parse_uuid(s: &str) -> Result<Uuid, TelegramError> {
    let s = s.trim();

    // Try full UUID first
    if let Ok(id) = Uuid::parse_str(s) {
        return Ok(id);
    }

    // Invalid format
    Err(TelegramError::InvalidCommand(format!(
        "Invalid ID format: {}. Expected a UUID.",
        s
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // HTML Escaping Tests
    // ========================================================================

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("normal text"), "normal text");
    }

    #[test]
    fn test_escape_html_all_special_chars() {
        assert_eq!(escape_html("<"), "&lt;");
        assert_eq!(escape_html(">"), "&gt;");
        assert_eq!(escape_html("&"), "&amp;");
        assert_eq!(escape_html("<>&"), "&lt;&gt;&amp;");
    }

    #[test]
    fn test_escape_html_multiple_occurrences() {
        assert_eq!(escape_html("<<>>"), "&lt;&lt;&gt;&gt;");
        assert_eq!(escape_html("a && b && c"), "a &amp;&amp; b &amp;&amp; c");
    }

    #[test]
    fn test_escape_html_mixed_content() {
        assert_eq!(
            escape_html("<b>Hello</b> & <i>World</i>"),
            "&lt;b&gt;Hello&lt;/b&gt; &amp; &lt;i&gt;World&lt;/i&gt;"
        );
    }

    #[test]
    fn test_escape_html_empty_string() {
        assert_eq!(escape_html(""), "");
    }

    #[test]
    fn test_escape_html_preserves_other_chars() {
        assert_eq!(escape_html("Hello, World!"), "Hello, World!");
        assert_eq!(escape_html("123 + 456 = 579"), "123 + 456 = 579");
        assert_eq!(escape_html("emoji: 🎉"), "emoji: 🎉");
    }

    // ========================================================================
    // Link Token Expiry Tests
    // ========================================================================

    #[test]
    fn test_link_token_expiry() {
        let fresh_token = LinkToken {
            token: "test".to_string(),
            created_at: Utc::now(),
        };
        assert!(!fresh_token.is_expired());

        let expired_token = LinkToken {
            token: "test".to_string(),
            created_at: Utc::now() - chrono::Duration::minutes(20),
        };
        assert!(expired_token.is_expired());
    }

    #[test]
    fn test_link_token_exactly_at_expiry_boundary() {
        // Token at exactly 15 minutes should not be expired yet
        let token_at_boundary = LinkToken {
            token: "test".to_string(),
            created_at: Utc::now() - chrono::Duration::minutes(15),
        };
        // At exactly 15 minutes, now > expiry is false, so not expired
        assert!(!token_at_boundary.is_expired());

        // Token at 15 minutes + 1 second should be expired
        let token_past_boundary = LinkToken {
            token: "test".to_string(),
            created_at: Utc::now() - chrono::Duration::minutes(15) - chrono::Duration::seconds(1),
        };
        assert!(token_past_boundary.is_expired());
    }

    #[test]
    fn test_link_token_just_created() {
        let token = LinkToken {
            token: "fresh".to_string(),
            created_at: Utc::now(),
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn test_link_token_14_minutes_old() {
        let token = LinkToken {
            token: "test".to_string(),
            created_at: Utc::now() - chrono::Duration::minutes(14),
        };
        assert!(!token.is_expired());
    }

    // ========================================================================
    // UUID Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_uuid_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = parse_uuid(uuid_str);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), uuid_str);
    }

    #[test]
    fn test_parse_uuid_with_whitespace() {
        let uuid_str = "  550e8400-e29b-41d4-a716-446655440000  ";
        let result = parse_uuid(uuid_str);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        let invalid = "not-a-uuid";
        let result = parse_uuid(invalid);
        assert!(result.is_err());
        match result {
            Err(TelegramError::InvalidCommand(msg)) => {
                assert!(msg.contains("Invalid ID format"));
            }
            _ => panic!("Expected InvalidCommand error"),
        }
    }

    #[test]
    fn test_parse_uuid_empty() {
        let result = parse_uuid("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_uuid_partial() {
        let result = parse_uuid("550e8400-e29b");
        assert!(result.is_err());
    }

    // ========================================================================
    // TelegramError Display Tests
    // ========================================================================

    #[test]
    fn test_telegram_error_display() {
        assert_eq!(
            TelegramError::NotConfigured.to_string(),
            "Bot token not configured"
        );
        assert_eq!(
            TelegramError::NotLinked.to_string(),
            "Account not linked"
        );
        assert_eq!(
            TelegramError::InvalidLinkToken.to_string(),
            "Invalid link token"
        );
        assert_eq!(
            TelegramError::LinkTokenExpired.to_string(),
            "Link token expired"
        );
        assert_eq!(
            TelegramError::NoActiveProject.to_string(),
            "No active project set. Use /project <id> to set one."
        );
    }

    #[test]
    fn test_telegram_error_project_not_found() {
        let id = Uuid::new_v4();
        let error = TelegramError::ProjectNotFound(id);
        assert!(error.to_string().contains(&id.to_string()));
    }

    #[test]
    fn test_telegram_error_task_not_found() {
        let id = Uuid::new_v4();
        let error = TelegramError::TaskNotFound(id);
        assert!(error.to_string().contains(&id.to_string()));
    }

    #[test]
    fn test_telegram_error_invalid_command() {
        let error = TelegramError::InvalidCommand("test error".to_string());
        assert_eq!(error.to_string(), "Invalid command: test error");
    }

    #[test]
    fn test_telegram_error_api() {
        let error = TelegramError::Api("connection failed".to_string());
        assert_eq!(error.to_string(), "Telegram API error: connection failed");
    }

    // ========================================================================
    // UpdateResult Tests
    // ========================================================================

    #[test]
    fn test_update_result_response() {
        let result = UpdateResult::Response("Hello".to_string());
        match result {
            UpdateResult::Response(msg) => assert_eq!(msg, "Hello"),
            _ => panic!("Expected Response variant"),
        }
    }

    #[test]
    fn test_update_result_no_response() {
        let result = UpdateResult::NoResponse;
        match result {
            UpdateResult::NoResponse => {}
            _ => panic!("Expected NoResponse variant"),
        }
    }

    #[test]
    fn test_update_result_link_completed() {
        let result = UpdateResult::LinkCompleted {
            chat_id: 12345,
            user_id: 67890,
            username: Some("testuser".to_string()),
        };
        match result {
            UpdateResult::LinkCompleted {
                chat_id,
                user_id,
                username,
            } => {
                assert_eq!(chat_id, 12345);
                assert_eq!(user_id, 67890);
                assert_eq!(username, Some("testuser".to_string()));
            }
            _ => panic!("Expected LinkCompleted variant"),
        }
    }

    #[test]
    fn test_update_result_link_completed_no_username() {
        let result = UpdateResult::LinkCompleted {
            chat_id: 12345,
            user_id: 67890,
            username: None,
        };
        match result {
            UpdateResult::LinkCompleted { username, .. } => {
                assert!(username.is_none());
            }
            _ => panic!("Expected LinkCompleted variant"),
        }
    }

    // ========================================================================
    // TelegramConfig Default Tests
    // ========================================================================

    #[test]
    fn test_telegram_config_default() {
        let config = TelegramConfig::default();
        assert!(config.chat_id.is_none());
        assert!(config.user_id.is_none());
        assert!(config.username.is_none());
        assert!(!config.notifications_enabled);
        assert!(!config.notify_on_task_done);
        assert!(!config.include_llm_summary);
    }
}
