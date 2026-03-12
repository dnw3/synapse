import { cn } from "../../lib/cn";

interface ToggleProps {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  className?: string;
}

function Toggle({ checked, onChange, disabled = false, className }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-[26px] w-[42px] shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-[cubic-bezier(0.2,0.8,0.2,1)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] focus-visible:ring-offset-2",
        checked ? "bg-[var(--success)]" : "bg-[var(--bg-grouped)]",
        disabled && "opacity-50 cursor-not-allowed",
        className
      )}
    >
      <span
        className={cn(
          "pointer-events-none inline-block h-[22px] w-[22px] rounded-full bg-white shadow-sm transition-transform duration-200 ease-[cubic-bezier(0.2,0.8,0.2,1)]",
          checked ? "translate-x-[18px]" : "translate-x-[2px]",
          "mt-[2px]"
        )}
      />
    </button>
  );
}

export { Toggle };
export type { ToggleProps };
