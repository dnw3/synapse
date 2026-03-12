import { useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import {
  BarChart, Bar, LineChart, Line, PieChart, Pie, Cell,
  ScatterChart, Scatter, XAxis, YAxis, CartesianGrid, Tooltip as RTooltip,
  ResponsiveContainer, Legend,
} from "recharts";
import {
  Trash2,
  Download,
  Code2,
  FileText,
  BarChart2,
  WrapText,
  ClipboardList,
} from "lucide-react";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";
import type { CanvasBlock, FormBlockMeta, FormFieldSchema } from "../types/canvas";

// ---------------------------------------------------------------------------
// Block renderers
// ---------------------------------------------------------------------------

function CodeBlock({ block }: { block: CanvasBlock }) {
  const { t } = useTranslation();
  const lang = block.language ?? "text";
  return (
    <div className="rounded-[var(--radius-lg)] overflow-hidden border border-[var(--border-subtle)] bg-[var(--bg-content)]">
      <div className="flex items-center justify-between px-3 py-1.5 bg-[var(--bg-grouped)] border-b border-[var(--border-subtle)]">
        <span className="text-xs font-mono font-medium text-[var(--text-secondary)]">{lang}</span>
        <button
          onClick={() => navigator.clipboard.writeText(block.content)}
          className="text-[10px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors"
        >
          {t("canvas.copy")}
        </button>
      </div>
      <SyntaxHighlighter
        style={oneDark}
        language={lang}
        PreTag="div"
        className="!rounded-none !text-xs !m-0 !border-0"
        customStyle={{ background: "var(--bg-content)", margin: 0 }}
      >
        {block.content}
      </SyntaxHighlighter>
    </div>
  );
}

function MarkdownBlock({ block }: { block: CanvasBlock }) {
  return (
    <div className="synapse-prose prose prose-sm max-w-none prose-p:leading-relaxed prose-pre:bg-[var(--bg-content)] prose-pre:border prose-pre:border-[var(--border-subtle)] px-1">
      <ReactMarkdown>{block.content}</ReactMarkdown>
    </div>
  );
}

function TextBlock({ block }: { block: CanvasBlock }) {
  return (
    <p className="text-sm text-[var(--text-primary)] leading-relaxed whitespace-pre-wrap">{block.content}</p>
  );
}

const CHART_COLORS = [
  "var(--chart-1)", "var(--chart-2)", "var(--chart-3)", "var(--chart-4)",
  "var(--chart-5)", "var(--chart-6)", "var(--chart-7)", "var(--chart-8)",
];

interface ChartData {
  labels?: string[];
  data?: number[];
  datasets?: Array<{ label?: string; data: number[] }>;
  items?: Array<Record<string, unknown>>;
}

function ChartBlock({ block }: { block: CanvasBlock }) {
  const { t } = useTranslation();
  let parsed: ChartData | null = null;
  try {
    parsed = JSON.parse(block.content) as ChartData;
  } catch {
    // fall through
  }
  const meta = block.metadata as { title?: string; chartType?: string; xLabel?: string; yLabel?: string } | undefined;
  const chartType = meta?.chartType ?? "bar";

  if (!parsed) {
    return (
      <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] p-4">
        <p className="text-xs text-[var(--error)]">{t("canvas.invalidChartData")}</p>
        <pre className="text-[10px] text-[var(--text-tertiary)] mt-2 overflow-auto max-h-20">{block.content}</pre>
      </div>
    );
  }

  let chartData: Record<string, unknown>[] = [];
  let dataKeys: string[] = ["value"];

  if (parsed.items && Array.isArray(parsed.items)) {
    chartData = parsed.items;
    const sample = parsed.items[0];
    if (sample) {
      dataKeys = Object.keys(sample).filter((k) => k !== "name" && k !== "label" && typeof sample[k] === "number");
    }
  } else if (parsed.labels && parsed.data) {
    chartData = parsed.labels.map((label, i) => ({ name: label, value: parsed!.data![i] ?? 0 }));
  } else if (parsed.labels && parsed.datasets) {
    chartData = parsed.labels.map((label, i) => {
      const point: Record<string, unknown> = { name: label };
      for (const ds of parsed!.datasets!) {
        point[ds.label ?? `series${parsed!.datasets!.indexOf(ds)}`] = ds.data[i] ?? 0;
      }
      return point;
    });
    dataKeys = parsed.datasets.map((ds, i) => ds.label ?? `series${i}`);
  }

  const tooltipStyle = { background: "var(--bg-content)", border: "1px solid var(--separator)", borderRadius: "var(--radius-md)", fontSize: 11, color: "var(--text-primary)" };

  const renderChart = () => {
    switch (chartType) {
      case "line":
        return (
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" />
            <XAxis dataKey="name" tick={{ fill: "var(--chart-tick)", fontSize: 10 }} />
            <YAxis tick={{ fill: "var(--chart-tick)", fontSize: 10 }} />
            <RTooltip contentStyle={tooltipStyle} />
            <Legend wrapperStyle={{ fontSize: 10 }} />
            {dataKeys.map((key, i) => (
              <Line key={key} type="monotone" dataKey={key} stroke={CHART_COLORS[i % CHART_COLORS.length]} strokeWidth={2} dot={{ r: 3 }} />
            ))}
          </LineChart>
        );
      case "pie":
        return (
          <PieChart>
            <Pie data={chartData} dataKey={dataKeys[0]} nameKey="name" cx="50%" cy="50%" outerRadius={60} label={({ name }) => name as string}>
              {chartData.map((_, i) => (
                <Cell key={i} fill={CHART_COLORS[i % CHART_COLORS.length]} />
              ))}
            </Pie>
            <RTooltip contentStyle={tooltipStyle} />
            <Legend wrapperStyle={{ fontSize: 10 }} />
          </PieChart>
        );
      case "scatter":
        return (
          <ScatterChart>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" />
            <XAxis dataKey="x" tick={{ fill: "var(--chart-tick)", fontSize: 10 }} name={meta?.xLabel ?? "x"} />
            <YAxis dataKey="y" tick={{ fill: "var(--chart-tick)", fontSize: 10 }} name={meta?.yLabel ?? "y"} />
            <RTooltip contentStyle={tooltipStyle} />
            <Scatter data={chartData} fill={CHART_COLORS[0]} />
          </ScatterChart>
        );
      default: // bar
        return (
          <BarChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" />
            <XAxis dataKey="name" tick={{ fill: "var(--chart-tick)", fontSize: 10 }} />
            <YAxis tick={{ fill: "var(--chart-tick)", fontSize: 10 }} />
            <RTooltip contentStyle={tooltipStyle} />
            <Legend wrapperStyle={{ fontSize: 10 }} />
            {dataKeys.map((key, i) => (
              <Bar key={key} dataKey={key} fill={CHART_COLORS[i % CHART_COLORS.length]} radius={[4, 4, 0, 0]} />
            ))}
          </BarChart>
        );
    }
  };

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden">
      {meta?.title && (
        <div className="px-3 py-2 border-b border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)]">
          {meta.title}
        </div>
      )}
      <div className="p-2">
        <ResponsiveContainer width="100%" height={180}>
          {renderChart()}
        </ResponsiveContainer>
      </div>
    </div>
  );
}

