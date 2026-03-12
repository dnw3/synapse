import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ShieldAlert, Check, X, ShieldCheck } from "lucide-react";
import { Button } from "./ui/button";

interface ApprovalRequest {
  tool_name: string;
  args_preview: string;
  risk_level: string;
}

interface ApprovalDialogProps {
  request: ApprovalRequest;
  onRespond: (approved: boolean, allowAll?: boolean) => void;
}

const RISK_STYLES: Record<string, { accentVar: string }> = {
  High: { accentVar: "var(--warning)" },
  Critical: { accentVar: "var(--error)" },
};

export default function ApprovalDialog({ request, onRespond }: ApprovalDialogProps) {
  const { t } = useTranslation();
  const [responded, setResponded] = useState(false);

  const riskStyle = RISK_STYLES[request.risk_level] ?? RISK_STYLES.High;
  const accentColor = riskStyle.accentVar;

  const handleRespond = (approved: boolean, allowAll = false) => {
    if (responded) return;
    setResponded(true);
    onRespond(approved, allowAll);
  };

  if (responded) return null;

  return (
    <div
      className="mx-4 my-2 rounded-[var(--radius-lg)] p-4 animate-fade-slide-in"
      style={{
        background: "var(--bg-content)",
        borderLeft: `3px solid ${accentColor}`,
        border: `1px solid color-mix(in srgb, ${accentColor} 25%, transparent)`,
        borderLeftWidth: 3,
        borderLeftColor: accentColor,
      }}
    >
      {/* Header */}
      <div className="flex items-center gap-2 mb-3">
        <ShieldAlert className="h-5 w-5 flex-shrink-0" style={{ color: accentColor }} />
        <span className="font-semibold text-sm text-[var(--text-primary)]">
          {t("approval.title", "Tool Approval Required")}
        </span>
        <span
          className="ml-auto px-2 py-0.5 rounded text-xs font-medium"
          style={{
            background: `color-mix(in srgb, ${accentColor} 15%, transparent)`,
            color: accentColor,
          }}
        >
          {request.risk_level}
        </span>
      </div>

      {/* Tool info */}
      <div className="space-y-2 mb-4">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-[var(--text-tertiary)] uppercase tracking-wide">
            {t("approval.tool", "Tool")}
          </span>
          <code
            className="px-2 py-0.5 rounded text-sm font-mono text-[var(--text-primary)] border border-[var(--border-subtle)]"
            style={{ background: "var(--bg-window)" }}
          >
            {request.tool_name}
          </code>
        </div>
        {request.args_preview && request.args_preview !== "{}" && (
          <div>
            <span className="text-xs font-medium text-[var(--text-tertiary)] uppercase tracking-wide">
              {t("approval.args", "Arguments")}
            </span>
            <pre
              className="mt-1 p-2 rounded text-xs font-mono overflow-x-auto max-h-32 overflow-y-auto whitespace-pre-wrap break-all border border-[var(--border-subtle)] text-[var(--text-secondary)]"
              style={{ background: "var(--bg-window)" }}
            >
              {request.args_preview}
            </pre>
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 justify-end">
        <Button
          size="sm"
          variant="destructive"
          onClick={() => handleRespond(false)}
          className="gap-1.5"
        >
          <X className="h-3.5 w-3.5" />
          {t("approval.deny", "Deny")}
        </Button>
        <Button
          size="sm"
          variant="default"
          onClick={() => handleRespond(true)}
          className="gap-1.5"
        >
          <Check className="h-3.5 w-3.5" />
          {t("approval.allow", "Allow")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => handleRespond(true, true)}
          className="gap-1.5"
        >
          <ShieldCheck className="h-3.5 w-3.5" />
          {t("approval.allowAll", "Allow All")}
        </Button>
      </div>
    </div>
  );
}
