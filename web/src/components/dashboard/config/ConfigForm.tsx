import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Settings2, Search, FormInput, Lock,
  Brain, Bot, Wrench, Database, FileText, History, Globe, Shield,
  FolderOpen, Users, Gauge, HeartPulse, Sparkles, ScrollText,
  Folder,
} from "lucide-react";
import { cn } from "../../../lib/cn";
import { SectionCard, EmptyState } from "../shared";
import {
  BooleanField, SecretField, EnumField, NumberField, TextField, ReadOnlyField,
} from "./ConfigFields";
import {
  unquote, isSensitiveKey, requote,
  type ConfigFieldSchema, type ConfigSchemaData, type TomlField,
} from "./tomlParser";

// Icon mapping for schema sections
const SECTION_ICONS: Record<string, React.ReactNode> = {
  "brain": <Brain className="h-3.5 w-3.5" />,
  "bot": <Bot className="h-3.5 w-3.5" />,
  "wrench": <Wrench className="h-3.5 w-3.5" />,
  "database": <Database className="h-3.5 w-3.5" />,
  "file-text": <FileText className="h-3.5 w-3.5" />,
  "history": <History className="h-3.5 w-3.5" />,
  "globe": <Globe className="h-3.5 w-3.5" />,
  "shield": <Shield className="h-3.5 w-3.5" />,
  "folder": <Folder className="h-3.5 w-3.5" />,
  "folder-open": <FolderOpen className="h-3.5 w-3.5" />,
  "users": <Users className="h-3.5 w-3.5" />,
  "gauge": <Gauge className="h-3.5 w-3.5" />,
  "lock": <Lock className="h-3.5 w-3.5" />,
  "heart-pulse": <HeartPulse className="h-3.5 w-3.5" />,
  "sparkles": <Sparkles className="h-3.5 w-3.5" />,
  "scroll-text": <ScrollText className="h-3.5 w-3.5" />,
};

export interface MergedSection {
  schema: ConfigFieldSchema extends never ? never : {
    key: string;
    label: string;
    description?: string;
    order: number;
    icon: string;
    fields: ConfigFieldSchema[];
  };
  toml: { key: string; fields: TomlField[]; isArrayTable?: boolean } | null;
  hasData: boolean;
}

interface ConfigFormProps {
  displaySections: MergedSection[];
  activeSection: string | null;
  searchQuery: string;
  hasChanges: boolean;
  diffLines: Array<{ lineNum: number; from: string; to: string }>;
  activeMerged: MergedSection | null;
  schema: ConfigSchemaData | null;
  hasSchema: boolean;
  sensitivePatterns: string[];
  content: string;
  onSectionSelect: (key: string) => void;
  onSearchChange: (q: string) => void;
  onFieldChange: (sectionKey: string, fieldKey: string, field: TomlField, newValue: string) => void;
  onAddField: (sectionKey: string, fieldKey: string, value: string, fieldType: string) => void;
}