function FormBlock({ block, onSubmit }: { block: CanvasBlock; onSubmit?: (blockId: string, values: Record<string, string | boolean>) => void }) {
  const { t } = useTranslation();
  let schema: FormBlockMeta | null = null;
  try {
    schema = JSON.parse(block.content) as FormBlockMeta;
  } catch {
    // fall through to error state
  }

  const [values, setValues] = useState<Record<string, string | boolean>>({});

  if (!schema || !Array.isArray(schema.fields)) {
    return (
      <div className="text-xs text-[var(--error)] px-1">{t("canvas.formInvalidSchema")}</div>
    );
  }

  const handleChange = (name: string, value: string | boolean) => {
    setValues((prev) => ({ ...prev, [name]: value }));
  };

  const renderField = (field: FormFieldSchema) => {
    const id = `canvas-form-${block.id}-${field.name}`;
    const labelEl = (
      <label htmlFor={id} className="text-xs text-[var(--text-secondary)] font-medium">
        {field.label ?? field.name}
        {field.required && <span className="text-[var(--error)] ml-0.5">*</span>}
      </label>
    );

    const inputClasses = "bg-[var(--bg-content)] border border-[var(--separator)] rounded-[var(--radius-sm)] px-2.5 py-1.5 text-xs text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent)]/30 focus:border-[var(--accent)]/40";

    if (field.type === "boolean") {
      return (
        <div key={field.name} className="flex items-center gap-2">
          <input
            id={id}
            type="checkbox"
            checked={Boolean(values[field.name] ?? field.defaultValue ?? false)}
            onChange={(e) => handleChange(field.name, e.target.checked)}
            className="h-3.5 w-3.5 accent-[var(--accent)]"
          />
          {labelEl}
        </div>
      );
    }

    if (field.type === "select" && Array.isArray(field.options)) {
      return (
        <div key={field.name} className="flex flex-col gap-1">
          {labelEl}
          <select
            id={id}
            value={String(values[field.name] ?? field.defaultValue ?? "")}
            onChange={(e) => handleChange(field.name, e.target.value)}
            className={inputClasses}
          >
            <option value="">{t("canvas.formSelectPlaceholder")}</option>
            {field.options.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
        </div>
      );
    }

    return (
      <div key={field.name} className="flex flex-col gap-1">
        {labelEl}
        <input
          id={id}
          type={field.type === "number" ? "number" : "text"}
          value={String(values[field.name] ?? field.defaultValue ?? "")}
          onChange={(e) => handleChange(field.name, e.target.value)}
          className={inputClasses}
        />
      </div>
    );
  };

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] p-4 space-y-3">
      {schema.fields.map(renderField)}
      <Button
        size="sm"
        className="mt-2 text-xs h-7"
        onClick={() => {
          onSubmit?.(block.id, values);
        }}
      >
        {schema.submitLabel ?? t("canvas.formSubmit")}
      </Button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Block header
// ---------------------------------------------------------------------------

const BLOCK_ICONS: Record<CanvasBlock["type"], React.ReactNode> = {
  code: <Code2 className="h-3.5 w-3.5" />,
  markdown: <FileText className="h-3.5 w-3.5" />,
  chart: <BarChart2 className="h-3.5 w-3.5" />,
  form: <ClipboardList className="h-3.5 w-3.5" />,
  text: <WrapText className="h-3.5 w-3.5" />,
};

function BlockWrapper({ block, onFormSubmit }: { block: CanvasBlock; onFormSubmit?: (blockId: string, values: Record<string, string | boolean>) => void }) {
  const ts = new Date(block.timestamp).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5 text-[10px] text-[var(--text-tertiary)]">
        {BLOCK_ICONS[block.type]}
        <span className="font-medium text-[var(--text-secondary)] capitalize">{block.type}</span>
        <span className="ml-auto">{ts}</span>
      </div>
      {block.type === "code" && <CodeBlock block={block} />}
      {block.type === "markdown" && <MarkdownBlock block={block} />}
      {block.type === "text" && <TextBlock block={block} />}
      {block.type === "chart" && <ChartBlock block={block} />}
      {block.type === "form" && <FormBlock block={block} onSubmit={onFormSubmit} />}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Export helpers
// ---------------------------------------------------------------------------

function exportBlocks(blocks: CanvasBlock[]) {
  const lines: string[] = [];
  for (const b of blocks) {
    lines.push(`--- ${b.type.toUpperCase()} (${new Date(b.timestamp).toISOString()}) ---`);
    lines.push(b.content);
    lines.push("");
  }
  const blob = new Blob([lines.join("\n")], { type: "text/plain" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `canvas-export-${Date.now()}.txt`;
  a.click();
  URL.revokeObjectURL(url);
}

// ---------------------------------------------------------------------------
// Canvas component — embedded in sidebar, no own collapse logic
// ---------------------------------------------------------------------------

interface CanvasProps {
  canvasBlocks: CanvasBlock[];
  onClear: () => void;
  onFormSubmit?: (blockId: string, values: Record<string, string | boolean>) => void;
}

export default function Canvas({ canvasBlocks, onClear, onFormSubmit }: CanvasProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* Toolbar */}
      {canvasBlocks.length > 0 && (
        <div className="flex items-center h-8 border-b border-[var(--border-subtle)] px-2.5 gap-1 flex-shrink-0">
          <span className="text-[10px] text-[var(--text-tertiary)] flex-1">{t("canvas.blocks", { count: canvasBlocks.length })}</span>
          <button
            onClick={() => exportBlocks(canvasBlocks)}
            title={t("canvas.export")}
            aria-label="Export canvas"
            className="p-1 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors rounded-[var(--radius-sm)]"
          >
            <Download className="h-3 w-3" />
          </button>
          <button
            onClick={onClear}
            title={t("canvas.clear")}
            aria-label="Clear canvas"
            className="p-1 text-[var(--text-tertiary)] hover:text-[var(--error)] transition-colors rounded-[var(--radius-sm)]"
          >
            <Trash2 className="h-3 w-3" />
          </button>
        </div>
      )}

      {/* Content */}
      <ScrollArea className="flex-1">
        {canvasBlocks.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 px-4 text-center gap-3">
            <div className="w-10 h-10 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] flex items-center justify-center">
              <BarChart2 className="h-5 w-5 text-[var(--text-tertiary)]" />
            </div>
            <p className="text-xs text-[var(--text-tertiary)] leading-relaxed">
              {t("canvas.emptyHint")}
              <br />
              {t("canvas.emptyHint2")}
            </p>
          </div>
        ) : (
          <div className="p-2.5 space-y-4">
            {canvasBlocks.map((block) => (
              <BlockWrapper key={block.id} block={block} onFormSubmit={onFormSubmit} />
            ))}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}
