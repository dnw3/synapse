import { useTranslation } from "react-i18next";
import { Bot, Save } from "lucide-react";
import { SectionHeader } from "../shared";

interface AgentEditFormProps {
  isCreate: boolean;
  editName: string;
  editModel: string;
  editPrompt: string;
  saving: boolean;
  onChangeName: (v: string) => void;
  onChangeModel: (v: string) => void;
  onChangePrompt: (v: string) => void;
  onSave: () => void;
  onCancel: () => void;
}

export default function AgentEditForm({
  isCreate,
  editName,
  editModel,
  editPrompt,
  saving,
  onChangeName,
  onChangeModel,
  onChangePrompt,
  onSave,
  onCancel,
}: AgentEditFormProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-4">
      <SectionHeader
        icon={<Bot className="h-4 w-4" />}
        title={isCreate ? t("agents.createAgent") : t("agents.editAgent")}
      />
      <div className="space-y-3">
        <div>
          <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
            {t("agents.name")}
          </label>
          <input
            value={editName}
            onChange={(e) => onChangeName(e.target.value)}
            disabled={!isCreate}
            className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors disabled:opacity-50"
            placeholder="my-agent"
          />
        </div>
        <div>
          <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
            {t("agents.model")}
          </label>
          <input
            value={editModel}
            onChange={(e) => onChangeModel(e.target.value)}
            className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] font-mono outline-none focus:border-[var(--accent)] transition-colors"
            placeholder="gpt-4o"
          />
        </div>
        <div>
          <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
            {t("agents.systemPrompt")}
          </label>
          <textarea
            value={editPrompt}
            onChange={(e) => onChangePrompt(e.target.value)}
            rows={5}
            className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors resize-none"
            placeholder={t("agents.promptPlaceholder")}
          />
        </div>
      </div>
      <div className="flex items-center gap-2 pt-2">
        <button
          onClick={onSave}
          disabled={saving || !editName.trim()}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[12px] font-medium hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)] transition-all cursor-pointer disabled:opacity-40"
        >
          <Save className="h-3.5 w-3.5" />
          {saving ? t("agents.saving") : t("agents.save")}
        </button>
        <button
          onClick={onCancel}
          className="px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
        >
          {t("agents.cancel")}
        </button>
      </div>
    </div>
  );
}
