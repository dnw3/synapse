import { useTranslation } from "react-i18next";

interface Props {
  screenshot: string | null;
}

export default function BrowserView({ screenshot }: Props) {
  const { t } = useTranslation();
  if (!screenshot) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-2 text-sm text-[var(--text-secondary)]">
        <p>{t("browser.noSession")}</p>
        <p className="text-xs text-[var(--text-tertiary)] text-center max-w-xs">{t("browser.screenshotHint")}</p>
      </div>
    );
  }

  return (
    <div className="h-full flex items-center justify-center bg-[var(--bg-window)] p-3">
      <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-content)] overflow-hidden shadow-[var(--shadow-md)] max-w-full max-h-full flex items-center justify-center">
        <img
          src={`data:image/png;base64,${screenshot}`}
          alt="Browser screenshot"
          className="max-w-full max-h-full object-contain rounded-[var(--radius-md)]"
        />
      </div>
    </div>
  );
}
