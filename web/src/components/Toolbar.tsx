import type { ReactNode } from "react";
import { Menu } from "lucide-react";
import { Badge } from "./ui/badge";
import { cn } from "../lib/cn";

interface ToolbarProps {
  title: string;
  subtitle?: string;
  modelBadge?: string;
  connected?: boolean;
  status?: string;
  actions?: ReactNode;
  /** Optional agent selector rendered before model badge */
  agentSelector?: ReactNode;
  onMenuClick?: () => void;
  showMenu?: boolean;
}

export default function Toolbar({
  title,
  subtitle,
  modelBadge,
  connected,
  status,
  actions,
  agentSelector,
  onMenuClick,
  showMenu,
}: ToolbarProps) {
  return (
    <div className="flex items-center justify-between h-[44px] px-4 flex-shrink-0 bg-[var(--bg-window)]/80 backdrop-blur-[20px] border-b border-[var(--separator)]">
      {/* Left: hamburger + title + subtitle */}
      <div className="flex items-center gap-2 min-w-0">
        {showMenu && (
          <button
            onClick={onMenuClick}
            className="md:hidden p-1.5 text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)]"
          >
            <Menu className="h-4 w-4" />
          </button>
        )}
        <h1 className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
          {title}
        </h1>
        {subtitle && (
          <Badge variant="neutral" className="flex-shrink-0">
            {subtitle}
          </Badge>
        )}
      </div>

      {/* Right: agent selector + model badge + connection dot + status badge + actions */}
      <div className="flex items-center gap-2 flex-shrink-0">
        {agentSelector}
        {modelBadge && (
          <span className="hidden sm:inline-flex items-center px-2 py-0.5 text-[11px] font-mono bg-[var(--bg-grouped)] text-[var(--text-secondary)] rounded-[var(--radius-sm)]">
            {modelBadge}
          </span>
        )}
        {connected !== undefined && (
          <span
            className={cn(
              "w-2 h-2 rounded-full flex-shrink-0",
              connected
                ? "bg-[var(--success)] shadow-[0_0_6px_var(--success)]"
                : "bg-[var(--text-tertiary)]"
            )}
          />
        )}
        {status && (
          <Badge variant="accent" className="animate-pulse">
            {status}
          </Badge>
        )}
        {actions}
      </div>
    </div>
  );
}
