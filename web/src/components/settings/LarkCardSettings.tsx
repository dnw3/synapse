import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Save, Eye, Check } from "lucide-react";
import { cn } from "../../lib/cn";
import { SectionCard, SectionHeader, Toggle, useToast, ToastContainer } from "../dashboard/shared";

const COLOR_MAP: Record<string, string> = {
  blue: "#3370FF",
  wathet: "#4DC6E1",
  turquoise: "#34C3A8",
  green: "#34C724",
  yellow: "#FAA63E",
  orange: "#F77234",
  red: "#F54A45",
  carmine: "#E8445A",
  violet: "#7B67EE",
  purple: "#B64FD3",
  indigo: "#4658CC",
  grey: "#8F959E",
  default: "#1F2329",
};

interface LarkCardConfig {
  template_color?: string;
  header_title?: string;
  header_icon?: string;
  show_feedback?: boolean;
  show_timestamp?: boolean;
}

export default function LarkCardSettings() {
  const { t } = useTranslation();
  const { toasts, addToast } = useToast();

  const [config, setConfig] = useState<LarkCardConfig>({
    template_color: "blue",
    header_title: "",
    header_icon: "",
    show_feedback: false,
    show_timestamp: false,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [previewJson, setPreviewJson] = useState<string | null>(null);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch("/api/config/lark-card");
      if (res.ok) {
        const data = await res.json();
        setConfig(data);
      }
    } catch {
      // use defaults
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  const handleSave = async () => {
    setSaving(true);
    try {
      const res = await fetch("/api/config/lark-card", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
      });
      if (res.ok) {
        addToast(t("larkCard.saved"), "success");
      } else {
        addToast("Failed to save", "error");
      }
    } catch {
      addToast("Failed to save", "error");
    } finally {
      setSaving(false);
    }
  };

  const handlePreview = async () => {
    try {
      const res = await fetch("/api/config/lark-card/preview", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          sample_text: "**Hello!** This is a preview of the Lark card styling.\n\n- Item 1\n- Item 2\n\n> A quote block",
          config,
        }),
      });
      if (res.ok) {
        const data = await res.json();
        setPreviewJson(JSON.stringify(data, null, 2));
      } else {
        addToast("Preview failed", "error");
      }
    } catch {
      addToast("Preview failed", "error");
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="h-5 w-5 rounded-full border-2 border-[var(--accent)] border-t-transparent animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-5">
      <SectionCard>
        <SectionHeader
          icon={<span className="text-lg">🎨</span>}
          title={t("larkCard.title")}
          right={
            <div className="flex items-center gap-2">
              <button
                onClick={handlePreview}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] bg-[var(--bg-grouped)] hover:bg-[var(--bg-elevated)] transition-colors cursor-pointer"
              >
                <Eye className="h-3.5 w-3.5" />
                {t("larkCard.preview")}
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium text-white bg-[var(--accent)] hover:opacity-90 disabled:opacity-50 transition-all cursor-pointer"
              >
                <Save className="h-3.5 w-3.5" />
                {saving ? "..." : t("config.save")}
              </button>
            </div>
          }
        />

        {/* Template color */}
        <div className="flex flex-col gap-2 mb-5">
          <label className="text-[12px] font-medium text-[var(--text-secondary)]">
            {t("larkCard.template")}
          </label>
          <div className="flex flex-wrap gap-2">
            {Object.entries(COLOR_MAP).map(([name, hex]) => (
              <button
                key={name}
                title={name}
                onClick={() => setConfig((prev) => ({ ...prev, template_color: name }))}
                className={cn(
                  "w-7 h-7 rounded-full transition-all duration-150 cursor-pointer",
                  "border-2",
                  config.template_color === name
                    ? "border-[var(--accent)] scale-110 shadow-[var(--shadow-sm)]"
                    : "border-transparent hover:scale-105"
                )}
                style={{ backgroundColor: hex }}
              >
                {config.template_color === name && (
                  <Check className="h-3.5 w-3.5 mx-auto text-white drop-shadow-sm" />
                )}
              </button>
            ))}
          </div>
        </div>

        {/* Header title */}
        <div className="flex flex-col gap-1.5 mb-4">
          <label className="text-[12px] font-medium text-[var(--text-secondary)]">
            {t("larkCard.headerTitle")}
          </label>
          <input
            type="text"
            value={config.header_title ?? ""}
            onChange={(e) => setConfig((prev) => ({ ...prev, header_title: e.target.value }))}
            placeholder={t("larkCard.headerTitlePlaceholder")}
            className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
          />
        </div>

        {/* Header icon */}
        <div className="flex flex-col gap-1.5 mb-5">
          <label className="text-[12px] font-medium text-[var(--text-secondary)]">
            {t("larkCard.headerIcon")}
          </label>
          <input
            type="text"
            value={config.header_icon ?? ""}
            onChange={(e) => setConfig((prev) => ({ ...prev, header_icon: e.target.value }))}
            placeholder="img_v2_xxx"
            className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
          />
        </div>

        {/* Toggle: show feedback */}
        <div className="flex items-center justify-between py-3 border-t border-[var(--border-subtle)]">
          <div className="flex flex-col gap-0.5">
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              {t("larkCard.showFeedback")}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {t("larkCard.showFeedbackDesc")}
            </span>
          </div>
          <Toggle
            checked={config.show_feedback ?? false}
            onChange={(v) => setConfig((prev) => ({ ...prev, show_feedback: v }))}
          />
        </div>

        {/* Toggle: show timestamp */}
        <div className="flex items-center justify-between py-3 border-t border-[var(--border-subtle)]">
          <div className="flex flex-col gap-0.5">
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              {t("larkCard.showTimestamp")}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {t("larkCard.showTimestampDesc")}
            </span>
          </div>
          <Toggle
            checked={config.show_timestamp ?? false}
            onChange={(v) => setConfig((prev) => ({ ...prev, show_timestamp: v }))}
          />
        </div>
      </SectionCard>

      {/* Preview output */}
      {previewJson && (
        <SectionCard>
          <SectionHeader
            icon={<Eye className="h-4 w-4" />}
            title={t("larkCard.preview")}
            right={
              <button
                onClick={() => setPreviewJson(null)}
                className="text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] cursor-pointer"
              >
                &times;
              </button>
            }
          />
          <pre className="text-[11px] font-mono leading-relaxed text-[var(--text-secondary)] bg-[var(--bg-grouped)] rounded-[var(--radius-md)] p-3 overflow-x-auto max-h-80">
            {previewJson}
          </pre>
        </SectionCard>
      )}

      <ToastContainer toasts={toasts} />
    </div>
  );
}
