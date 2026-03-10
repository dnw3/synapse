import { useTranslation } from "react-i18next";
import { cn } from "../lib/cn";

interface Props {
  conversationId: string | null;
  messageCount: number;
  status: string;
  connected: boolean;
}

export default function StatusBar({
  conversationId,
  messageCount,
  status,
  connected,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center justify-between px-3 sm:px-4 py-1 h-7 text-[11px] border-t border-[var(--border-subtle)] bg-[var(--bg-elevated)]/80 text-[var(--text-tertiary)] font-mono">
      <div className="flex items-center gap-3 sm:gap-4 truncate">
        <span className="hidden sm:inline">
          {t("status.session")}: {conversationId ? conversationId.slice(0, 8) : t("status.none")}
        </span>
        <span>{t("status.messages")}: {messageCount}</span>
      </div>
      <div className="flex items-center gap-3 sm:gap-4">
        <span>
          {t("status.status")}: {t(`status.${status}`, { defaultValue: status })}
        </span>
        <span className="flex items-center gap-1.5">
          <span
            className={cn(
              "w-1.5 h-1.5 rounded-full flex-shrink-0",
              connected ? "bg-[var(--success)] shadow-[0_0_6px_var(--success)]" : "bg-[var(--error)]"
            )}
          />
          <span className="hidden sm:inline">
            {connected ? t("status.wsConnected") : t("status.wsDisconnected")}
          </span>
        </span>
      </div>
    </div>
  );
}
