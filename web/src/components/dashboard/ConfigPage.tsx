import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Settings2, Save, Check, X, RefreshCw, Code, FormInput, Search,
  Brain, Bot, Wrench, Database, FileText, History, Globe, Shield,
  FolderOpen, Users, Gauge, Lock, HeartPulse, Sparkles, ScrollText,
  Folder, Eye, EyeOff, AlertTriangle,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import { SectionCard, EmptyState, LoadingSpinner, useToast, ToastContainer } from "./shared";

type EditorMode = "form" | "raw";

// Schema types matching backend
interface ConfigFieldSchema {
  key: string;
  label: string;
  type: "string" | "number" | "boolean" | "enum" | "array" | "secret";
  description?: string;
  placeholder?: string;
  options?: string[];
  default_value?: string;
  sensitive: boolean;
}

interface ConfigSectionSchema {
  key: string;
  label: string;
  description?: string;
  order: number;
  icon: string;
  fields: ConfigFieldSchema[];
}

interface ConfigSchemaData {
  sections: ConfigSectionSchema[];
  sensitive_patterns: string[];
}

// TOML parsing types
interface TomlField {
  key: string;
  value: string;
  line: number;
  // For multi-line values, endLine tracks the last line
  endLine: number;
}

interface TomlSection {
  key: string;
  fields: TomlField[];
  isArrayTable?: boolean;
}

// --- TOML helpers ---

/** Parse TOML into sections. Handles multi-line arrays/strings. */
function parseTomlSections(content: string): TomlSection[] {
  const lines = content.split("\n");
  const sections: TomlSection[] = [];
  let current: TomlSection | null = null;
  let multiLineKey: string | null = null;
  let multiLineValue = "";
  let multiLineStart = 0;
  let bracketDepth = 0;

  function pushField(section: TomlSection, key: string, value: string, startLine: number, endLine: number) {
    section.fields.push({ key, value: value.trim(), line: startLine, endLine });
  }

  function ensureSection(): TomlSection {
    if (current) return current;
    if (!sections.length || sections[0].key !== "(root)") {
      const root: TomlSection = { key: "(root)", fields: [] };
      sections.unshift(root);
      current = root;
    } else {
      current = sections[0];
    }
    return current;
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();

    // If we're accumulating a multi-line value
    if (multiLineKey !== null) {
      multiLineValue += "\n" + lines[i];
      // Count brackets
      for (const ch of line) {
        if (ch === "[") bracketDepth++;
        if (ch === "]") bracketDepth--;
      }
      if (bracketDepth <= 0) {
        // Multi-line value complete
        const section = ensureSection();
        pushField(section, multiLineKey, multiLineValue.trim(), multiLineStart, i);
        multiLineKey = null;
        multiLineValue = "";
        bracketDepth = 0;
      }
      continue;
    }

    // Section header: [[array_table]] (TOML array tables)
    const arrayTableMatch = line.match(/^\[\[([^[\]]+)\]\]$/);
    if (arrayTableMatch) {
      current = { key: arrayTableMatch[1], fields: [], isArrayTable: true };
      sections.push(current);
      continue;
    }

    // Section header: [name] or [name.sub]
    const sectionMatch = line.match(/^\[([^[\]]+)\]$/);
    if (sectionMatch) {
      current = { key: sectionMatch[1], fields: [] };
      sections.push(current);
      continue;
    }

    // Skip blanks and comments
    if (!line || line.startsWith("#")) continue;

    // Key-value pair
    const kvMatch = line.match(/^([^=]+?)\s*=\s*(.*)$/);
    if (kvMatch) {
      const key = kvMatch[1].trim();
      const value = kvMatch[2].trim();

      // Check if value starts a multi-line array
      if (value.startsWith("[") && !value.endsWith("]")) {
        multiLineKey = key;
        multiLineValue = value;
        multiLineStart = i;
        bracketDepth = 0;
        for (const ch of value) {
          if (ch === "[") bracketDepth++;
          if (ch === "]") bracketDepth--;
        }
        const section = ensureSection();
        // Don't push yet — wait for closing bracket
        void section;
        continue;
      }

      // Check for multi-line basic string (triple quotes)
      if (value === '"""' || value.startsWith('"""')) {
        if (value !== '"""' && value.endsWith('"""') && value.length > 3) {
          // Single-line triple-quoted
          const section = ensureSection();
          pushField(section, key, value, i, i);
        } else {
          void key; // multiLineKey will be used when multi-line string support is completed
          multiLineValue = value;
          multiLineStart = i;
          // We'll look for closing """ later
          // For simplicity, just treat as single line
          const section = ensureSection();
          pushField(section, key, value, i, i);
          multiLineKey = null;
        }
        continue;
      }

      const section = ensureSection();
      pushField(section, key, value, i, i);
    }
  }

  // If we still have an unclosed multi-line value, push it as-is
  if (multiLineKey !== null) {
    const section = ensureSection();
    pushField(section, multiLineKey, multiLineValue.trim(), multiLineStart, lines.length - 1);
  }

  return sections;
}

