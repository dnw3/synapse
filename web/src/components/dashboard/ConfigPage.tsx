import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Settings2, Save, Check, X, RefreshCw, Code, FormInput, Search } from "lucide-react";
import { cn } from "../../lib/cn";
import type { ConfigData } from "../../types/dashboard";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import { SectionCard, SectionHeader, EmptyState, LoadingSpinner, useToast, ToastContainer } from "./shared";

type EditorMode = "form" | "raw";

interface TomlSection {
  key: string;
  fields: Array<{ key: string; value: string; line: number }>;
}

/** Naive TOML parser that extracts top-level sections and their key-value pairs. */
function parseTomlSections(content: string): TomlSection[] {
  const lines = content.split("\n");
  const sections: TomlSection[] = [];
  let current: TomlSection | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    // Section header: [name] or [name.sub]
    const sectionMatch = line.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      current = { key: sectionMatch[1], fields: [] };
      sections.push(current);
      continue;
    }
    // Key-value pair (skip comments and blank lines)
    if (!line || line.startsWith("#")) continue;
    const kvMatch = line.match(/^([^=]+?)\s*=\s*(.+)$/);
    if (kvMatch && current) {
      current.fields.push({
        key: kvMatch[1].trim(),
        value: kvMatch[2].trim(),
        line: i,
      });
    } else if (kvMatch && !current) {
      // Top-level keys without a section header go under "[root]"
      if (!sections.length || sections[0].key !== "(root)") {
        const root: TomlSection = { key: "(root)", fields: [] };
        sections.unshift(root);
        current = root;
      } else {
        current = sections[0];
      }
      current.fields.push({
        key: kvMatch[1].trim(),
        value: kvMatch[2].trim(),
        line: i,
      });
    }
  }
  return sections;
}

/** Try to validate TOML content by checking basic structure. */
function validateToml(content: string): { valid: boolean; error?: string } {
  try {
    const lines = content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line || line.startsWith("#")) continue;
      // Section headers
      if (line.startsWith("[")) {
        if (!line.match(/^\[{1,2}[^\]]+\]{1,2}$/)) {
          return { valid: false, error: `Line ${i + 1}: malformed section header` };
        }
        continue;
      }
      // Key-value: must contain =
      if (!line.includes("=")) {
        return { valid: false, error: `Line ${i + 1}: expected key = value` };
      }
    }
    return { valid: true };
  } catch (e) {
    return { valid: false, error: String(e) };
  }
}

/** Strip surrounding quotes from a TOML value for form display. */
function unquote(v: string): string {
  if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
    return v.slice(1, -1);
  }
  return v;
}

/** Re-quote a value if the original was quoted. */
function requote(original: string, updated: string): string {
  if (original.startsWith('"') && original.endsWith('"')) return `"${updated}"`;
  if (original.startsWith("'") && original.endsWith("'")) return `'${updated}'`;
  return updated;
}

