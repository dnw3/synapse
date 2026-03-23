import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Save, Wifi, WifiOff } from "lucide-react";
import type { ChannelEntry } from "../../../types/dashboard";
import { cn } from "../../../lib/cn";

// Per-channel config field definitions
export const CHANNEL_CONFIG_FIELDS: Record<string, { key: string; label: string; placeholder: string; sensitive?: boolean; required?: boolean }[]> = {
  telegram: [
    { key: "bot_token", label: "Bot Token", placeholder: "123456:ABC-DEF...", sensitive: true, required: true },
    { key: "allowed_users", label: "Allowed Users", placeholder: "user1,user2 (comma-separated)" },
    { key: "webhook_url", label: "Webhook URL", placeholder: "https://example.com/webhook (optional)" },
  ],
  discord: [
    { key: "bot_token", label: "Bot Token", placeholder: "Bot token from Discord Developer Portal", sensitive: true, required: true },
    { key: "allowed_guilds", label: "Allowed Guilds", placeholder: "guild_id1,guild_id2" },
    { key: "allowed_channels", label: "Allowed Channels", placeholder: "channel_id1,channel_id2" },
  ],
  slack: [
    { key: "bot_token", label: "Bot Token", placeholder: "xoxb-...", sensitive: true, required: true },
    { key: "app_token", label: "App Token", placeholder: "xapp-...", sensitive: true, required: true },
    { key: "signing_secret", label: "Signing Secret", placeholder: "Signing secret from Slack app settings", sensitive: true },
    { key: "allowed_channels", label: "Allowed Channels", placeholder: "channel1,channel2" },
  ],
  lark: [
    { key: "app_id", label: "App ID", placeholder: "cli_...", required: true },
    { key: "app_secret", label: "App Secret", placeholder: "App secret from Lark console", sensitive: true, required: true },
    { key: "verification_token", label: "Verification Token", placeholder: "Verification token", sensitive: true },
    { key: "encrypt_key", label: "Encrypt Key", placeholder: "Encrypt key (optional)", sensitive: true },
  ],
  dingtalk: [
    { key: "app_key", label: "App Key", placeholder: "App key from DingTalk console", required: true },
    { key: "app_secret", label: "App Secret", placeholder: "App secret", sensitive: true, required: true },
    { key: "robot_code", label: "Robot Code", placeholder: "Robot code", required: true },
    { key: "webhook_url", label: "Webhook URL", placeholder: "https://oapi.dingtalk.com/..." },
  ],
  mattermost: [
    { key: "url", label: "Server URL", placeholder: "https://mattermost.example.com", required: true },
    { key: "token", label: "Bot Token", placeholder: "Bot access token", sensitive: true, required: true },
    { key: "team_id", label: "Team ID", placeholder: "Team ID" },
    { key: "allowed_channels", label: "Allowed Channels", placeholder: "channel1,channel2" },
  ],
  whatsapp: [
    { key: "phone_number_id", label: "Phone Number ID", placeholder: "Phone number ID from Meta", required: true },
    { key: "access_token", label: "Access Token", placeholder: "Access token", sensitive: true, required: true },
    { key: "verify_token", label: "Verify Token", placeholder: "Webhook verify token", sensitive: true },
    { key: "webhook_url", label: "Webhook URL", placeholder: "https://example.com/webhook" },
  ],
  webchat: [
    { key: "enabled", label: "Enabled", placeholder: "true / false" },
  ],
};