export function ConfigForm({
  displaySections,
  activeSection,
  searchQuery,
  hasChanges,
  diffLines,
  activeMerged,
  schema,
  hasSchema,
  sensitivePatterns,
  onSectionSelect,
  onSearchChange,
  onFieldChange,
  onAddField,
}: ConfigFormProps) {
  const { t } = useTranslation();

  // Render a field from schema definition
  const renderSchemaField = useCallback((fieldSchema: ConfigFieldSchema, tomlField: TomlField | undefined) => {
    const rawValue = tomlField ? unquote(tomlField.value) : "";
    const hasValue = !!tomlField;
    const sensitive = fieldSchema.sensitive || isSensitiveKey(fieldSchema.key, sensitivePatterns);
    const isArray = tomlField?.value.startsWith("[");

    const onChangeExisting = (v: string) => onFieldChange(activeMerged!.schema.key, fieldSchema.key, tomlField!, v);
    const onChangeNew = (v: string) => onAddField(activeMerged!.schema.key, fieldSchema.key, v, fieldSchema.type);
    const onChange = hasValue ? onChangeExisting : onChangeNew;

    // Dynamic placeholder for base_url: use provider_defaults map from schema
    let effectivePlaceholder = fieldSchema.placeholder ?? fieldSchema.default_value;
    if (fieldSchema.key === "base_url" && schema?.provider_defaults) {
      const providerField = activeMerged?.toml?.fields.find((f) => f.key === "provider");
      const currentProvider = providerField ? unquote(providerField.value) : "";
      const providerUrl = currentProvider ? schema.provider_defaults[currentProvider] : undefined;
      if (providerUrl) {
        effectivePlaceholder = `${t("config.default")}: ${providerUrl}`;
      }
    }

    return (
      <div key={fieldSchema.key} className="group py-3 first:pt-0 border-b border-[var(--border-subtle)]/50 last:border-b-0">
        <div className="flex items-start gap-3">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-0.5">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {fieldSchema.label}
              </span>
              {sensitive && <Lock className="h-3 w-3 text-[var(--warning)] flex-shrink-0" />}
              {fieldSchema.default_value && !hasValue && (
                <span className="text-[10px] text-[var(--text-tertiary)] bg-[var(--bg-content)] px-1.5 py-0.5 rounded-md">
                  {t("config.default")}: {fieldSchema.default_value}
                </span>
              )}
            </div>
            {fieldSchema.description && (
              <p className="text-[11px] text-[var(--text-tertiary)] mb-2 leading-relaxed">
                {fieldSchema.description}
              </p>
            )}
          </div>
          <span className="text-[10px] font-mono text-[var(--text-tertiary)] opacity-0 group-hover:opacity-100 transition-opacity mt-1 flex-shrink-0">
            {fieldSchema.key}
          </span>
        </div>

        {/* Array values are read-only in form mode */}
        {isArray ? (
          <ReadOnlyField value={tomlField!.value} label={t("config.editInRaw")} />
        ) : fieldSchema.type === "boolean" ? (
          <div className="flex items-center gap-3">
            <BooleanField value={rawValue === "true"} onChange={(v) => onChange(v ? "true" : "false")} />
            <span className="text-[12px] text-[var(--text-secondary)]">
              {rawValue === "true" ? t("config.enabled") : t("config.notSet")}
            </span>
          </div>
        ) : fieldSchema.type === "secret" || sensitive ? (
          <SecretField value={rawValue} onChange={onChange} placeholder={fieldSchema.placeholder} />
        ) : fieldSchema.type === "enum" && fieldSchema.options ? (
          <EnumField value={rawValue} options={fieldSchema.options} onChange={onChange} />
        ) : fieldSchema.type === "number" ? (
          <NumberField value={rawValue} onChange={onChange} placeholder={effectivePlaceholder} />
        ) : (
          <TextField value={rawValue} onChange={onChange} placeholder={effectivePlaceholder} />
        )}
      </div>
    );
  }, [activeMerged, schema, sensitivePatterns, t, onFieldChange, onAddField]);

  // Render a TOML-only field (no schema match)
  const renderTomlField = useCallback((f: TomlField) => {
    const displayValue = unquote(f.value);
    const isBool = f.value === "true" || f.value === "false";
    const sensitive = isSensitiveKey(f.key, sensitivePatterns);
    const isArray = f.value.startsWith("[");

    return (
      <div key={f.key} className="group py-3 first:pt-0 border-b border-[var(--border-subtle)]/50 last:border-b-0">
        <div className="flex items-center gap-2 mb-2">
          <span className="text-[13px] font-medium text-[var(--text-secondary)] font-mono">
            {f.key}
          </span>
          {sensitive && <Lock className="h-3 w-3 text-[var(--warning)]" />}
          {hasSchema && (
            <span className="text-[9px] text-[var(--text-tertiary)] bg-[var(--bg-content)] px-1.5 py-0.5 rounded-md">
              {t("config.customField")}
            </span>
          )}
        </div>
        {isArray ? (
          <ReadOnlyField value={f.value} label={t("config.editInRaw")} />
        ) : isBool ? (
          <div className="flex items-center gap-3">
            <BooleanField
              value={f.value === "true"}
              onChange={(v) => onFieldChange(activeMerged!.schema.key, f.key, f, v ? "true" : "false")}
            />
            <span className="text-[12px] text-[var(--text-secondary)]">{f.value}</span>
          </div>
        ) : sensitive ? (
          <SecretField
            value={displayValue}
            onChange={(v) => onFieldChange(activeMerged!.schema.key, f.key, f, requote(f.value, v))}
          />
        ) : (
          <TextField
            value={displayValue}
            onChange={(v) => onFieldChange(activeMerged!.schema.key, f.key, f, requote(f.value, v))}
          />
        )}
      </div>
    );
  }, [activeMerged, hasSchema, sensitivePatterns, t, onFieldChange]);

  return (
    <div className="flex-1 min-h-0 flex gap-3">
      {/* Left nav */}
      <div className="w-[200px] flex-shrink-0 flex flex-col min-h-0">
        <SectionCard className="flex-1 flex flex-col min-h-0 !p-2">
          <div className="relative mb-2">
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-[var(--text-tertiary)]" />
            <input
              type="text" value={searchQuery} onChange={(e) => onSearchChange(e.target.value)}
              placeholder={t("config.searchFields")}
              className="w-full pl-7 pr-2 py-1.5 rounded-[var(--radius-lg)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[11px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)]"
            />
          </div>
          <div className="flex-1 overflow-y-auto space-y-0.5">
            {displaySections.map((s) => {
              const icon = SECTION_ICONS[s.schema.icon] ?? <Settings2 className="h-3.5 w-3.5" />;
              const isActive = activeSection === s.schema.key;
              return (
                <button key={s.schema.key} onClick={() => onSectionSelect(s.schema.key)}
                  className={cn(
                    "w-full text-left px-2.5 py-2 rounded-[var(--radius-lg)] text-[12px] font-medium transition-colors cursor-pointer flex items-center gap-2",
                    isActive ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                      : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                  )} title={s.schema.description}>
                  <span className={cn("flex-shrink-0", isActive ? "opacity-100" : "opacity-50")}>{icon}</span>
                  <span className="truncate">{s.schema.label}</span>
                  {s.hasData && (
                    <span className="ml-auto text-[10px] text-[var(--text-tertiary)] tabular-nums flex-shrink-0 bg-[var(--bg-content)] px-1.5 rounded-full">
                      {s.toml!.fields.length}
                    </span>
                  )}
                </button>
              );
            })}
            {displaySections.length === 0 && (
              <EmptyState icon={<Settings2 className="h-5 w-5" />} message={t("config.noSections")} />
            )}
          </div>
        </SectionCard>
      </div>

      {/* Center: fields */}
      <div className="flex-1 min-w-0 flex flex-col min-h-0">
        <SectionCard className="flex-1 flex flex-col min-h-0 overflow-y-auto">
          {activeMerged ? (
            <>
              <div className="flex items-center gap-3 mb-1">
                <span className="text-[var(--text-tertiary)]">
                  {SECTION_ICONS[activeMerged.schema.icon] ?? <Settings2 className="h-4.5 w-4.5" />}
                </span>
                <div>
                  <h3 className="text-[15px] font-semibold text-[var(--text-primary)]">
                    {activeMerged.schema.label}
                  </h3>
                  {activeMerged.schema.description && (
                    <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5">
                      {activeMerged.schema.description}
                    </p>
                  )}
                </div>
                <span className="ml-auto text-[10px] font-mono text-[var(--text-tertiary)] bg-[var(--bg-content)] px-2 py-0.5 rounded-md">
                  {activeMerged.toml?.isArrayTable ? `[[${activeMerged.schema.key}]]` : `[${activeMerged.schema.key}]`}
                </span>
              </div>

              <div className="mt-3">
                {/* Schema-defined fields */}
                {activeMerged.schema.fields.map((fs) => {
                  const tomlField = activeMerged.toml?.fields.find((f) => f.key === fs.key);
                  return renderSchemaField(fs, tomlField);
                })}

                {/* Extra TOML fields not in schema */}
                {activeMerged.toml?.fields
                  .filter((f) => !activeMerged.schema.fields.some((sf) => sf.key === f.key))
                  .map((f) => renderTomlField(f))}

                {activeMerged.schema.fields.length === 0 && (!activeMerged.toml || activeMerged.toml.fields.length === 0) && (
                  <div className="text-[12px] text-[var(--text-tertiary)] py-8 text-center">
                    {t("config.noFields")}
                  </div>
                )}
              </div>
            </>
          ) : (
            <EmptyState icon={<FormInput className="h-5 w-5" />} message={t("config.selectSection")} />
          )}
        </SectionCard>
      </div>

      {/* Right: diff preview */}
      {hasChanges && (
        <div className="w-[240px] flex-shrink-0 flex flex-col min-h-0">
          <SectionCard className="flex-1 flex flex-col min-h-0 !p-0">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border-subtle)]">
              <span className="text-[11px] font-medium text-[var(--text-secondary)]">{t("config.changesPreview")}</span>
              <span className="text-[10px] text-[var(--accent)] font-mono tabular-nums ml-auto">{diffLines.length}</span>
            </div>
            <div className="flex-1 overflow-y-auto p-2 space-y-1.5">
              {diffLines.map((d) => (
                <div key={d.lineNum} className="text-[10px] font-mono leading-relaxed">
                  <div className="text-[var(--text-tertiary)] mb-0.5">L{d.lineNum}</div>
                  {d.from && <div className="px-1.5 py-0.5 rounded bg-[var(--error)]/8 text-[var(--error)] truncate" title={d.from}>- {d.from}</div>}
                  {d.to && <div className="px-1.5 py-0.5 rounded bg-[var(--success)]/8 text-[var(--success)] truncate mt-0.5" title={d.to}>+ {d.to}</div>}
                </div>
              ))}
            </div>
          </SectionCard>
        </div>
      )}
    </div>
  );
}
