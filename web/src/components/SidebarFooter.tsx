import { useTranslation } from "react-i18next";
import { Sun, Moon, Monitor, Globe } from "lucide-react";

const MODE_ICONS = { light: Sun, dark: Moon, system: Monitor } as const;

interface SidebarFooterProps {
  themeMode: string;
  onCycleTheme: () => void;
  onToggleLanguage: () => void;
  // Legacy props — kept for backward compat, ignored
  identity?: unknown;
  connected?: boolean;
  sidebarMode?: string;
  onSwitchMode?: () => void;
}

export default function SidebarFooter({
  themeMode,
  onCycleTheme,
  onToggleLanguage,
}: SidebarFooterProps) {
  const { t } = useTranslation();
  const ModeIcon = MODE_ICONS[themeMode as keyof typeof MODE_ICONS] ?? Monitor;

  return (
    <div className="flex-shrink-0 border-t border-[var(--separator)] px-3 py-2 flex items-center justify-between">
      {/* Brand credit */}
      <span className="text-[10px] text-[var(--text-tertiary)] truncate">
        {t("dashboard.poweredBy")}
      </span>
      {/* Utility buttons */}
      <div className="flex items-center gap-0.5 flex-shrink-0">
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