function unquote(v: string): string {
  if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
    return v.slice(1, -1);
  }
  return v;
}

/** Format a TOML array value for display. */
function formatArrayValue(v: string): string {
  // Try to extract array items for readable display
  const match = v.match(/^\[([\s\S]*)\]$/);
  if (!match) return v;
  const inner = match[1].trim();
  if (!inner) return "[]";
  // Extract quoted strings
  const items: string[] = [];
  const re = /"([^"]+)"/g;
  let m;
  while ((m = re.exec(inner)) !== null) {
    items.push(m[1]);
  }
  if (items.length > 0) return items.join(", ");
  return inner;
}

function requote(original: string, updated: string): string {
  if (original.startsWith('"') && original.endsWith('"')) return `"${updated}"`;
  if (original.startsWith("'") && original.endsWith("'")) return `'${updated}'`;
  return updated;
}

// Keys that should NOT be treated as sensitive despite matching patterns
const SENSITIVE_WHITELIST = new Set([
  "max_tokens", "max_output_tokens", "token_count", "total_tokens",
  "tokens_per_minute", "token_budget", "compact_threshold",
]);

function isSensitiveKey(key: string, patterns: string[]): boolean {
  const lower = key.toLowerCase();
  if (SENSITIVE_WHITELIST.has(lower)) return false;
  return patterns.some((p) => lower.includes(p)) && !lower.endsWith("_env");
}

// Icon mapping for schema sections
const SECTION_ICONS: Record<string, React.ReactNode> = {
  "brain": <Brain className="h-3.5 w-3.5" />,
  "bot": <Bot className="h-3.5 w-3.5" />,
  "wrench": <Wrench className="h-3.5 w-3.5" />,
  "database": <Database className="h-3.5 w-3.5" />,
  "file-text": <FileText className="h-3.5 w-3.5" />,
  "history": <History className="h-3.5 w-3.5" />,
  "globe": <Globe className="h-3.5 w-3.5" />,
  "shield": <Shield className="h-3.5 w-3.5" />,
  "folder": <Folder className="h-3.5 w-3.5" />,
  "folder-open": <FolderOpen className="h-3.5 w-3.5" />,
  "users": <Users className="h-3.5 w-3.5" />,
  "gauge": <Gauge className="h-3.5 w-3.5" />,
  "lock": <Lock className="h-3.5 w-3.5" />,
  "heart-pulse": <HeartPulse className="h-3.5 w-3.5" />,
  "sparkles": <Sparkles className="h-3.5 w-3.5" />,
  "scroll-text": <ScrollText className="h-3.5 w-3.5" />,
};

// --- Field rendering components ---

