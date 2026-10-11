import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import * as dl from "../lib/deadlock";
import type { DlCategory, DlExtras, DlLibrary, DlLibraryMod, DlModDetail, DlSort, DlStoreMod } from "../lib/types";
import { Icon, Select, Switch } from "./ui";
import { Popover } from "./Popover";

/* Deadlock's left-rail pages, modelled on Deadlock Mod Manager: Home (play
   with or without mods, trending), Mod Library, Mod Store, Hero Skins and
   Extras. Mods come from GameBanana; see launcher-core games/deadlock.rs for
   how they're placed in the game. */

type Open = (tab: string) => void;

/** Store filters another page asked for ("See all skins", "Find skins for…"). */
let storeIntent: { search?: string; category?: string } | null = null;

/* ------------------------------- shared --------------------------------- */

function useLibraryActions() {
  const { showToast } = useLauncher();
  const run = async (p: Promise<DlLibrary>, ok?: string) => {
    try {
      const next = await p;
      dl.setLibrary(next);
      if (next.syncError) showToast(next.syncError);
      else if (ok) showToast(ok);
      return next;
    } catch (e) {
      showToast(`${e}`);
      return null;
    }
  };
  return { run };
}

/** Installs from the store, shared by every page that shows store cards. */
function useInstaller() {
  const { showToast, refreshGames } = useLauncher();
  const [installing, setInstalling] = useState<Set<number>>(new Set());
  const install = async (m: { id: number; name: string }, fileId: number | null = null) => {
    setInstalling((s) => new Set(s).add(m.id));
    try {
      const next = await dl.install(m.id, fileId);
      dl.setLibrary(next);
      showToast(next.syncError ?? `${m.name} installed and switched on`);
      void refreshGames();
    } catch (e) {
      showToast(`Couldn't install ${m.name}: ${e}`);
    } finally {
      setInstalling((s) => {
        const n = new Set(s);
        n.delete(m.id);
        return n;
      });
    }
  };
  return { installing, install };
}

function ModArt({ src, name, hero, audio }: { src: string | null; name: string; hero?: string | null; audio?: boolean }) {
  const [broken, setBroken] = useState(false);
  if (src && !broken) return <img src={src} alt="" loading="lazy" onError={() => setBroken(true)} />;
  return (
    <span className="dl-art-empty">{audio ? <Icon.music size={30} /> : <span className="dl-art-letter">{(hero ?? name).slice(0, 1)}</span>}</span>
  );
}

function InstallButton({ mod, owned, busy, onInstall }: { mod: DlStoreMod; owned?: DlLibraryMod; busy: boolean; onInstall: () => void }) {
  const fraction = dl.useInstallProgress(mod.id);
  if (busy) {
    return (
      <span className="dl-ring" style={{ ["--p" as string]: `${Math.round((fraction ?? 0) * 100)}%` }} title="Downloading">
        <Icon.download size={14} />
      </span>
    );
  }
  if (owned && !owned.updateAvailable) {
    return (
      <span className="dl-owned" title="In your library">
        <Icon.check size={15} />
      </span>
    );
  }
  return (
    <button
      className="dl-icon-btn"
      title={owned?.updateAvailable ? "Update" : "Download"}
      onClick={(e) => {
        e.stopPropagation();
        onInstall();
      }}
    >
      {owned?.updateAvailable ? <Icon.refresh size={15} /> : <Icon.download size={15} />}
    </button>
  );
}

function StoreCard({
  mod,
  owned,
  busy,
  onInstall,
  onOpen,
  compact,
}: {
  mod: DlStoreMod;
  owned?: DlLibraryMod;
  busy: boolean;
  onInstall: () => void;
  onOpen: () => void;
  compact?: boolean;
}) {
  return (
    <div className={`dl-card ${compact ? "compact" : ""}`} onClick={onOpen} role="button" tabIndex={0}>
      <div className="dl-art">
        <ModArt src={mod.image} name={mod.name} hero={mod.hero} audio={mod.audio} />
        <div className="dl-badges">
          {owned && <span className="dl-badge on">Installed</span>}
          {owned?.updateAvailable && <span className="dl-badge warn">Update available</span>}
          {mod.nsfw && <span className="dl-badge warn">NSFW</span>}
        </div>
        {mod.hero && <span className="dl-hero-tag">{mod.hero}</span>}
      </div>
      <div className="dl-body">
        <div className="dl-name" title={mod.name}>
          {mod.name}
        </div>
        <div className="dl-by">By {mod.author || "unknown"}</div>
        <div className="dl-foot">
          <div className="dl-meta">
            <span title="Downloads">
              <Icon.download size={12} /> {dl.fmtCount(mod.downloads || mod.views)}
            </span>
            <span title="Likes">
              <Icon.heart size={12} /> {dl.fmtCount(mod.likes)}
            </span>
            {!compact && <span className="dl-date">{dl.fmtDate(mod.updated)}</span>}
          </div>
          <InstallButton mod={mod} owned={owned} busy={busy} onInstall={onInstall} />
        </div>
      </div>
    </div>
  );
}

function ownedBy(lib: DlLibrary | null) {
  const map = new Map<number, DlLibraryMod>();
  lib?.mods.forEach((m) => m.gamebananaId != null && map.set(m.gamebananaId, m));
  return map;
}

/* ------------------------------ mod detail ------------------------------ */

