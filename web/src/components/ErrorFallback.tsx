import { useTranslation } from "react-i18next";
import { AlertTriangle, RefreshCw } from "lucide-react";

interface ErrorFallbackProps {
  error: Error;
  resetErrorBoundary: () => void;
}

export default function ErrorFallback({ error, resetErrorBoundary }: ErrorFallbackProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 p-8">
      <AlertTriangle className="h-10 w-10 text-[var(--error)] opacity-60" />
      <p className="text-[16px] font-semibold text-[var(--text-secondary)]">
        {t("error.unexpected")}
      </p>
      <p className="text-[13px] text-[var(--text-tertiary)] text-center max-w-md">
        {error.message}
      </p>
      <button
        onClick={resetErrorBoundary}
        className="inline-flex items-center gap-2 px-4 py-2 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[13px] font-medium hover:brightness-110 transition-all cursor-pointer"
      >
        <RefreshCw className="h-3.5 w-3.5" />
        {t("error.retry")}
      </button>
    </div>
  );
}
