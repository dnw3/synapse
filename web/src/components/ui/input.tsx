import { Search } from "lucide-react";
import { forwardRef } from "react";
import { cn } from "../../lib/cn";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "search";
}

const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, variant = "default", ...props }, ref) => {
    if (variant === "search") {
      return (
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)] pointer-events-none" />
          <input
            ref={ref}
            className={cn(
              "w-full h-8 pl-8 pr-3 text-[13px] rounded-[var(--radius-md)] bg-[var(--bg-hover)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]",
              "border-none outline-none",
              "focus:ring-2 focus:ring-[var(--accent)] focus:ring-offset-0 focus:shadow-[0_0_0_3px_var(--accent-glow)]",
              "transition-shadow duration-150",
              className
            )}
            {...props}
          />
        </div>
      );
    }

    return (
      <input
        ref={ref}
        className={cn(
          "w-full h-8 px-3 text-[13px] rounded-[var(--radius-md)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]",
          "bg-[var(--bg-content)]",
          "border border-[var(--border-subtle)]",
          "outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)] focus:ring-offset-0 focus:shadow-[0_0_0_3px_var(--accent-glow)]",
          "transition-all duration-150",
          className
        )}
        {...props}
      />
    );
  }
);
Input.displayName = "Input";

export { Input };
export type { InputProps };
