import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Wand2, X, CheckCircle2, AlertCircle } from "lucide-react";
import { cn } from "../lib/cn";

interface WizardStep {
  session_id: string;
  step_id: string;
  title: string;
  description?: string;
  input_type: "text" | "password" | "select";
  options?: string[];
  total_steps?: number;
  current_step?: number;
}

interface WizardDone {
  done: true;
  session_id: string;
}

export interface SetupWizardProps {
  open: boolean;
  onClose: () => void;
  onCall: <T = unknown>(method: string, params?: Record<string, unknown>) => Promise<T>;
}

export default function SetupWizard({ open, onClose, onCall }: SetupWizardProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<WizardStep | null>(null);
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);

  // Start wizard on open
  useEffect(() => {
    if (!open) return;
    setDone(false);
    setError(null);
    setValue("");
    setStep(null);
    setSessionId(null);

    setLoading(true);
    onCall<WizardStep>("wizard.start", { mode: "setup" })
      .then((s) => {
        setStep(s);
        setSessionId(s.session_id);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleNext = async () => {
    if (!step || !sessionId) return;
    setLoading(true);
    setError(null);
    try {
      const res = await onCall<WizardStep | WizardDone>("wizard.next", {
        session_id: sessionId,
        answer: { step_id: step.step_id, value },
      });
      if ((res as WizardDone).done) {
        setDone(true);
        setTimeout(() => { onClose(); }, 1500);
      } else {
        setStep(res as WizardStep);
        setValue("");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = async () => {
    if (sessionId) {
      try { await onCall("wizard.cancel", { session_id: sessionId }); } catch { /* ignore */ }
    }
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && step?.input_type !== "select") handleNext();
    if (e.key === "Escape") handleCancel();
  };

  if (!open) return null;

  const totalSteps = step?.total_steps ?? 1;
  const currentStep = step?.current_step ?? 1;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.45)" }}
      onClick={handleCancel}
    >
      <div className="absolute inset-0 backdrop-blur-sm" />

      <div
        className="relative w-full max-w-md mx-4 rounded-[var(--radius-xl)] shadow-2xl border border-[var(--border-subtle)] overflow-hidden"
        style={{ background: "var(--bg-elevated)" }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border-subtle)]">
          <div className="flex items-center gap-2.5">
            <Wand2 className="h-4 w-4 text-[var(--accent)]" />
            <span className="text-[14px] font-semibold text-[var(--text-primary)]">
              {t("wizard.title")}
            </span>
          </div>
          <button
            onClick={handleCancel}
            className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Step dots */}
        {totalSteps > 1 && (
          <div className="flex items-center justify-center gap-1.5 pt-4">
            {Array.from({ length: totalSteps }).map((_, i) => (
              <div
                key={i}
                className={cn(
                  "rounded-full transition-all",
                  i + 1 === currentStep
                    ? "w-4 h-2 bg-[var(--accent)]"
                    : i + 1 < currentStep
                      ? "w-2 h-2 bg-[var(--accent)]/50"
                      : "w-2 h-2 bg-[var(--border-subtle)]"
                )}
              />
            ))}
          </div>
        )}

        {/* Body */}
        <div className="px-5 py-5 space-y-4">
          {loading && !step && (
            <div className="flex items-center justify-center py-8">
              <div className="h-6 w-6 rounded-full border-2 border-[var(--accent)] border-t-transparent animate-spin" />
            </div>
          )}

          {done && (
            <div className="flex flex-col items-center gap-3 py-6">
              <CheckCircle2 className="h-10 w-10 text-[var(--success)]" />
              <span className="text-[14px] font-medium text-[var(--text-primary)]">
                {t("wizard.success")}
              </span>
            </div>
          )}

          {error && (
            <div className="flex items-center gap-2 p-3 rounded-[var(--radius-md)] bg-[var(--error)]/10 border border-[var(--error)]/20">
              <AlertCircle className="h-4 w-4 text-[var(--error)] flex-shrink-0" />
              <span className="text-[12px] text-[var(--error)]">{t("wizard.error")}: {error}</span>
            </div>
          )}

          {step && !done && (
            <>
              <div className="space-y-1">
                <h3 className="text-[15px] font-semibold text-[var(--text-primary)]">
                  {step.title}
                </h3>
                {step.description && (
                  <p className="text-[13px] text-[var(--text-secondary)] leading-relaxed">
                    {step.description}
                  </p>
                )}
              </div>

              {step.input_type === "select" && step.options ? (
                <select
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  autoFocus
                  className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors"
                >
                  <option value="">—</option>
                  {step.options.map((opt) => (
                    <option key={opt} value={opt}>{opt}</option>
                  ))}
                </select>
              ) : (
                <input
                  type={step.input_type === "password" ? "password" : "text"}
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  autoFocus
                  className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors"
                />
              )}
            </>
          )}
        </div>

        {/* Footer */}
        {step && !done && (
          <div className="flex items-center justify-between px-5 pb-5 gap-3">
            <button
              onClick={handleCancel}
              className="px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
            >
              {t("wizard.cancel")}
            </button>
            <button
              onClick={handleNext}
              disabled={loading || (!value && step.input_type !== "select")}
              className="flex items-center gap-1.5 px-4 py-1.5 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[12px] font-medium hover:brightness-110 active:scale-[0.97] transition-all cursor-pointer disabled:opacity-40"
            >
              {loading ? (
                <span className="h-3.5 w-3.5 rounded-full border-2 border-white border-t-transparent animate-spin" />
              ) : null}
              {t("wizard.next")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