function BooleanField({ value, onChange }: { value: boolean; onChange: (v: boolean) => void }) {
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

function SecretField({ value, onChange, placeholder }: {
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

function EnumField({ value, options, onChange }: {
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

function NumberField({ value, onChange, placeholder }: {
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

function TextField({ value, onChange, placeholder }: {
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
function ReadOnlyField({ value, label }: { value: string; label?: string }) {
  const displayVal = value.startsWith("[") ? formatArrayValue(value) : unquote(value);
  return (
    <div className="px-3 py-2 rounded-[var(--radius-lg)] bg-[var(--bg-content)]/60 border border-[var(--border-subtle)] border-dashed text-[13px] font-mono text-[var(--text-secondary)]">
      {displayVal}
      {label && <span className="ml-2 text-[10px] text-[var(--text-tertiary)]">({label})</span>}
    </div>
  );
}

// --- Main component ---

// Section filter mappings for split settings pages
const SECTION_FILTERS: Record<string, string[]> = {
  // Communications: channels, messages, broadcast, voice/audio
  communications: ["lark", "slack", "telegram", "discord", "dingtalk", "mattermost", "matrix",
    "whatsapp", "teams", "signal", "wechat", "imessage", "line", "googlechat", "irc", "webchat",
    "twitch", "nostr", "nextcloud", "synology", "tlon", "zalo", "voice", "broadcast_group"],
  // Automation: commands, hooks, bindings, approvals, heartbeat, tool_policy
  automation: ["command", "bindings", "heartbeat", "tool_policy", "subagent", "reflection"],
  // Infrastructure: serve, auth, security, docker, gateway, rate_limit, hub
  infrastructure: ["serve", "auth", "security", "docker", "gateway", "rate_limit", "hub", "logging"],
  // AI & Agents: agents, models, providers, skills, memory, session, context
  "ai-agents": ["model", "agent", "agents", "models", "providers", "channel_models",
    "skills", "skill_overrides", "memory", "session", "context", "fallback_models"],
};

export default function ConfigPage({ filterSection }: { filterSection?: string } = {}) {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();

  const [loading, setLoading] = useState(true);
  const [configPath, setConfigPath] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [content, setContent] = useState("");
  const [mode, setMode] = useState<EditorMode>("form");
  const [activeSection, setActiveSection] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [schema, setSchema] = useState<ConfigSchemaData | null>(null);
  const [validationErrors, setValidationErrors] = useState<string[]>([]);
  const [validating, setValidating] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Load config
  const loadConfig = useCallback(async () => {
    setLoading(true);
    const data = await api.fetchConfig();
    if (data) {
      setConfigPath(data.path);
      setOriginalContent(data.content);
      setContent(data.content);
    }
    setLoading(false);
  }, [api]);

  // Load schema separately (fire-and-forget, cached)
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/dashboard/config/schema");
        if (!res.ok) return;
        const data = await res.json();
        if (!cancelled && data && Array.isArray(data.sections)) {
          setSchema(data as ConfigSchemaData);
        }
      } catch {
        // Schema unavailable — form falls back to TOML-only mode
      }
    })();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => { loadConfig(); }, [loadConfig]);

  // Parsed TOML sections
  const parsedSections = useMemo(() => parseTomlSections(content), [content]);

  // Merge schema sections with actual TOML data
  const mergedSections = useMemo(() => {
    const tomlMap = new Map<string, TomlSection>();
    for (const s of parsedSections) tomlMap.set(s.key, s);

    type MergedEntry = { schema: ConfigSectionSchema; toml: TomlSection | null; hasData: boolean };

    // Helper: merge dotted child sections into parents
    // e.g. "agent.tools" fields merge into "agent", "logging.file" into "logging"
    const mergeChildren = (entries: MergedEntry[]): MergedEntry[] => {
      const parentMap = new Map<string, MergedEntry>();

      // First pass: collect all entries into map
      for (const entry of entries) parentMap.set(entry.schema.key, entry);

      // Second pass: merge dotted children into parents (create virtual parents for orphans)
      const merged = new Set<string>();
      for (const entry of entries) {
        const key = entry.schema.key;
        if (!key.includes(".")) continue;

        const parentKey = key.split(".")[0];
        let parent = parentMap.get(parentKey);

        // Create virtual parent if it doesn't exist
        if (!parent) {
          const label = parentKey.split("_").map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
          parent = {
            schema: { key: parentKey, label, description: "", order: 998, icon: "settings", fields: [] },
            toml: null,
            hasData: false,
          };
          parentMap.set(parentKey, parent);
        }

        // Merge child's schema fields into parent
        if (entry.schema.fields.length > 0) {
          parent.schema = { ...parent.schema, fields: [...parent.schema.fields, ...entry.schema.fields] };
        }
        // Merge child's toml fields into parent
        if (entry.toml && entry.toml.fields.length > 0) {
          if (parent.toml) {
            parent.toml = { ...parent.toml, fields: [...parent.toml.fields, ...entry.toml.fields] };
          } else {
            parent.toml = entry.toml;
          }
          parent.hasData = true;
        }
        merged.add(key);
      }

      // Third pass: collect non-merged entries (including newly created virtual parents)
      const result: MergedEntry[] = [];
      const seen = new Set<string>();
      for (const entry of entries) {
        if (merged.has(entry.schema.key)) continue;
        result.push(entry);
        seen.add(entry.schema.key);
      }
      // Add virtual parents that weren't in original entries
      for (const [key, entry] of parentMap) {
        if (!seen.has(key) && !merged.has(key) && entry.hasData) {
          result.push(entry);
        }
      }
      return result;
    };

    if (schema) {
      const result: MergedEntry[] = schema.sections.map((ss) => {
        const tomlSection = tomlMap.get(ss.key);
        return { schema: ss, toml: tomlSection ?? null, hasData: !!tomlSection && tomlSection.fields.length > 0 };
      });
      // Add TOML sections not in schema
      const schemaKeys = new Set(schema.sections.map((s) => s.key));
      for (const ts of parsedSections) {
        if (!schemaKeys.has(ts.key)) {
          result.push({
            schema: { key: ts.key, label: ts.key, order: 999, icon: "settings", fields: [] } as ConfigSectionSchema,
            toml: ts,
            hasData: ts.fields.length > 0,
          });
        }
      }
      return mergeChildren(result).sort((a, b) => a.schema.order - b.schema.order);
    }

    // Fallback: no schema — show TOML sections directly
    const fallback: MergedEntry[] = parsedSections.map((ts, idx) => ({
      schema: { key: ts.key, label: ts.key, order: idx * 10, icon: "settings", fields: [] } as ConfigSectionSchema,
      toml: ts,
      hasData: ts.fields.length > 0,
    }));
    return mergeChildren(fallback);
  }, [schema, parsedSections]);

  // Apply section filter for split settings pages
  const filteredSections = useMemo(() => {
    if (!filterSection) return mergedSections;
    const allowedKeys = SECTION_FILTERS[filterSection];
    if (!allowedKeys) return mergedSections;
    return mergedSections.filter((s) => allowedKeys.includes(s.schema.key));
  }, [mergedSections, filterSection]);

  // Reset active section when filter changes
  useEffect(() => {
    setActiveSection(null);
  }, [filterSection]);

  // Auto-select first section with data
  useEffect(() => {
    if (filteredSections.length > 0 && !activeSection) {
      const withData = filteredSections.find((s) => s.hasData);
      setActiveSection((withData ?? filteredSections[0]).schema.key);
    }
  }, [filteredSections, activeSection]);

  // Debounced server-side validation
  useEffect(() => {
    if (content === originalContent) { setValidationErrors([]); return; }
    const timer = setTimeout(async () => {
      if (!content.trim()) { setValidationErrors([]); return; }
      setValidating(true);
      const result = await api.validateConfig(content);
      setValidating(false);
      if (result) setValidationErrors(result.errors ?? []);
    }, 800);
    return () => clearTimeout(timer);
  }, [content, originalContent, api]);

  const isValid = validationErrors.length === 0;
  const hasChanges = content !== originalContent;

  const changeCount = useMemo(() => {
    if (!hasChanges) return 0;
    const a = originalContent.split("\n"), b = content.split("\n");
    let n = 0;
    for (let i = 0; i < Math.max(a.length, b.length); i++) if (a[i] !== b[i]) n++;
    return n;
  }, [originalContent, content, hasChanges]);

  // Save
  const handleSave = useCallback(async () => {
    if (!isValid) { addToast(t("config.invalidToml"), "error"); return; }
    setSaving(true);
    const ok = await api.saveConfig(content);
    setSaving(false);
    if (ok) { setOriginalContent(content); setValidationErrors([]); addToast(t("config.saved"), "success"); }
    else addToast(t("config.saveFailed"), "error");
  }, [api, content, isValid, addToast, t]);

  const handleReload = useCallback(async () => {
    await loadConfig(); setValidationErrors([]); addToast(t("config.reloaded"), "success");
  }, [loadConfig, addToast, t]);

  const handleDiscard = useCallback(() => {
    setContent(originalContent); setValidationErrors([]);
  }, [originalContent]);

  // Update an existing TOML field value
  const handleFieldChange = useCallback((sectionKey: string, fieldKey: string, field: TomlField, newValue: string) => {
    const lines = content.split("\n");
    if (field.line === field.endLine) {
      // Single-line value
      const line = lines[field.line];
      if (!line) return;
      const quoted = requote(field.value, newValue);
      lines[field.line] = line.replace(/=\s*.*$/, `= ${quoted}`);
    } else {
      // Multi-line value — replace entire range with single line
      const quoted = requote(field.value, newValue);
      const prefix = lines[field.line].match(/^([^=]+=\s*)/)?.[1] ?? `${fieldKey} = `;
      lines.splice(field.line, field.endLine - field.line + 1, `${prefix}${quoted}`);
    }
    setContent(lines.join("\n"));
  }, [content]);

  // Add a new field to TOML
  const handleAddField = useCallback((sectionKey: string, fieldKey: string, value: string, fieldType: string) => {
    const lines = content.split("\n");
    let sectionIdx = -1;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].trim() === `[${sectionKey}]`) { sectionIdx = i; break; }
    }
    let tomlValue = value;
    if (fieldType === "string" || fieldType === "secret") tomlValue = `"${value}"`;

    if (sectionIdx === -1) {
      const newLines = [...lines];
      if (newLines[newLines.length - 1]?.trim() !== "") newLines.push("");
      newLines.push(`[${sectionKey}]`);
      newLines.push(`${fieldKey} = ${tomlValue}`);
      setContent(newLines.join("\n"));
    } else {
      let insertIdx = sectionIdx + 1;
      while (insertIdx < lines.length) {
        const l = lines[insertIdx].trim();
        if (l.startsWith("[") && l !== `[${sectionKey}]`) break;
        insertIdx++;
      }
      lines.splice(insertIdx, 0, `${fieldKey} = ${tomlValue}`);
      setContent(lines.join("\n"));
    }
  }, [content]);

  // Search filter
  const displaySections = useMemo(() => {
    if (!searchQuery) return filteredSections;
    const q = searchQuery.toLowerCase();
    return filteredSections.filter((s) => {
      return s.schema.label.toLowerCase().includes(q)
        || s.schema.key.toLowerCase().includes(q)
        || s.schema.description?.toLowerCase().includes(q)
        || s.schema.fields.some((f) => f.label.toLowerCase().includes(q) || f.key.toLowerCase().includes(q))
        || s.toml?.fields.some((f) => f.key.toLowerCase().includes(q) || f.value.toLowerCase().includes(q));
    });
  }, [filteredSections, searchQuery]);

  const activeMerged = useMemo(() => {
    return displaySections.find((s) => s.schema.key === activeSection) ?? null;
  }, [displaySections, activeSection]);

  // Diff
  const diffLines = useMemo(() => {
    if (!hasChanges) return [];
    const a = originalContent.split("\n"), b = content.split("\n");
    const diffs: Array<{ lineNum: number; from: string; to: string }> = [];
    for (let i = 0; i < Math.max(a.length, b.length); i++) {
      if (a[i] !== b[i]) diffs.push({ lineNum: i + 1, from: a[i] || "", to: b[i] || "" });
    }
    return diffs;
  }, [originalContent, content, hasChanges]);

  if (loading) return <LoadingSpinner />;

  const sensitivePatterns = schema?.sensitive_patterns ?? ["api_key", "token", "secret", "password", "app_secret"];
  const hasSchema = !!schema;

  // Render a field from schema definition
  const renderSchemaField = (fieldSchema: ConfigFieldSchema, tomlField: TomlField | undefined) => {
    const rawValue = tomlField ? unquote(tomlField.value) : "";
    const hasValue = !!tomlField;
    const sensitive = fieldSchema.sensitive || isSensitiveKey(fieldSchema.key, sensitivePatterns);
    const isArray = tomlField?.value.startsWith("[");

    const onChangeExisting = (v: string) => handleFieldChange(activeMerged!.schema.key, fieldSchema.key, tomlField!, v);
    const onChangeNew = (v: string) => handleAddField(activeMerged!.schema.key, fieldSchema.key, v, fieldSchema.type);
    const onChange = hasValue ? onChangeExisting : onChangeNew;

    return (
      <div key={fieldSchema.key} className="group py-3 first:pt-0 border-b border-[var(--border-subtle)]/50 last:border-b-0">
        <div className="flex items-start gap-3">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-0.5">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {fieldSchema.label}
              </span>
              {sensitive && <Lock className="h-3 w-3 text-[var(--warning)] flex-shrink-0" />}
              {fieldSchema.default_value && !hasValue && (
                <span className="text-[10px] text-[var(--text-tertiary)] bg-[var(--bg-content)] px-1.5 py-0.5 rounded-md">
                  {t("config.default")}: {fieldSchema.default_value}
                </span>
              )}
            </div>
            {fieldSchema.description && (
              <p className="text-[11px] text-[var(--text-tertiary)] mb-2 leading-relaxed">
                {fieldSchema.description}
              </p>
            )}
          </div>
          <span className="text-[10px] font-mono text-[var(--text-tertiary)] opacity-0 group-hover:opacity-100 transition-opacity mt-1 flex-shrink-0">
            {fieldSchema.key}
          </span>
        </div>

        {/* Array values are read-only in form mode */}
        {isArray ? (
          <ReadOnlyField value={tomlField!.value} label={t("config.editInRaw")} />
        ) : fieldSchema.type === "boolean" ? (
          <div className="flex items-center gap-3">
            <BooleanField value={rawValue === "true"} onChange={(v) => onChange(v ? "true" : "false")} />
            <span className="text-[12px] text-[var(--text-secondary)]">
              {rawValue === "true" ? t("config.enabled") : t("config.notSet")}
            </span>
          </div>
        ) : fieldSchema.type === "secret" || sensitive ? (
          <SecretField value={rawValue} onChange={onChange} placeholder={fieldSchema.placeholder} />
        ) : fieldSchema.type === "enum" && fieldSchema.options ? (
          <EnumField value={rawValue} options={fieldSchema.options} onChange={onChange} />
        ) : fieldSchema.type === "number" ? (
          <NumberField value={rawValue} onChange={onChange} placeholder={fieldSchema.placeholder ?? fieldSchema.default_value} />
        ) : (
          <TextField value={rawValue} onChange={onChange} placeholder={fieldSchema.placeholder ?? fieldSchema.default_value} />
        )}
      </div>
    );
  };

  // Render a TOML-only field (no schema match)
  const renderTomlField = (f: TomlField) => {
    const displayValue = unquote(f.value);
    const isBool = f.value === "true" || f.value === "false";
    const sensitive = isSensitiveKey(f.key, sensitivePatterns);
    const isArray = f.value.startsWith("[");

    return (
      <div key={f.key} className="group py-3 first:pt-0 border-b border-[var(--border-subtle)]/50 last:border-b-0">
        <div className="flex items-center gap-2 mb-2">
          <span className="text-[13px] font-medium text-[var(--text-secondary)] font-mono">
            {f.key}
          </span>
          {sensitive && <Lock className="h-3 w-3 text-[var(--warning)]" />}
          {hasSchema && (
            <span className="text-[9px] text-[var(--text-tertiary)] bg-[var(--bg-content)] px-1.5 py-0.5 rounded-md">
              {t("config.customField")}
            </span>
          )}
        </div>
        {isArray ? (
          <ReadOnlyField value={f.value} label={t("config.editInRaw")} />
        ) : isBool ? (
          <div className="flex items-center gap-3">
            <BooleanField
              value={f.value === "true"}
              onChange={(v) => handleFieldChange(activeMerged!.schema.key, f.key, f, v ? "true" : "false")}
            />
            <span className="text-[12px] text-[var(--text-secondary)]">{f.value}</span>
          </div>
        ) : sensitive ? (
          <SecretField
            value={displayValue}
            onChange={(v) => handleFieldChange(activeMerged!.schema.key, f.key, f, v)}
          />
        ) : (
          <TextField
            value={displayValue}
            onChange={(v) => handleFieldChange(activeMerged!.schema.key, f.key, f, v)}
          />
        )}
      </div>
    );
  };

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Top bar */}
      <div className="flex items-center gap-3 px-1 pb-4 flex-shrink-0 flex-wrap">
        <div className="flex items-center gap-2 min-w-0 flex-shrink">
          <Settings2 className="h-4 w-4 text-[var(--text-tertiary)] flex-shrink-0" />
          <span className="text-[12px] font-mono text-[var(--text-secondary)] truncate max-w-[300px]" title={configPath}>
            {configPath || t("config.unknownPath")}
          </span>
        </div>

        <div className={cn(
          "flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium",
          validating ? "bg-[var(--accent)]/10 text-[var(--accent)]"
            : isValid ? "bg-[var(--success)]/10 text-[var(--success)]"
            : "bg-[var(--error)]/10 text-[var(--error)]"
        )} title={validationErrors.join("\n")}>
          {validating ? <RefreshCw className="h-3 w-3 animate-spin" />
            : isValid ? <Check className="h-3 w-3" />
            : <X className="h-3 w-3" />}
          {validating ? t("config.validating") : isValid ? t("config.valid") : t("config.invalid")}
        </div>

        {validationErrors.length > 0 && (
          <div className="flex items-center gap-1 text-[10px] text-[var(--error)]">
            <AlertTriangle className="h-3 w-3" />
            <span className="truncate max-w-[200px]">{validationErrors[0]}</span>
          </div>
        )}

        {changeCount > 0 && (
          <div className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium tabular-nums">
            {changeCount} {t("config.changes", { count: changeCount })}
          </div>
        )}

        <div className="flex-1" />

        {hasChanges && (
          <button onClick={handleDiscard} className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-lg)] text-[11px] font-medium text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/5 transition-colors cursor-pointer">
            <X className="h-3.5 w-3.5" />
            {t("config.discard")}
          </button>
        )}

        <button onClick={handleReload} className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-lg)] text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer">
          <RefreshCw className="h-3.5 w-3.5" />
          {t("config.reload")}
        </button>

        <button onClick={handleSave} disabled={!hasChanges || saving || !isValid}
          className={cn("flex items-center gap-1.5 px-4 py-1.5 rounded-[var(--radius-lg)] text-[11px] font-medium transition-colors cursor-pointer",
            hasChanges && isValid ? "bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]" : "bg-[var(--bg-content)] text-[var(--text-tertiary)] cursor-not-allowed"
          )}>
          {saving ? <RefreshCw className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
          {saving ? t("config.saving") : t("config.save")}
        </button>
      </div>

      {/* Main area */}
      <div className="flex-1 min-h-0 flex flex-col">
        {mode === "raw" ? (
          <SectionCard className="flex-1 flex flex-col min-h-0 !p-0">
            <div className="flex items-center gap-2 px-4 py-2.5 border-b border-[var(--border-subtle)]">
              <Code className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
              <span className="text-[11px] font-medium text-[var(--text-secondary)]">{t("config.tomlEditor")}</span>
              {validationErrors.length > 0 && (
                <span className="text-[10px] text-[var(--error)] ml-auto font-mono truncate max-w-[300px]">{validationErrors[0]}</span>
              )}
            </div>
            <textarea
              ref={textareaRef}
              value={content}
              onChange={(e) => setContent(e.target.value)}
              spellCheck={false}
              className="flex-1 w-full resize-none bg-transparent text-[13px] font-mono text-[var(--text-primary)] p-4 focus:outline-none leading-relaxed placeholder:text-[var(--text-tertiary)]"
              placeholder={t("config.rawPlaceholder")}
            />
          </SectionCard>
        ) : (
          <div className="flex-1 min-h-0 flex gap-3">
            {/* Left nav */}
            <div className="w-[200px] flex-shrink-0 flex flex-col min-h-0">
              <SectionCard className="flex-1 flex flex-col min-h-0 !p-2">
                <div className="relative mb-2">
                  <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-[var(--text-tertiary)]" />
                  <input
                    type="text" value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder={t("config.searchFields")}
                    className="w-full pl-7 pr-2 py-1.5 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[11px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)]"
                  />
                </div>
                <div className="flex-1 overflow-y-auto space-y-0.5">
                  {displaySections.map((s) => {
                    const icon = SECTION_ICONS[s.schema.icon] ?? <Settings2 className="h-3.5 w-3.5" />;
                    const isActive = activeSection === s.schema.key;
                    return (
                      <button key={s.schema.key} onClick={() => setActiveSection(s.schema.key)}
                        className={cn(
                          "w-full text-left px-2.5 py-2 rounded-[var(--radius-lg)] text-[12px] font-medium transition-colors cursor-pointer flex items-center gap-2",
                          isActive ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                            : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                        )} title={s.schema.description}>
                        <span className={cn("flex-shrink-0", isActive ? "opacity-100" : "opacity-50")}>{icon}</span>
                        <span className="truncate">{s.schema.label}</span>
                        {s.hasData && (
                          <span className="ml-auto text-[10px] text-[var(--text-tertiary)] tabular-nums flex-shrink-0 bg-[var(--bg-content)] px-1.5 rounded-full">
                            {s.toml!.fields.length}
                          </span>
                        )}
                      </button>
                    );
                  })}
                  {displaySections.length === 0 && (
                    <EmptyState icon={<Settings2 className="h-5 w-5" />} message={t("config.noSections")} />
                  )}
                </div>
              </SectionCard>
            </div>

            {/* Center: fields */}
            <div className="flex-1 min-w-0 flex flex-col min-h-0">
              <SectionCard className="flex-1 flex flex-col min-h-0 overflow-y-auto">
                {activeMerged ? (
                  <>
                    <div className="flex items-center gap-3 mb-1">
                      <span className="text-[var(--text-tertiary)]">
                        {SECTION_ICONS[activeMerged.schema.icon] ?? <Settings2 className="h-4.5 w-4.5" />}
                      </span>
                      <div>
                        <h3 className="text-[15px] font-semibold text-[var(--text-primary)]">
                          {activeMerged.schema.label}
                        </h3>
                        {activeMerged.schema.description && (
                          <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5">
                            {activeMerged.schema.description}
                          </p>
                        )}
                      </div>
                      <span className="ml-auto text-[10px] font-mono text-[var(--text-tertiary)] bg-[var(--bg-content)] px-2 py-0.5 rounded-md">
                        {activeMerged.toml?.isArrayTable ? `[[${activeMerged.schema.key}]]` : `[${activeMerged.schema.key}]`}
                      </span>
                    </div>

                    <div className="mt-3">
                      {/* Schema-defined fields */}
                      {activeMerged.schema.fields.map((fs) => {
                        const tomlField = activeMerged.toml?.fields.find((f) => f.key === fs.key);
                        return renderSchemaField(fs, tomlField);
                      })}

                      {/* Extra TOML fields not in schema */}
                      {activeMerged.toml?.fields
                        .filter((f) => !activeMerged.schema.fields.some((sf) => sf.key === f.key))
                        .map((f) => renderTomlField(f))}

                      {activeMerged.schema.fields.length === 0 && (!activeMerged.toml || activeMerged.toml.fields.length === 0) && (
                        <div className="text-[12px] text-[var(--text-tertiary)] py-8 text-center">
                          {t("config.noFields")}
                        </div>
                      )}
                    </div>
                  </>
                ) : (
                  <EmptyState icon={<FormInput className="h-5 w-5" />} message={t("config.selectSection")} />
                )}
              </SectionCard>
            </div>

            {/* Right: diff preview */}
            {hasChanges && (
              <div className="w-[240px] flex-shrink-0 flex flex-col min-h-0">
                <SectionCard className="flex-1 flex flex-col min-h-0 !p-0">
                  <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border-subtle)]">
                    <span className="text-[11px] font-medium text-[var(--text-secondary)]">{t("config.changesPreview")}</span>
                    <span className="text-[10px] text-[var(--accent)] font-mono tabular-nums ml-auto">{diffLines.length}</span>
                  </div>
                  <div className="flex-1 overflow-y-auto p-2 space-y-1.5">
                    {diffLines.map((d) => (
                      <div key={d.lineNum} className="text-[10px] font-mono leading-relaxed">
                        <div className="text-[var(--text-tertiary)] mb-0.5">L{d.lineNum}</div>
                        {d.from && <div className="px-1.5 py-0.5 rounded bg-[var(--error)]/8 text-[var(--error)] truncate" title={d.from}>- {d.from}</div>}
                        {d.to && <div className="px-1.5 py-0.5 rounded bg-[var(--success)]/8 text-[var(--success)] truncate mt-0.5" title={d.to}>+ {d.to}</div>}
                      </div>
                    ))}
                  </div>
                </SectionCard>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Bottom mode toggle */}
      <div className="flex items-center justify-center pt-3 flex-shrink-0">
        <div className="inline-flex rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-0.5">
          <button onClick={() => setMode("form")}
            className={cn("flex items-center gap-1.5 px-3 py-1 rounded-md text-[11px] font-medium transition-colors cursor-pointer",
              mode === "form" ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm" : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
            )}>
            <FormInput className="h-3 w-3" />Form
          </button>
          <button onClick={() => setMode("raw")}
            className={cn("flex items-center gap-1.5 px-3 py-1 rounded-md text-[11px] font-medium transition-colors cursor-pointer",
              mode === "raw" ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm" : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
            )}>
            <Code className="h-3 w-3" />TOML
          </button>
        </div>
      </div>

      <ToastContainer toasts={toasts} />
    </div>
  );
}
