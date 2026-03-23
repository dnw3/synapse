import { useTranslation } from "react-i18next";

export default function McpServersPage() {
  const { t } = useTranslation();
  return <div className="p-6 text-[var(--text-primary)]">{t("dashboard.mcpServers.title")}</div>;
}
