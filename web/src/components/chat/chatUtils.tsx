import type { Message } from "../../types";

/** Get the earliest timestamp from a turn's messages */
export function turnTimestamp(messages: Message[]): number | undefined {
  for (const m of messages) {
    if (m.timestamp) return m.timestamp;
  }
  return undefined;
}

/** Format a timestamp for time separator display */
export function formatSeparatorTime(ms: number): string {
  const d = new Date(ms);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const isYesterday = d.toDateString() === yesterday.toDateString();

  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (isToday) return time;
  if (isYesterday) return `Yesterday ${time}`;
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })} ${time}`;
}

export const TIME_GAP_MS = 5 * 60 * 1000; // 5 minutes

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function formatRelativeTime(dateStr: string): string {
  try {
    // Handle both ISO strings and millisecond timestamps
    const ts = /^\d+$/.test(dateStr) ? Number(dateStr) : new Date(dateStr).getTime();
    if (isNaN(ts)) return "";
    const diff = Date.now() - ts;
    const mins = Math.floor(diff / 60_000);
    if (mins < 1) return "now";
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    return `${days}d`;
  } catch {
    return "";
  }
}

export function truncateLabel(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + "...";
}

export function exportToMarkdown(messages: Message[]): string {
  return messages
    .map((msg) => {
      if (msg.role === "human") return `## Human\n\n${msg.content}\n`;
      if (msg.role === "assistant") {
        let md = `## Assistant\n\n${msg.content}\n`;
        if (msg.tool_calls?.length) {
          for (const tc of msg.tool_calls) {
            md += `\n### Tool: ${tc.name}\n\`\`\`json\n${JSON.stringify(tc.arguments, null, 2)}\n\`\`\`\n`;
          }
        }
        return md;
      }
      if (msg.role === "tool") return `> **Tool Result:**\n> ${msg.content.slice(0, 500)}\n`;
      return "";
    })
    .join("\n");
}

export function MessageDivider({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 py-2">
      <div className="flex-1 h-px bg-[var(--separator)]" />
      <span className="text-[10px] text-[var(--text-tertiary)] font-medium uppercase tracking-wider">{label}</span>
      <div className="flex-1 h-px bg-[var(--separator)]" />
    </div>
  );
}
