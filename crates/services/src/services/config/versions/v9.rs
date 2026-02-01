use anyhow::Error;
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub use v8::{
    EditorConfig, EditorType, GitHubConfig, NotificationConfig, SendMessageShortcut, ShowcaseState,
    SoundFile, ThemeMode, UiLanguage,
};

use crate::services::config::versions::v8;

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
pub struct TelegramConfig {
    pub chat_id: Option<i64>,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub notifications_enabled: bool,
    pub notify_on_task_done: bool,
    pub include_llm_summary: bool,
    #[serde(default)]
    pub stream_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Config {
    pub config_version: String,
    pub theme: ThemeMode,
    pub executor_profile: ExecutorProfileId,
    pub disclaimer_acknowledged: bool,
    pub onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    pub analytics_enabled: bool,
    pub workspace_dir: Option<String>,
    pub last_app_version: Option<String>,
    pub show_release_notes: bool,
    #[serde(default)]
    pub language: UiLanguage,
    #[serde(default = "default_git_branch_prefix")]
    pub git_branch_prefix: String,
    #[serde(default)]
    pub showcases: ShowcaseState,
    #[serde(default = "default_pr_auto_description_enabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(default)]
    pub pr_auto_description_prompt: Option<String>,
    #[serde(default)]
    pub beta_workspaces: bool,
    #[serde(default)]
    pub beta_workspaces_invitation_sent: bool,
    #[serde(default)]
    pub commit_reminder: bool,
    #[serde(default)]
    pub send_message_shortcut: SendMessageShortcut,
    #[serde(default)]
    pub telegram: TelegramConfig,
}

impl Config {
    fn from_v8_config(old_config: v8::Config) -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: old_config.theme,
            executor_profile: old_config.executor_profile,
            disclaimer_acknowledged: old_config.disclaimer_acknowledged,
            onboarding_acknowledged: old_config.onboarding_acknowledged,
            notifications: old_config.notifications,
            editor: old_config.editor,
            github: old_config.github,
            analytics_enabled: old_config.analytics_enabled,
            workspace_dir: old_config.workspace_dir,
            last_app_version: old_config.last_app_version,
            show_release_notes: old_config.show_release_notes,
            language: old_config.language,
            git_branch_prefix: old_config.git_branch_prefix,
            showcases: old_config.showcases,
            pr_auto_description_enabled: old_config.pr_auto_description_enabled,
            pr_auto_description_prompt: old_config.pr_auto_description_prompt,
            beta_workspaces: old_config.beta_workspaces,
            beta_workspaces_invitation_sent: old_config.beta_workspaces_invitation_sent,
            commit_reminder: old_config.commit_reminder,
            send_message_shortcut: old_config.send_message_shortcut,
            telegram: TelegramConfig::default(),
        }
    }

    pub fn from_previous_version(raw_config: &str) -> Result<Self, Error> {
        let old_config = v8::Config::from(raw_config.to_string());
        Ok(Self::from_v8_config(old_config))
    }
}

