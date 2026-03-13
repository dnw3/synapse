import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Radio, Globe, Terminal, RefreshCw, ChevronDown, ChevronRight, Save, Wifi, WifiOff } from "lucide-react";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { ChannelEntry, McpEntry } from "../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  StatusDot,
  Toggle,
  useToast,
  ToastContainer,
} from "./shared";
import { cn } from "../../lib/cn";

interface LiveChannelEntry {
  name: string;
  enabled: boolean;
}

// ---------- Live Status Section ----------

function LiveStatusSection({ api }: { api: ReturnType<typeof import("../../hooks/useDashboardAPI").useDashboardAPI> }) {
  const { t } = useTranslation();
  const [liveChannels, setLiveChannels] = useState<LiveChannelEntry[] | null>(null);
  const [loadingLive, setLoadingLive] = useState(true);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchLiveStatus = useCallback(async () => {
    const resp = await api.debugInvoke({ method: "channels.status", params: {} });
    if (resp?.ok && Array.isArray(resp.result)) {
      setLiveChannels(resp.result as LiveChannelEntry[]);
    } else {
      setLiveChannels([]);
    }
    setLoadingLive(false);
  }, [api]);

  useEffect(() => {
    fetchLiveStatus();
    intervalRef.current = setInterval(fetchLiveStatus, 30_000);
    return () => {
      if (intervalRef.current !== null) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [fetchLiveStatus]);

  const online = liveChannels?.filter((c) => c.enabled) ?? [];

  return (
    <SectionCard className="lg:col-span-2">
      <SectionHeader
        icon={<Wifi className="h-4 w-4" />}
        title={t("channels.liveStatus")}
        right={
          online.length > 0 && (
            <span className="px-1.5 py-0.5 rounded-full bg-[var(--success)]/15 text-[var(--success)] text-[10px] font-mono tabular-nums border border-[var(--success)]/25">
              {online.length} {t("channels.registered")}
            </span>
          )
        }
      />
      {loadingLive ? (
        <div className="flex gap-2 flex-wrap px-0.5">
          {Array.from({ length: 4 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-7 w-24" />
          ))}
        </div>
      ) : !liveChannels || liveChannels.length === 0 ? (
        <EmptyState
          icon={<WifiOff className="h-5 w-5" />}
          message={t("channels.noChannels")}
        />
      ) : (
        <div className="flex flex-wrap gap-2">
          {liveChannels.map((ch) => (
            <div
              key={ch.name}
              className={cn(
                "flex items-center gap-2 px-3 py-1.5 rounded-[var(--radius-md)] border text-[12px] font-medium transition-all",
                ch.enabled
                  ? "bg-[var(--success)]/10 border-[var(--success)]/25 text-[var(--text-primary)]"
                  : "bg-[var(--bg-content)]/50 border-[var(--border-subtle)] text-[var(--text-tertiary)]"
              )}
            >
              <span
                className={cn(
                  "w-2 h-2 rounded-full flex-shrink-0",
                  ch.enabled ? "bg-[var(--success)]" : "bg-[var(--text-tertiary)]/40"
                )}
              />
              <span>{ch.name}</span>
              <span className="text-[10px] font-normal opacity-70">
                {ch.enabled ? t("channels.online") : t("channels.offline")}
              </span>
            </div>
          ))}
        </div>
      )}
    </SectionCard>
  );
}

function transportBadgeClass(transport: string): string {
  switch (transport.toLowerCase()) {
    case "stdio":
      return "bg-[var(--accent)]/15 text-[var(--accent)] border-[var(--accent)]/30";
    case "sse":
      return "bg-[var(--warning)]/15 text-[var(--warning)] border-[var(--warning)]/30";
    case "streamable-http":
      return "bg-[var(--success)]/15 text-[var(--success)] border-[var(--success)]/30";
    default:
      return "bg-[var(--bg-content)] text-[var(--text-secondary)] border-[var(--separator)]";
  }
}

// Per-channel config field definitions
const CHANNEL_CONFIG_FIELDS: Record<string, { key: string; label: string; placeholder: string; sensitive?: boolean; required?: boolean }[]> = {
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

// ---------- Channel Detail Panel ----------

function ChannelDetailPanel({
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

// ---------- Main Component ----------

export default function ChannelsPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();

  const [channels, setChannels] = useState<ChannelEntry[]>([]);
  const [mcpServers, setMcpServers] = useState<McpEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedChannel, setExpandedChannel] = useState<string | null>(null);
  const [savingChannel, setSavingChannel] = useState<string | null>(null);
  const [validationErrors, setValidationErrors] = useState<Set<string>>(new Set());

  const loadData = useCallback(async () => {
    const [ch, mcp] = await Promise.all([
      api.fetchChannels(),
      api.fetchMcp(),
    ]);
    if (ch) setChannels(ch);
    if (mcp) setMcpServers(mcp);
    setLoading(false);
  }, [api]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleToggleChannel = async (channel: ChannelEntry) => {
    // When enabling, validate required fields are filled
    if (!channel.enabled) {
      const fields = CHANNEL_CONFIG_FIELDS[channel.name];
      if (fields) {
        const missing = fields.filter(
          (f) => f.required && !(channel.config[f.key] ?? "").trim()
        );
        if (missing.length > 0) {
          setValidationErrors(new Set(missing.map((f) => f.key)));
          setExpandedChannel(channel.name);
          return;
        }
      }
    }

    // Optimistic update
    const prevEnabled = channel.enabled;
    setChannels((prev) =>
      prev.map((c) =>
        c.name === channel.name ? { ...c, enabled: !prevEnabled } : c
      )
    );

    const result = await api.toggleChannel(channel.name);
    if (result === null) {
      // Rollback
      setChannels((prev) =>
        prev.map((c) =>
          c.name === channel.name ? { ...c, enabled: prevEnabled } : c
        )
      );
      addToast(t("dashboard.channelToggleFailed", "Failed to toggle channel"), "error");
    } else {
      addToast(
        result.enabled
          ? t("dashboard.channelEnabled", "Channel enabled")
          : t("dashboard.channelDisabled", "Channel disabled"),
        "success"
      );
    }
  };

  const handleSaveConfig = async (name: string, config: Record<string, string>) => {
    setSavingChannel(name);
    const result = await api.updateChannelConfig(name, config);
    setSavingChannel(null);

    if (result?.ok) {
      // Update local state with new config
      setChannels((prev) =>
        prev.map((c) =>
          c.name === name ? { ...c, config } : c
        )
      );
      addToast(t("dashboard.channelConfigSaved", "Channel config saved"), "success");
    } else {
      addToast(t("dashboard.channelConfigFailed", "Failed to save channel config"), "error");
    }
  };

  const handleRefresh = () => {
    setLoading(true);
    loadData();
  };

  const toggleExpand = (name: string) => {
    setExpandedChannel((prev) => (prev === name ? null : name));
  };

  if (loading) {
    return (
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <div className="space-y-3">
          <LoadingSkeleton className="h-8 w-48" />
          {Array.from({ length: 3 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-16" />
          ))}
        </div>
        <div className="space-y-3">
          <LoadingSkeleton className="h-8 w-48" />
          {Array.from({ length: 3 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-16" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
      {/* Live Channel Status */}
      <LiveStatusSection api={api} />

      {/* Bot Channels */}
      <SectionCard>
        <SectionHeader
          icon={<Radio className="h-4 w-4" />}
          title={t("dashboard.botChannels", "Bot Channels")}
          right={
            <div className="flex items-center gap-2">
              <span className="px-1.5 py-0.5 rounded-full bg-[var(--bg-content)] text-[10px] font-mono text-[var(--text-tertiary)] tabular-nums">
                {channels.length}
              </span>
              <button
                onClick={handleRefresh}
                className="p-1 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
              >
                <RefreshCw className="h-3.5 w-3.5" />
              </button>
            </div>
          }
        />

        {channels.length === 0 ? (
          <EmptyState
            icon={<Radio className="h-5 w-5" />}
            message={t("dashboard.noChannels", "No bot channels configured")}
          />
        ) : (
          <div className="space-y-2">
            {channels.map((channel) => {
              const isExpanded = expandedChannel === channel.name;
              const hasConfig = channel.name in CHANNEL_CONFIG_FIELDS;

              return (
                <div
                  key={channel.name}
                  className={cn(
                    "rounded-[var(--radius-md)] border transition-all overflow-hidden",
                    channel.enabled
                      ? "bg-[var(--bg-content)]/60 border-[var(--border-subtle)] hover:border-[var(--separator)]"
                      : "bg-[var(--bg-content)]/30 border-[var(--border-subtle)]/50 opacity-80"
                  )}
                >
                  {/* Channel header row */}
                  <div className="flex items-center justify-between px-3.5 py-3">
                    <div
                      className="flex items-center gap-3 min-w-0 flex-1 cursor-pointer"
                      onClick={() => toggleExpand(channel.name)}
                    >
                      {hasConfig ? (
                        isExpanded ? (
                          <ChevronDown className="h-3.5 w-3.5 text-[var(--text-tertiary)] flex-shrink-0" />
                        ) : (
                          <ChevronRight className="h-3.5 w-3.5 text-[var(--text-tertiary)] flex-shrink-0" />
                        )
                      ) : (
                        <span className="w-3.5 flex-shrink-0" />
                      )}
                      <StatusDot status={channel.enabled ? "online" : "offline"} />
                      <div className="min-w-0">
                        <div className="text-[13px] font-medium text-[var(--text-primary)] truncate">
                          {channel.name}
                        </div>
                        <div className="text-[11px] text-[var(--text-secondary)] font-mono">
                          {channel.platform}
                          {Object.keys(channel.config).length > 0 && (
                            <span className="ml-2 text-[var(--accent)]">
                              {Object.keys(channel.config).length} fields
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                    <Toggle
                      checked={channel.enabled}
                      onChange={() => handleToggleChannel(channel)}
                      size="sm"
                    />
                  </div>

                  {/* Expandable detail section */}
                  {isExpanded && (
                    <div className="border-t border-[var(--border-subtle)]/50">
                      <ChannelDetailPanel
                        channel={channel}
                        onSave={handleSaveConfig}
                        saving={savingChannel === channel.name}
                        validationErrors={expandedChannel === channel.name ? validationErrors : undefined}
                        onClearValidation={(key) => setValidationErrors((prev) => {
                          const next = new Set(prev);
                          next.delete(key);
                          return next;
                        })}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </SectionCard>

      {/* MCP Servers */}
      <SectionCard>
        <SectionHeader
          icon={<Globe className="h-4 w-4" />}
          title={t("dashboard.mcpServers", "MCP Servers")}
          right={
            <span className="px-1.5 py-0.5 rounded-full bg-[var(--bg-content)] text-[10px] font-mono text-[var(--text-tertiary)] tabular-nums">
              {mcpServers.length}
            </span>
          }
        />

        {mcpServers.length === 0 ? (
          <EmptyState
            icon={<Terminal className="h-5 w-5" />}
            message={t("dashboard.noMcp", "No MCP servers configured")}
          />
        ) : (
          <div className="space-y-2">
            {mcpServers.map((mcp) => (
              <div
                key={mcp.name}
                className="px-3.5 py-3 rounded-[var(--radius-md)] bg-[var(--bg-content)]/60 border border-[var(--border-subtle)] hover:border-[var(--separator)] transition-all"
              >
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-[13px] font-medium text-[var(--text-primary)] truncate">
                    {mcp.name}
                  </span>
                  <span
                    className={cn(
                      "px-2 py-0.5 rounded-full text-[10px] font-medium border flex-shrink-0",
                      transportBadgeClass(mcp.transport)
                    )}
                  >
                    {mcp.transport}
                  </span>
                </div>
                {mcp.command && (
                  <div className="flex items-center gap-1.5 text-[11px] font-mono text-[var(--text-tertiary)] truncate">
                    <Terminal className="h-3 w-3 flex-shrink-0" />
                    <span className="truncate">{mcp.command}</span>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </SectionCard>

      <ToastContainer toasts={toasts} />
    </div>
  );
}
