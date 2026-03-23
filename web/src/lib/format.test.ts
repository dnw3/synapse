import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  formatTokens,
  formatCost,
  formatDate,
  formatUptime,
  formatDuration,
  formatRelativeTime,
  formatBytes,
} from "./format";

describe("formatTokens", () => {
  it("formats millions", () => {
    expect(formatTokens(1_500_000)).toBe("1.5M");
  });

  it("formats exactly 1M", () => {
    expect(formatTokens(1_000_000)).toBe("1.0M");
  });

  it("formats thousands", () => {
    expect(formatTokens(2_500)).toBe("2.5K");
  });

  it("formats exactly 1K", () => {
    expect(formatTokens(1_000)).toBe("1.0K");
  });

  it("formats small numbers as-is", () => {
    expect(formatTokens(42)).toBe("42");
    expect(formatTokens(0)).toBe("0");
    expect(formatTokens(999)).toBe("999");
  });
});

describe("formatCost", () => {
  it("formats with 2 decimals and dollar sign", () => {
    expect(formatCost(3.14159)).toBe("$3.14");
  });

  it("formats zero", () => {
    expect(formatCost(0)).toBe("$0.00");
  });

  it("pads to 2 decimals", () => {
    expect(formatCost(5)).toBe("$5.00");
  });
});

describe("formatDate", () => {
  it("formats ISO string in en-US", () => {
    const result = formatDate("2025-06-15T14:30:00Z");
    // Should contain month and day
    expect(result).toContain("Jun");
    expect(result).toContain("15");
  });

  it("formats numeric timestamp string", () => {
    const ts = String(new Date("2025-01-01T00:00:00Z").getTime());
    const result = formatDate(ts);
    expect(result).toContain("Jan");
    expect(result).toContain("1");
  });

  it("returns original string for invalid date", () => {
    expect(formatDate("not-a-date")).toBe("not-a-date");
  });

  it("uses zh-CN locale when locale starts with zh", () => {
    const result = formatDate("2025-06-15T14:30:00Z", "zh-CN");
    // zh-CN uses Chinese month format like "6月"
    expect(result).toContain("6");
    expect(result).toContain("15");
  });
});

describe("formatUptime", () => {
  it("formats seconds only", () => {
    expect(formatUptime(45)).toBe("00:00:45");
  });

  it("formats hours and minutes", () => {
    // 2h 30m 15s = 9015s
    expect(formatUptime(9015)).toBe("02:30:15");
  });

  it("formats with days", () => {
    // 1d 2h 3m 4s = 86400 + 7200 + 180 + 4 = 93784
    expect(formatUptime(93784)).toBe("1d 02:03:04");
  });

  it("formats zero", () => {
    expect(formatUptime(0)).toBe("00:00:00");
  });
});

describe("formatDuration", () => {
  it("formats milliseconds", () => {
    expect(formatDuration(500)).toBe("500ms");
    expect(formatDuration(0)).toBe("0ms");
    expect(formatDuration(999)).toBe("999ms");
  });

  it("formats seconds", () => {
    expect(formatDuration(1000)).toBe("1.0s");
    expect(formatDuration(1500)).toBe("1.5s");
    expect(formatDuration(60000)).toBe("60.0s");
  });
});

describe("formatRelativeTime", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2025-06-15T12:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("formats seconds ago", () => {
    const result = formatRelativeTime("2025-06-15T11:59:30Z");
    // 30 seconds ago
    expect(result).toContain("30");
    expect(result).toContain("second");
  });

  it("formats minutes ago", () => {
    const result = formatRelativeTime("2025-06-15T11:55:00Z");
    // 5 minutes ago
    expect(result).toContain("5");
    expect(result).toContain("minute");
  });

  it("formats hours ago", () => {
    const result = formatRelativeTime("2025-06-15T09:00:00Z");
    // 3 hours ago
    expect(result).toContain("3");
    expect(result).toContain("hour");
  });

  it("formats days ago", () => {
    const result = formatRelativeTime("2025-06-13T12:00:00Z");
    // 2 days ago
    expect(result).toContain("2");
    expect(result).toContain("day");
  });
});

describe("formatBytes", () => {
  it("formats bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("formats kilobytes", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it("formats megabytes", () => {
    expect(formatBytes(1048576)).toBe("1.0 MB");
    expect(formatBytes(1572864)).toBe("1.5 MB");
  });
});