impl From<String> for Config {
    fn from(raw_config: String) -> Self {
        if let Ok(config) = serde_json::from_str::<Config>(&raw_config)
            && config.config_version == "v9"
        {
            return config;
        }

        match Self::from_previous_version(&raw_config) {
            Ok(config) => {
                tracing::info!("Config upgraded to v9");
                config
            }
            Err(e) => {
                tracing::warn!("Config migration failed: {}, using default", e);
                Self::default()
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: ThemeMode::System,
            executor_profile: ExecutorProfileId::new(BaseCodingAgent::ClaudeCode),
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            analytics_enabled: true,
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            git_branch_prefix: default_git_branch_prefix(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: true,
            pr_auto_description_prompt: None,
            beta_workspaces: false,
            beta_workspaces_invitation_sent: false,
            commit_reminder: false,
            send_message_shortcut: SendMessageShortcut::default(),
            telegram: TelegramConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TelegramConfig Tests
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
        assert!(!config.stream_enabled);
    }

    #[test]
    fn test_telegram_config_serialization() {
        let config = TelegramConfig {
            chat_id: Some(12345),
            user_id: Some(67890),
            username: Some("testuser".to_string()),
            notifications_enabled: true,
            notify_on_task_done: true,
            include_llm_summary: false,
            stream_enabled: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TelegramConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.chat_id, Some(12345));
        assert_eq!(deserialized.user_id, Some(67890));
        assert_eq!(deserialized.username, Some("testuser".to_string()));
        assert!(deserialized.notifications_enabled);
        assert!(deserialized.notify_on_task_done);
        assert!(!deserialized.include_llm_summary);
        assert!(deserialized.stream_enabled);
    }

    // ========================================================================
    // Config Version Tests
    // ========================================================================

    #[test]
    fn test_config_default_version() {
        let config = Config::default();
        assert_eq!(config.config_version, "v9");
    }

    #[test]
    fn test_config_default_has_telegram() {
        let config = Config::default();
        assert!(config.telegram.chat_id.is_none());
        assert!(!config.telegram.notifications_enabled);
    }

    // ========================================================================
    // Config Migration Tests (v8 -> v9)
    // ========================================================================

    #[test]
    fn test_config_migration_from_v8() {
        // Create a v8 config JSON (without telegram field)
        let v8_json = r#"{
            "config_version": "v8",
            "theme": "System",
            "executor_profile": "claude-code",
            "disclaimer_acknowledged": true,
            "onboarding_acknowledged": true,
            "notifications": {
                "enabled": true,
                "sound_enabled": false,
                "sound_file": "Default"
            },
            "editor": {
                "type": "VsCode",
                "path": null
            },
            "github": {
                "token": null
            },
            "analytics_enabled": true,
            "workspace_dir": "/test/workspace",
            "last_app_version": "1.0.0",
            "show_release_notes": false,
            "language": "English",
            "git_branch_prefix": "vk",
            "showcases": {},
            "pr_auto_description_enabled": true,
            "pr_auto_description_prompt": null,
            "beta_workspaces": false,
            "beta_workspaces_invitation_sent": false,
            "commit_reminder": true,
            "send_message_shortcut": "ModifierEnter"
        }"#;

        let config = Config::from(v8_json.to_string());

        // Verify migration
        assert_eq!(config.config_version, "v9");
        assert!(config.disclaimer_acknowledged);
        assert!(config.onboarding_acknowledged);
        assert_eq!(config.workspace_dir, Some("/test/workspace".to_string()));
        assert!(config.commit_reminder);

        // Verify telegram is set to default
        assert!(config.telegram.chat_id.is_none());
        assert!(config.telegram.user_id.is_none());
        assert!(config.telegram.username.is_none());
        assert!(!config.telegram.notifications_enabled);
        assert!(!config.telegram.notify_on_task_done);
        assert!(!config.telegram.include_llm_summary);
        assert!(!config.telegram.stream_enabled);
    }

    #[test]
    fn test_config_v9_loads_directly() {
        // Create a v9 config JSON with telegram field
        let v9_json = r#"{
            "config_version": "v9",
            "theme": "Dark",
            "executor_profile": "claude-code",
            "disclaimer_acknowledged": true,
            "onboarding_acknowledged": true,
            "notifications": {
                "enabled": true,
                "sound_enabled": false,
                "sound_file": "Default"
            },
            "editor": {
                "type": "VsCode",
                "path": null
            },
            "github": {
                "token": null
            },
            "analytics_enabled": true,
            "workspace_dir": null,
            "last_app_version": null,
            "show_release_notes": false,
            "language": "English",
            "git_branch_prefix": "vk",
            "showcases": {},
            "pr_auto_description_enabled": true,
            "pr_auto_description_prompt": null,
            "beta_workspaces": false,
            "beta_workspaces_invitation_sent": false,
            "commit_reminder": false,
            "send_message_shortcut": "Enter",
            "telegram": {
                "chat_id": 12345,
                "user_id": 67890,
                "username": "testuser",
                "notifications_enabled": true,
                "notify_on_task_done": true,
                "include_llm_summary": true,
                "stream_enabled": true
            }
        }"#;

        let config = Config::from(v9_json.to_string());

        assert_eq!(config.config_version, "v9");
        assert_eq!(config.telegram.chat_id, Some(12345));
        assert_eq!(config.telegram.user_id, Some(67890));
        assert_eq!(config.telegram.username, Some("testuser".to_string()));
        assert!(config.telegram.notifications_enabled);
        assert!(config.telegram.notify_on_task_done);
        assert!(config.telegram.include_llm_summary);
        assert!(config.telegram.stream_enabled);
    }

    #[test]
    fn test_config_v9_with_empty_telegram() {
        // v9 config with telegram field set to defaults
        let v9_json = r#"{
            "config_version": "v9",
            "theme": "System",
            "executor_profile": "claude-code",
            "disclaimer_acknowledged": false,
            "onboarding_acknowledged": false,
            "notifications": {
                "enabled": true,
                "sound_enabled": false,
                "sound_file": "Default"
            },
            "editor": {
                "type": "VsCode",
                "path": null
            },
            "github": {
                "token": null
            },
            "analytics_enabled": true,
            "workspace_dir": null,
            "last_app_version": null,
            "show_release_notes": false,
            "language": "English",
            "git_branch_prefix": "vk",
            "showcases": {},
            "pr_auto_description_enabled": true,
            "pr_auto_description_prompt": null,
            "beta_workspaces": false,
            "beta_workspaces_invitation_sent": false,
            "commit_reminder": false,
            "send_message_shortcut": "ModifierEnter",
            "telegram": {
                "chat_id": null,
                "user_id": null,
                "username": null,
                "notifications_enabled": false,
                "notify_on_task_done": false,
                "include_llm_summary": false,
                "stream_enabled": false
            }
        }"#;

        let config = Config::from(v9_json.to_string());

        assert_eq!(config.config_version, "v9");
        assert!(config.telegram.chat_id.is_none());
        assert!(!config.telegram.notifications_enabled);
    }

    #[test]
    fn test_config_invalid_json_returns_default() {
        let invalid_json = "{ invalid json }";
        let config = Config::from(invalid_json.to_string());

        // Should return default config
        assert_eq!(config.config_version, "v9");
        assert!(!config.disclaimer_acknowledged);
        assert!(config.telegram.chat_id.is_none());
    }

    #[test]
    fn test_config_empty_string_returns_default() {
        let config = Config::from(String::new());

        assert_eq!(config.config_version, "v9");
        assert!(!config.disclaimer_acknowledged);
    }

    // ========================================================================
    // Config Field Default Tests
    // ========================================================================

    #[test]
    fn test_default_git_branch_prefix() {
        let prefix = default_git_branch_prefix();
        assert_eq!(prefix, "vk");
    }

    #[test]
    fn test_default_pr_auto_description_enabled() {
        let enabled = default_pr_auto_description_enabled();
        assert!(enabled);
    }

    #[test]
    fn test_config_default_values() {
        let config = Config::default();

        assert_eq!(config.git_branch_prefix, "vk");
        assert!(config.pr_auto_description_enabled);
        assert!(config.analytics_enabled);
        assert!(!config.beta_workspaces);
        assert!(!config.commit_reminder);
    }
}
