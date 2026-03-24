import { useState, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  X, Play, Loader2, Copy, Check, ChevronDown, ChevronRight, Trash2,
} from "lucide-react";
import { useInvokeMcpTool } from "../../hooks/queries/useMcpQueries";
import type { McpServerInfo } from "../../types/dashboard";
import { cn } from "../../lib/cn";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface PlaygroundHistoryEntry {
  id: string;
  serverName: string;
  toolName: string;
  arguments: Record<string, unknown>;
  success: boolean;
  latencyMs: number;
  resultPreview: string; // first 500 chars
  error: string | null;
  timestamp: number;
}

const HISTORY_KEY = "synapse:mcp-playground-history";
const MAX_HISTORY = 50;

// ---------------------------------------------------------------------------
// History helpers (sessionStorage)
// ---------------------------------------------------------------------------

function loadHistory(): PlaygroundHistoryEntry[] {
  try {
    const raw = sessionStorage.getItem(HISTORY_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveHistory(entries: PlaygroundHistoryEntry[]) {
  try {
    sessionStorage.setItem(HISTORY_KEY, JSON.stringify(entries.slice(0, MAX_HISTORY)));
  } catch { /* sessionStorage full — silently drop */ }
}

// ---------------------------------------------------------------------------
// Schema → default value helpers
// ---------------------------------------------------------------------------

function defaultForSchema(schema: Record<string, unknown> | null | undefined): Record<string, unknown> {
  if (!schema || !schema.properties) return {};
  const props = schema.properties as Record<string, Record<string, unknown>>;
  const result: Record<string, unknown> = {};
  for (const [key, prop] of Object.entries(props)) {
    if (prop.default !== undefined) {
      result[key] = prop.default;
    } else if (prop.type === "string") {
      result[key] = "";
    } else if (prop.type === "number" || prop.type === "integer") {
      result[key] = 0;
    } else if (prop.type === "boolean") {
      result[key] = false;
    }
  }
  return result;
}

function schemaProperties(schema: Record<string, unknown> | null | undefined): Array<{
  name: string;
  type: string;
  description: string;
  required: boolean;
  enumValues?: string[];
  default?: unknown;
}> {
  if (!schema || !schema.properties) return [];
  const props = schema.properties as Record<string, Record<string, unknown>>;
  const required = (schema.required as string[]) ?? [];
  return Object.entries(props).map(([name, prop]) => ({
    name,
    type: (prop.type as string) ?? "string",
    description: (prop.description as string) ?? "",
    required: required.includes(name),
    enumValues: prop.enum as string[] | undefined,
    default: prop.default,
  }));
}

// ---------------------------------------------------------------------------
// Relative time helper
// ---------------------------------------------------------------------------

function relativeTime(ts: number): string {
  const now = performance.timeOrigin + performance.now();
  const diff = Math.floor((now - ts) / 1000);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface ToolPlaygroundDrawerProps {
  servers: McpServerInfo[];
  initialServer?: string;
  initialTool?: string;
  onClose: () => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function ToolPlaygroundDrawer({
  servers,
  initialServer,
  initialTool,
  onClose,
}: ToolPlaygroundDrawerProps) {
  const { t } = useTranslation();
  const invokeMut = useInvokeMcpTool();

  // Server / tool selection
  const [serverName, setServerName] = useState(initialServer ?? servers[0]?.name ?? "");
  const [toolName, setToolName] = useState(initialTool ?? "");

  const selectedServer = useMemo(
    () => servers.find((s) => s.name === serverName),
    [servers, serverName],
  );
  const selectedTool = useMemo(
    () => selectedServer?.tools.find((t) => t.name === toolName),
    [selectedServer, toolName],
  );

  // Params
  const [mode, setMode] = useState<"form" | "json">("form");
  const [formValues, setFormValues] = useState<Record<string, unknown>>({});
  const [jsonText, setJsonText] = useState("{}");
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});

  // Reset params state for a given tool schema
  const resetParamsForTool = useCallback((schema: Record<string, unknown> | null | undefined) => {
    const defaults = defaultForSchema(schema);
    setFormValues(defaults);
    setJsonText(JSON.stringify(defaults, null, 2));
    setJsonError(null);
    setValidationErrors({});
  }, []);

  // Handle server change: auto-select first tool + reset params
  const handleServerChange = useCallback((newServer: string) => {
    setServerName(newServer);
    const srv = servers.find((s) => s.name === newServer);
    if (srv && srv.tools.length > 0) {
      const firstTool = srv.tools[0];
      setToolName(firstTool.name);
      resetParamsForTool(firstTool.parameters);
    } else {
      setToolName("");
      resetParamsForTool(null);
    }
  }, [servers, resetParamsForTool]);

  // Handle tool change: reset params
  const handleToolChange = useCallback((newTool: string) => {
    setToolName(newTool);
    const tool = selectedServer?.tools.find((t) => t.name === newTool);
    resetParamsForTool(tool?.parameters);
  }, [selectedServer, resetParamsForTool]);

  // Result
  const [lastResult, setLastResult] = useState<{
    success: boolean;
    result: unknown;
    latencyMs: number;
    error: string | null;
  } | null>(null);

  // History
  const [history, setHistory] = useState<PlaygroundHistoryEntry[]>(loadHistory);
  const [showHistory, setShowHistory] = useState(false);

  // Copy state
  const [copied, setCopied] = useState(false);

  const properties = useMemo(() => schemaProperties(selectedTool?.parameters), [selectedTool]);

  // Sync form ↔ json
  const switchMode = useCallback((newMode: "form" | "json") => {
    if (newMode === "json" && mode === "form") {
      setJsonText(JSON.stringify(formValues, null, 2));
      setJsonError(null);
    } else if (newMode === "form" && mode === "json") {
      try {
        const parsed = JSON.parse(jsonText);
        setFormValues(parsed);
        setJsonError(null);
      } catch {
        // keep existing form values if JSON is invalid
      }
    }
    setMode(newMode);
  }, [mode, formValues, jsonText]);

  // Execute
  const handleExecute = useCallback(() => {
    let args: Record<string, unknown>;

    if (mode === "json") {
      try {
        args = JSON.parse(jsonText);
        setJsonError(null);
      } catch {
        setJsonError(t("dashboard.mcpServers.playground.invalidJson"));
        return;
      }
    } else {
      // Validate required fields
      const errors: Record<string, string> = {};
      for (const prop of properties) {
        if (prop.required) {
          const val = formValues[prop.name];
          if (val === undefined || val === null || val === "") {
            errors[prop.name] = t("dashboard.mcpServers.playground.required");
          }
        }
      }
      if (Object.keys(errors).length > 0) {
        setValidationErrors(errors);
        return;
      }
      setValidationErrors({});
      // Strip empty strings for optional fields
      args = { ...formValues };
      for (const [k, v] of Object.entries(args)) {
        if (v === "" && !properties.find((p) => p.name === k)?.required) {
          delete args[k];
        }
      }
    }

    invokeMut.mutate(
      { serverName, toolName, arguments: args },
      {
        onSuccess: (data) => {
          setLastResult({
            success: data.success,
            result: data.result,
            latencyMs: data.latencyMs,
            error: data.error ?? null,
          });
          const entry: PlaygroundHistoryEntry = {
            id: crypto.randomUUID(),
            serverName,
            toolName,
            arguments: args,
            success: data.success,
            latencyMs: data.latencyMs,
            resultPreview: JSON.stringify(data.result ?? data.error ?? "").slice(0, 500),
            error: data.error ?? null,
            timestamp: Date.now(),
          };
          const newHistory = [entry, ...history].slice(0, MAX_HISTORY);
          setHistory(newHistory);
          saveHistory(newHistory);
        },
        onError: (err: Error) => {
          setLastResult({
            success: false,
            result: null,
            latencyMs: 0,
            error: err.message,
          });
        },
      },
    );
  }, [serverName, toolName, mode, formValues, jsonText, properties, history, invokeMut, t]);

  const handleCopy = useCallback(() => {
    const text = JSON.stringify(lastResult?.result ?? lastResult?.error, null, 2);
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [lastResult]);

  const handleLoadHistory = useCallback((entry: PlaygroundHistoryEntry) => {
    setServerName(entry.serverName);
    setToolName(entry.toolName);
    setFormValues(entry.arguments);
    setJsonText(JSON.stringify(entry.arguments, null, 2));
    setJsonError(null);
    setValidationErrors({});
  }, []);

  const handleClearHistory = useCallback(() => {
    setHistory([]);
    saveHistory([]);
  }, []);

  return (
    <div className="w-[480px] flex-shrink-0 bg-[var(--bg-elevated)] border border-[var(--separator)] rounded-[var(--radius-lg)] overflow-hidden flex flex-col max-h-[calc(100vh-220px)] animate-fade-in">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--separator)]">
        <span className="text-sm font-semibold text-[var(--text-primary)]" style={{ fontFamily: "var(--font-heading)" }}>
          {t("dashboard.mcpServers.playground.title")}
        </span>
        <div className="flex items-center gap-2">
          {history.length > 0 && (
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {t("dashboard.mcpServers.playground.history")}: {history.length}
            </span>
          )}
          <button
            onClick={onClose}
            className="p-1 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>

      {/* Server / Tool selector */}
      <div className="px-4 py-3 border-b border-[var(--separator)] bg-[var(--bg-window)]">
        <div className="flex gap-2 mb-2">
          <div className="flex-1">
            <label className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)] block mb-1">Server</label>
            <select
              value={serverName}
              onChange={(e) => handleServerChange(e.target.value)}
              className="w-full text-xs bg-[var(--bg-elevated)] border border-[var(--border-subtle)] rounded-[var(--radius-sm)] px-2 py-1.5 text-[var(--text-primary)]"
            >
              {servers.map((s) => (
                <option key={s.name} value={s.name}>{s.name}</option>
              ))}
            </select>
          </div>
          <div className="flex-1">
            <label className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)] block mb-1">Tool</label>
            <select
              value={toolName}
              onChange={(e) => handleToolChange(e.target.value)}
              className="w-full text-xs bg-[var(--bg-elevated)] border border-[var(--border-subtle)] rounded-[var(--radius-sm)] px-2 py-1.5 text-[var(--text-primary)]"
            >
              {(selectedServer?.tools ?? []).map((tool) => (
                <option key={tool.name} value={tool.name}>{tool.name}</option>
              ))}
            </select>
          </div>
        </div>
        {selectedTool && (
          <p className="text-[11px] text-[var(--text-tertiary)] leading-relaxed line-clamp-2">
            {selectedTool.description}
          </p>
        )}
      </div>

      {/* Parameters */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        <div className="flex justify-between items-center mb-3">
          <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)]">
            {t("dashboard.mcpServers.playground.parameters")}
          </span>
          <div className="flex gap-0.5 bg-[var(--bg-window)] rounded-[var(--radius-sm)] p-0.5 border border-[var(--border-subtle)]">
            <button
              onClick={() => switchMode("form")}
              className={cn(
                "px-2.5 py-1 text-[11px] rounded-[var(--radius-xs)] transition-colors cursor-pointer",
                mode === "form"
                  ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] font-semibold shadow-sm"
                  : "text-[var(--text-tertiary)]"
              )}
            >
              {t("dashboard.mcpServers.playground.formMode")}
            </button>
            <button
              onClick={() => switchMode("json")}
              className={cn(
                "px-2.5 py-1 text-[11px] rounded-[var(--radius-xs)] transition-colors cursor-pointer",
                mode === "json"
                  ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] font-semibold shadow-sm"
                  : "text-[var(--text-tertiary)]"
              )}
            >
              {t("dashboard.mcpServers.playground.jsonMode")}
            </button>
          </div>
        </div>

        {mode === "form" ? (
          properties.length === 0 ? (
            <p className="text-xs text-[var(--text-tertiary)] italic py-2">
              {t("dashboard.mcpServers.playground.noParams")}
            </p>
          ) : (
            <div className="space-y-3">
              {properties.map((prop) => (
                <ParamFormField
                  key={prop.name}
                  prop={prop}
                  value={formValues[prop.name]}
                  onChange={(v) => {
                    setFormValues((prev) => ({ ...prev, [prop.name]: v }));
                    setValidationErrors((prev) => {
                      const next = { ...prev };
                      delete next[prop.name];
                      return next;
                    });
                  }}
                  error={validationErrors[prop.name]}
                />
              ))}
            </div>
          )
        ) : (
          <div>
            <textarea
              value={jsonText}
              onChange={(e) => {
                setJsonText(e.target.value);
                setJsonError(null);
              }}
              spellCheck={false}
              className={cn(
                "w-full h-40 text-xs font-mono bg-[var(--bg-window)] border rounded-[var(--radius-sm)] px-3 py-2 text-[var(--text-primary)] resize-y",
                jsonError ? "border-[var(--error)]" : "border-[var(--border-subtle)]"
              )}
            />
            {jsonError && (
              <p className="text-[11px] text-[var(--error)] mt-1">{jsonError}</p>
            )}
          </div>
        )}

        {/* Execute */}
        <button
          onClick={handleExecute}
          disabled={invokeMut.isPending || !selectedTool}
          className="w-full mt-4 flex items-center justify-center gap-2 py-2.5 rounded-[var(--radius-md)] text-sm font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
        >
          {invokeMut.isPending ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              {t("dashboard.mcpServers.playground.executing")}
            </>
          ) : (
            <>
              <Play className="h-4 w-4" />
              {t("dashboard.mcpServers.playground.execute")}
            </>
          )}
        </button>
      </div>

      {/* Result */}
      {lastResult && (
        <div className="border-t-2 border-[var(--accent)] bg-[var(--bg-window)]">
          <div className="flex items-center justify-between px-4 py-2 border-b border-[var(--separator)]">
            <div className="flex items-center gap-2">
              <span className={cn(
                "inline-block w-2 h-2 rounded-full",
                lastResult.success ? "bg-[var(--success)]" : "bg-[var(--error)]"
              )} />
              <span className="text-xs font-semibold text-[var(--text-primary)]">
                {lastResult.success
                  ? t("dashboard.mcpServers.playground.success")
                  : t("dashboard.mcpServers.playground.error")}
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">{lastResult.latencyMs}ms</span>
            </div>
            <button
              onClick={handleCopy}
              className="flex items-center gap-1 px-2 py-1 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
            >
              {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
              {copied ? t("dashboard.mcpServers.playground.copied") : t("dashboard.mcpServers.playground.copy")}
            </button>
          </div>
          <pre className="px-4 py-3 text-[12px] font-mono text-[var(--text-secondary)] max-h-[200px] overflow-auto leading-relaxed whitespace-pre-wrap break-all">
            {lastResult.success
              ? JSON.stringify(lastResult.result, null, 2)
              : lastResult.error}
          </pre>
        </div>
      )}

      {/* History */}
      {history.length > 0 && (
        <div className="border-t border-[var(--separator)]">
          <button
            onClick={() => setShowHistory(!showHistory)}
            className="w-full flex items-center justify-between px-4 py-2 text-[11px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          >
            <div className="flex items-center gap-1">
              {showHistory ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
              <span className="font-medium uppercase tracking-wider">
                {t("dashboard.mcpServers.playground.history")} ({history.length})
              </span>
            </div>
            {showHistory && (
              <span
                onClick={(e) => { e.stopPropagation(); handleClearHistory(); }}
                className="flex items-center gap-1 text-[var(--text-tertiary)] hover:text-[var(--error)] cursor-pointer"
              >
                <Trash2 className="h-3 w-3" />
                {t("dashboard.mcpServers.playground.historyClear")}
              </span>
            )}
          </button>
          {showHistory && (
            <div className="max-h-[200px] overflow-y-auto">
              {history.map((entry) => (
                <div
                  key={entry.id}
                  onClick={() => handleLoadHistory(entry)}
                  className="flex items-center justify-between px-4 py-2 hover:bg-[var(--bg-hover)] transition-colors cursor-pointer border-t border-[var(--separator)]"
                >
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    <span className={cn(
                      "inline-block w-1.5 h-1.5 rounded-full flex-shrink-0",
                      entry.success ? "bg-[var(--success)]" : "bg-[var(--error)]"
                    )} />
                    <span className="text-[11px] font-mono font-semibold text-[var(--text-primary)] truncate">
                      {entry.toolName}
                    </span>
                    <span className="text-[10px] text-[var(--text-tertiary)] truncate">
                      {JSON.stringify(entry.arguments).slice(0, 40)}
                    </span>
                  </div>
                  <span className="text-[10px] text-[var(--text-tertiary)] flex-shrink-0 ml-2">
                    {entry.latencyMs}ms · {relativeTime(entry.timestamp)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// ParamFormField
// ---------------------------------------------------------------------------

function ParamFormField({ prop, value, onChange, error }: {
  prop: {
    name: string;
    type: string;
    description: string;
    required: boolean;
    enumValues?: string[];
    default?: unknown;
  };
  value: unknown;
  onChange: (v: unknown) => void;
  error?: string;
}) {
  if (prop.enumValues) {
    return (
      <div>
        <label className="text-xs font-semibold text-[var(--text-primary)] block mb-1">
          {prop.name}
          {prop.required && <span className="text-[var(--error)] ml-0.5">*</span>}
        </label>
        <select
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          className="text-xs bg-[var(--bg-window)] border border-[var(--border-subtle)] rounded-[var(--radius-sm)] px-2 py-1.5 text-[var(--text-primary)]"
        >
          <option value="">—</option>
          {prop.enumValues.map((v) => (
            <option key={v} value={v}>{v}</option>
          ))}
        </select>
        <span className="text-[10px] text-[var(--text-tertiary)] block mt-0.5">{prop.description}</span>
        {error && <span className="text-[10px] text-[var(--error)]">{error}</span>}
      </div>
    );
  }

  if (prop.type === "boolean") {
    return (
      <div>
        <label className="text-xs font-semibold text-[var(--text-primary)] block mb-1">
          {prop.name}
          {prop.required && <span className="text-[var(--error)] ml-0.5">*</span>}
        </label>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => onChange(!value)}
            className={cn(
              "w-9 h-5 rounded-full relative transition-colors cursor-pointer",
              value ? "bg-[var(--accent)]" : "bg-[var(--border-subtle)]"
            )}
          >
            <div className={cn(
              "w-4 h-4 rounded-full bg-white absolute top-0.5 transition-all",
              value ? "left-[18px]" : "left-0.5"
            )} />
          </button>
          <span className="text-xs text-[var(--text-secondary)]">{String(value ?? false)}</span>
        </div>
        <span className="text-[10px] text-[var(--text-tertiary)] block mt-0.5">{prop.description}</span>
      </div>
    );
  }

  if (prop.type === "number" || prop.type === "integer") {
    return (
      <div>
        <label className="text-xs font-semibold text-[var(--text-primary)] block mb-1">
          {prop.name}
          {prop.required && <span className="text-[var(--error)] ml-0.5">*</span>}
        </label>
        <input
          type="number"
          value={value !== undefined && value !== null ? String(value) : ""}
          onChange={(e) => {
            const num = prop.type === "integer" ? parseInt(e.target.value, 10) : parseFloat(e.target.value);
            onChange(isNaN(num) ? "" : num);
          }}
          className={cn(
            "w-32 text-xs bg-[var(--bg-window)] border rounded-[var(--radius-sm)] px-2 py-1.5 text-[var(--text-primary)]",
            error ? "border-[var(--error)]" : "border-[var(--border-subtle)]"
          )}
        />
        <span className="text-[10px] text-[var(--text-tertiary)] block mt-0.5">
          {prop.type} — {prop.description}
        </span>
        {error && <span className="text-[10px] text-[var(--error)]">{error}</span>}
      </div>
    );
  }

  if (prop.type === "object" || prop.type === "array") {
    const stringVal = typeof value === "string" ? value : JSON.stringify(value ?? (prop.type === "array" ? [] : {}), null, 2);
    return (
      <div>
        <label className="text-xs font-semibold text-[var(--text-primary)] block mb-1">
          {prop.name}
          {prop.required && <span className="text-[var(--error)] ml-0.5">*</span>}
        </label>
        <textarea
          value={stringVal}
          onChange={(e) => {
            try {
              onChange(JSON.parse(e.target.value));
            } catch {
              onChange(e.target.value);
            }
          }}
          spellCheck={false}
          className={cn(
            "w-full h-20 text-xs font-mono bg-[var(--bg-window)] border rounded-[var(--radius-sm)] px-2 py-1.5 text-[var(--text-primary)] resize-y",
            error ? "border-[var(--error)]" : "border-[var(--border-subtle)]"
          )}
        />
        <span className="text-[10px] text-[var(--text-tertiary)] block mt-0.5">
          {prop.type} — {prop.description}
        </span>
        {error && <span className="text-[10px] text-[var(--error)]">{error}</span>}
      </div>
    );
  }

  // Default: string
  return (
    <div>
      <label className="text-xs font-semibold text-[var(--text-primary)] block mb-1">
        {prop.name}
        {prop.required && <span className="text-[var(--error)] ml-0.5">*</span>}
      </label>
      <input
        type="text"
        value={String(value ?? "")}
        onChange={(e) => onChange(e.target.value)}
        placeholder={prop.description}
        className={cn(
          "w-full text-xs bg-[var(--bg-window)] border rounded-[var(--radius-sm)] px-2 py-1.5 text-[var(--text-primary)]",
          error ? "border-[var(--error)]" : "border-[var(--border-subtle)]"
        )}
      />
      <span className="text-[10px] text-[var(--text-tertiary)] block mt-0.5">
        {prop.type} — {prop.description}
      </span>
      {error && <span className="text-[10px] text-[var(--error)]">{error}</span>}
    </div>
  );
}
