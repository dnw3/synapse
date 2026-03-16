import { useTranslation } from "react-i18next";

interface CanvasBlockProps {
  type: string;
  content: string;
  language?: string;
  attributes?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// DiagramCanvas — Mermaid diagram (pre.mermaid; actual render can be wired later)
// ---------------------------------------------------------------------------
function DiagramCanvas({ content }: { content: string }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden">
      <div className="px-3 py-1.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)] flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-[var(--accent)]" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {t("canvas.diagram")}
        </span>
      </div>
      <div className="p-4">
        <pre className="mermaid text-[13px] font-mono text-[var(--text-primary)] whitespace-pre-wrap break-words">
          {content}
        </pre>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// FormCanvas — renders form fields from JSON content
// ---------------------------------------------------------------------------
interface FormField {
  name: string;
  label?: string;
  type?: string;
  placeholder?: string;
  required?: boolean;
  options?: string[];
}

function FormCanvas({
  content,
  attributes,
}: {
  content: string;
  attributes?: Record<string, unknown>;
}) {
  const { t } = useTranslation();

  let fields: FormField[] = [];
  try {
    const parsed = JSON.parse(content);
    fields = Array.isArray(parsed) ? parsed : parsed.fields ?? [];
  } catch {
    // content is not valid JSON — show raw
  }

  const title =
    (attributes?.title as string | undefined) ??
    (attributes?.form_title as string | undefined);

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden">
      <div className="px-3 py-1.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)] flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-[var(--accent)]" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {t("canvas.form")}
        </span>
        {title && (
          <span className="ml-1 text-[13px] font-medium text-[var(--text-primary)]">
            {title}
          </span>
        )}
      </div>
      <div className="p-4">
        {fields.length === 0 ? (
          <pre className="text-[13px] font-mono text-[var(--text-tertiary)] whitespace-pre-wrap break-words">
            {content}
          </pre>
        ) : (
          <div className="flex flex-col gap-4">
            {fields.map((field, i) => {
              const labelText = field.label ?? field.name;
              const inputId = `canvas-form-field-${i}`;
              if (field.type === "select" && field.options) {
                return (
                  <div key={i} className="flex flex-col gap-1.5">
                    <label
                      htmlFor={inputId}
                      className="text-[13px] font-medium text-[var(--text-primary)]"
                    >
                      {labelText}
                      {field.required && (
                        <span className="text-[var(--error)] ml-0.5">*</span>
                      )}
                    </label>
                    <select
                      id={inputId}
                      className="px-3 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-window)] text-[13px] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
                    >
                      {field.options.map((opt, j) => (
                        <option key={j} value={opt}>
                          {opt}
                        </option>
                      ))}
                    </select>
                  </div>
                );
              }
              if (field.type === "textarea") {
                return (
                  <div key={i} className="flex flex-col gap-1.5">
                    <label
                      htmlFor={inputId}
                      className="text-[13px] font-medium text-[var(--text-primary)]"
                    >
                      {labelText}
                      {field.required && (
                        <span className="text-[var(--error)] ml-0.5">*</span>
                      )}
                    </label>
                    <textarea
                      id={inputId}
                      placeholder={field.placeholder}
                      rows={3}
                      className="px-3 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-window)] text-[13px] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] transition-colors resize-none"
                    />
                  </div>
                );
              }
              return (
                <div key={i} className="flex flex-col gap-1.5">
                  <label
                    htmlFor={inputId}
                    className="text-[13px] font-medium text-[var(--text-primary)]"
                  >
                    {labelText}
                    {field.required && (
                      <span className="text-[var(--error)] ml-0.5">*</span>
                    )}
                  </label>
                  <input
                    id={inputId}
                    type={field.type ?? "text"}
                    placeholder={field.placeholder}
                    className="px-3 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-window)] text-[13px] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// TableCanvas — renders data table with headers + rows
// ---------------------------------------------------------------------------
interface TableData {
  headers?: string[];
  rows?: unknown[][];
  columns?: string[];
  data?: unknown[][];
}

function TableCanvas({ content }: { content: string }) {
  const { t } = useTranslation();

  let parsed: TableData | null = null;
  try {
    parsed = JSON.parse(content) as TableData;
  } catch {
    // not JSON
  }

  const headers: string[] = parsed?.headers ?? parsed?.columns ?? [];
  const rows: unknown[][] = parsed?.rows ?? parsed?.data ?? [];

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden">
      <div className="px-3 py-1.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)] flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-[var(--accent)]" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {t("canvas.table")}
        </span>
      </div>
      {parsed && headers.length > 0 ? (
        <div className="overflow-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--separator)]">
                {headers.map((h, i) => (
                  <th
                    key={i}
                    className="h-9 px-3 text-left text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)] bg-[var(--bg-grouped)]"
                  >
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, ri) => (
                <tr
                  key={ri}
                  className="border-b border-[var(--border-subtle)] last:border-b-0 even:bg-[var(--bg-hover)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  {(Array.isArray(row) ? row : []).map((cell, ci) => (
                    <td
                      key={ci}
                      className="px-3 py-2 text-[13px] text-[var(--text-primary)]"
                    >
                      {String(cell ?? "")}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="p-4">
          <pre className="text-[13px] font-mono text-[var(--text-tertiary)] whitespace-pre-wrap break-words">
            {content}
          </pre>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// PlotCanvas — placeholder for Recharts integration; shows JSON data preview
// ---------------------------------------------------------------------------
function PlotCanvas({ content }: { content: string }) {
  const { t } = useTranslation();

  let preview = content;
  try {
    const parsed = JSON.parse(content);
    preview = JSON.stringify(parsed, null, 2);
  } catch {
    // keep raw
  }

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden">
      <div className="px-3 py-1.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)] flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-[var(--accent)]" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {t("canvas.plot")}
        </span>
      </div>
      <div className="p-4 flex flex-col items-center gap-3">
        {/* Chart placeholder area */}
        <div className="w-full h-40 rounded-[var(--radius-md)] border border-dashed border-[var(--border-subtle)] bg-[var(--bg-grouped)] flex items-center justify-center">
          <div className="flex flex-col items-center gap-2 text-[var(--text-tertiary)]">
            {/* Simple bar chart icon using divs */}
            <div className="flex items-end gap-1 h-8">
              <span className="w-3 rounded-t bg-[var(--accent)]/40" style={{ height: "40%" }} />
              <span className="w-3 rounded-t bg-[var(--accent)]/50" style={{ height: "70%" }} />
              <span className="w-3 rounded-t bg-[var(--accent)]/60" style={{ height: "55%" }} />
              <span className="w-3 rounded-t bg-[var(--accent)]/70" style={{ height: "90%" }} />
              <span className="w-3 rounded-t bg-[var(--accent)]/50" style={{ height: "65%" }} />
            </div>
            <span className="text-[11px]">{t("canvas.plot")}</span>
          </div>
        </div>
        <pre className="w-full text-[11px] font-mono text-[var(--text-tertiary)] bg-[var(--bg-grouped)] rounded-[var(--radius-md)] px-3 py-2 max-h-28 overflow-auto whitespace-pre-wrap break-words border border-[var(--border-subtle)]">
          {preview}
        </pre>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// CardCanvas — styled card with title, description, optional image
// ---------------------------------------------------------------------------
function CardCanvas({
  content,
  attributes,
}: {
  content: string;
  attributes?: Record<string, unknown>;
}) {
  const { t } = useTranslation();

  // Accept data from content (JSON) or from attributes
  let title = (attributes?.title as string | undefined) ?? "";
  let description = (attributes?.description as string | undefined) ?? content;
  let imageUrl = (attributes?.image as string | undefined) ?? (attributes?.image_url as string | undefined);
  let tag = (attributes?.tag as string | undefined) ?? (attributes?.badge as string | undefined);

  try {
    const parsed = JSON.parse(content) as Record<string, unknown>;
    if (!title && parsed.title) title = String(parsed.title);
    if (parsed.description) description = String(parsed.description);
    if (!imageUrl && parsed.image) imageUrl = String(parsed.image);
    if (!imageUrl && parsed.image_url) imageUrl = String(parsed.image_url);
    if (!tag && parsed.tag) tag = String(parsed.tag);
    if (!tag && parsed.badge) tag = String(parsed.badge);
  } catch {
    // content is plain text — use as description
  }

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden"
      style={{ boxShadow: "var(--shadow-sm)" }}
    >
      <div className="px-3 py-1.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)] flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-[var(--accent)]" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {t("canvas.card")}
        </span>
      </div>
      {imageUrl && (
        <div className="w-full overflow-hidden bg-[var(--bg-grouped)]" style={{ maxHeight: 180 }}>
          <img
            src={imageUrl}
            alt={title || ""}
            className="w-full object-cover"
            style={{ maxHeight: 180 }}
          />
        </div>
      )}
      <div className="p-4 flex flex-col gap-2">
        <div className="flex items-start justify-between gap-2">
          {title && (
            <h3 className="text-[15px] font-semibold text-[var(--text-primary)] leading-snug">
              {title}
            </h3>
          )}
          {tag && (
            <span className="flex-shrink-0 px-2 py-0.5 rounded-full text-[11px] font-medium bg-[var(--accent-glow)] text-[var(--accent-light)] border border-[var(--accent)]/20">
              {tag}
            </span>
          )}
        </div>
        {description && (
          <p className="text-[13px] text-[var(--text-secondary)] leading-relaxed">
            {description}
          </p>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// FallbackCanvas — raw content in a code block for unknown types
// ---------------------------------------------------------------------------
function FallbackCanvas({ type, content }: { type: string; content: string }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden">
      <div className="px-3 py-1.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)] flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-[var(--text-tertiary)]" />
          <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
            {t("canvas.unknown")}
          </span>
        </div>
        <span className="text-[11px] font-mono text-[var(--text-tertiary)] bg-[var(--bg-grouped)] px-1.5 py-0.5 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]">
          {type}
        </span>
      </div>
      <div className="p-4">
        <p className="text-[11px] text-[var(--text-tertiary)] mb-2">{t("canvas.fallback")}</p>
        <pre className="text-[13px] font-mono text-[var(--text-primary)] bg-[var(--bg-grouped)] rounded-[var(--radius-md)] px-3 py-2 overflow-auto whitespace-pre-wrap break-words border border-[var(--border-subtle)]">
          {content}
        </pre>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// CanvasBlock — main dispatcher
// ---------------------------------------------------------------------------
export function CanvasBlock({ type, content, language: _language, attributes }: CanvasBlockProps) {
  switch (type) {
    case "diagram":
      return <DiagramCanvas content={content} />;
    case "form":
      return <FormCanvas content={content} attributes={attributes} />;
    case "table":
      return <TableCanvas content={content} />;
    case "plot":
      return <PlotCanvas content={content} />;
    case "card":
      return <CardCanvas content={content} attributes={attributes} />;
    default:
      return <FallbackCanvas type={type} content={content} />;
  }
}
