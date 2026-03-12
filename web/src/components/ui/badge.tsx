import { cn } from "../../lib/cn";

type BadgeVariant = "success" | "warning" | "error" | "info" | "accent" | "neutral";

interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  children: React.ReactNode;
}

const VARIANT_CLASSES: Record<BadgeVariant, string> = {
  success:
    "bg-[var(--success)]/10 text-[var(--success)] dark:bg-[var(--success)]/15",
  warning:
    "bg-[var(--warning)]/10 text-[var(--warning)] dark:bg-[var(--warning)]/15",
  error:
    "bg-[var(--error)]/10 text-[var(--error)] dark:bg-[var(--error)]/15",
  info:
    "bg-[var(--info)]/10 text-[var(--info)] dark:bg-[var(--info)]/15",
  accent:
    "bg-[var(--accent)]/10 text-[var(--accent)] dark:bg-[var(--accent)]/15",
  neutral:
    "bg-[var(--bg-grouped)] text-[var(--text-secondary)]",
};

function Badge({ variant = "neutral", className, children, ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-[10px] py-[3px] text-[11px] font-medium leading-tight",
        VARIANT_CLASSES[variant],
        className
      )}
      {...props}
    >
      {children}
    </span>
  );
}

export { Badge };
export type { BadgeProps, BadgeVariant };