export default function ConfigPage() {
  const { t, i18n } = useTranslation();
  const isZh = i18n.language?.startsWith("zh");
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

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  // Parsed sections
  const sections = useMemo(() => parseTomlSections(content), [content]);

  // Auto-select first section
  useEffect(() => {
    if (sections.length > 0 && !activeSection) {
      setActiveSection(sections[0].key);
    }
  }, [sections, activeSection]);

  // Validation
  const validation = useMemo(() => validateToml(content), [content]);

  // Change count
  const changeCount = useMemo(() => {
    if (originalContent === content) return 0;
    const origLines = originalContent.split("\n");
    const currLines = content.split("\n");
    let count = 0;
    const maxLen = Math.max(origLines.length, currLines.length);
    for (let i = 0; i < maxLen; i++) {
      if (origLines[i] !== currLines[i]) count++;
    }
    return count;
  }, [originalContent, content]);

  // Has changes
  const hasChanges = content !== originalContent;

  // Save
  const handleSave = useCallback(async () => {
    if (!validation.valid) {
      addToast(isZh ? "TOML 格式无效，无法保存" : "Invalid TOML, cannot save", "error");
      return;
    }
    setSaving(true);
    const ok = await api.saveConfig(content);
    setSaving(false);
    if (ok) {
      setOriginalContent(content);
      addToast(isZh ? "配置已保存" : "Config saved successfully", "success");
    } else {
      addToast(isZh ? "保存失败" : "Failed to save config", "error");
    }
  }, [api, content, validation, addToast, isZh]);

  // Reload
  const handleReload = useCallback(async () => {
    await loadConfig();
    addToast(isZh ? "配置已重新加载" : "Config reloaded", "success");
  }, [loadConfig, addToast, isZh]);

  // Update a field value in form mode
  const handleFieldChange = useCallback((sectionKey: string, fieldKey: string, lineIdx: number, originalValue: string, newValue: string) => {
    const lines = content.split("\n");
    const line = lines[lineIdx];
    if (!line) return;
    const quoted = requote(originalValue, newValue);
    lines[lineIdx] = line.replace(/=\s*.+$/, `= ${quoted}`);
    setContent(lines.join("\n"));
  }, [content]);

  // Filtered sections for search
  const filteredSections = useMemo(() => {
    if (!searchQuery) return sections;
    const q = searchQuery.toLowerCase();
    return sections
      .map((s) => ({
        ...s,
        fields: s.fields.filter(
          (f) => f.key.toLowerCase().includes(q) || f.value.toLowerCase().includes(q) || s.key.toLowerCase().includes(q)
        ),
      }))
      .filter((s) => s.fields.length > 0 || s.key.toLowerCase().includes(q));
  }, [sections, searchQuery]);

  // Active section data
  const activeSectionData = useMemo(() => {
    const pool = searchQuery ? filteredSections : sections;
    return pool.find((s) => s.key === activeSection) || null;
  }, [sections, filteredSections, activeSection, searchQuery]);

  // Diff lines for right panel
  const diffLines = useMemo(() => {
    if (!hasChanges) return [];
    const origLines = originalContent.split("\n");
    const currLines = content.split("\n");
    const diffs: Array<{ lineNum: number; from: string; to: string }> = [];
    const maxLen = Math.max(origLines.length, currLines.length);
    for (let i = 0; i < maxLen; i++) {
      if (origLines[i] !== currLines[i]) {
        diffs.push({ lineNum: i + 1, from: origLines[i] || "", to: currLines[i] || "" });
      }
    }
    return diffs;
  }, [originalContent, content, hasChanges]);

  if (loading) return <LoadingSpinner />;

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Top bar */}
      <div className="flex items-center gap-3 px-1 pb-4 flex-shrink-0 flex-wrap">
        {/* Config path */}
        <div className="flex items-center gap-2 min-w-0 flex-shrink">
          <Settings2 className="h-4 w-4 text-[var(--text-tertiary)] flex-shrink-0" />
          <span className="text-[12px] font-mono text-[var(--text-secondary)] truncate max-w-[300px]" title={configPath}>
            {configPath || (isZh ? "未知路径" : "Unknown path")}
          </span>
        </div>

        {/* Validation badge */}
        <div className={cn(
          "flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium",
          validation.valid
            ? "bg-[var(--success)]/10 text-[var(--success)]"
            : "bg-[var(--error)]/10 text-[var(--error)]"
        )}>
          {validation.valid ? <Check className="h-3 w-3" /> : <X className="h-3 w-3" />}
          {validation.valid ? (isZh ? "有效" : "Valid") : (isZh ? "无效" : "Invalid")}
        </div>

        {/* Change counter */}
        {changeCount > 0 && (
          <div className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium tabular-nums">
            {changeCount} {isZh ? "处修改" : changeCount === 1 ? "change" : "changes"}
          </div>
        )}

        <div className="flex-1" />

        {/* Reload button */}
        <button
          onClick={handleReload}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          title={isZh ? "重新加载" : "Reload"}
        >
          <RefreshCw className="h-3.5 w-3.5" />
          {isZh ? "重载" : "Reload"}
        </button>

        {/* Save button */}
        <button
          onClick={handleSave}
          disabled={!hasChanges || saving || !validation.valid}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[11px] font-medium transition-colors cursor-pointer",
            hasChanges && validation.valid
              ? "bg-[var(--accent)] text-white hover:opacity-90"
              : "bg-[var(--bg-surface)] text-[var(--text-tertiary)] cursor-not-allowed"
          )}
        >
          {saving ? <RefreshCw className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
          {saving ? (isZh ? "保存中..." : "Saving...") : (isZh ? "保存" : "Save")}
        </button>
      </div>

      {/* Main editor area */}
      <div className="flex-1 min-h-0 flex flex-col">
        {mode === "raw" ? (
          /* Raw TOML editor */
          <SectionCard className="flex-1 flex flex-col min-h-0 !p-0">
            <div className="flex items-center gap-2 px-4 py-2.5 border-b border-[var(--border-subtle)]">
              <Code className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
              <span className="text-[11px] font-medium text-[var(--text-secondary)]">
                {isZh ? "TOML 编辑器" : "TOML Editor"}
              </span>
              {!validation.valid && validation.error && (
                <span className="text-[10px] text-[var(--error)] ml-auto font-mono">{validation.error}</span>
              )}
            </div>
            <textarea
              ref={textareaRef}
              value={content}
              onChange={(e) => setContent(e.target.value)}
              spellCheck={false}
              className="flex-1 w-full resize-none bg-transparent text-[12px] font-mono text-[var(--text-primary)] p-4 focus:outline-none leading-relaxed placeholder:text-[var(--text-tertiary)]"
              placeholder={isZh ? "粘贴或编辑 TOML 配置..." : "Paste or edit TOML config..."}
            />
          </SectionCard>
        ) : (
          /* Form mode: three-column layout */
          <div className="flex-1 min-h-0 flex gap-3">
            {/* Left nav: section list */}
            <div className="w-[180px] flex-shrink-0 flex flex-col min-h-0">
              <SectionCard className="flex-1 flex flex-col min-h-0 !p-2">
                {/* Search */}
                <div className="relative mb-2">
                  <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-[var(--text-tertiary)]" />
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder={isZh ? "搜索字段..." : "Search fields..."}
                    className="w-full pl-7 pr-2 py-1.5 rounded-[var(--radius-sm)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[11px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)]"
                  />
                </div>
                {/* Section list */}
                <div className="flex-1 overflow-y-auto space-y-0.5">
                  {(searchQuery ? filteredSections : sections).map((s) => (
                    <button
                      key={s.key}
                      onClick={() => setActiveSection(s.key)}
                      className={cn(
                        "w-full text-left px-2.5 py-1.5 rounded-[var(--radius-sm)] text-[11px] font-medium transition-colors cursor-pointer truncate",
                        activeSection === s.key
                          ? "bg-[var(--accent)]/12 text-[var(--accent)]"
                          : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                      )}
                      title={s.key}
                    >
                      <span className="font-mono">[{s.key}]</span>
                      <span className="ml-1.5 text-[var(--text-tertiary)] text-[10px]">({s.fields.length})</span>
                    </button>
                  ))}
                  {sections.length === 0 && (
                    <EmptyState
                      icon={<Settings2 className="h-5 w-5" />}
                      message={isZh ? "无配置节" : "No sections found"}
                    />
                  )}
                </div>
              </SectionCard>
            </div>

            {/* Center: form fields */}
            <div className="flex-1 min-w-0 flex flex-col min-h-0">
              <SectionCard className="flex-1 flex flex-col min-h-0 overflow-y-auto">
                {activeSectionData ? (
                  <>
                    <SectionHeader
                      icon={<FormInput className="h-4 w-4" />}
                      title={`[${activeSectionData.key}]`}
                      right={
                        <span className="text-[10px] text-[var(--text-tertiary)] font-mono tabular-nums">
                          {activeSectionData.fields.length} {isZh ? "个字段" : activeSectionData.fields.length === 1 ? "field" : "fields"}
                        </span>
                      }
                    />
                    <div className="space-y-3">
                      {activeSectionData.fields.map((f) => {
                        const displayValue = unquote(f.value);
                        const isBool = f.value === "true" || f.value === "false";
                        return (
                          <div key={`${activeSectionData.key}-${f.key}-${f.line}`} className="flex flex-col gap-1">
                            <label className="text-[11px] font-medium text-[var(--text-secondary)] font-mono">
                              {f.key}
                            </label>
                            {isBool ? (
                              <button
                                onClick={() =>
                                  handleFieldChange(
                                    activeSectionData.key,
                                    f.key,
                                    f.line,
                                    f.value,
                                    f.value === "true" ? "false" : "true"
                                  )
                                }
                                className={cn(
                                  "w-fit px-3 py-1 rounded-[var(--radius-sm)] text-[11px] font-mono font-medium transition-colors cursor-pointer",
                                  f.value === "true"
                                    ? "bg-[var(--success)]/10 text-[var(--success)]"
                                    : "bg-[var(--bg-surface)] text-[var(--text-tertiary)]"
                                )}
                              >
                                {f.value}
                              </button>
                            ) : (
                              <input
                                type="text"
                                value={displayValue}
                                onChange={(e) =>
                                  handleFieldChange(
                                    activeSectionData.key,
                                    f.key,
                                    f.line,
                                    f.value,
                                    e.target.value
                                  )
                                }
                                className="w-full px-2.5 py-1.5 rounded-[var(--radius-sm)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[12px] font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
                              />
                            )}
                          </div>
                        );
                      })}
                      {activeSectionData.fields.length === 0 && (
                        <div className="text-[11px] text-[var(--text-tertiary)] py-6 text-center">
                          {isZh ? "该节无可编辑字段" : "No editable fields in this section"}
                        </div>
                      )}
                    </div>
                  </>
                ) : (
                  <EmptyState
                    icon={<FormInput className="h-5 w-5" />}
                    message={isZh ? "选择一个配置节" : "Select a section"}
                  />
                )}
              </SectionCard>
            </div>

            {/* Right: diff preview */}
            {hasChanges && (
              <div className="w-[240px] flex-shrink-0 flex flex-col min-h-0">
                <SectionCard className="flex-1 flex flex-col min-h-0 !p-0">
                  <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border-subtle)]">
                    <span className="text-[11px] font-medium text-[var(--text-secondary)]">
                      {isZh ? "变更预览" : "Changes"}
                    </span>
                    <span className="text-[10px] text-[var(--accent)] font-mono tabular-nums ml-auto">
                      {diffLines.length}
                    </span>
                  </div>
                  <div className="flex-1 overflow-y-auto p-2 space-y-1.5">
                    {diffLines.map((d) => (
                      <div key={d.lineNum} className="text-[10px] font-mono leading-relaxed">
                        <div className="text-[var(--text-tertiary)] mb-0.5">L{d.lineNum}</div>
                        {d.from && (
                          <div className="px-1.5 py-0.5 rounded-[var(--radius-sm)] bg-[var(--error)]/8 text-[var(--error)] truncate" title={d.from}>
                            - {d.from}
                          </div>
                        )}
                        {d.to && (
                          <div className="px-1.5 py-0.5 rounded-[var(--radius-sm)] bg-[var(--success)]/8 text-[var(--success)] truncate mt-0.5" title={d.to}>
                            + {d.to}
                          </div>
                        )}
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
        <div className="inline-flex rounded-[var(--radius-md)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] p-0.5">
          <button
            onClick={() => setMode("form")}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1 rounded-[var(--radius-sm)] text-[11px] font-medium transition-colors cursor-pointer",
              mode === "form"
                ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
            )}
          >
            <FormInput className="h-3 w-3" />
            Form
          </button>
          <button
            onClick={() => setMode("raw")}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1 rounded-[var(--radius-sm)] text-[11px] font-medium transition-colors cursor-pointer",
              mode === "raw"
                ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
            )}
          >
            <Code className="h-3 w-3" />
            Raw
          </button>
        </div>
      </div>

      <ToastContainer toasts={toasts} />
    </div>
  );
}
