import { forwardRef } from "react";
import { cn } from "../../lib/cn";

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  hoverable?: boolean;
  padding?: "compact" | "default" | "spacious";
}

const PADDING_MAP = {
  compact: "p-[14px]",
  default: "p-[16px]",
  spacious: "p-[20px]",
} as const;

const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, hoverable = false, padding = "default", ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)]",
        "[data-theme='light']_&:shadow-[var(--shadow-sm)]",
        PADDING_MAP[padding],
        hoverable &&
          "transition-all duration-200 ease-[cubic-bezier(0.2,0.8,0.2,1)] hover:-translate-y-[2px] hover:shadow-[var(--shadow-md)] cursor-pointer",
        className
      )}
      {...props}
    />
  )
);
Card.displayName = "Card";

export { Card };
export type { CardProps };
