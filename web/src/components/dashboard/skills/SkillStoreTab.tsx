import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Search, Download, Star, ArrowDownWideNarrow,
  Store, Loader2, CheckCircle2, GitBranch, User,
} from "lucide-react";
import { fetchJSON, postJSON } from "../../../lib/api";
import type { StoreSkillItem, StoreSkillDetail, StoreSearchResult, StoreStatus } from "../../../types/dashboard";
import {
  EmptyState,
  LoadingSkeleton,
} from "../shared";
import { useToast } from "../../ui/toast";
import { cn } from "../../../lib/cn";
import { formatNumber } from "./skillsConstants";
import { StoreSkillDetailModal } from "./StoreSkillDetailModal";

// ===========================================================================
// STORE TAB
// ===========================================================================

type SortMode = "downloads" | "stars" | "recent";
type FilterMode = "all" | "installed" | "not-installed";

const PAGE_SIZE = 30;

export function SkillStoreTab({ toast }: { toast: ReturnType<typeof useToast>["toast"] }) {
  const { t } = useTranslation();

  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortMode>("downloads");
  const [filter, setFilter] = useState<FilterMode>("all");
  const [items, setItems] = useState<StoreSkillItem[]>([]);
  const [searchResults, setSearchResults] = useState<StoreSearchResult[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState<Set<string>>(new Set());
  const [configured, setConfigured] = useState(true);
  const [detailSlug, setDetailSlug] = useState<string | null>(null);
  const searchTimeout = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Load status + initial list
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    setItems([]);
    setHasMore(true);
    (async () => {
      const [status, list] = await Promise.all([
        fetchJSON<StoreStatus>("/store/status").catch(() => null),
        fetchJSON<{ items: StoreSkillItem[]; source: string }>(`/store/skills?limit=${PAGE_SIZE}&sort=${sort}`).catch(() => null),
      ]);
      if (status) {
        setConfigured(status.configured);
        setInstalled(new Set(status.installed));
      }
      if (list) {
        setItems(list.items);
        setHasMore(list.items.length >= PAGE_SIZE);
      }
      setLoading(false);
    })();
  }, [sort]);

  // Load more
  const loadMore = useCallback(async () => {
    if (loadingMore || !hasMore) return;
    setLoadingMore(true);
    try {
      const list = await fetchJSON<{ items: StoreSkillItem[]; source: string }>(`/store/skills?limit=${PAGE_SIZE}&sort=${sort}&cursor=${items.length}`);
      if (list) {
        setItems((prev) => [...prev, ...list.items]);
        setHasMore(list.items.length >= PAGE_SIZE);
      } else {
        setHasMore(false);
      }
    } catch {
      setHasMore(false);
    }
    setLoadingMore(false);
  }, [sort, items.length, loadingMore, hasMore]);

  // Debounced search
  useEffect(() => {
    if (!search.trim()) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setSearchResults(null);
      return;
    }
    clearTimeout(searchTimeout.current);
    searchTimeout.current = setTimeout(async () => {
      try {
        const data = await fetchJSON<{ results: StoreSearchResult[]; source: string }>(`/store/search?q=${encodeURIComponent(search.trim())}&limit=50`);
        if (data) setSearchResults(data.results);
      } catch { /* ignore */ }
    }, 400);
    return () => clearTimeout(searchTimeout.current);
  }, [search]);

  const handleInstall = async (slug: string) => {
    setInstalling((prev) => new Set(prev).add(slug));
    const result = await postJSON<{ ok: boolean }>("/store/install", { slug }).catch(() => null);
    setInstalling((prev) => {
      const next = new Set(prev);
      next.delete(slug);
      return next;
    });
    if (result?.ok) {
      setInstalled((prev) => new Set(prev).add(slug));
      toast({ variant: "success", title: t("dashboard.storeInstallSuccess", "Skill installed successfully") });
    } else {
      toast({ variant: "error", title: t("dashboard.storeInstallFailed", "Failed to install skill") });
    }
  };

  if (!configured) {
    return (
      <EmptyState
        icon={<Store className="h-5 w-5" />}
        message={t("dashboard.storeNotConfigured", "Store not configured. Set CLAWHUB_API_KEY in .env")}
      />
    );
  }

  if (loading) {
    return (
      <div className="space-y-4">
        <LoadingSkeleton className="h-10 w-full" />
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
          {Array.from({ length: 9 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-40" />
          ))}
        </div>
      </div>
    );
  }

  let displayItems: StoreSkillItem[] = searchResults
    ? searchResults.map((r) => ({
        slug: r.slug,
        displayName: r.displayName,
        summary: r.summary,
        version: r.version,
      } as StoreSkillItem))
    : items;

  // Apply filter
  if (filter === "installed") {
    displayItems = displayItems.filter((i) => installed.has(i.slug));
  } else if (filter === "not-installed") {
    displayItems = displayItems.filter((i) => !installed.has(i.slug));
  }

  return (
    <div>
      {/* Search + sort + filter */}
      <div className="flex items-center gap-3 mb-5 flex-wrap">
        <div className="relative flex-1 max-w-[400px] min-w-[200px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("dashboard.storeSearch", "Search skills store...")}
            className="w-full pl-9 pr-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-colors"
          />
        </div>
        {!searchResults && (
          <div className="flex items-center gap-1 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
            <SortButton active={sort === "downloads"} onClick={() => setSort("downloads")}>
              <Download className="h-3 w-3" />
              {t("dashboard.storeSortDownloads", "Downloads")}
            </SortButton>
            <SortButton active={sort === "stars"} onClick={() => setSort("stars")}>
              <Star className="h-3 w-3" />
              {t("dashboard.storeSortStars", "Stars")}
            </SortButton>
            <SortButton active={sort === "recent"} onClick={() => setSort("recent")}>
              <ArrowDownWideNarrow className="h-3 w-3" />
              {t("dashboard.storeSortRecent", "Recent")}
            </SortButton>
          </div>
        )}
        {/* Filter */}
        <div className="flex items-center gap-1 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
          <SortButton active={filter === "all"} onClick={() => setFilter("all")}>
            {t("dashboard.storeFilterAll", "All")}
          </SortButton>
          <SortButton active={filter === "installed"} onClick={() => setFilter("installed")}>
            <CheckCircle2 className="h-3 w-3" />
            {t("dashboard.storeFilterInstalled", "Installed")}
          </SortButton>
          <SortButton active={filter === "not-installed"} onClick={() => setFilter("not-installed")}>
            {t("dashboard.storeFilterNew", "New")}
          </SortButton>
        </div>
      </div>

      {displayItems.length === 0 ? (
        <EmptyState
          icon={<Store className="h-5 w-5" />}
          message={t("dashboard.storeNoResults", "No skills found")}
        />
      ) : (
        <>
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {displayItems.map((item) => (
              <StoreSkillCard
                key={item.slug}
                item={item}
                isInstalled={installed.has(item.slug)}
                isInstalling={installing.has(item.slug)}
                onInstall={() => handleInstall(item.slug)}
                onDetail={() => setDetailSlug(item.slug)}
              />
            ))}
          </div>
          {/* Load more */}
          {!searchResults && hasMore && (
            <div className="flex justify-center mt-6">
              <button
                onClick={loadMore}
                disabled={loadingMore}
                className="flex items-center gap-2 px-6 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer disabled:opacity-50"
              >
                {loadingMore ? (
                  <>
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    {t("dashboard.storeLoading", "Loading...")}
                  </>
                ) : (
                  t("dashboard.storeLoadMore", "Load more")
                )}
              </button>
            </div>
          )}
        </>
      )}

      {/* Detail modal */}
      {detailSlug && (
        <StoreSkillDetailModal
          slug={detailSlug}
          isInstalled={installed.has(detailSlug)}
          isInstalling={installing.has(detailSlug)}
          onInstall={() => handleInstall(detailSlug)}
          onClose={() => setDetailSlug(null)}
        />
      )}
    </div>
  );
}

function SortButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-1 px-2.5 py-1 rounded-[var(--radius-sm)] text-[10px] font-medium transition-all cursor-pointer",
        active
          ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
          : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
      )}
    >
      {children}
    </button>
  );
}

function StoreSkillCard({
  item,
  isInstalled,
  isInstalling,
  onInstall,
  onDetail,
}: {
  item: StoreSkillItem;
  isInstalled: boolean;
  isInstalling: boolean;
  onInstall: () => void;
  onDetail: () => void;
}) {
  const { t } = useTranslation();
  const downloads = item.stats?.downloads ?? item.stats?.installsAllTime ?? 0;
  const stars = item.stats?.stars ?? 0;
  const versions = item.stats?.versions ?? 0;
  const version = item.latestVersion?.version || item.displayName;
  const osTags = item.metadata?.os?.filter(Boolean) ?? [];
  const [owner, setOwner] = useState<{ handle?: string | null; image?: string | null; displayName?: string | null } | null>(null);

  // Lazy-load owner on hover
  const ownerLoaded = useRef(false);
  const onHover = useCallback(() => {
    if (ownerLoaded.current) return;
    ownerLoaded.current = true;
    fetchJSON<StoreSkillDetail>(`/store/skills/${encodeURIComponent(item.slug)}`).then((d) => {
      if (d?.owner) setOwner(d.owner);
    }).catch(() => {});
  }, [item.slug]);

  return (
    <div
      className="group relative rounded-[var(--radius-lg)] border bg-[var(--bg-elevated)]/70 border-[var(--border-subtle)] hover:border-[var(--separator)] hover:shadow-[var(--shadow-md)] overflow-hidden transition-all duration-200 cursor-pointer"
      onMouseEnter={onHover}
      onClick={onDetail}
    >
      {/* Accent bar */}
      <div className="h-[2px] bg-gradient-to-r from-[var(--accent)]/60 to-transparent" />

      <div className="p-4">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 mb-2">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
                {item.displayName || item.slug}
              </span>
              {version && (
                <span className="px-1.5 py-[1px] rounded-[var(--radius-sm)] bg-[var(--bg-content)] text-[9px] font-mono text-[var(--text-tertiary)] border border-[var(--border-subtle)] shrink-0">
                  v{version}
                </span>
              )}
              {osTags.length > 0 && osTags.map((os) => (
                <span key={os} className="px-1.5 py-[1px] rounded-full bg-[var(--accent)]/8 text-[9px] font-medium text-[var(--accent)] border border-[var(--accent)]/15 shrink-0">
                  {os === "darwin" ? "macOS" : os === "win32" ? "Windows" : os.charAt(0).toUpperCase() + os.slice(1)}
                </span>
              ))}
            </div>
            <span className="text-[10px] font-mono text-[var(--text-tertiary)] mt-0.5 block truncate">
              /{item.slug}
            </span>
          </div>

          {/* Install button */}
          {isInstalled ? (
            <span className="flex items-center gap-1 px-2.5 py-1.5 rounded-[var(--radius-md)] bg-[var(--success)]/10 text-[var(--success)] text-[10px] font-medium border border-[var(--success)]/20 shrink-0">
              <CheckCircle2 className="h-3 w-3" />
              {t("dashboard.storeInstalled", "Installed")}
            </span>
          ) : (
            <button
              onClick={(e) => { e.stopPropagation(); onInstall(); }}
              disabled={isInstalling}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[10px] font-medium transition-all shrink-0 cursor-pointer",
                isInstalling
                  ? "bg-[var(--bg-content)] text-[var(--text-tertiary)] border border-[var(--border-subtle)]"
                  : "bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]"
              )}
            >
              {isInstalling ? (
                <>
                  <Loader2 className="h-3 w-3 animate-spin" />
                  {t("dashboard.storeInstalling", "Installing...")}
                </>
              ) : (
                <>
                  <Download className="h-3 w-3" />
                  {t("dashboard.storeInstall", "Install")}
                </>
              )}
            </button>
          )}
        </div>

        {/* Summary */}
        {item.summary && (
          <p className="text-[11px] text-[var(--text-secondary)] leading-[1.6] mb-3 line-clamp-2">
            {item.summary}
          </p>
        )}

        {/* Footer: stats + author */}
        <div className="flex items-center justify-between pt-2 border-t border-[var(--border-subtle)]/50">
          <div className="flex items-center gap-3">
            {downloads > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                <Download className="h-3 w-3" />
                {formatNumber(downloads)}
              </span>
            )}
            {stars > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                <Star className="h-3 w-3" />
                {formatNumber(stars)}
              </span>
            )}
            {versions > 1 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                <GitBranch className="h-3 w-3" />
                {versions}v
              </span>
            )}
          </div>
          {owner && (
            <div className="flex items-center gap-1.5">
              {owner.image ? (
                <img src={owner.image} alt={owner.handle ?? undefined} className="h-4 w-4 rounded-full" />
              ) : (
                <User className="h-3 w-3 text-[var(--text-tertiary)]" />
              )}
              <span className="text-[10px] text-[var(--text-tertiary)]">@{owner.handle}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
