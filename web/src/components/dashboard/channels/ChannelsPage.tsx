import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Radio, RefreshCw, ChevronDown, ChevronRight } from "lucide-react";
import { useChannels, useToggleChannel, useUpdateChannelConfig } from "../../../hooks/queries/useChannelsQueries";
import { useBindings } from "../../../hooks/queries/useAgentsQueries";
import type { ChannelEntry, BindingEntry } from "../../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  StatusDot,
  Toggle,
} from "../shared";
import { useToast } from "../../ui/toast";
import { cn } from "../../../lib/cn";
import { LiveStatusSection } from "./ChannelLiveStatus";
import { ChannelDetailPanel, CHANNEL_CONFIG_FIELDS } from "./ChannelDetail";
import { DmPairingSection } from "./DmPairingManager";

export default function ChannelsPage() {
  const { t } = useTranslation();
  const { toast } = useToast();

  const channelsQ = useChannels();
  const bindingsQ = useBindings();
  const toggleMut = useToggleChannel();
  const updateConfigMut = useUpdateChannelConfig();

  const channels = channelsQ.data ?? [];
  const bindings: BindingEntry[] = bindingsQ.data ?? [];
  const loading = channelsQ.isPending;

  const [expandedChannel, setExpandedChannel] = useState<string | null>(null);
  const [savingChannel, setSavingChannel] = useState<string | null>(null);
  const [validationErrors, setValidationErrors] = useState<Set<string>>(new Set());

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

    toggleMut.mutate(channel.name, {
      onError: () => {
        toast({ variant: "error", title: t("dashboard.channelToggleFailed", "Failed to toggle channel") });
      },
    });
  };

  const handleSaveConfig = async (name: string, config: Record<string, string>) => {
    setSavingChannel(name);
    try {
      await updateConfigMut.mutateAsync({ name, config });
      toast({ variant: "success", title: t("dashboard.channelConfigSaved", "Channel config saved") });
    } catch {
      toast({ variant: "error", title: t("dashboard.channelConfigFailed", "Failed to save channel config") });
    }
    setSavingChannel(null);
  };

  const handleRefresh = () => {
    channelsQ.refetch();
    bindingsQ.refetch();
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
      <LiveStatusSection />

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
              const channelBindings = bindings.filter((b) => b.channel === channel.name);

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
                        {channelBindings.length > 0 && (
                          <div className="flex gap-1 flex-wrap">
                            {channelBindings.map((b, i) => (
                              <span key={i} className="px-1.5 py-0.5 rounded bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium border border-[var(--accent)]/20">
                                {t("channels.boundTo")} {b.agent}{b.account_id ? ` (${b.account_id})` : ""}
                              </span>
                            ))}
                          </div>
                        )}
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

      {/* DM Pairing */}
      <DmPairingSection />

    </div>
  );
}
