import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles, FolderOpen, Store } from "lucide-react";
import {
  SectionCard,
  SectionHeader,
} from "../shared";
import { useToast } from "../../ui/toast";
import { cn } from "../../../lib/cn";
import { LocalSkillsTab } from "./LocalSkillsTab";
import { SkillStoreTab } from "./SkillStoreTab";

// ---------------------------------------------------------------------------
// Tab type
// ---------------------------------------------------------------------------

type Tab = "local" | "store";

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function SkillsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("local");
  const { toast } = useToast();

  return (
    <div className="space-y-4">
      <SectionCard>
        <SectionHeader
          icon={<Sparkles className="h-4 w-4" />}
          title={t("dashboard.skills", "Skills")}
          right={
            <div className="flex items-center gap-1 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
              <TabButton active={tab === "local"} onClick={() => setTab("local")}>
                <FolderOpen className="h-3 w-3" />
                {t("dashboard.skillsLocal", "Local")}
              </TabButton>
              <TabButton active={tab === "store"} onClick={() => setTab("store")}>
                <Store className="h-3 w-3" />
                {t("dashboard.skillsStore", "Store")}
              </TabButton>
            </div>
          }
        />

        {tab === "local" ? <LocalSkillsTab toast={toast} /> : <SkillStoreTab toast={toast} />}
      </SectionCard>
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-sm)] text-[11px] font-medium transition-all cursor-pointer",
        active
          ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
          : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
      )}
    >
      {children}
    </button>
  );
}
