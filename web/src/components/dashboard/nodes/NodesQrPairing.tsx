import { useTranslation } from "react-i18next";
import { QrCode, Clock, Copy, RotateCw } from "lucide-react";
import { SectionCard, SectionHeader } from "../shared";
import { cn } from "../../../lib/cn";

interface QrData {
  qr_svg: string;
  setup_code: string;
  gateway_url: string;
  bootstrap_token: string;
  ttl_ms: number;
}

interface NodesQrPairingProps {
  qrData: QrData | null;
  qrLoading: boolean;
  qrExpiry: number;
  onGenerateQr: () => void;
  onCopySetupCode: () => void;
}

function formatExpiry(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export default function NodesQrPairing({
  qrData,
  qrLoading,
  qrExpiry,
  onGenerateQr,
  onCopySetupCode,
}: NodesQrPairingProps) {
  const { t } = useTranslation();

  return (
    <SectionCard>
      <SectionHeader
        icon={<QrCode className="h-4 w-4" />}
        title={t("nodes.devicePairing")}
        right={
          <button
            onClick={onGenerateQr}
            disabled={qrLoading}
            className={cn(
              "inline-flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium transition-colors",
              "bg-[var(--accent)] text-white hover:opacity-90",
              qrLoading && "opacity-50 cursor-not-allowed",
            )}
          >
            {qrLoading ? (
              <RotateCw className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <QrCode className="h-3.5 w-3.5" />
            )}
            {qrData ? t("nodes.regenerateQr") : t("nodes.generateQr")}
          </button>
        }
      />

      {qrData ? (
        <div className="mt-4 flex flex-col sm:flex-row items-start gap-6">
          {/* QR Code SVG */}
          <div className="flex-shrink-0 p-4 rounded-[var(--radius-lg)] bg-white border border-[var(--border-subtle)]">
            <div
              className="w-[200px] h-[200px]"
              dangerouslySetInnerHTML={{ __html: qrData.qr_svg }}
            />
          </div>

          {/* Pairing Info */}
          <div className="flex flex-col gap-3 flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <Clock className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
              <span
                className={cn(
                  "text-[12px] font-mono font-medium",
                  qrExpiry < 60
                    ? "text-[var(--error)]"
                    : "text-[var(--text-secondary)]",
                )}
              >
                {t("nodes.expiresIn", { time: formatExpiry(qrExpiry) })}
              </span>
            </div>

            <div className="space-y-2">
              <label className="text-[11px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider">
                {t("nodes.setupCode")}
              </label>
              <div className="flex items-center gap-2">
                <code className="flex-1 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-secondary)] border border-[var(--border-subtle)] text-[11px] font-mono text-[var(--text-secondary)] truncate select-all">
                  {qrData.setup_code}
                </code>
                <button
                  onClick={onCopySetupCode}
                  className="p-2 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <Copy className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-[11px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider">
                {t("nodes.gatewayUrl")}
              </label>
              <span className="text-[12px] font-mono text-[var(--text-secondary)]">
                {qrData.gateway_url}
              </span>
            </div>

            <p className="text-[11px] text-[var(--text-tertiary)] leading-relaxed mt-1">
              {t("nodes.qrHint")}
            </p>
          </div>
        </div>
      ) : (
        <div className="mt-4 flex flex-col items-center justify-center py-8 text-center">
          <QrCode className="h-10 w-10 text-[var(--text-tertiary)] opacity-40 mb-3" />
          <p className="text-[13px] text-[var(--text-secondary)]">
            {t("nodes.qrDescription")}
          </p>
        </div>
      )}
    </SectionCard>
  );
}
