import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ShieldAlert, Check, X, ShieldCheck } from "lucide-react";
import { Button } from "./ui/button";
import { cn } from "../lib/cn";

interface ApprovalRequest {
  tool_name: string;
  args_preview: string;
  risk_level: string;
}

interface ApprovalDialogProps {
  request: ApprovalRequest;
  onRespond: (approved: boolean, allowAll?: boolean) => void;
}

const RISK_STYLES: Record<string, { bg: string; border: string; icon: string; badge: string }> = {
  High: {
    bg: "bg-amber-50 dark:bg-amber-950/30",
    border: "border-amber-300 dark:border-amber-700",
    icon: "text-amber-600 dark:text-amber-400",
    badge: "bg-amber-100 text-amber-800 dark:bg-amber-900/50 dark:text-amber-300",
  },
  Critical: {
    bg: "bg-red-50 dark:bg-red-950/30",
    border: "border-red-300 dark:border-red-700",
    icon: "text-red-600 dark:text-red-400",
    badge: "bg-red-100 text-red-800 dark:bg-red-900/50 dark:text-red-300",
  },
};

export default function ApprovalDialog({ request, onRespond }: ApprovalDialogProps) {
  const { t } = useTranslation();
  const [responded, setResponded] = useState(false);

  const style = RISK_STYLES[request.risk_level] ?? RISK_STYLES.High;

  const handleRespond = (approved: boolean, allowAll = false) => {
    if (responded) return;
    setResponded(true);
    onRespond(approved, allowAll);
  };

  if (responded) return null;

  return (
    <div
      className={cn(
        "mx-4 my-2 rounded-lg border-2 p-4 shadow-lg animate-in slide-in-from-bottom-2",
        style.bg,
        style.border
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-2 mb-3">
        <ShieldAlert className={cn("h-5 w-5", style.icon)} />
        <span className="font-semibold text-sm">
          {t("approval.title", "Tool Approval Required")}
        </span>
        <span className={cn("ml-auto px-2 py-0.5 rounded text-xs font-medium", style.badge)}>
          {request.risk_level}
        </span>
      </div>

      {/* Tool info */}
      <div className="space-y-2 mb-4">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            {t("approval.tool", "Tool")}
          </span>
          <code className="px-2 py-0.5 rounded bg-background/80 text-sm font-mono border">
            {request.tool_name}
          </code>
        </div>
        {request.args_preview && request.args_preview !== "{}" && (
          <div>
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              {t("approval.args", "Arguments")}
            </span>
            <pre className="mt-1 p-2 rounded bg-background/80 border text-xs font-mono overflow-x-auto max-h-32 overflow-y-auto whitespace-pre-wrap break-all">
              {request.args_preview}
            </pre>
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2">
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
          variant="destructive"
          onClick={() => handleRespond(false)}
          className="gap-1.5"
        >
          <X className="h-3.5 w-3.5" />
          {t("approval.deny", "Deny")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => handleRespond(true, true)}
          className="gap-1.5 ml-auto"
        >
          <ShieldCheck className="h-3.5 w-3.5" />
          {t("approval.allowAll", "Allow All")}
        </Button>
      </div>
    </div>
  );
}
