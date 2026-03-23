import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Settings2, Save, Check, X, RefreshCw, Code, FormInput, AlertTriangle,
} from "lucide-react";
import { cn } from "../../../lib/cn";
import { useConfig, useConfigSchema, useSaveConfig, useValidateConfig } from "../../../hooks/queries/useConfigQueries";
import { SectionCard, LoadingSpinner } from "../shared";
import { useToast } from "../../ui/toast";
import { parseTomlSections, requote, type ConfigSchemaData, type TomlField, type TomlSection } from "./tomlParser";
import { ConfigForm, type MergedSection } from "./ConfigForm";
import type { ConfigFieldSchema } from "./tomlParser";

type EditorMode = "form" | "raw";

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
  const { toast } = useToast();

  const configQ = useConfig();
  const schemaQ = useConfigSchema();
  const saveMut = useSaveConfig();
  const validateMut = useValidateConfig();

  const loading = configQ.isPending;
  const schema = (schemaQ.data && typeof schemaQ.data === "object" && "sections" in schemaQ.data && Array.isArray((schemaQ.data as unknown as ConfigSchemaData).sections))
    ? schemaQ.data as unknown as ConfigSchemaData : null;

  const [configPath, setConfigPath] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [content, setContent] = useState("");
  const [mode, setMode] = useState<EditorMode>("form");
  const [activeSection, setActiveSection] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [validationErrors, setValidationErrors] = useState<string[]>([]);
  const [validating, setValidating] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Sync config data from query
  useEffect(() => {
    if (configQ.data) {
      setConfigPath(configQ.data.path);
      setOriginalContent(configQ.data.content);
      setContent(configQ.data.content);
    }
  }, [configQ.data]);

  // Parsed TOML sections
  const parsedSections = useMemo(() => parseTomlSections(content), [content]);

  // Merge schema sections with actual TOML data
  const mergedSections = useMemo(() => {
    const tomlMap = new Map<string, TomlSection>();
    for (const s of parsedSections) tomlMap.set(s.key, s);

    type MergedEntry = { schema: ConfigFieldSchema extends never ? never : { key: string; label: string; description?: string; order: number; icon: string; fields: ConfigFieldSchema[] }; toml: TomlSection | null; hasData: boolean };

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
            schema: { key: ts.key, label: ts.key, order: 999, icon: "settings", fields: [] } as { key: string; label: string; description?: string; order: number; icon: string; fields: ConfigFieldSchema[] },
            toml: ts,
            hasData: ts.fields.length > 0,
          });
        }
      }
      return mergeChildren(result).sort((a, b) => a.schema.order - b.schema.order);
    }

    // Fallback: no schema — show TOML sections directly
    const fallback: MergedEntry[] = parsedSections.map((ts, idx) => ({
      schema: { key: ts.key, label: ts.key, order: idx * 10, icon: "settings", fields: [] } as { key: string; label: string; description?: string; order: number; icon: string; fields: ConfigFieldSchema[] },
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
      try {
        const result = await validateMut.mutateAsync(content);
        if (result) setValidationErrors(result.errors ?? []);
      } catch {
        // validation request failed, don't update errors
      }
      setValidating(false);
    }, 800);
    return () => clearTimeout(timer);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [content, originalContent]);

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
  const handleSave = async () => {
    if (!isValid) { toast({ variant: "error", title: t("config.invalidToml") }); return; }
    setSaving(true);
    try {
      await saveMut.mutateAsync(content);
      setOriginalContent(content);
      setValidationErrors([]);
    } catch {
      // toast handled by mutation
    }
    setSaving(false);
  };

  const handleReload = async () => {
    await configQ.refetch(); setValidationErrors([]); toast({ variant: "success", title: t("config.reloaded") });
  };

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
          <ConfigForm
            displaySections={displaySections as MergedSection[]}
            activeSection={activeSection}
            searchQuery={searchQuery}
            hasChanges={hasChanges}
            diffLines={diffLines}
            activeMerged={activeMerged as MergedSection | null}
            schema={schema}
            hasSchema={hasSchema}
            sensitivePatterns={sensitivePatterns}
            content={content}
            onSectionSelect={setActiveSection}
            onSearchChange={setSearchQuery}
            onFieldChange={handleFieldChange}
            onAddField={handleAddField}
          />
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

    </div>
  );
}
