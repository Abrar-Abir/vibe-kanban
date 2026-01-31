import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { TelegramLogoIcon, LinkBreakIcon, ArrowSquareUpRightIcon, Copy, Check } from '@phosphor-icons/react';

import { telegramApi } from '@/lib/api';
import { SettingsCard, SettingsCheckbox } from './SettingsComponents';
import { PrimaryButton } from '../../primitives/PrimaryButton';

interface TelegramStatus {
  linked: boolean;
  username: string | null;
  notifications_enabled: boolean;
  notify_on_task_done: boolean;
  include_llm_summary: boolean;
  bot_configured: boolean;
}

interface TelegramLinkInfo {
  token: string;
  deep_link: string;
  bot_configured: boolean;
}

export function IntegrationsSettingsSectionContent() {
  const { t } = useTranslation('settings');
  const [status, setStatus] = useState<TelegramStatus | null>(null);
  const [linkInfo, setLinkInfo] = useState<TelegramLinkInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [unlinking, setUnlinking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const fetchStatus = useCallback(async () => {
    try {
      const statusResponse = await telegramApi.getStatus();
      setStatus(statusResponse);

      // If not linked and bot is configured, get link info
      if (!statusResponse.linked && statusResponse.bot_configured) {
        const linkResponse = await telegramApi.getLinkInfo();
        setLinkInfo(linkResponse);
      }
    } catch (err) {
      setError(t('integrations.telegram.loadError'));
      console.error('Failed to load Telegram status:', err);
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const handleUnlink = async () => {
    if (!status?.linked) return;

    setUnlinking(true);
    setError(null);

    try {
      await telegramApi.unlink();
      // Refresh status
      await fetchStatus();
    } catch (err) {
      setError(t('integrations.telegram.unlinkError'));
      console.error('Failed to unlink Telegram:', err);
    } finally {
      setUnlinking(false);
    }
  };

  const handleOpenTelegram = () => {
    if (linkInfo?.deep_link) {
      window.open(linkInfo.deep_link, '_blank');
    }
  };

  const handleCopyCommand = async () => {
    if (linkInfo?.token) {
      await navigator.clipboard.writeText(`/start ${linkInfo.token}`);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <span className="text-sm text-low">
          {t('integrations.loading')}
        </span>
      </div>
    );
  }

  return (
    <div className="space-y-6 pb-6">
      <SettingsCard
        title={t('integrations.telegram.title')}
        description={t('integrations.telegram.description')}
      >
        {error && (
          <div className="p-3 rounded-sm bg-error/10 border border-error/20 text-sm text-error mb-4">
            {error}
          </div>
        )}

        {!status?.bot_configured ? (
          // Bot not configured
          <div className="p-4 rounded-sm bg-secondary/50 border border-border">
            <p className="text-sm text-low">
              {t('integrations.telegram.notConfigured')}
            </p>
          </div>
        ) : status?.linked ? (
          // Linked state
          <div className="space-y-4">
            {/* Account info */}
            <div className="flex items-center justify-between p-4 rounded-sm bg-secondary/50 border border-border">
              <div className="flex items-center gap-3">
                <div className="p-2 rounded-sm bg-[#0088cc]/10">
                  <TelegramLogoIcon
                    className="size-icon-base text-[#0088cc]"
                    weight="fill"
                  />
                </div>
                <div>
                  <p className="text-sm font-medium text-high">
                    {status.username
                      ? `@${status.username}`
                      : t('integrations.telegram.linkedAccount')}
                  </p>
                  <p className="text-xs text-low">
                    {t('integrations.telegram.connected')}
                  </p>
                </div>
              </div>
              <PrimaryButton
                variant="tertiary"
                value={t('integrations.telegram.unlink')}
                onClick={handleUnlink}
                disabled={unlinking}
                actionIcon={unlinking ? 'spinner' : LinkBreakIcon}
              />
            </div>

            {/* Notification settings (read-only display) */}
            <div className="space-y-3">
              <p className="text-sm font-medium text-normal">
                {t('integrations.telegram.notificationSettings')}
              </p>

              <SettingsCheckbox
                id="telegram-notifications-enabled"
                label={t(
                  'integrations.telegram.notificationsEnabled.label'
                )}
                description={t(
                  'integrations.telegram.notificationsEnabled.description'
                )}
                checked={status.notifications_enabled}
                onChange={() => {}}
                disabled={true}
              />

              <SettingsCheckbox
                id="telegram-notify-on-task-done"
                label={t(
                  'integrations.telegram.notifyOnTaskDone.label'
                )}
                description={t(
                  'integrations.telegram.notifyOnTaskDone.description'
                )}
                checked={status.notify_on_task_done}
                onChange={() => {}}
                disabled={true}
              />

              <SettingsCheckbox
                id="telegram-include-llm-summary"
                label={t(
                  'integrations.telegram.includeLlmSummary.label'
                )}
                description={t(
                  'integrations.telegram.includeLlmSummary.description'
                )}
                checked={status.include_llm_summary}
                onChange={() => {}}
                disabled={true}
              />

              <p className="text-xs text-low mt-2">
                {t('integrations.telegram.settingsNote')}
              </p>
            </div>
          </div>
        ) : (
          // Unlinked state
          <div className="space-y-4">
            <div className="p-4 rounded-sm bg-secondary/50 border border-border">
              <div className="flex items-center gap-3 mb-3">
                <div className="p-2 rounded-sm bg-[#0088cc]/10">
                  <TelegramLogoIcon
                    className="size-icon-base text-[#0088cc]"
                    weight="fill"
                  />
                </div>
                <div>
                  <p className="text-sm font-medium text-high">
                    {t('integrations.telegram.notLinked')}
                  </p>
                  <p className="text-xs text-low">
                    {t('integrations.telegram.linkDescription')}
                  </p>
                </div>
              </div>

              <PrimaryButton
                value={t('integrations.telegram.linkButton')}
                onClick={handleOpenTelegram}
                disabled={!linkInfo?.deep_link}
                actionIcon={ArrowSquareUpRightIcon}
              />
            </div>

            {linkInfo?.deep_link && (
              <p className="text-xs text-low">
                {t('integrations.telegram.linkInstructions')}
              </p>
            )}

            {linkInfo?.token && (
              <div className="mt-4 pt-4 border-t border-border">
                <p className="text-xs text-low mb-2">
                  {t('integrations.telegram.manualLinkTitle')}
                </p>
                <ol className="text-xs text-low list-decimal list-inside space-y-1 mb-3">
                  <li>{t('integrations.telegram.manualStep1', { botUsername: 'kanban_vibe_bot' })}</li>
                  <li>{t('integrations.telegram.manualStep2')}</li>
                </ol>
                <div className="flex items-center gap-2 bg-secondary/50 rounded-sm p-2">
                  <code className="text-xs text-high flex-1 font-mono truncate">
                    /start {linkInfo.token}
                  </code>
                  <button
                    onClick={handleCopyCommand}
                    className="p-1 hover:bg-secondary rounded-sm transition-colors"
                    title={copied ? t('integrations.telegram.copied') : t('integrations.telegram.copyCommand')}
                  >
                    {copied ? (
                      <Check className="size-4 text-green-500" weight="bold" />
                    ) : (
                      <Copy className="size-4 text-low" />
                    )}
                  </button>
                </div>
                <p className="text-xs text-low mt-2">
                  {t('integrations.telegram.tokenExpiry')}
                </p>
              </div>
            )}
          </div>
        )}
      </SettingsCard>
    </div>
  );
}
