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