export function ChannelDetailPanel({
  channel,
  onSave,
  saving,
  validationErrors,
  onClearValidation,
}: {
  channel: ChannelEntry;
  onSave: (name: string, config: Record<string, string>) => void;
  saving: boolean;
  validationErrors?: Set<string>;
  onClearValidation?: (key: string) => void;
}) {
  const { t } = useTranslation();
  const fields = CHANNEL_CONFIG_FIELDS[channel.name];
  const [formValues, setFormValues] = useState<Record<string, string>>({});
  const [revealedFields, setRevealedFields] = useState<Set<string>>(new Set());

  // Initialize form values from channel config
  useEffect(() => {
    const initial: Record<string, string> = {};
    if (fields) {
      for (const f of fields) {
        initial[f.key] = channel.config[f.key] ?? "";
      }
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setFormValues(initial);
    setRevealedFields(new Set());
  }, [channel.name, channel.config, fields]);

  const handleFieldChange = (key: string, value: string) => {
    setFormValues((prev) => ({ ...prev, [key]: value }));
    if (validationErrors?.has(key)) onClearValidation?.(key);
  };

  const toggleReveal = (key: string) => {
    setRevealedFields((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const hasChanges = fields
    ? fields.some((f) => (formValues[f.key] ?? "") !== (channel.config[f.key] ?? ""))
    : false;

  if (!fields) {
    return (
      <div className="px-3.5 py-3 text-[12px] text-[var(--text-tertiary)] italic">
        {t("dashboard.channelConfigToml", "Configuration managed via synapse.toml")}
      </div>
    );
  }

  return (
    <div className="px-3.5 pb-3 space-y-3">
      {/* Status indicators */}
      <div className="flex items-center gap-4 text-[11px]">
        <div className="flex items-center gap-1.5">
          <span className={cn(
            "w-1.5 h-1.5 rounded-full",
            channel.enabled ? "bg-[var(--success)]" : "bg-[var(--error)]"
          )} />
          <span className="text-[var(--text-tertiary)]">
            {channel.enabled
              ? t("dashboard.channelRunning", "Running")
              : t("dashboard.channelStopped", "Stopped")}
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className={cn(
            "w-1.5 h-1.5 rounded-full",
            Object.keys(channel.config).length > 0 ? "bg-[var(--accent)]" : "bg-[var(--text-tertiary)]/40"
          )} />
          <span className="text-[var(--text-tertiary)]">
            {Object.keys(channel.config).length > 0
              ? t("dashboard.channelConfigured", "Configured")
              : t("dashboard.channelNotConfigured", "Not configured")}
          </span>
        </div>
        {/* Reconnect button (visual) */}
        {channel.enabled && (
          <button className="flex items-center gap-1 text-[var(--accent)] hover:text-[var(--accent-light)] transition-colors cursor-pointer ml-auto">
            <Wifi className="h-3 w-3" />
            <span>{t("dashboard.reconnect", "Reconnect")}</span>
          </button>
        )}
      </div>

      {/* Config fields */}
      <div className="space-y-2">
        {fields.map((field) => (
          <div key={field.key} className="space-y-1">
            <label className="text-[11px] font-medium text-[var(--text-secondary)] uppercase tracking-[0.05em]">
              {field.label}{field.required && <span className="text-[var(--error)] ml-0.5">*</span>}
            </label>
            <div className="flex items-center gap-1.5">
              <input
                type={field.sensitive && !revealedFields.has(field.key) ? "password" : "text"}
                value={formValues[field.key] ?? ""}
                onChange={(e) => handleFieldChange(field.key, e.target.value)}
                placeholder={field.placeholder}
                className={cn(
                  "flex-1 px-2.5 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-mono",
                  "bg-[var(--bg-window)] border",
                  "text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]/50",
                  "focus:outline-none focus:border-[var(--accent)]/50 focus:ring-1 focus:ring-[var(--accent)]/20",
                  "transition-colors",
                  validationErrors?.has(field.key)
                    ? "border-[var(--error)] ring-1 ring-[var(--error)]/20"
                    : "border-[var(--border-subtle)]"
                )}
              />
              {field.sensitive && (formValues[field.key] ?? "").length > 0 && (
                <button
                  onClick={() => toggleReveal(field.key)}
                  className="px-1.5 py-1.5 rounded-[var(--radius-sm)] text-[10px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  title={revealedFields.has(field.key) ? "Hide" : "Show"}
                >
                  {revealedFields.has(field.key) ? (
                    <WifiOff className="h-3 w-3" />
                  ) : (
                    <Wifi className="h-3 w-3" />
                  )}
                </button>
              )}
            </div>
            {validationErrors?.has(field.key) && (
              <span className="text-[10px] text-[var(--error)]">
                {t("dashboard.fieldRequired", "This field is required")}
              </span>
            )}
          </div>
        ))}
      </div>

      {/* Save button */}
      <div className="flex justify-end pt-1">
        <button
          onClick={() => onSave(channel.name, formValues)}
          disabled={saving || !hasChanges}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium transition-all cursor-pointer",
            hasChanges
              ? "bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]"
              : "bg-[var(--bg-content)] text-[var(--text-tertiary)] cursor-not-allowed"
          )}
        >
          <Save className="h-3 w-3" />
          {saving
            ? t("dashboard.saving", "Saving...")
            : t("dashboard.saveConfig", "Save Config")}
        </button>
      </div>
    </div>
  );
}