function ModDetailModal({
  mod,
  owned,
  busy,
  onInstall,
  onClose,
}: {
  mod: DlStoreMod;
  owned?: DlLibraryMod;
  busy: boolean;
  onInstall: (fileId: number | null) => void;
  onClose: () => void;
}) {
  const [d, setD] = useState<DlModDetail | null>(null);
  const [idx, setIdx] = useState(0);
  const [fileId, setFileId] = useState<number | null>(null);
  useEffect(() => {
    dl.modDetail(mod.id)
      .then((x) => {
        setD(x);
        const newest = [...x.files].sort((a, b) => b.added - a.added)[0];
        setFileId(newest?.id ?? null);
      })
      .catch(() => {});
  }, [mod.id]);
  const imgs = d?.images.length ? d.images : mod.image ? [mod.image] : [];

  // Portalled: the page's own transform would otherwise trap a fixed overlay.
  return createPortal(
    <div className="dash-overlay" onClick={onClose}>
      <div className="dash dl-detail" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="eyebrow">
              {mod.category ?? "Mod"}
              {mod.hero ? ` · ${mod.hero}` : ""}
            </div>
            <div className="dl-detail-title">{mod.name}</div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              By {mod.author} · {dl.fmtCount(d?.downloads || mod.downloads || mod.views)} downloads · {dl.fmtCount(mod.likes)} likes · updated{" "}
              {dl.fmtDate(mod.updated)}
            </div>
          </div>
          <button className="btn ghost" onClick={onClose}>
            <Icon.close size={16} /> Close
          </button>
        </div>

        <div className="gallery">
          {imgs.length ? (
            <>
              <img className="gallery-img" src={imgs[idx % imgs.length]} alt="" />
              {imgs.length > 1 && (
                <>
                  <button className="gallery-nav left" onClick={() => setIdx((i) => (i - 1 + imgs.length) % imgs.length)} aria-label="Previous">
                    ‹
                  </button>
                  <button className="gallery-nav right" onClick={() => setIdx((i) => (i + 1) % imgs.length)} aria-label="Next">
                    ›
                  </button>
                  <div className="gallery-count">
                    {(idx % imgs.length) + 1} / {imgs.length}
                  </div>
                </>
              )}
            </>
          ) : (
            <div className="gallery-img dl-art-empty">{mod.audio ? <Icon.music size={40} /> : <span>{d ? "No screenshots" : "Loading…"}</span>}</div>
          )}
        </div>

        <div className="patch-notes" style={{ maxHeight: "18vh", flex: "0 0 auto", whiteSpace: "pre-wrap" }}>
          {d ? d.description || "No description." : "Loading details…"}
        </div>

        {d && d.files.length > 1 && (
          <div className="dl-files">
            <div className="dl-files-label">Files</div>
            {d.files.map((f) => (
              <button key={f.id} className={`dl-file ${fileId === f.id ? "on" : ""}`} onClick={() => setFileId(f.id)}>
                <Icon.file size={15} />
                <span className="grow">
                  <b>{f.name}</b>
                  <span className="muted">
                    {dl.fmtSize(f.size)} · {dl.fmtDate(f.added)}
                    {f.description ? ` · ${f.description}` : ""}
                  </span>
                </span>
                {fileId === f.id && <Icon.check size={15} />}
              </button>
            ))}
          </div>
        )}

        <div className="row" style={{ justifyContent: "flex-end", gap: 8 }}>
          <button className="btn ghost" onClick={() => void api.openUrl(mod.url)}>
            <Icon.link size={15} /> GameBanana
          </button>
          <button className="btn-play dl-cta" disabled={busy || !d || d.files.length === 0} onClick={() => onInstall(fileId)}>
            <Icon.download size={15} /> {busy ? "Downloading…" : owned ? (owned.updateAvailable ? "Update" : "Reinstall") : "Download"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

/* -------------------------------- home ---------------------------------- */

function Shelf({ accent, mods, lib, onSeeAll }: { accent: string; mods: DlStoreMod[] | null; lib: DlLibrary | null; onSeeAll: () => void }) {
  const row = useRef<HTMLDivElement>(null);
  const { installing, install } = useInstaller();
  const [detail, setDetail] = useState<DlStoreMod | null>(null);
  const owned = ownedBy(lib);
  const scroll = (dir: number) => row.current?.scrollBy({ left: dir * row.current.clientWidth * 0.8, behavior: "smooth" });
  return (
    <section className="dl-shelf">
      <div className="dl-shelf-head">
        <div className="dl-shelf-title">
          <Icon.sparkles size={16} /> Trending <em>{accent}</em>
        </div>
        <div className="row" style={{ gap: 6 }}>
          <button className="dl-icon-btn ghost" onClick={() => scroll(-1)} aria-label="Scroll left">
            <Icon.arrowLeft size={15} />
          </button>
          <button className="dl-icon-btn ghost" onClick={() => scroll(1)} aria-label="Scroll right">
            <Icon.arrowRight size={15} />
          </button>
          <button className="btn ghost dl-see-all" onClick={onSeeAll}>
            See all <Icon.arrowRight size={14} />
          </button>
        </div>
      </div>
      <div className="dl-row" ref={row}>
        {mods === null && Array.from({ length: 5 }, (_, i) => <div key={i} className="dl-card compact skeleton" />)}
        {mods?.length === 0 && <div className="muted">GameBanana didn't answer. Check your connection and open the store to try again.</div>}
        {mods?.map((m) => (
          <StoreCard
            key={m.id}
            compact
            mod={m}
            owned={owned.get(m.id)}
            busy={installing.has(m.id)}
            onInstall={() => void install(m)}
            onOpen={() => setDetail(m)}
          />
        ))}
      </div>
      {detail && (
        <ModDetailModal
          mod={detail}
          owned={owned.get(detail.id)}
          busy={installing.has(detail.id)}
          onInstall={(f) => void install(detail, f)}
          onClose={() => setDetail(null)}
        />
      )}
    </section>
  );
}

let trendingCache: DlStoreMod[] | null = null;

export function DeadlockHome({ onOpen, onPlay }: { onOpen: Open; onPlay: (modded: boolean) => void }) {
  const { games } = useLauncher();
  const info = games?.deadlock;
  const lib = dl.useDeadlockLibrary();
  const [trend, setTrend] = useState<DlStoreMod[] | null>(trendingCache);
  useEffect(() => {
    if (trendingCache) return;
    dl.trending()
      .then((t) => setTrend((trendingCache = t)))
      .catch(() => setTrend([]));
  }, []);

  const enabled = lib?.mods.filter((m) => m.enabled) ?? [];
  const skins = enabled.filter((m) => dl.effectiveHero(m));
  const updates = lib?.mods.filter((m) => m.updateAvailable).length ?? 0;
  const skinTrend = trend?.filter((m) => dl.isSkin(m)).slice(0, 14) ?? null;
  const modTrend = trend?.filter((m) => !dl.isSkin(m)).slice(0, 14) ?? null;
  const seeAll = (category: string) => {
    storeIntent = { category };
    onOpen("Mod Store");
  };

  return (
    <div className="dl-home">
      <div className="dl-play-grid">
        <button className="dl-play modded" disabled={!info?.installed} onClick={() => onPlay(true)}>
          <span className="dl-play-ico">
            <Icon.play size={18} />
          </span>
          <span className="dl-play-txt">
            <span className="dl-play-title">Play with mods</span>
            <span className="dl-play-sub">
              {enabled.length} mod{enabled.length === 1 ? "" : "s"} on
              {skins.length ? ` · ${skins.length} hero skin${skins.length === 1 ? "" : "s"}` : ""}
            </span>
          </span>
          <Icon.mods size={64} />
        </button>
        <button className="dl-play vanilla" disabled={!info?.installed} onClick={() => onPlay(false)}>
          <span className="dl-play-ico">
            <Icon.play size={18} />
          </span>
          <span className="dl-play-txt">
            <span className="dl-play-title">Play without mods</span>
            <span className="dl-play-sub">Vanilla Deadlock. Your mods stay installed.</span>
          </span>
          <Icon.deadlock size={64} />
        </button>
      </div>

      <div className="dl-stats">
        <button className="dl-stat" onClick={() => onOpen("Mod Library")}>
          <Icon.mods size={20} />
          <span>
            <span className="dl-stat-label">Installed</span>
            <span className="dl-stat-value">{lib?.mods.length ?? "–"}</span>
          </span>
        </button>
        <button className="dl-stat" onClick={() => onOpen("Mod Library")}>
          <Icon.check size={20} />
          <span>
            <span className="dl-stat-label">Enabled</span>
            <span className="dl-stat-value">{lib ? enabled.length : "–"}</span>
          </span>
        </button>
        <button className={`dl-stat ${updates ? "hot" : ""}`} onClick={() => onOpen("Mod Library")}>
          <Icon.refresh size={20} />
          <span>
            <span className="dl-stat-label">Updates</span>
            <span className="dl-stat-value">{updates}</span>
          </span>
        </button>
        <div className="dl-stat">
          <Icon.file size={20} />
          <span>
            <span className="dl-stat-label">Addon slots</span>
            <span className="dl-stat-value">{lib ? `${lib.slotsUsed}/99` : "–"}</span>
          </span>
        </div>
        <div className="grow" />
        <button className="btn ghost" onClick={() => onOpen("Hero Skins")}>
          <Icon.shirt size={15} /> Hero skins
        </button>
        <button className="btn-play dl-cta" onClick={() => onOpen("Mod Store")}>
          Browse store <Icon.arrowRight size={15} />
        </button>
      </div>

      {info && !info.installed && (
        <div className="dl-banner">
          <Icon.warn size={18} />
          <span className="grow">
            <b>Deadlock wasn't found in your Steam libraries.</b>
            <span className="muted">You can still browse and download mods; they go in once the game is installed.</span>
          </span>
        </div>
      )}

      <Shelf accent="Skins" mods={skinTrend} lib={lib} onSeeAll={() => seeAll("Skins")} />
      <Shelf accent="Mods" mods={modTrend} lib={lib} onSeeAll={() => seeAll("")} />
    </div>
  );
}

/* ------------------------------- library -------------------------------- */

function LibraryCard({ mod, index, total }: { mod: DlLibraryMod; index: number; total: number }) {
  const { run } = useLibraryActions();
  const { installing, install } = useInstaller();
  const [menu, setMenu] = useState(false);
  const moreRef = useRef<HTMLButtonElement>(null);
  const hero = dl.effectiveHero(mod);
  const variants = mod.files.length > 1;
  return (
    <div className={`dl-card lib ${mod.enabled ? "" : "off"}`}>
      <div className="dl-art">
        <ModArt src={mod.image} name={mod.name} hero={hero} />
        <div className="dl-badges">
          {mod.updateAvailable && <span className="dl-badge warn">Update available</span>}
          {variants && (
            <span className="dl-badge">
              {mod.files.filter((f) => f.enabled).length}/{mod.files.length} files
            </span>
          )}
        </div>
        {hero && <span className="dl-hero-tag">{hero}</span>}
        <span className="dl-prio" title="Load order: lower numbers win conflicts">
          #{index + 1}
        </span>
      </div>
      <div className="dl-body">
        <div className="dl-name" title={mod.name}>
          {mod.name}
        </div>
        <div className="dl-by">By {mod.author || "unknown"}</div>
        <div className="dl-foot">
          <Switch value={mod.enabled} onChange={(v) => void run(dl.setEnabled(mod.id, v))} label={`Enable ${mod.name}`} />
          <div className="row" style={{ gap: 6 }}>
            {mod.updateAvailable && mod.gamebananaId != null && (
              <button
                className="dl-icon-btn"
                title="Update"
                disabled={installing.has(mod.gamebananaId)}
                onClick={() => void install({ id: mod.gamebananaId!, name: mod.name })}
              >
                <Icon.refresh size={15} />
              </button>
            )}
            <button ref={moreRef} className={`dl-icon-btn ghost ${menu ? "on" : ""}`} title="More" onClick={() => setMenu((o) => !o)}>
              <Icon.dots size={15} />
            </button>
            <button className="dl-icon-btn danger" title="Delete" onClick={() => void run(dl.remove(mod.id), `${mod.name} deleted`)}>
              <Icon.trash size={15} />
            </button>
          </div>
        </div>
        <Popover
          anchor={moreRef}
          open={menu}
          onClose={() => setMenu(false)}
          placement="bottom-end"
          offset={8}
          className="row-menu surface dl-menu"
          style={{ width: 290 }}
        >
          <div className="dl-menu-name">{mod.name}</div>
          <label className="dl-menu-row">
            <span>Hero</span>
            <Select
              value={mod.heroOverride ?? ""}
              minWidth={150}
              onChange={(v) => void run(dl.setHero(mod.id, v || null))}
              options={[{ value: "", label: mod.hero ? `Auto (${mod.hero})` : "Not a hero skin" }, ...dl.HEROES.map((h) => ({ value: h, label: h }))]}
            />
          </label>
          <div className="dl-menu-row">
            <span>Load order</span>
            <span className="row" style={{ gap: 6 }}>
              <button className="dl-icon-btn ghost" disabled={index === 0} title="Higher priority" onClick={() => void run(dl.reorder(mod.id, -1))}>
                <Icon.arrowUp size={14} />
              </button>
              <button
                className="dl-icon-btn ghost"
                disabled={index === total - 1}
                title="Lower priority"
                onClick={() => void run(dl.reorder(mod.id, 1))}
              >
                <Icon.arrowDown size={14} />
              </button>
            </span>
          </div>
          {variants &&
            mod.files.map((f) => (
              <div className="dl-menu-row" key={f.name}>
                <span className="dl-file-label" title={f.label}>
                  {f.label}
                </span>
                <Switch value={f.enabled} onChange={(v) => void run(dl.setFileEnabled(mod.id, f.name, v))} label={f.label} />
              </div>
            ))}
          {mod.gamebananaId != null && (
            <button className="btn ghost dl-menu-link" onClick={() => void api.openUrl(`https://gamebanana.com/mods/${mod.gamebananaId}`)}>
              <Icon.link size={14} /> Open on GameBanana
            </button>
          )}
        </Popover>
      </div>
    </div>
  );
}

export function DeadlockLibrary({ onOpen }: { onOpen: Open }) {
  const { games, showToast } = useLauncher();
  const lib = dl.useDeadlockLibrary();
  const { run } = useLibraryActions();
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<"all" | "on" | "off" | "updates">("all");
  const [checking, setChecking] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  const mods = lib?.mods ?? [];
  const shown = mods
    .map((m, i) => ({ m, i }))
    .filter(
      ({ m }) =>
        (filter === "all" || (filter === "on" ? m.enabled : filter === "off" ? !m.enabled : m.updateAvailable)) &&
        (!q.trim() || `${m.name} ${m.author} ${dl.effectiveHero(m) ?? ""}`.toLowerCase().includes(q.trim().toLowerCase())),
    );
  const on = mods.filter((m) => m.enabled).length;

  const check = async () => {
    setChecking(true);
    const next = await run(dl.checkUpdates());
    setChecking(false);
    if (next) showToast(next.mods.some((m) => m.updateAvailable) ? "Updates found" : "Everything is up to date");
  };

  return (
    <div className="dl-page">
      <div className="dl-page-head">
        <div>
          <div className="dl-page-title">Mod Library</div>
          <div className="dl-page-sub">Mods you've downloaded. Switch them on and off any time.</div>
        </div>
        <div className="row wrap" style={{ gap: 8 }}>
          <span className="dl-count">
            <b>{on}</b>/{mods.length} mods · <b>{lib?.slotsUsed ?? 0}</b>/99 addon files
          </span>
          <button className="btn ghost" disabled={checking || mods.length === 0} onClick={() => void check()}>
            <Icon.refresh size={15} /> {checking ? "Checking…" : "Check for updates"}
          </button>
          {games?.deadlock.addons_dir && (
            <button className="btn ghost" onClick={() => void api.openPath(games.deadlock.addons_dir!)} title="Open the game's addons folder">
              <Icon.folder size={15} />
            </button>
          )}
        </div>
      </div>

      {lib?.syncError && (
        <div className="dl-banner warn">
          <Icon.warn size={18} />
          <span className="grow">
            <b>{lib.syncError}</b>
            <span className="muted">Your changes are saved and go into the game next time it's closed.</span>
          </span>
        </div>
      )}

      {!!lib?.unmanaged.length && !dismissed && (
        <div className="dl-banner">
          <Icon.warn size={18} />
          <span className="grow">
            <b>
              {lib.unmanaged.length} addon file{lib.unmanaged.length === 1 ? " isn't" : "s aren't"} in your library
            </b>
            <span className="muted">{lib.unmanaged.join(", ")} were added by hand or by another mod manager.</span>
          </span>
          <button className="btn-play dl-cta" onClick={() => void run(dl.adopt(), "Added to your library")}>
            Add to library
          </button>
          <button className="dl-icon-btn ghost" onClick={() => setDismissed(true)} aria-label="Dismiss">
            <Icon.close size={14} />
          </button>
        </div>
      )}

      <div className="dl-controls">
        <label className="dl-search">
          <Icon.search size={16} />
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search your mods" />
        </label>
        <div className="seg">
          {(
            [
              ["all", `All (${mods.length})`],
              ["on", `Enabled (${on})`],
              ["off", `Disabled (${mods.length - on})`],
              ["updates", "Updates"],
            ] as const
          ).map(([k, label]) => (
            <button key={k} className={filter === k ? "on" : ""} onClick={() => setFilter(k)}>
              {label}
            </button>
          ))}
        </div>
      </div>

      {lib && mods.length === 0 ? (
        <div className="dl-empty">
          <Icon.mods size={34} />
          <b>No mods yet</b>
          <span className="muted">Find skins, HUDs and sounds in the store. They show up here once downloaded.</span>
          <button className="btn-play dl-cta" onClick={() => onOpen("Mod Store")}>
            Browse store <Icon.arrowRight size={15} />
          </button>
        </div>
      ) : (
        <div className="dl-grid">
          {shown.map(({ m, i }) => (
            <LibraryCard key={m.id} mod={m} index={i} total={mods.length} />
          ))}
          {lib && shown.length === 0 && <div className="muted">No mods match.</div>}
        </div>
      )}
      <p className="muted dl-note">
        Load order follows the <b>#</b> on each card: when two mods change the same thing, the lower number wins. Changes go straight into the game's
        addons folder.
      </p>
    </div>
  );
}

/* -------------------------------- store --------------------------------- */

const SORTS: { value: DlSort; label: string }[] = [
  { value: "liked", label: "Most liked" },
  { value: "viewed", label: "Most viewed" },
  { value: "updated", label: "Recently updated" },
  { value: "newest", label: "Newest" },
];

let categoriesCache: DlCategory[] | null = null;

export function DeadlockStore() {
  const lib = dl.useDeadlockLibrary();
  const owned = ownedBy(lib);
  const { installing, install } = useInstaller();
  const [intent] = useState(() => storeIntent);
  useEffect(() => {
    storeIntent = null;
  }, []);
  const [q, setQ] = useState(intent?.search ?? "");
  const [query, setQuery] = useState(q);
  const [sort, setSort] = useState<DlSort>("liked");
  const [cats, setCats] = useState<DlCategory[]>(categoriesCache ?? []);
  const [cat, setCat] = useState<number | null>(null);
  const [catName, setCatName] = useState(intent?.category ?? "");
  const [hideNsfw, setHideNsfw] = useState(true);
  const [mods, setMods] = useState<DlStoreMod[]>([]);
  const [page, setPage] = useState(1);
  const [complete, setComplete] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<DlStoreMod | null>(null);

  useEffect(() => {
    dl.categories()
      .then((c) => {
        categoriesCache = c;
        setCats(c);
      })
      .catch(() => {});
  }, []);
  // A category asked for by name ("See all skins") resolves once the list loads.
  useEffect(() => {
    if (!catName) return;
    const c = cats.find((x) => x.name.toLowerCase() === catName.toLowerCase());
    if (c) {
      setCat(c.id);
      setCatName("");
    }
  }, [cats, catName]);
  // Search as you type, after a short pause.
  useEffect(() => {
    const t = setTimeout(() => setQuery(q), 350);
    return () => clearTimeout(t);
  }, [q]);

  const load = (p: number) => {
    setLoading(true);
    setError(null);
    dl.store(sort, query, cat, p)
      .then((r) => {
        setMods((prev) => (p === 1 ? r.mods : [...prev, ...r.mods.filter((m) => !prev.some((x) => x.id === m.id))]));
        setComplete(r.complete);
        setPage(p);
      })
      .catch((e) => setError(`${e}`))
      .finally(() => setLoading(false));
  };
  useEffect(() => {
    if (catName && !cats.length) return; // wait for the category to resolve
    load(1);
  }, [sort, query, cat, catName, cats.length]);

  const shown = mods.filter((m) => !(hideNsfw && m.nsfw));
  const filters = [cat != null && cats.find((c) => c.id === cat)?.name, hideNsfw && "Hide NSFW", query && `“${query}”`].filter(Boolean) as string[];

  return (
    <div className="dl-page">
      <div className="dl-page-head">
        <div>
          <div className="dl-page-title">Mod Store</div>
          <div className="dl-page-sub">Every Deadlock mod on GameBanana, ready to download.</div>
        </div>
      </div>

      <div className="dl-controls">
        <label className="dl-search wide">
          <Icon.search size={16} />
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search mods, heroes, authors" />
          {q && (
            <button className="dl-clear" onClick={() => setQ("")} aria-label="Clear search">
              <Icon.close size={13} />
            </button>
          )}
        </label>
        <label className="row" style={{ gap: 8, alignItems: "center" }}>
          <Switch value={hideNsfw} onChange={setHideNsfw} label="Hide NSFW" />
          <span className="muted" style={{ fontSize: 12.5 }}>
            Hide NSFW
          </span>
        </label>
        <Select value={sort} onChange={(v) => setSort(v as DlSort)} options={SORTS} minWidth={170} disabled={!!query} />
      </div>

      {cats.length > 0 && (
        <div className="dl-chips">
          <button className={`dl-chip ${cat == null ? "on" : ""}`} onClick={() => setCat(null)}>
            All
          </button>
          {cats.map((c) => (
            <button key={c.id} className={`dl-chip ${cat === c.id ? "on" : ""}`} onClick={() => setCat(c.id)}>
              {c.name}
              {c.count > 0 && <span>{dl.fmtCount(c.count)}</span>}
            </button>
          ))}
        </div>
      )}
      {filters.length > 0 && (
        <div className="dl-filters">
          Including:{" "}
          {filters.map((f) => (
            <span key={f}>{f}</span>
          ))}
          {(cat != null || query) && (
            <button
              className="linklike"
              onClick={() => {
                setCat(null);
                setQ("");
              }}
            >
              Clear all
            </button>
          )}
        </div>
      )}

      {error && (
        <div className="dl-banner warn">
          <Icon.warn size={18} />
          <span className="grow">
            <b>GameBanana didn't answer</b>
            <span className="muted">{error}</span>
          </span>
          <button className="btn ghost" onClick={() => load(1)}>
            Retry
          </button>
        </div>
      )}

      <div className="dl-grid">
        {shown.map((m) => (
          <StoreCard
            key={m.id}
            mod={m}
            owned={owned.get(m.id)}
            busy={installing.has(m.id)}
            onInstall={() => void install(m)}
            onOpen={() => setDetail(m)}
          />
        ))}
        {loading && mods.length === 0 && Array.from({ length: 10 }, (_, i) => <div key={i} className="dl-card skeleton" />)}
      </div>
      {!loading && !error && shown.length === 0 && <p className="muted">No mods match those filters.</p>}
      {!complete && mods.length > 0 && (
        <div className="row" style={{ justifyContent: "center", marginTop: 16 }}>
          <button className="btn ghost" disabled={loading} onClick={() => load(page + 1)}>
            {loading ? "Loading…" : "Load more"}
          </button>
        </div>
      )}

      {detail && (
        <ModDetailModal
          mod={detail}
          owned={owned.get(detail.id)}
          busy={installing.has(detail.id)}
          onInstall={(f) => void install(detail, f)}
          onClose={() => setDetail(null)}
        />
      )}
    </div>
  );
}

/* ------------------------------ hero skins ------------------------------ */

export function DeadlockSkins({ onOpen }: { onOpen: Open }) {
  const lib = dl.useDeadlockLibrary();
  const { run } = useLibraryActions();
  const [q, setQ] = useState("");
  const byHero = useMemo(() => {
    const map = new Map<string, DlLibraryMod[]>();
    lib?.mods.forEach((m) => {
      const h = dl.effectiveHero(m);
      if (h) map.set(h, [...(map.get(h) ?? []), m]);
    });
    return map;
  }, [lib]);
  // Heroes with skins first, then everyone else, alphabetically within each.
  const heroes = useMemo(() => {
    const all = Array.from(new Set([...dl.HEROES, ...byHero.keys()]));
    return all
      .filter((h) => !q.trim() || h.toLowerCase().includes(q.trim().toLowerCase()))
      .sort((a, b) => Number(byHero.has(b)) - Number(byHero.has(a)) || a.localeCompare(b));
  }, [byHero, q]);
  const [hero, setHero] = useState<string | null>(null);
  const current = hero ?? heroes[0] ?? null;
  const skins = current ? (byHero.get(current) ?? []) : [];
  const active = skins.find((s) => s.enabled);

  return (
    <div className="dl-page">
      <div className="dl-page-head">
        <div>
          <div className="dl-page-title">Hero Skins</div>
          <div className="dl-page-sub">Pick the skin each hero wears when you have more than one.</div>
        </div>
      </div>

      <div className="dl-skins">
        <div className="dl-heroes">
          <label className="dl-search">
            <Icon.search size={15} />
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search heroes" />
          </label>
          <div className="dl-hero-list">
            {heroes.map((h) => {
              const list = byHero.get(h) ?? [];
              const on = list.find((s) => s.enabled);
              return (
                <button key={h} className={`dl-hero ${h === current ? "on" : ""}`} onClick={() => setHero(h)}>
                  <span className="dl-hero-av">{h.slice(0, 1)}</span>
                  <span className="grow">
                    <span className="dl-hero-name">{h}</span>
                    <span className="dl-hero-sub">{on ? on.name : "Default"}</span>
                  </span>
                  {list.length > 0 && <span className="dl-hero-count">{list.length}</span>}
                </button>
              );
            })}
          </div>
        </div>

        <div className="dl-skin-pane">
          {current && (
            <>
              <div className="dl-skin-head">
                <div>
                  <div className="dl-skin-title">{current}</div>
                  <div className="muted">
                    {skins.length} skin{skins.length === 1 ? "" : "s"} downloaded
                  </div>
                </div>
                <button
                  className="btn ghost"
                  onClick={() => {
                    storeIntent = { search: current, category: "Skins" };
                    onOpen("Mod Store");
                  }}
                >
                  <Icon.search size={15} /> Find skins for {current}
                </button>
              </div>
              <div className="dl-skin-grid">
                <button className={`dl-skin ${!active ? "on" : ""}`} onClick={() => void run(dl.setActiveSkin(current, null))}>
                  <span className="dl-skin-art plain">Default</span>
                  <span className="dl-skin-body">
                    <b>Default</b>
                    <span className="muted">Vanilla look, no skin</span>
                  </span>
                  {!active && (
                    <span className="dl-skin-check">
                      <Icon.check size={14} />
                    </span>
                  )}
                </button>
                {skins.map((s) => (
                  <button key={s.id} className={`dl-skin ${s.enabled ? "on" : ""}`} onClick={() => void run(dl.setActiveSkin(current, s.id))}>
                    <span className="dl-skin-art">
                      <ModArt src={s.image} name={s.name} hero={current} />
                    </span>
                    <span className="dl-skin-body">
                      <b title={s.name}>{s.name}</b>
                      <span className="muted">{s.author}</span>
                    </span>
                    {s.enabled && (
                      <span className="dl-skin-check">
                        <Icon.check size={14} />
                      </span>
                    )}
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
      <p className="muted dl-note">
        Skin showing under the wrong hero? Set its hero from the <b>⋯</b> menu in{" "}
        <button className="linklike" onClick={() => onOpen("Mod Library")}>
          Mod Library
        </button>
        .
      </p>
    </div>
  );
}

/* -------------------------------- extras -------------------------------- */

/** Community-known client commands, like Deadlock Mod Manager's library.
 *  Valve can change or remove any of these in an update. */
const COMMANDS: { group: string; cmd: string; desc: string }[] = [
  { group: "Performance", cmd: "fps_max 240", desc: "Main FPS limit. 0 means unlimited." },
  { group: "Performance", cmd: "fps_max_ui 144", desc: "FPS limit while menus are open." },
  { group: "Performance", cmd: "engine_no_focus_sleep 0", desc: "Keep full FPS when the window isn't focused." },
  { group: "Performance", cmd: "r_citadel_ssao 0", desc: "Turn off ambient occlusion for more FPS." },
  { group: "Network", cmd: "rate 786432", desc: "Max bytes per second the server may send." },
  { group: "Network", cmd: "cl_interp_ratio 1", desc: "Interpolation ratio. Lower feels snappier on a stable connection." },
  { group: "HUD", cmd: "cl_showfps 1", desc: "Show an FPS counter." },
  { group: "HUD", cmd: "citadel_hud_damage_numbers_enabled 1", desc: "Show floating damage numbers." },
  { group: "Mouse", cmd: "m_rawinput 1", desc: "Read the mouse directly, ignoring Windows acceleration." },
  { group: "Mouse", cmd: "sensitivity 1.2", desc: "Mouse sensitivity." },
];

const LAUNCH_FLAGS: { flag: string; desc: string }[] = [
  { flag: "-novid", desc: "Skip the intro video" },
  { flag: "-high", desc: "High CPU priority" },
  { flag: "-dx11", desc: "DirectX 11" },
  { flag: "-vulkan", desc: "Vulkan renderer" },
  { flag: "-fullscreen", desc: "Fullscreen" },
  { flag: "-windowed", desc: "Windowed" },
  { flag: "-console", desc: "Open the console" },
];

export function DeadlockExtras() {
  const { showToast, refreshGames } = useLauncher();
  const [x, setX] = useState<DlExtras | null>(null);
  const [opts, setOpts] = useState("");
  const [cfg, setCfg] = useState("");
  const [group, setGroup] = useState("All");
  useEffect(() => {
    dl.extras()
      .then((e) => {
        setX(e);
        setOpts(e.launchOptions);
        setCfg(e.autoexec);
      })
      .catch((e) => showToast(`${e}`));
  }, []);

  const act = async (p: Promise<DlExtras | string>) => {
    try {
      const r = await p;
      if (typeof r === "string") {
        showToast(r);
        setX(await dl.extras());
      } else {
        setX(r);
      }
      void refreshGames();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  const tokens = opts.split(/\s+/).filter(Boolean);
  const toggleFlag = (f: string) => setOpts((tokens.includes(f) ? tokens.filter((t) => t !== f) : [...tokens, f]).join(" "));
  const cfgLines = cfg.split(/\r?\n/).map((l) => l.trim());
  const has = (cmd: string) => cfgLines.some((l) => l.split(/\s+/)[0] === cmd.split(" ")[0]);
  const addCmd = (cmd: string) => setCfg((c) => `${c.replace(/\s*$/, "")}${c.trim() ? "\n" : ""}${cmd}\n`);
  const groups = ["All", ...Array.from(new Set(COMMANDS.map((c) => c.group)))];

  return (
    <div className="dl-page">
      <div className="dl-page-head">
        <div>
          <div className="dl-page-title">Extras</div>
          <div className="dl-page-sub">Launch options, autoexec and the game's own files.</div>
        </div>
      </div>

      <div className="dl-extras">
        <section className="dl-panel">
          <div className="dl-panel-title">
            <Icon.terminal size={16} /> Launch options
          </div>
          <div className="dl-panel-sub">Added every time the launcher starts Deadlock, with or without mods.</div>
          <div className="row" style={{ gap: 8 }}>
            <input className="input grow mono" value={opts} onChange={(e) => setOpts(e.target.value)} placeholder="-novid -high" spellCheck={false} />
            <button className="btn-play dl-cta" disabled={!x || opts.trim() === x.launchOptions} onClick={() => void act(dl.setLaunchOptions(opts))}>
              Save
            </button>
          </div>
          <div className="dl-chips">
            {LAUNCH_FLAGS.map((f) => (
              <button key={f.flag} className={`dl-chip ${tokens.includes(f.flag) ? "on" : ""}`} title={f.desc} onClick={() => toggleFlag(f.flag)}>
                {f.flag}
              </button>
            ))}
          </div>
          {x && (
            <div className="dl-codeline">
              <span className="muted">Launches with</span> <code>{x.effectiveArgs || "no extra options"}</code>
            </div>
          )}
        </section>

        <section className="dl-panel">
          <div className="dl-panel-title">
            <Icon.sliders size={16} /> Game files
          </div>
          <div className="dl-panel-sub">Mods load through gameinfo.gi. The launcher turns this on for modded launches and off for vanilla ones.</div>
          <div className="dl-setting">
            <span className="grow">
              <b>Load mods</b>
              <span className="muted">
                {x?.gameinfoFound === false
                  ? "gameinfo.gi wasn't found"
                  : x?.modsActive
                    ? "citadel/addons is in gameinfo.gi"
                    : "Deadlock starts vanilla"}
              </span>
            </span>
            <Switch value={!!x?.modsActive} onChange={(v) => void act(dl.setModsActive(v))} label="Load mods" />
          </div>
          <div className="dl-setting">
            <span className="grow">
              <b>Reset gameinfo.gi</b>
              <span className="muted">
                {x?.backupExists ? "Restores the copy saved before the launcher first changed it." : "Takes the mod paths back out."}
              </span>
            </span>
            <button className="btn ghost" onClick={() => void act(dl.resetGameinfo())}>
              <Icon.refresh size={14} /> Reset
            </button>
          </div>
          <div className="dl-setting">
            <span className="grow">
              <b>Remove mod files from the game</b>
              <span className="muted">Clears the launcher's files from addons. Your library is kept.</span>
            </span>
            <button className="btn ghost danger" onClick={() => void act(dl.clearAddons())}>
              <Icon.trash size={14} /> Remove
            </button>
          </div>
          <div className="dl-setting">
            <span className="grow">
              <b>Verify game files</b>
              <span className="muted">Asks Steam to repair Deadlock if something broke after an update.</span>
            </span>
            <button className="btn ghost" onClick={() => void api.openUrl("steam://validate/1422450")}>
              <Icon.check size={14} /> Verify
            </button>
          </div>
          <div className="row wrap" style={{ gap: 8, marginTop: 4 }}>
            {x?.installDir && (
              <button className="btn ghost" onClick={() => void api.openPath(x.installDir!)}>
                <Icon.folder size={14} /> Game folder
              </button>
            )}
            {x?.addonsDir && (
              <button className="btn ghost" onClick={() => void api.openPath(x.addonsDir!)}>
                <Icon.folder size={14} /> Addons
              </button>
            )}
            {x && (
              <button className="btn ghost" onClick={() => void api.openPath(x.libraryDir)}>
                <Icon.folder size={14} /> Downloaded mods
              </button>
            )}
          </div>
        </section>

        <section className="dl-panel span">
          <div className="dl-panel-title">
            <Icon.file size={16} /> Autoexec config
          </div>
          <div className="dl-panel-sub">
            Console commands that run every time the game starts. The launcher adds <code>+exec autoexec</code> for you.
          </div>
          <div className="dl-warn">
            <Icon.warn size={15} /> These commands are community-sourced and Valve can change or remove them in any update. Test what you add.
          </div>
          <div className="dl-autoexec">
            <div className="dl-cmds">
              <div className="dl-chips">
                {groups.map((g) => (
                  <button key={g} className={`dl-chip ${group === g ? "on" : ""}`} onClick={() => setGroup(g)}>
                    {g}
                  </button>
                ))}
              </div>
              <div className="dl-cmd-list">
                {COMMANDS.filter((c) => group === "All" || c.group === group).map((c) => (
                  <div className="dl-cmd" key={c.cmd}>
                    <span className="grow">
                      <code>{c.cmd}</code>
                      <span className="muted">{c.desc}</span>
                    </span>
                    <button className="btn ghost" disabled={has(c.cmd)} onClick={() => addCmd(c.cmd)}>
                      {has(c.cmd) ? <Icon.check size={14} /> : <Icon.plus size={14} />} {has(c.cmd) ? "Added" : "Add"}
                    </button>
                  </div>
                ))}
              </div>
            </div>
            <div className="dl-cfg">
              <textarea
                className="input mono"
                value={cfg}
                onChange={(e) => setCfg(e.target.value)}
                spellCheck={false}
                placeholder="// one command per line"
              />
              <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 8 }}>
                <span className="muted dl-path" title={x?.autoexecPath ?? ""}>
                  {x?.autoexecPath ?? ""}
                </span>
                <button className="btn-play dl-cta" disabled={!x || cfg === x.autoexec} onClick={() => void act(dl.saveAutoexec(cfg))}>
                  Save autoexec
                </button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
