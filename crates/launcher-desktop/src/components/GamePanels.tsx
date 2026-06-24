import { useEffect, useState } from "react";

import { useLauncher, loadServers, saveServers } from "../store";
import * as api from "../lib/api";
import type { GameKey, ServerEntry } from "../lib/types";
import { Field, Pill, Icon, Select, HostAddress } from "./ui";

/* ------------------------------ Home ----------------------------------- */

const GAME_TILES: {
  id: GameKey;
  name: string;
  tag: string;
  rgb: string;
  icon: (p: { size?: number }) => JSX.Element;
}[] = [
  { id: "minecraft", name: "Minecraft", tag: "Instances · servers · modpacks", rgb: "61, 220, 132", icon: Icon.minecraft },
  { id: "skyrim", name: "Skyrim", tag: "SKSE · Skyrim Together co-op", rgb: "158, 197, 255", icon: Icon.skyrim },
  { id: "eldenring", name: "Elden Ring", tag: "Seamless Co-op · Mod Engine 2", rgb: "243, 201, 105", icon: Icon.elden },
  { id: "cyberpunk", name: "Cyberpunk 2077", tag: "CyberpunkMP · Cyber Engine Tweaks", rgb: "252, 238, 10", icon: Icon.cyberpunk },
];

/** Landing page — big game tiles in the launcher's own aurora palette. */
/** Human "time ago" from a unix-seconds timestamp. */
function timeAgo(sec: number): string {
  if (!sec) return "";
  const d = Math.floor(Date.now() / 1000) - sec;
  if (d < 90) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)}m ago`;
  if (d < 86400) return `${Math.floor(d / 3600)}h ago`;
  if (d < 7 * 86400) return `${Math.floor(d / 86400)}d ago`;
  return `${Math.floor(d / (7 * 86400))}w ago`;
}
function playtime(sec: number): string {
  if (sec < 60) return "—";
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

const KIND_ICON: Record<string, (p: { size?: number }) => JSX.Element> = {
  instance: Icon.minecraft,
  skyrim: Icon.skyrim,
  eldenring: Icon.elden,
  cyberpunk: Icon.cyberpunk,
};

/** "Continue playing" — recently played instances & games with playtime. */
function RecentlyPlayed({ onContinue }: { onContinue: (r: api.PlayRecord) => void }) {
  const [recs, setRecs] = useState<api.PlayRecord[]>([]);
  useEffect(() => {
    api.playStats().then((r) => setRecs(r.slice(0, 6))).catch(() => {});
  }, []);
  if (recs.length === 0) return null;

  return (
    <div className="recent">
      <div className="sect-title" style={{ marginBottom: 10 }}>Jump back in</div>
      <div className="recent-grid">
        {recs.map((r) => {
          const KIcon = KIND_ICON[r.kind] ?? Icon.play;
          return (
            <button key={r.key} className="recent-card" onClick={() => onContinue(r)}>
              <span className="recent-art">
                {r.icon ? <img src={r.icon} alt="" /> : <KIcon size={24} />}
              </span>
              <span className="recent-info">
                <span className="recent-name">{r.name}</span>
                <span className="recent-sub">
                  {playtime(r.totalSeconds)} played · {timeAgo(r.lastPlayed)}
                </span>
              </span>
              <span className="recent-go">
                <Icon.play size={13} /> Continue
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

interface LastSession {
  id: string;
  name: string;
  address: string;
}

export function HomePanel({
  onSelect,
  onContinue,
  onRejoin,
}: {
  onSelect: (g: GameKey) => void;
  onContinue: (r: api.PlayRecord) => void;
  onRejoin: (s: LastSession) => void;
}) {
  const [last, setLast] = useState<LastSession | null>(null);
  useEffect(() => {
    try {
      const s = localStorage.getItem("aurora:lastSession");
      if (s) setLast(JSON.parse(s));
    } catch {
      /* ignore */
    }
  }, []);

  return (
    <div className="hero">
      <div className="eyebrow">Welcome to</div>
      <h1 className="title">Aurora</h1>
      <p className="subtitle">One launcher for playing, modding and hosting — together.</p>

      {last && (
        <button className="rejoin-card" onClick={() => onRejoin(last)}>
          <Icon.coop size={18} />
          <span className="grow">
            <b>Rejoin {last.name}</b>
            <span className="muted" style={{ display: "block", fontSize: 12 }}>{last.address}</span>
          </span>
          <Icon.play size={14} />
        </button>
      )}

      <RecentlyPlayed onContinue={onContinue} />

      <div className="home-grid">
        {GAME_TILES.map((t) => {
          const TileIcon = t.icon;
          return (
            <button
              key={t.id}
              className="game-tile"
              style={{ "--t": t.rgb } as React.CSSProperties}
              onClick={() => onSelect(t.id)}
            >
              <span className="tile-icon">
                <TileIcon size={30} />
              </span>
              <span className="tile-name">{t.name}</span>
              <span className="tile-tag">{t.tag}</span>
              <span className="tile-go">
                <Icon.play size={14} /> Open
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function NotInstalled({ title }: { title: string }) {
  return (
    <div className="hero">
      <div className="eyebrow">Not detected</div>
      <h1 className="title">{title} isn't installed</h1>
      <p className="muted" style={{ marginTop: 8 }}>
        Aurora scans your Steam libraries for this game. Install it through Steam (or add the library
        that holds it), then hit refresh on the Play tab — your saved settings stay intact.
      </p>
    </div>
  );
}

/** Shared saved-server list for co-op tabs (stored locally per game). */
function CoopServers({
  game,
  hint,
  onJoin,
}: {
  game: string;
  hint: string;
  /** When provided, each saved server gets a one-click Join (copies the
   *  address then launches the game's co-op client). */
  onJoin?: (address: string) => void;
}) {
  const { showToast } = useLauncher();
  const [list, setList] = useState<ServerEntry[]>(() => loadServers(game));
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");

  const persist = (next: ServerEntry[]) => {
    setList(next);
    saveServers(game, next);
  };

  return (
    <>
      <div className="row wrap" style={{ alignItems: "flex-end" }}>
        <Field label="Server name">
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Friends" />
        </Field>
        <Field label="Address : port">
          <input
            className="input"
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            placeholder="123.45.67.89:10578"
          />
        </Field>
        <button
          className="btn"
          onClick={() => {
            if (!name.trim() || !address.trim()) return;
            persist([...list, { id: crypto.randomUUID(), name: name.trim(), address: address.trim() }]);
            setName("");
            setAddress("");
          }}
        >
          <Icon.plus size={16} /> Add
        </button>
      </div>

      <div className="col" style={{ gap: 2, marginTop: 6 }}>
        {list.length === 0 && <p className="muted">No co-op servers saved yet.</p>}
        {list.map((s) => (
          <div className="lrow" key={s.id}>
            <div className="avatar">
              <Icon.coop size={18} />
            </div>
            <div className="grow">
              <div className="name">{s.name}</div>
              <div className="sub">{s.address}</div>
            </div>
            {onJoin && (
              <button className="btn-play" style={{ padding: "8px 14px" }} onClick={() => onJoin(s.address)}>
                <Icon.coop size={15} /> Join
              </button>
            )}
            <button
              className="btn ghost"
              onClick={() => {
                navigator.clipboard?.writeText(s.address);
                showToast("Address copied");
              }}
            >
              <Icon.copy size={15} /> Copy
            </button>
            <button className="btn danger ghost" onClick={() => persist(list.filter((x) => x.id !== s.id))}>
              <Icon.trash size={15} />
            </button>
          </div>
        ))}
      </div>
      <p className="muted">{hint}</p>
    </>
  );
}

/* ----------------------------- Skyrim ---------------------------------- */

/** Guided Skyrim Together setup: the mod itself + the Address Library it
 *  requires at runtime. Both are Nexus-only, so each is two clicks. */
function TogetherSetup() {
  const { games, installTogether, refreshGames, showToast, busy } = useLauncher();
  const sky = games?.skyrim;
  const needTogether = !sky?.has_skyrim_together;
  const needAddrLib = !sky?.has_address_library;

  const installAddrLib = async () => {
    try {
      showToast(await api.installAddressLibrary());
      await refreshGames();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  return (
    <div className="surface" style={{ padding: 16, borderRadius: 16, marginTop: 10 }}>
      <div style={{ fontWeight: 600, marginBottom: 6 }}>Set up Skyrim Together Reborn</div>
      <p className="muted" style={{ marginBottom: 10 }}>
        Nexus Mods requires signed-in downloads, so each part is two clicks: grab the file, then
        Aurora finds it in your Downloads and installs it to the right place.
      </p>
      {needTogether && (
        <div className="row wrap" style={{ marginBottom: needAddrLib ? 10 : 0 }}>
          <Pill tone="warn">Skyrim Together</Pill>
          <button className="btn" onClick={() => api.openTogetherPage()}>
            <Icon.link size={15} /> 1 · Open download page
          </button>
          <button className="btn" disabled={busy} onClick={installTogether}>
            <Icon.check size={15} /> 2 · I downloaded it — install
          </button>
        </div>
      )}
      {needAddrLib && (
        <div className="row wrap">
          <Pill tone="warn">Address Library (required)</Pill>
          <button className="btn" onClick={() => api.openAddressLibraryPage()}>
            <Icon.link size={15} /> 1 · Open download page
          </button>
          <button className="btn" disabled={busy} onClick={installAddrLib}>
            <Icon.check size={15} /> 2 · I downloaded it — install
          </button>
        </div>
      )}
      {needAddrLib && (
        <p className="muted" style={{ marginTop: 8, fontSize: 12.5 }}>
          On the Address Library page download <b>"All in one"</b> for your Skyrim version (1.6.x
          on current Steam). Without it Skyrim Together fails with "Failed to load Skyrim Address
          Library".
        </p>
      )}
    </div>
  );
}

export function SkyrimPlay() {
  const { games, launchSkyrim, refreshGames, installTool, busy } = useLauncher();
  const sky = games?.skyrim;

  return (
    <div className="hero">
      <div className="eyebrow">The Elder Scrolls V</div>
      <h1 className="title">Skyrim Special Edition</h1>
      <p className="subtitle">{sky?.installed ? sky.install_dir : "Detecting your Steam install…"}</p>

      <div className="action-bar surface">
        <div className="row wrap">
          <Pill tone={sky?.installed ? "ok" : "warn"}>
            {sky?.installed ? (sky.source === "epic" ? "Installed · Epic" : "Installed") : "Not found"}
          </Pill>
          <Pill tone={sky?.has_skse ? "ok" : "default"}>SKSE {sky?.has_skse ? "ready" : "—"}</Pill>
          <Pill tone={sky?.has_skyrim_together ? "ok" : "default"}>
            Skyrim Together {sky?.has_skyrim_together ? "ready" : "—"}
          </Pill>
        </div>
        <button className="btn-play" disabled={!sky?.installed} onClick={() => launchSkyrim("vanilla")}>
          <Icon.play size={20} /> Play
        </button>
      </div>

      <div className="sect" style={{ marginTop: 28 }}>
        <div className="sect-head">
          <div className="sect-title">Launch options</div>
          <button className="btn ghost" onClick={refreshGames}>
            <Icon.refresh size={15} /> Refresh
          </button>
        </div>
        <div className="row wrap">
          <button className="btn" disabled={!sky?.installed} onClick={() => launchSkyrim("vanilla")}>
            Vanilla / Official
          </button>
          <button className="btn" disabled={!sky?.has_skse} onClick={() => launchSkyrim("skse")}>
            SKSE (modded)
          </button>
          <button className="btn" disabled={!sky?.has_skyrim_together} onClick={() => launchSkyrim("together")}>
            <Icon.coop size={16} /> Skyrim Together Reborn
          </button>
        </div>

        {sky?.installed && (!sky.has_skse || !sky.has_skyrim_together || !sky.has_address_library) && (
          <>
            <div className="sect-head" style={{ marginTop: 18 }}>
              <div className="sect-title">One-click setup</div>
            </div>
            <div className="row wrap">
              {!sky.has_skse && (
                <button className="btn" disabled={busy} onClick={() => installTool("skse")}>
                  <Icon.upgrade size={15} /> Install SKSE64
                </button>
              )}
            </div>
            {(!sky.has_skyrim_together || !sky.has_address_library) && <TogetherSetup />}
          </>
        )}
      </div>
    </div>
  );
}

/** Stable id of the Skyrim Together server in the shared server machinery. */
const STR_SERVER_ID = "skyrim:together";

/** Host a Skyrim Together session: configure + launch the dedicated server. */
function SkyrimHost() {
  const { showToast, serverStatuses, openConsole, stopServer } = useLauncher();
  const [cfg, setCfg] = useState<api.TogetherServerConfig | null>(null);
  const [busy, setBusy] = useState(false);
  const running = !!serverStatuses[STR_SERVER_ID]?.running;

  useEffect(() => {
    api.skyrimServerConfig().then(setCfg).catch(() => {});
  }, []);

  if (!cfg) return null;
  const set = (p: Partial<api.TogetherServerConfig>) => setCfg({ ...cfg, ...p });

  const start = async () => {
    setBusy(true);
    try {
      await api.saveSkyrimServerConfig(cfg);
      await api.startSkyrimServer();
      openConsole(STR_SERVER_ID); // embedded dashboard, same as Minecraft
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(false);
    }
  };

  const toggles: [keyof api.TogetherServerConfig, string][] = [
    ["pvp", "PvP"],
    ["deathSystem", "Death system"],
    ["xpSync", "Sync XP"],
    ["itemDrops", "Item drops"],
    ["autoPartyJoin", "Auto party-join"],
  ];

  return (
    <div className="surface" style={{ padding: 16, borderRadius: 16 }}>
      {!cfg.available ? (
        <p className="muted">
          The Together dedicated server isn't present — reinstall Skyrim Together to host.
        </p>
      ) : (
        <>
          <div className="row wrap" style={{ alignItems: "flex-end" }}>
            <Field label="Server name">
              <input className="input" value={cfg.serverName} onChange={(e) => set({ serverName: e.target.value })} />
            </Field>
            <Field label="Password (optional)">
              <input className="input" value={cfg.password} onChange={(e) => set({ password: e.target.value })} placeholder="none" />
            </Field>
            <Field label="Max players">
              <input className="input" style={{ width: 90 }} type="number" value={cfg.maxPlayers} onChange={(e) => set({ maxPlayers: Number(e.target.value) || 1 })} />
            </Field>
            <Field label="Port">
              <input className="input" style={{ width: 100 }} type="number" value={cfg.port} onChange={(e) => set({ port: Number(e.target.value) || 10578 })} />
            </Field>
            <Field label="Difficulty">
              <Select
                minWidth={150}
                value={String(cfg.difficulty)}
                onChange={(v) => set({ difficulty: Number(v) })}
                options={[
                  { value: "0", label: "Novice" },
                  { value: "1", label: "Apprentice" },
                  { value: "2", label: "Adept" },
                  { value: "3", label: "Expert" },
                  { value: "4", label: "Master" },
                  { value: "5", label: "Legendary" },
                ]}
              />
            </Field>
          </div>
          <div className="row wrap" style={{ gap: 18, marginTop: 12 }}>
            {toggles.map(([key, label]) => (
              <label key={key} className="row" style={{ gap: 7, alignItems: "center", cursor: "pointer" }}>
                <input type="checkbox" checked={cfg[key] as boolean} onChange={(e) => set({ [key]: e.target.checked } as Partial<api.TogetherServerConfig>)} />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <div className="row" style={{ marginTop: 14, gap: 10, alignItems: "center" }}>
            {running ? (
              <>
                <button className="btn" onClick={() => openConsole(STR_SERVER_ID)}>
                  <Icon.terminal size={15} /> Dashboard
                </button>
                <button className="btn danger" onClick={() => stopServer(STR_SERVER_ID)}>
                  <Icon.stop size={13} /> Stop
                </button>
              </>
            ) : (
              <button className="btn-play" disabled={busy} onClick={start}>
                <Icon.host size={16} /> {busy ? "Starting…" : "Save & start server"}
              </button>
            )}
          </div>
          <HostAddress port={cfg.port} onCopy={showToast} />
        </>
      )}
    </div>
  );
}

export function SkyrimCoop() {
  const { games, launchSkyrim } = useLauncher();
  const sky = games?.skyrim;
  if (!sky?.installed) return <NotInstalled title="Skyrim" />;

  const ready = sky.has_skyrim_together && sky.has_address_library;

  return (
    <div className="sect">
      {!ready && <TogetherSetup />}

      {/* Join */}
      <div className="sect-head" style={{ marginTop: ready ? 0 : 16 }}>
        <div className="sect-title">Join a friend</div>
        <button
          className="btn-play"
          style={{ padding: "11px 22px", fontSize: 14 }}
          disabled={!ready}
          onClick={() => launchSkyrim("together")}
        >
          <Icon.coop size={17} /> Launch Together
        </button>
      </div>
      <ol className="steps">
        <li>On the <b>Aurora Net</b> screen, paste your friend's <b>friend code</b> to join their network — they'll show up in your Friends list with their address.</li>
        <li>Hit <b>Launch Together</b>, then open the <b>Skyrim Together</b> menu in-game.</li>
        <li><b>Paste</b> the host's address (e.g. <code className="md-code">100.x.x.x:10578</code>) into the server field and click <b>Connect</b>.</li>
      </ol>

      {/* Host */}
      <div className="sect-head" style={{ marginTop: 22 }}>
        <div className="sect-title">Host a session</div>
      </div>
      <p className="muted">Run your own Skyrim Together server, then connect to it from the game. Your friends use the address shown below — share it with no port forwarding via <b>Aurora Net</b>.</p>
      <SkyrimHost />
    </div>
  );
}

const MOD_CATEGORIES = ["All", "Graphics", "Weather & Lighting", "Interface", "Essentials", "Gameplay"];
const CAT_COLOR: Record<string, string> = {
  Graphics: "#22d3ee",
  "Weather & Lighting": "#a78bfa",
  Interface: "#34d399",
  Essentials: "#f59e0b",
  Gameplay: "#f472b6",
};
const PER_PAGE = 6;
const fmtN = (n?: number | null) =>
  !n ? "0" : n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(0)}K` : String(n);

