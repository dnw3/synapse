import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { X, Plus, Trash2, Loader2, CheckCircle2, AlertCircle } from "lucide-react";
import type { McpServerInfo, McpTestResult } from "../../types/dashboard";

interface McpServerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">) => Promise<void>;
  onTest: (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">) => Promise<McpTestResult | null>;
  editServer?: McpServerInfo | null;
}

type Transport = "stdio" | "sse" | "streamable-http";
type FlowState = "idle" | "testing" | "test-passed" | "test-failed" | "saving";

interface KVRow {
  key: string;
  value: string;
}

function recordToRows(rec?: Record<string, string> | null): KVRow[] {
  if (!rec) return [];
  return Object.entries(rec).map(([key, value]) => ({ key, value }));
}

function rowsToRecord(rows: KVRow[]): Record<string, string> | undefined {
  const filtered = rows.filter((r) => r.key.trim());
  if (filtered.length === 0) return undefined;
  return Object.fromEntries(filtered.map((r) => [r.key, r.value]));
}

/**
 * Wrapper that remounts the inner form whenever the modal opens or editServer changes,
 * so all form state resets naturally via initial useState values.
 */
export default function McpServerModal(props: McpServerModalProps) {
  const { isOpen, editServer, onClose } = props;
  if (!isOpen) return null;

  // Key forces remount when switching between add/edit or different servers
  const formKey = editServer ? `edit:${editServer.name}` : "add";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" onClick={onClose} />
      <McpServerForm key={formKey} {...props} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Inner form — remounted on open so useState initializers handle reset
// ---------------------------------------------------------------------------

function McpServerForm({ onClose, onSave, onTest, editServer }: McpServerModalProps) {
  const { t } = useTranslation();
  const isEdit = !!editServer;

  const [name, setName] = useState(editServer?.name ?? "");
  const [transport, setTransport] = useState<Transport>(editServer?.transport ?? "stdio");
  const [command, setCommand] = useState(editServer?.command ?? "");
  const [args, setArgs] = useState(editServer?.args?.join(" ") ?? "");
  const [url, setUrl] = useState(editServer?.url ?? "");
  const [envVars, setEnvVars] = useState<KVRow[]>(() => recordToRows(editServer?.env));
  const [headers, setHeaders] = useState<KVRow[]>(() => recordToRows(editServer?.headers));
  const [transient, setTransient] = useState(editServer?.transient ?? false);

  const [flowState, setFlowState] = useState<FlowState>("idle");
  const [testResult, setTestResult] = useState<McpTestResult | null>(null);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  const buildPayload = useCallback((): Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error"> => {
    const base = { name: name.trim(), transport, transient };
    if (transport === "stdio") {
      return {
        ...base,
        command: command.trim(),
        args: args.trim() ? args.trim().split(/\s+/) : [],
        env: rowsToRecord(envVars),
      };
    }
    return {
      ...base,
      url: url.trim(),
      headers: rowsToRecord(headers),
    };
  }, [name, transport, command, args, url, envVars, headers, transient]);

  const isValid = useCallback((): boolean => {
    if (!name.trim()) return false;
    if (transport === "stdio" && !command.trim()) return false;
    if ((transport === "sse" || transport === "streamable-http") && !url.trim()) return false;
    return true;
  }, [name, transport, command, url]);

  const handleTestAndSave = async () => {
    if (!isValid()) return;
    setFlowState("testing");
    setTestResult(null);
    try {
      const result = await onTest(buildPayload());
      setTestResult(result ?? null);
      if (result?.success) {
        setFlowState("test-passed");
        // Auto-save on success
        try {
          setFlowState("saving");
          await onSave(buildPayload());
          onClose();
        } catch {
          setFlowState("idle");
        }
      } else {
        setFlowState("test-failed");
      }
    } catch {
      setFlowState("test-failed");
    }
  };

  const handleSaveAnyway = async () => {
    if (!isValid()) return;
    setFlowState("saving");
    try {
      await onSave(buildPayload());
      onClose();
    } catch {
      setFlowState("idle");
    }
  };

  const updateKVRow = (
    rows: KVRow[],
    setRows: React.Dispatch<React.SetStateAction<KVRow[]>>,
    index: number,
    field: "key" | "value",
    val: string,
  ) => {
    setRows(rows.map((r, i) => (i === index ? { ...r, [field]: val } : r)));
  };

  const removeKVRow = (
    rows: KVRow[],
    setRows: React.Dispatch<React.SetStateAction<KVRow[]>>,
    index: number,
  ) => {
    setRows(rows.filter((_, i) => i !== index));
  };

  const isBusy = flowState === "testing" || flowState === "saving";

  return (
    <div className="relative bg-[var(--bg-elevated)] rounded-[var(--radius-lg)] border border-[var(--separator)] shadow-[var(--shadow-lg)] p-6 w-full max-w-lg animate-fade-in max-h-[85vh] overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-5">
        <h3
          className="text-base font-semibold text-[var(--text-primary)]"
          style={{ fontFamily: "var(--font-heading)" }}
        >
          {isEdit ? t("dashboard.mcpServers.editServer") : t("dashboard.mcpServers.addServer")}
        </h3>
        <button
          onClick={onClose}
          className="p-1 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {/* Name */}
      <FieldLabel label={t("dashboard.mcpServers.form.name")} />
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        disabled={isEdit}
        placeholder={t("dashboard.mcpServers.form.namePlaceholder")}
        className="w-full mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] outline-none focus:border-[var(--accent)] transition-colors disabled:opacity-50"
      />

      {/* Transport */}
      <FieldLabel label={t("dashboard.mcpServers.form.transport")} />
      <select
        value={transport}
        onChange={(e) => setTransport(e.target.value as Transport)}
        className="w-full mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-sm text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors cursor-pointer"
      >
        <option value="stdio">stdio</option>
        <option value="sse">sse</option>
        <option value="streamable-http">streamable-http</option>
      </select>

      {/* Transport-specific fields */}
      {transport === "stdio" ? (
        <>
          {/* Command */}
          <FieldLabel label={t("dashboard.mcpServers.form.command")} />
          <input
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder={t("dashboard.mcpServers.form.commandPlaceholder")}
            className="w-full mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] outline-none focus:border-[var(--accent)] transition-colors"
          />

          {/* Args */}
          <FieldLabel label={t("dashboard.mcpServers.form.args")} />
          <input
            type="text"
            value={args}
            onChange={(e) => setArgs(e.target.value)}
            placeholder={t("dashboard.mcpServers.form.argsPlaceholder")}
            className="w-full mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] outline-none focus:border-[var(--accent)] transition-colors font-mono text-[13px]"
          />

          {/* Env Vars */}
          <FieldLabel label={t("dashboard.mcpServers.form.envVars")} />
          <KVEditor
            rows={envVars}
            setRows={setEnvVars}
            updateRow={(i, f, v) => updateKVRow(envVars, setEnvVars, i, f, v)}
            removeRow={(i) => removeKVRow(envVars, setEnvVars, i)}
            addLabel={t("dashboard.mcpServers.form.addVariable")}
            keyPlaceholder={t("dashboard.mcpServers.form.key")}
            valuePlaceholder={t("dashboard.mcpServers.form.value")}
          />
        </>
      ) : (
        <>
          {/* URL */}
          <FieldLabel label={t("dashboard.mcpServers.form.url")} />
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={t("dashboard.mcpServers.form.urlPlaceholder")}
            className="w-full mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] outline-none focus:border-[var(--accent)] transition-colors font-mono text-[13px]"
          />

          {/* Headers */}
          <FieldLabel label={t("dashboard.mcpServers.form.headers")} />
          <KVEditor
            rows={headers}
            setRows={setHeaders}
            updateRow={(i, f, v) => updateKVRow(headers, setHeaders, i, f, v)}
            removeRow={(i) => removeKVRow(headers, setHeaders, i)}
            addLabel={t("dashboard.mcpServers.form.addHeader")}
            keyPlaceholder={t("dashboard.mcpServers.form.key")}
            valuePlaceholder={t("dashboard.mcpServers.form.value")}
          />
        </>
      )}

      {/* Transient checkbox */}
      <label className="flex items-center gap-2 mb-5 cursor-pointer select-none">
        <input
          type="checkbox"
          checked={transient}
          onChange={(e) => setTransient(e.target.checked)}
          className="rounded border-[var(--border-subtle)] accent-[var(--accent)]"
        />
        <span className="text-xs text-[var(--text-secondary)]">
          {t("dashboard.mcpServers.form.transient")}
        </span>
      </label>

      {/* Test result indicator */}
      {testResult && flowState === "test-failed" && (
        <div className="flex items-center gap-2 mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--error)]/8 border border-[var(--error)]/20">
          <AlertCircle className="h-4 w-4 text-[var(--error)] flex-shrink-0" />
          <span className="text-xs text-[var(--error)]">
            {testResult.error ?? t("dashboard.mcpServers.testFailed")}
          </span>
        </div>
      )}

      {flowState === "test-passed" && (
        <div className="flex items-center gap-2 mb-4 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--success)]/8 border border-[var(--success)]/20">
          <CheckCircle2 className="h-4 w-4 text-[var(--success)] flex-shrink-0" />
          <span className="text-xs text-[var(--success)]">
            {t("dashboard.mcpServers.testSuccess")}
          </span>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center justify-end gap-2 pt-2 border-t border-[var(--separator)]">
        <button
          onClick={onClose}
          disabled={isBusy}
          className="px-4 py-2 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
        >
          {t("dashboard.mcpServers.form.cancel")}
        </button>

        {flowState === "test-failed" && (
          <button
            onClick={handleSaveAnyway}
            className="px-4 py-2 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--warning)] border border-[var(--warning)]/30 hover:bg-[var(--warning)]/10 transition-colors cursor-pointer"
          >
            {t("dashboard.mcpServers.form.forceSave")}
          </button>
        )}

        <button
          onClick={handleTestAndSave}
          disabled={!isValid() || isBusy}
          className="flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] text-[12px] font-medium bg-[var(--accent)] text-white hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
        >
          {isBusy && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
          {t("dashboard.mcpServers.form.testAndSave")}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function FieldLabel({ label }: { label: string }) {
  return (
    <label className="block text-xs text-[var(--text-secondary)] mb-1.5 font-medium">
      {label}
    </label>
  );
}

function KVEditor({
  rows,
  setRows,
  updateRow,
  removeRow,
  addLabel,
  keyPlaceholder,
  valuePlaceholder,
}: {
  rows: KVRow[];
  setRows: React.Dispatch<React.SetStateAction<KVRow[]>>;
  updateRow: (index: number, field: "key" | "value", val: string) => void;
  removeRow: (index: number) => void;
  addLabel: string;
  keyPlaceholder: string;
  valuePlaceholder: string;
}) {
  return (
    <div className="mb-4">
      <div className="flex flex-col gap-2">
        {rows.map((row, i) => (
          <div key={i} className="flex items-center gap-2">
            <input
              type="text"
              value={row.key}
              onChange={(e) => updateRow(i, "key", e.target.value)}
              placeholder={keyPlaceholder}
              className="flex-1 px-2.5 py-1.5 rounded-[var(--radius-sm)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] outline-none focus:border-[var(--accent)] transition-colors font-mono"
            />
            <input
              type="text"
              value={row.value}
              onChange={(e) => updateRow(i, "value", e.target.value)}
              placeholder={valuePlaceholder}
              className="flex-1 px-2.5 py-1.5 rounded-[var(--radius-sm)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] outline-none focus:border-[var(--accent)] transition-colors font-mono"
            />
            <button
              onClick={() => removeRow(i)}
              className="p-1 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/10 transition-colors cursor-pointer"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </div>
      <button
        onClick={() => setRows((prev) => [...prev, { key: "", value: "" }])}
        className="flex items-center gap-1 mt-2 px-2 py-1 rounded-[var(--radius-sm)] text-[11px] font-medium text-[var(--accent)] hover:bg-[var(--accent)]/10 transition-colors cursor-pointer"
      >
        <Plus className="h-3 w-3" />
        {addLabel}
      </button>
    </div>
  );
}
