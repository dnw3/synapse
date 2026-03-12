import { cn } from "../../lib/cn";

interface SegmentedControlProps {
  items: { label: string; value: string }[];
  value: string;
  onChange: (value: string) => void;
  className?: string;
}

function SegmentedControl({ items, value, onChange, className }: SegmentedControlProps) {
  return (
    <div
      className={cn(
        "inline-flex rounded-[var(--radius-md)] bg-[var(--bg-grouped)] p-[2px]",
        className
      )}
    >
      {items.map((item) => (
        <button
          key={item.value}
          type="button"
          onClick={() => onChange(item.value)}
          className={cn(
            "relative px-3 py-1.5 text-[13px] font-medium rounded-[var(--radius-sm)] transition-all duration-150 cursor-pointer",
            value === item.value
              ? "bg-white text-[var(--text-primary)] shadow-[var(--shadow-sm)] dark:bg-[#636366] dark:text-[var(--text-primary)]"
              : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
          )}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

export { SegmentedControl };
export type { SegmentedControlProps };