/** Full-detail modal for one mod: flickable screenshot gallery + description. */
function ModDetailModal({ mod, onClose }: { mod: api.CatalogMod; onClose: () => void }) {
  const { showToast, refreshGames } = useLauncher();
  const [d, setD] = useState<api.ModDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [i, setI] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setLoading(true);
    api.skyrimModDetail(mod.nexusId).then(setD).catch(() => setD(null)).finally(() => setLoading(false));
  }, [mod.nexusId]);

  const imgs = d?.images?.length ? d.images : mod.imageUrl ? [mod.imageUrl] : [];
  const idx = imgs.length ? ((i % imgs.length) + imgs.length) % imgs.length : 0;
  const go = (delta: number) => setI((p) => p + delta);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft") go(-1);
      else if (e.key === "ArrowRight") go(1);
      else if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onClose]);

  const install = async () => {
    setBusy(true);
    try {
      showToast(await api.installSkyrimMod(mod.keywords, mod.name));
      refreshGames();
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="dash-overlay" onClick={onClose}>
      <div className="dash" style={{ maxWidth: 860 }} onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="eyebrow">{mod.category}</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 22 }}>{d?.name || mod.name}</div>
          </div>
          <button className="btn ghost" onClick={onClose}>
            <Icon.close size={16} /> Close
          </button>
        </div>

        {/* Gallery */}
        <div className="gallery">
          {imgs.length ? (
            <>
              <img className="gallery-img" src={imgs[idx]} alt="" />
              {imgs.length > 1 && (
                <>
                  <button className="gallery-nav left" onClick={() => go(-1)} aria-label="Previous">‹</button>
                  <button className="gallery-nav right" onClick={() => go(1)} aria-label="Next">›</button>
                  <div className="gallery-count">{idx + 1} / {imgs.length}</div>
                </>
              )}
            </>
          ) : (
            <div className="gallery-img" style={{ display: "grid", placeItems: "center", background: `linear-gradient(135deg, ${CAT_COLOR[mod.category] ?? "#888"}, transparent)` }}>
              <span style={{ opacity: 0.6 }}>{loading ? "Loading…" : "No screenshots"}</span>
            </div>
          )}
        </div>
        {imgs.length > 1 && (
          <div className="gallery-strip">
            {imgs.map((src, n) => (
              <button key={n} className={`gallery-thumb ${n === idx ? "on" : ""}`} onClick={() => setI(n)}>
                <img src={src} alt="" loading="lazy" />
              </button>
            ))}
          </div>
        )}

        {/* Meta */}
        <div className="row wrap" style={{ gap: 8, alignItems: "center", marginTop: 4 }}>
          <Pill tone={mod.strCompatible ? "ok" : "warn"}>{mod.strCompatible ? "Co-op safe" : "Co-op risk"}</Pill>
          <span className="muted" style={{ fontSize: 12.5 }}>↓ {fmtN(d?.downloads ?? mod.downloads)} · ★ {fmtN(d?.endorsements ?? mod.endorsements)}</span>
          {d?.version && <span className="muted" style={{ fontSize: 12.5 }}>· v{d.version}</span>}
          {d?.author && <span className="muted" style={{ fontSize: 12.5 }}>· by {d.author}</span>}
          {d?.updated && <span className="muted" style={{ fontSize: 12.5 }}>· updated {d.updated}</span>}
        </div>

        {mod.note && <p className="muted" style={{ fontStyle: "italic", margin: "4px 0 0" }}>{mod.note}</p>}

        <div className="patch-notes" style={{ maxHeight: "26vh", whiteSpace: "pre-wrap" }}>
          {loading ? "Loading details…" : d?.description || d?.summary || mod.summary}
        </div>

        <div className="row" style={{ justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
          <button className="btn ghost" onClick={() => void api.openUrl(mod.nexusUrl)}>
            <Icon.link size={15} /> Open page
          </button>
          {mod.installable && (
            <button className="btn-play" disabled={busy} onClick={install}>
              <Icon.upgrade size={15} /> {busy ? "Installing…" : "Install downloaded"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/** Browsable, filterable Skyrim mod catalog with real Nexus screenshots. */
function SkyrimModBrowser() {
  const { showToast, refreshGames } = useLauncher();
  const [mods, setMods] = useState<api.CatalogMod[]>([]);
  const [loading, setLoading] = useState(true);
  const [hasKey, setHasKey] = useState(false);
  const [keyOpen, setKeyOpen] = useState(false);
  const [key, setKey] = useState("");
  const [savingKey, setSavingKey] = useState(false);
  const [installing, setInstalling] = useState<number | null>(null);

  const [q, setQ] = useState("");
  const [cat, setCat] = useState("All");
  const [strOnly, setStrOnly] = useState(false);
  const [sort, setSort] = useState("popular");
  const [page, setPage] = useState(0);
  const [detail, setDetail] = useState<api.CatalogMod | null>(null);

  const load = () => {
    setLoading(true);
    api.skyrimCatalog().then(setMods).catch(() => {}).finally(() => setLoading(false));
  };
  useEffect(() => {
    api.nexusConfig().then((c) => setHasKey(c.hasKey)).catch(() => {});
    load();
  }, []);
  useEffect(() => setPage(0), [q, cat, strOnly, sort]);

  const saveKey = async () => {
    setSavingKey(true);
    try {
      await api.nexusSetKey(key.trim());
      setHasKey(true);
      setKeyOpen(false);
      setKey("");
      showToast("Nexus key saved — loading screenshots…");
      load();
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setSavingKey(false);
    }
  };

  const installMod = async (m: api.CatalogMod) => {
    setInstalling(m.nexusId);
    try {
      showToast(await api.installSkyrimMod(m.keywords, m.name));
      refreshGames();
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setInstalling(null);
    }
  };

  const filtered = mods.filter(
    (m) =>
      (cat === "All" || m.category === cat) &&
      (!strOnly || m.strCompatible) &&
      (q.trim() === "" || `${m.name} ${m.summary}`.toLowerCase().includes(q.trim().toLowerCase()))
  );
  const sorted = [...filtered].sort((a, b) =>
    sort === "name"
      ? a.name.localeCompare(b.name)
      : sort === "endorsed"
      ? (b.endorsements ?? 0) - (a.endorsements ?? 0)
      : (b.downloads ?? 0) - (a.downloads ?? 0)
  );
  const pageCount = Math.max(1, Math.ceil(sorted.length / PER_PAGE));
  const cur = Math.min(page, pageCount - 1);
  const shown = sorted.slice(cur * PER_PAGE, cur * PER_PAGE + PER_PAGE);

  return (
    <>
      <div className="sect-head" style={{ marginTop: 22 }}>
        <div className="sect-title">Mod browser</div>
        {hasKey ? (
          <Pill tone="ok">Nexus connected</Pill>
        ) : (
          <button className="btn ghost" onClick={() => setKeyOpen((o) => !o)}>
            <Icon.link size={14} /> Connect Nexus for screenshots
          </button>
        )}
      </div>

      {!hasKey && keyOpen && (
        <div className="surface" style={{ padding: 14, borderRadius: 14 }}>
          <p className="muted" style={{ marginTop: 0 }}>
            Paste your <b>free</b> Nexus personal API key to load real screenshots &amp; descriptions (no
            Premium needed; it's only used to read mod info). Get one at{" "}
            <button className="linklike" onClick={() => void api.openUrl("https://www.nexusmods.com/users/myaccount?tab=api")}>
              nexusmods.com → Account → API
            </button>
            .
          </p>
          <div className="row wrap" style={{ alignItems: "flex-end" }}>
            <Field label="Nexus personal API key">
              <input className="input" style={{ minWidth: 340 }} type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder="paste key…" />
            </Field>
            <button className="btn" disabled={savingKey || !key.trim()} onClick={saveKey}>
              <Icon.check size={15} /> {savingKey ? "Checking…" : "Save key"}
            </button>
          </div>
        </div>
      )}

      {/* Controls */}
      <div className="row wrap" style={{ gap: 10, alignItems: "flex-end", marginTop: 4 }}>
        <Field label="Search">
          <input className="input" style={{ minWidth: 200 }} value={q} onChange={(e) => setQ(e.target.value)} placeholder="ENB, water, UI…" />
        </Field>
        <Field label="Sort by">
          <Select
            value={sort}
            onChange={setSort}
            minWidth={150}
            options={[
              { value: "popular", label: "Most downloaded" },
              { value: "endorsed", label: "Most endorsed" },
              { value: "name", label: "Name (A–Z)" },
            ]}
          />
        </Field>
        <label className="row" style={{ gap: 7, alignItems: "center", cursor: "pointer", paddingBottom: 8 }}>
          <input type="checkbox" checked={strOnly} onChange={(e) => setStrOnly(e.target.checked)} />
          <span>Co-op safe only</span>
        </label>
      </div>
      <div className="row wrap" style={{ gap: 6, marginTop: 4 }}>
        {MOD_CATEGORIES.map((c) => (
          <button key={c} className={`pill ${cat === c ? "on" : ""}`} style={{ cursor: "pointer", opacity: cat === c ? 1 : 0.65 }} onClick={() => setCat(c)}>
            {c}
          </button>
        ))}
      </div>

      {loading ? (
        <p className="muted">Loading mods…</p>
      ) : (
        <>
          <div className="tiles" style={{ marginTop: 12 }}>
            {shown.map((m) => (
              <div className="tile" key={m.nexusId}>
                {m.imageUrl ? (
                  <img className="thumb" src={m.imageUrl} alt="" loading="lazy" style={{ cursor: "pointer" }} onClick={() => setDetail(m)} />
                ) : (
                  <div className="thumb" style={{ display: "grid", placeItems: "center", cursor: "pointer", background: `linear-gradient(135deg, ${CAT_COLOR[m.category] ?? "#888"}, transparent)` }} onClick={() => setDetail(m)}>
                    <span style={{ fontFamily: "var(--font-display)", fontSize: 30, fontWeight: 800, opacity: 0.55 }}>{m.name.slice(0, 1)}</span>
                  </div>
                )}
                <div className="body">
                  <div className="row" style={{ justifyContent: "space-between", gap: 6, marginBottom: 4 }}>
                    <span className="pill" style={{ fontSize: 11 }}>{m.category}</span>
                    <Pill tone={m.strCompatible ? "ok" : "warn"}>{m.strCompatible ? "Co-op safe" : "Co-op risk"}</Pill>
                  </div>
                  <div style={{ fontWeight: 600, fontSize: 14, cursor: "pointer" }} onClick={() => setDetail(m)}>{m.name}</div>
                  <div className="muted" style={{ fontSize: 12, margin: "4px 0", maxHeight: 52, overflow: "hidden" }}>{m.summary}</div>
                  {m.note && <div className="muted" style={{ fontSize: 11, fontStyle: "italic", marginBottom: 4 }}>{m.note}</div>}
                  <div className="sub" style={{ color: "var(--text-mute)", fontSize: 11.5, marginBottom: 8 }}>
                    ↓ {fmtN(m.downloads)} · ★ {fmtN(m.endorsements)}
                  </div>
                  <div className="row" style={{ gap: 6 }}>
                    <button className="btn" style={{ padding: "7px 12px", fontSize: 12.5 }} onClick={() => setDetail(m)}>
                      <Icon.mods size={13} /> Details
                    </button>
                    {m.installable && (
                      <button className="btn ghost" style={{ padding: "7px 12px", fontSize: 12.5 }} disabled={installing !== null} onClick={() => void installMod(m)}>
                        <Icon.upgrade size={13} /> {installing === m.nexusId ? "Installing…" : "Install"}
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
          {sorted.length === 0 && <p className="muted">No mods match those filters.</p>}
          {pageCount > 1 && (
            <div className="row" style={{ justifyContent: "center", gap: 12, marginTop: 14, alignItems: "center" }}>
              <button className="btn ghost" disabled={cur === 0} onClick={() => setPage(cur - 1)}>
                ← Prev
              </button>
              <span className="muted" style={{ fontSize: 12.5 }}>Page {cur + 1} of {pageCount}</span>
              <button className="btn ghost" disabled={cur >= pageCount - 1} onClick={() => setPage(cur + 1)}>
                Next →
              </button>
            </div>
          )}
        </>
      )}
      <p className="muted" style={{ marginTop: 12, fontSize: 12 }}>
        Nexus doesn't allow automatic downloads, so installs are guided: <b>Open page</b> → download the
        main file → <b>Install downloaded</b>. Big FOMOD/texture mods are page-only — use a mod manager
        (MO2 / Vortex). Everyone in a co-op session should run the same mods.
      </p>

      {detail && <ModDetailModal mod={detail} onClose={() => setDetail(null)} />}
    </>
  );
}

export function SkyrimMods() {
  const { games, installTool, busy } = useLauncher();
  const sky = games?.skyrim;
  if (!sky?.installed) return <NotInstalled title="Skyrim" />;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Modding</div>
        <button className="btn ghost" onClick={() => sky.install_dir && api.openPath(sky.install_dir + "\\Data")}>
          <Icon.folder size={15} /> Open Data folder
        </button>
      </div>
      <div className="row wrap">
        <Pill tone={sky.has_skse ? "ok" : "warn"}>SKSE64 {sky.has_skse ? "installed" : "missing"}</Pill>
        <Pill tone={sky.has_skyrim_together ? "ok" : "warn"}>
          Skyrim Together {sky.has_skyrim_together ? "installed" : "missing"}
        </Pill>
      </div>
      <div className="row wrap" style={{ marginTop: 8 }}>
        {!sky.has_skse && (
          <button className="btn" disabled={busy} onClick={() => installTool("skse")}>
            <Icon.upgrade size={15} /> Install SKSE64 <Pill tone="ok">1-click</Pill>
          </button>
        )}
      </div>
      {(!sky.has_skyrim_together || !sky.has_address_library) && <TogetherSetup />}

      <SkyrimModBrowser />
    </div>
  );
}

/* --------------------------- Elden Ring -------------------------------- */

export function EldenRingPlay() {
  const { games, launchEldenRing, refreshGames, installTool, showToast, busy } = useLauncher();
  const er = games?.eldenRing;

  const setUltrawide = async (on: boolean) => {
    try {
      await api.setEldenringUltrawide(on);
      await refreshGames();
    } catch (e) {
      showToast(`${e}`);
    }
  };
  const installUltrawide = async () => {
    try {
      showToast(await api.installEldenringUltrawide());
      await refreshGames();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  return (
    <div className="hero">
      <div className="eyebrow">FromSoftware</div>
      <h1 className="title">Elden Ring</h1>
      <p className="subtitle">{er?.installed ? er.install_dir : "Detecting your Steam install…"}</p>

      <div className="action-bar surface">
        <div className="row wrap">
          <Pill tone={er?.installed ? "ok" : "warn"}>{er?.installed ? "Installed" : "Not found"}</Pill>
          <Pill tone={er?.has_seamless_coop ? "ok" : "default"}>
            Seamless Co-op {er?.has_seamless_coop ? "ready" : "—"}
          </Pill>
          <Pill tone={er?.has_mod_engine ? "ok" : "default"}>Mod Engine {er?.has_mod_engine ? "ready" : "—"}</Pill>
        </div>
        <button className="btn-play" disabled={!er?.installed} onClick={() => launchEldenRing("vanilla")}>
          <Icon.play size={20} /> Play
        </button>
      </div>

      <div className="sect" style={{ marginTop: 28 }}>
        <div className="sect-head">
          <div className="sect-title">Launch options</div>
          <button className="btn ghost" onClick={refreshGames}>
            <Icon.refresh size={15} /> Refresh
          </button>
        </div>
        <div className="row wrap">
          <button className="btn" disabled={!er?.installed} onClick={() => launchEldenRing("vanilla")}>
            Official · EasyAntiCheat
          </button>
          <button className="btn" disabled={!er?.has_seamless_coop} onClick={() => launchEldenRing("seamless")}>
            <Icon.coop size={16} /> Seamless Co-op (EAC off)
          </button>
          <button className="btn" disabled={!er?.has_mod_engine} onClick={() => launchEldenRing("modded")}>
            <Icon.mods size={16} /> Modded (Mod Engine 2)
          </button>
        </div>

        {er?.installed && !er.has_seamless_coop && (
          <>
            <div className="sect-head" style={{ marginTop: 18 }}>
              <div className="sect-title">One-click setup</div>
            </div>
            <div className="row wrap">
              <button className="btn" disabled={busy} onClick={() => installTool("seamless")}>
                <Icon.coop size={15} /> Install Seamless Co-op
              </button>
            </div>
          </>
        )}
        <p className="muted">
          Seamless Co-op and Mod Engine launch with anti-cheat disabled — never use them on official
          servers. Official play goes through Steam so EAC and online services start normally.
        </p>
      </div>

      {/* Ultrawide — a native-feeling toggle (Elden Ring has no built-in 21:9/32:9). */}
      <div className="sect" style={{ marginTop: 24 }}>
        <div className="sect-head">
          <div className="sect-title">Ultrawide display</div>
          {er?.ultrawide_installed && (
            <Pill tone={er.ultrawide_enabled ? "ok" : "default"}>{er.ultrawide_enabled ? "On" : "Off"}</Pill>
          )}
        </div>
        {er?.ultrawide_installed ? (
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ fontWeight: 600 }}>21:9 / 32:9 ultrawide</div>
              <div className="muted">Removes the black bars and widens the FOV. Co-op/offline only (anti-cheat off).</div>
            </div>
            <div className="seg">
              <button className={er.ultrawide_enabled ? "on" : ""} onClick={() => setUltrawide(true)}>On</button>
              <button className={!er.ultrawide_enabled ? "on" : ""} onClick={() => setUltrawide(false)}>Off</button>
            </div>
          </div>
        ) : (
          <>
            <p className="muted" style={{ marginTop: -4 }}>
              Elden Ring has no native ultrawide. Enable true 21:9 / 32:9 (no black bars, wider FOV) with
              a one-time setup — then it's a simple toggle. Use it with Seamless Co-op or Modded (anti-cheat off).
            </p>
            <div className="row wrap">
              <button className="btn" disabled={!er?.installed} onClick={() => void api.openEldenringUltrawidePage()}>
                <Icon.link size={15} /> 1 · Open Ultrawide Fix page
              </button>
              <button className="btn-play" disabled={!er?.installed || busy} onClick={installUltrawide}>
                <Icon.check size={15} /> 2 · I downloaded it — enable
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export function EldenRingCoop() {
  const { games, launchEldenRing, setEldenRingPassword, installTool, refreshGames, showToast, busy } =
    useLauncher();
  const er = games?.eldenRing;
  const [pw, setPw] = useState(er?.coop_password ?? "");

  const updateSeamless = async () => {
    try {
      showToast(await api.installSeamlessUpdate());
      await refreshGames();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  if (!er?.installed) return <NotInstalled title="Elden Ring" />;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Seamless Co-op</div>
      </div>
      <p className="muted">
        Everyone in your session must share the same co-op password. Set it here — Aurora writes it
        straight into <code>ersc_settings.ini</code>.
      </p>
      {er.has_seamless_coop ? (
        <div className="row wrap" style={{ alignItems: "flex-end" }}>
          <Field label="Co-op password">
            <input
              className="input"
              value={pw}
              onChange={(e) => setPw(e.target.value)}
              placeholder="shared-password"
              style={{ minWidth: 240 }}
            />
          </Field>
          <button className="btn" onClick={() => setEldenRingPassword(pw)}>
            <Icon.check size={16} /> Save
          </button>
          <button
            className="btn-play"
            style={{ padding: "11px 22px", fontSize: 14 }}
            onClick={() => launchEldenRing("seamless")}
          >
            <Icon.coop size={17} /> Launch Co-op
          </button>
        </div>
      ) : (
        <div className="row wrap">
          <button className="btn" disabled={busy} onClick={() => installTool("seamless")}>
            <Icon.coop size={15} /> Install Seamless Co-op
          </button>
        </div>
      )}

      <div className="surface" style={{ padding: 16, borderRadius: 16, marginTop: 14 }}>
        <div style={{ fontWeight: 600, marginBottom: 6 }}>Mod says "out of date"?</div>
        <p className="muted" style={{ marginBottom: 10 }}>
          Seamless Co-op updates land on Nexus before its GitHub releases. Download the newest main
          file there, then Aurora installs it straight from your Downloads.
        </p>
        <div className="row wrap">
          <button className="btn" onClick={() => api.openSeamlessPage()}>
            <Icon.link size={15} /> 1 · Open Nexus page
          </button>
          <button className="btn" disabled={busy} onClick={updateSeamless}>
            <Icon.check size={15} /> 2 · I downloaded it — install
          </button>
        </div>
      </div>

      {/* Modded co-op — Mod Engine 2, folded into the Co-op tab. */}
      <div className="sect-head" style={{ marginTop: 22 }}>
        <div className="sect-title">Modded co-op · Mod Engine 2</div>
        {er.has_mod_engine && er.mods_dir && (
          <button className="btn ghost" onClick={() => api.openPath(er.mods_dir!)}>
            <Icon.folder size={15} /> Open mods folder
          </button>
        )}
      </div>
      <p className="muted" style={{ marginTop: -4 }}>
        Want mods <i>and</i> co-op? Mod Engine 2 loads mods without touching your game files and runs
        with anti-cheat off — drop mods in the folder, then launch. Everyone should run the same mods.
      </p>
      {er.has_mod_engine ? (
        <div className="row wrap" style={{ alignItems: "center" }}>
          <Pill tone="ok">Mod Engine 2 ready</Pill>
          <button
            className="btn-play"
            style={{ padding: "11px 22px", fontSize: 14 }}
            onClick={() => launchEldenRing("modded")}
          >
            <Icon.play size={16} /> Launch Modded
          </button>
        </div>
      ) : (
        <div className="row wrap" style={{ alignItems: "center" }}>
          <Pill tone="warn">Mod Engine 2 not installed</Pill>
          <button className="btn" disabled={busy} onClick={() => installTool("modengine2")}>
            <Icon.upgrade size={15} /> Install Mod Engine 2 (1-click)
          </button>
        </div>
      )}
    </div>
  );
}

/** Cheats for a cinematic/offline experience. Master toggle (default off);
 *  the cheat list greys out until it's on. */
const ER_CHEATS: { id: string; name: string; note: string }[] = [
  { id: "god", name: "God mode", note: "Take no damage — wander and film freely." },
  { id: "stamina", name: "Infinite stamina", note: "Sprint, dodge and attack without draining." },
  { id: "fp", name: "Infinite FP", note: "Cast spells and skills with no cost." },
  { id: "oneshot", name: "One-shot kills", note: "Drop anything instantly for clips." },
  { id: "runes", name: "Infinite runes", note: "Level up however you like." },
  { id: "keepRunes", name: "Keep runes on death", note: "Never lose progress." },
  { id: "speed", name: "Super speed", note: "Zip across the map — great for travel shots." },
  { id: "freecam", name: "Free camera", note: "Detach the camera for cinematic angles." },
];

export function EldenRingCheats() {
  const { games, showToast } = useLauncher();
  const er = games?.eldenRing;
  const [on, setOn] = useState<boolean>(() => localStorage.getItem("aurora:er-cheats") === "1");
  const [sel, setSel] = useState<Record<string, boolean>>(() => {
    try {
      return JSON.parse(localStorage.getItem("aurora:er-cheat-sel") || "{}");
    } catch {
      return {};
    }
  });
  const [running, setRunning] = useState(false);
  useEffect(() => {
    const tick = () => api.erCheatStatus().then((s) => setRunning(s.running)).catch(() => {});
    tick();
    const t = setInterval(tick, 4000);
    return () => clearInterval(t);
  }, []);
  if (!er?.installed) return <NotInstalled title="Elden Ring" />;

  const setMaster = (v: boolean) => {
    setOn(v);
    localStorage.setItem("aurora:er-cheats", v ? "1" : "0");
    if (!v) {
      // Turning the master off reverts any applied cheats in the running game.
      Object.keys(sel).filter((k) => sel[k]).forEach((id) => api.erCheatSet(id, false).catch(() => {}));
    }
  };
  const toggle = async (id: string) => {
    const next = { ...sel, [id]: !sel[id] };
    setSel(next);
    localStorage.setItem("aurora:er-cheat-sel", JSON.stringify(next));
    try {
      await api.erCheatSet(id, next[id]); // apply/undo live in the running game
    } catch (e) {
      showToast(`${e}`);
    }
  };

  return (
    <div className="sect">
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <div className="sect-title">Cheats</div>
          <div className="muted" style={{ fontSize: 13 }}>Cinematic toolkit — for offline / co-op only (anti-cheat off).</div>
        </div>
        <div className="seg">
          <button className={on ? "on" : ""} onClick={() => setMaster(true)}>On</button>
          <button className={!on ? "on" : ""} onClick={() => setMaster(false)}>Off</button>
        </div>
      </div>

      <div
        className="surface"
        style={{ padding: "10px 14px", marginTop: 10, fontSize: 12.5, border: "1px solid rgba(255,180,80,0.35)", background: "rgba(255,180,80,0.07)" }}
      >
        ⚠ Built-in trainer — applies live to the running game. <b>Offline / co-op only</b> (anti-cheat off,
        i.e. Seamless Co-op or Modded); never on official online play.{" "}
        <span style={{ color: running ? "var(--accent)" : "var(--text-mute)" }}>
          Elden Ring is {running ? "running — toggles apply now." : "not running — launch it from Co-op first, then toggle."}
        </span>
      </div>

      <div className="col" style={{ gap: 2, marginTop: 12, opacity: on ? 1 : 0.4, pointerEvents: on ? "auto" : "none" }}>
        {ER_CHEATS.map((c) => (
          <label className="lrow" key={c.id} style={{ cursor: on ? "pointer" : "default" }}>
            <input type="checkbox" disabled={!on} checked={!!sel[c.id]} onChange={() => toggle(c.id)} style={{ width: 18, height: 18 }} />
            <div className="grow" style={{ marginLeft: 12 }}>
              <div className="name">{c.name}</div>
              <div className="sub">{c.note}</div>
            </div>
          </label>
        ))}
      </div>

      <p className="muted" style={{ marginTop: 12, fontSize: 12 }}>
        These cheats are built into Aurora — no separate trainer. They edit the running game's memory and
        only take effect with anti-cheat off (launch from the <b>Co-op</b> tab). This is an early build:
        a cheat that can't find its spot in your game version will simply do nothing (never crash) while
        we fine-tune it.
      </p>
    </div>
  );
}

/* -------------------------- Cyberpunk 2077 ------------------------------ */

export function CyberpunkPlay() {
  const { games, launchCyberpunk, refreshGames, installTool, busy } = useLauncher();
  const cp = games?.cyberpunk;

  return (
    <div className="hero">
      <div className="eyebrow">CD Projekt Red</div>
      <h1 className="title">Cyberpunk 2077</h1>
      <p className="subtitle">{cp?.installed ? cp.install_dir : "Detecting your Steam install…"}</p>

      <div className="action-bar surface">
        <div className="row wrap">
          <Pill tone={cp?.installed ? "ok" : "warn"}>
            {cp?.installed ? (cp.source === "epic" ? "Installed · Epic" : "Installed") : "Not found"}
          </Pill>
          <Pill tone={cp?.has_mp ? "ok" : "default"}>CyberpunkMP {cp?.has_mp ? "ready" : "—"}</Pill>
          <Pill tone={cp?.has_cet ? "ok" : "default"}>CET {cp?.has_cet ? "ready" : "—"}</Pill>
        </div>
        <button className="btn-play" disabled={!cp?.installed} onClick={() => launchCyberpunk("vanilla")}>
          <Icon.play size={20} /> Play
        </button>
      </div>

      <div className="sect" style={{ marginTop: 28 }}>
        <div className="sect-head">
          <div className="sect-title">Launch options</div>
          <button className="btn ghost" onClick={refreshGames}>
            <Icon.refresh size={15} /> Refresh
          </button>
        </div>
        <div className="row wrap">
          <button className="btn" disabled={!cp?.installed} onClick={() => launchCyberpunk("vanilla")}>
            Official (Steam)
          </button>
          <button className="btn" disabled={!cp?.installed} onClick={() => launchCyberpunk("skip-launcher")}>
            Skip launcher
          </button>
          <button className="btn" disabled={!cp?.has_mp} onClick={() => launchCyberpunk("mp")}>
            <Icon.coop size={16} /> CyberpunkMP (co-op)
          </button>
        </div>

        {cp?.installed && (!cp.has_mp || !cp.has_cet) && (
          <>
            <div className="sect-head" style={{ marginTop: 18 }}>
              <div className="sect-title">One-click setup</div>
            </div>
            <div className="row wrap">
              {!cp.has_mp && (
                <button className="btn" disabled={busy} onClick={() => installTool("cyberpunkmp")}>
                  <Icon.coop size={15} /> Install CyberpunkMP
                </button>
              )}
              {!cp.has_cet && (
                <button className="btn" disabled={busy} onClick={() => installTool("cet")}>
                  <Icon.mods size={15} /> Install Cyber Engine Tweaks
                </button>
              )}
            </div>
          </>
        )}
        <p className="muted">
          CyberpunkMP is an early, experimental multiplayer mod by the Skyrim Together team — expect
          rough edges. It runs through its own launcher and never touches official online services.
        </p>
      </div>
    </div>
  );
}

export function CyberpunkCoop() {
  const { games, launchCyberpunk, installTool, busy } = useLauncher();
  const cp = games?.cyberpunk;
  if (!cp?.installed) return <NotInstalled title="Cyberpunk 2077" />;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">CyberpunkMP — servers</div>
        <button
          className="btn-play"
          style={{ padding: "11px 22px", fontSize: 14 }}
          disabled={!cp.has_mp}
          onClick={() => launchCyberpunk("mp")}
        >
          <Icon.coop size={17} /> Launch
        </button>
      </div>
      {!cp.has_mp && (
        <div className="row wrap">
          <button className="btn" disabled={busy} onClick={() => installTool("cyberpunkmp")}>
            <Icon.coop size={15} /> Install CyberpunkMP
          </button>
        </div>
      )}
      <CoopServers
        game="cyberpunk"
        hint="CyberpunkMP's launcher has its own server browser — saved addresses here are for sharing with friends."
      />
    </div>
  );
}

export function CyberpunkMods() {
  const { games, installTool, busy } = useLauncher();
  const cp = games?.cyberpunk;
  if (!cp?.installed) return <NotInstalled title="Cyberpunk 2077" />;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Cyber Engine Tweaks</div>
        {cp.mods_dir && (
          <button className="btn ghost" onClick={() => api.openPath(cp.mods_dir!)}>
            <Icon.folder size={15} /> Open mods folder
          </button>
        )}
      </div>
      <div className="row wrap">
        <Pill tone={cp.has_cet ? "ok" : "warn"}>CET {cp.has_cet ? "installed" : "missing"}</Pill>
      </div>

      {cp.has_cet ? (
        <p className="muted" style={{ marginTop: 10 }}>
          Cyber Engine Tweaks is installed — most Cyberpunk mods (graphics tweaks, quality-of-life,
          scripts) from Nexus Mods extract straight into the game folder or the CET mods folder
          above. Press <code>~</code> in-game for the CET console.
        </p>
      ) : (
        <>
          <div className="row wrap" style={{ marginTop: 8 }}>
            <button className="btn" disabled={busy} onClick={() => installTool("cet")}>
              <Icon.upgrade size={15} /> Install Cyber Engine Tweaks
            </button>
          </div>
          <p className="muted" style={{ marginTop: 10 }}>
            CET is the scripting layer most Cyberpunk mods are built on — one click installs it from
            the official release.
          </p>
        </>
      )}
    </div>
  );
}
