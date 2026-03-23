import { useState } from "react";
import { Eye, EyeOff } from "lucide-react";
import { cn } from "../../../lib/cn";
import { formatArrayValue, unquote } from "./tomlParser";

export function BooleanField({ value, onChange }: { value: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!value)}
      className={cn(
        "relative inline-flex h-5 w-9 items-center rounded-full transition-colors cursor-pointer flex-shrink-0",
        value ? "bg-[var(--accent)]" : "bg-[var(--bg-content)] border border-[var(--border-subtle)]"
      )}
    >
      <span className={cn(
        "inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform shadow-sm",
        value ? "translate-x-[18px]" : "translate-x-[3px]"
      )} />
    </button>
  );
}

export function SecretField({ value, onChange, placeholder }: {
  value: string; onChange: (v: string) => void; placeholder?: string;
}) {
  const [revealed, setRevealed] = useState(false);
  return (
    <div className="relative">
      <input
        type={revealed ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full px-3 py-2 pr-9 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-all"
      />
      <button
        onClick={() => setRevealed(!revealed)}
        className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] cursor-pointer"
      >
        {revealed ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
      </button>
    </div>
  );
}

export function EnumField({ value, options, onChange }: {
  value: string; options: string[]; onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full px-3 py-2 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-all cursor-pointer"
    >
      <option value="">—</option>
      {options.map((o) => <option key={o} value={o}>{o}</option>)}
    </select>
  );
}

export function NumberField({ value, onChange, placeholder }: {
  value: string; onChange: (v: string) => void; placeholder?: string;
}) {
  return (
    <input
      type="number"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className="w-full px-3 py-2 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-all [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
    />
  );
}

export function TextField({ value, onChange, placeholder }: {
  value: string; onChange: (v: string) => void; placeholder?: string;
}) {
  return (
    <input
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className="w-full px-3 py-2 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-all"
    />
  );
}

/** Read-only display for complex values (arrays, etc.) */
export function ReadOnlyField({ value, label }: { value: string; label?: string }) {
  const displayVal = value.startsWith("[") ? formatArrayValue(value) : unquote(value);
  return (
    <div className="px-3 py-2 rounded-[var(--radius-lg)] bg-[var(--bg-content)]/60 border border-[var(--border-subtle)] border-dashed text-[13px] font-mono text-[var(--text-secondary)]">
      {displayVal}
      {label && <span className="ml-2 text-[10px] text-[var(--text-tertiary)]">({label})</span>}
    </div>
  );
}
