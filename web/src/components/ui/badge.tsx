import { cn } from "../../lib/cn";

type BadgeVariant = "success" | "warning" | "error" | "info" | "accent" | "neutral";

interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  children: React.ReactNode;
}

const VARIANT_CLASSES: Record<BadgeVariant, string> = {
  success: "bg-[var(--success)]/12 text-[var(--success)]",
  warning: "bg-[var(--warning)]/12 text-[var(--warning)]",
  error: "bg-[var(--error)]/12 text-[var(--error)]",
  info: "bg-[var(--info)]/12 text-[var(--info)]",
  accent: "bg-[var(--accent)]/12 text-[var(--accent)]",
  neutral: "bg-[var(--bg-grouped)] text-[var(--text-secondary)]",
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
