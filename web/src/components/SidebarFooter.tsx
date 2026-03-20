import { useTranslation } from "react-i18next";
import { Sun, Moon, Monitor, Globe, LayoutDashboard, MessageSquare } from "lucide-react";
import type { IdentityInfo } from "../types/dashboard";

const MODE_ICONS = { light: Sun, dark: Moon, system: Monitor } as const;

interface SidebarFooterProps {
  identity: IdentityInfo | null;
  themeMode: string;
  onCycleTheme: () => void;
  onToggleLanguage: () => void;
  /** WebSocket connection status — shows as a status dot next to the brand name. */
  connected?: boolean;
  /** @deprecated Mode switch is no longer needed with unified sidebar */
  sidebarMode?: "chat" | "dashboard";
  /** @deprecated Mode switch is no longer needed with unified sidebar */
  onSwitchMode?: () => void;
}

export default function SidebarFooter({
  identity,
  themeMode,
  onCycleTheme,
  onToggleLanguage,
  connected,
  sidebarMode,
  onSwitchMode,
}: SidebarFooterProps) {
  const { t } = useTranslation();
  const ModeIcon = MODE_ICONS[themeMode as keyof typeof MODE_ICONS] ?? Monitor;
  const SwitchIcon = sidebarMode === "chat" ? LayoutDashboard : MessageSquare;
  const switchTitle = sidebarMode === "chat" ? t("sidebar.switchToDashboard") : t("sidebar.switchToChat");

  return (
    <div className="flex-shrink-0 border-t border-[var(--separator)] px-2.5 py-2.5 flex items-center justify-between">
      {/* Left: brand */}
      <div className="flex items-center gap-2 min-w-0">
        {identity?.avatar_url ? (
          <img src={identity.avatar_url} alt="" className="w-6 h-6 rounded-full object-cover" />
        ) : (
          <div className="w-6 h-6 rounded-full bg-gradient-to-br from-[var(--accent)] to-[var(--accent-gradient-end)] flex items-center justify-center text-white text-[10px] font-semibold flex-shrink-0">
            {identity?.emoji || "S"}
          </div>
        )}
        <span className="text-[12px] font-medium text-[var(--text-primary)] truncate">
          {identity?.name || t("sidebar.brand")}
        </span>
        {/* Connection status dot */}
        {connected !== undefined && (
          <span
            className={`w-2 h-2 rounded-full flex-shrink-0 transition-colors ${
              connected
                ? "bg-[var(--status-ok,#34c759)]"
                : "bg-[var(--status-danger,#ff3b30)]"
            }`}
            style={{
              boxShadow: connected
                ? "0 0 0 3px color-mix(in srgb, var(--status-ok, #34c759) 14%, transparent)"
                : "0 0 0 3px color-mix(in srgb, var(--status-danger, #ff3b30) 14%, transparent)",
            }}
            role="img"
            aria-live="polite"
            aria-label={connected ? t("status.connected") : t("status.disconnected")}
            title={connected ? t("status.connected") : t("status.disconnected")}
          />
        )}
      </div>
      {/* Right: mode switch (legacy) + theme + language */}
      <div className="flex items-center gap-0.5 flex-shrink-0">
        {onSwitchMode && (
          <button
            onClick={onSwitchMode}
            className="p-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
            title={switchTitle}
          >
            <SwitchIcon className="h-3.5 w-3.5" />
          </button>
        )}
        <button
          onClick={onCycleTheme}
          className="p-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
          title={t("settings.theme")}
        >
          <ModeIcon className="h-3.5 w-3.5" />
        </button>
        <button
          onClick={onToggleLanguage}
          className="p-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
          title={t("settings.language")}
        >
          <Globe className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
