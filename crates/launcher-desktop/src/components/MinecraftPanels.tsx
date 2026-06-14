import { useEffect, useState, type ReactNode } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { InstanceConfig, PackHit, ServerConfig } from "../lib/types";
import { Field, Progress, Pill, Icon, Select, HostAddress } from "./ui";
import { MotdEditor } from "./MotdEditor";

function fmtDl(n: number) {
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(0)}K`;
  return String(n);
}

const INSTANCE_LOADERS = [
  { id: "vanilla", name: "Vanilla" },
  { id: "fabric", name: "Fabric" },
  { id: "quilt", name: "Quilt" },
  { id: "forge", name: "Forge" },
  { id: "neoforge", name: "NeoForge" },
];

/** A sensible default allocation: ~half of RAM, clamped, leaving OS headroom. */
function recommendRam(totalMb: number) {
  const half = Math.round(totalMb / 2 / 1024) * 1024;
  return Math.max(2048, Math.min(half, Math.max(2048, totalMb - 2048)));
}
function ramMax(totalMb: number) {
  return Math.max(2048, Math.floor(totalMb / 512) * 512);
}

function loaderLabel(id: string): string {
  return INSTANCE_LOADERS.find((l) => l.id === id)?.name ?? "Vanilla";
}

/** Pick a newer Minecraft version → bump config + update mods to match. */
export function UpgradeModal() {
  const { upgradeTarget, closeUpgrade, upgrade, versions, busy } = useLauncher();
  const [target, setTarget] = useState("");
  if (!upgradeTarget) return null;
  const current = upgradeTarget.version;
  const choices = versions.filter((v) => v !== current);
  const pick = target || choices[0] || current;

  return (
    <div className="dash-overlay" onClick={closeUpgrade}>
      <div className="dash" style={{ height: "auto", minHeight: 0 }} onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div className="eyebrow">Upgrade · {upgradeTarget.kind === "server" ? "Server" : "Instance"}</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 22 }}>{upgradeTarget.name}</div>
            <div className="sub" style={{ color: "var(--text-mute)" }}>Currently on {current}</div>
          </div>
          <button className="btn ghost" onClick={closeUpgrade}>
            <Icon.close size={16} /> Close
          </button>
        </div>
        <div className="row wrap" style={{ alignItems: "flex-end" }}>
          <Field label="Upgrade to">
            <Select value={pick} onChange={setTarget} minWidth={160} options={choices.map((v) => ({ value: v, label: v }))} />
          </Field>
          <button
            className="btn-play"
            style={{ padding: "11px 22px", fontSize: 14 }}
            disabled={busy}
            onClick={async () => {
              await upgrade(upgradeTarget.kind, upgradeTarget.id, pick);
              closeUpgrade();
            }}
          >
            <Icon.upgrade size={16} /> {busy ? "Upgrading…" : "Upgrade"}
          </button>
        </div>
        <p className="muted">
          Bumps the {upgradeTarget.kind}'s Minecraft version and updates every installed mod to a
          build for {pick}. Mods without a compatible build are flagged so the upgrade can't silently
          break — check the Content tab afterwards.
        </p>
      </div>
    </div>
  );
}

const PACK_SOURCES = [
  { id: "modrinth", name: "Modrinth" },
  { id: "ftb", name: "Feed the Beast" },
  { id: "curseforge", name: "CurseForge" },
  { id: "technic", name: "Technic" },
];
const MODPACK_CHIPS = ["All the Mods", "Cobblemon", "Create", "Better MC", "Prominence", "Adventure", "Skyblock", "Tech"];
/** Avatar that shows a modpack/custom icon, falling back to the Minecraft cube. */
function PackAvatar({ icon }: { icon?: string | null }) {
  if (icon) {
    return (
      <div className="avatar" style={{ padding: 0, overflow: "hidden" }}>
        <img src={icon} alt="" style={{ width: "100%", height: "100%", objectFit: "cover", borderRadius: "inherit" }} />
      </div>
    );
  }
  return (
    <div className="avatar">
      <Icon.minecraft size={19} />
    </div>
  );
}

type MenuItem = { label: string; icon?: ReactNode; onClick: () => void; danger?: boolean };

/** A ⋯ overflow menu for secondary row actions. */
function RowMenu({ items }: { items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  return (
    <div style={{ position: "relative" }}>
      <button className="btn ghost" title="More" onClick={() => setOpen((o) => !o)}>
        <Icon.dots size={18} />
      </button>
      {open && (
        <>
          <div onClick={() => setOpen(false)} style={{ position: "fixed", inset: 0, zIndex: 40 }} />
          <div
            className="row-menu surface"
            style={{ position: "absolute", right: 0, top: "calc(100% + 6px)", zIndex: 41, minWidth: 186, padding: 6 }}
          >
            {items.map((it, i) => (
              <button
                key={i}
                className={`row-menu-item ${it.danger ? "danger" : ""}`}
                onClick={() => {
                  it.onClick();
                  setOpen(false);
                }}
              >
                {it.icon}
                {it.label}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function PackTile({ hit, onCreate }: { hit: PackHit; onCreate: () => void }) {
  return (
    <div className="tile">
      {hit.icon ? (
        <img className="thumb" src={hit.icon} alt="" />
      ) : (
        <div className="thumb" style={{ display: "grid", placeItems: "center" }}>
          <span style={{ fontFamily: "var(--font-display)", fontSize: 26, opacity: 0.5 }}>{hit.name.slice(0, 1)}</span>
        </div>
      )}
      <div className="body">
        <div style={{ fontWeight: 600, fontSize: 14 }}>{hit.name}</div>
        <div className="muted" style={{ fontSize: 12, margin: "4px 0 10px", maxHeight: 34, overflow: "hidden" }}>
          {hit.summary}
        </div>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span className="sub" style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
            {hit.downloads ? `↓ ${fmtDl(hit.downloads)}` : ""}
          </span>
          <button className="btn" style={{ padding: "7px 12px", fontSize: 12.5 }} onClick={onCreate}>
            <Icon.plus size={13} /> Create
          </button>
        </div>
      </div>
    </div>
  );
}

/** Inline multi-source modpack browser used inside the New Instance editor. */
function ModpackBrowse({
  onCreate,
  onCancel,
}: {
  onCreate: (source: string, id: string, name: string, icon: string | null) => void;
  onCancel: () => void;
}) {
  const [source, setSource] = useState("modrinth");
  const [query, setQuery] = useState("");
  const [packs, setPacks] = useState<PackHit[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [slug, setSlug] = useState("");

  // Modrinth / FTB / CurseForge are grid-backed (CurseForge by Project ID or curated).
  const grid = source !== "technic";

  const search = async (q: string) => {
    if (!grid) return;
    setLoading(true);
    setError("");
    try {
      setPacks(await api.packSearch(source, q.trim()));
    } catch (e) {
      setError(String(e));
      setPacks([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    setPacks([]);
    setQuery("");
    setError("");
    setSlug("");
    if (source !== "technic") search("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source]);

  const isCf = source === "curseforge";
  const searchLabel = isCf ? "CurseForge Project ID" : "Find a modpack";
  const searchPlaceholder = isCf ? "e.g. 715572 (blank = popular)" : `Search ${source === "ftb" ? "FTB" : "Modrinth"} modpacks…`;

  return (
    <div className="col" style={{ gap: 12 }}>
      <div className="row wrap" style={{ alignItems: "flex-end" }}>
        <Field label="Source">
          <Select value={source} onChange={setSource} minWidth={170} options={PACK_SOURCES.map((s) => ({ value: s.id, label: s.name }))} />
        </Field>
        {grid && (
          <>
            <Field label={searchLabel}>
              <input
                className="input"
                value={query}
                onChange={(e) => setQuery(isCf ? e.target.value.replace(/[^0-9]/g, "") : e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && search(query)}
                placeholder={searchPlaceholder}
                style={{ minWidth: isCf ? 200 : 260 }}
              />
            </Field>
            <button className="btn" onClick={() => search(query)}>
              <Icon.refresh size={15} /> {isCf ? "Find" : "Search"}
            </button>
          </>
        )}
        <button className="btn ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>

      {source === "modrinth" && (
        <div className="row wrap" style={{ gap: 6 }}>
          {MODPACK_CHIPS.map((c) => (
            <button key={c} className="pill" style={{ cursor: "pointer" }} onClick={() => { setQuery(c); search(c); }}>
              {c}
            </button>
          ))}
        </div>
      )}

      {isCf && (
        <p className="muted">
          CurseForge blocks live text search on the built-in key, so this shows popular packs — or paste a
          {" "}<b>Project ID</b> (the number on the modpack's CurseForge page) to add any pack.
        </p>
      )}

      {source === "technic" && (
        <>
          <p className="muted">
            Technic's API blocks search and most listed packs use Solder (not yet supported). Paste a pack{" "}
            <b>slug</b> (from its technicpack.net URL) to try — works only for direct-download packs.
          </p>
          <div className="row wrap" style={{ alignItems: "flex-end" }}>
            <Field label="Technic pack slug">
              <input className="input" value={slug} onChange={(e) => setSlug(e.target.value)} placeholder="e.g. tekkit" style={{ width: 220 }} />
            </Field>
            <button className="btn" disabled={!slug} onClick={() => onCreate("technic", slug, slug, null)}>
              <Icon.plus size={14} /> Create
            </button>
          </div>
        </>
      )}

      {grid && (
        <div style={{ maxHeight: 340, overflowY: "auto", paddingRight: 6 }}>
          {loading && <p className="muted">Searching…</p>}
          {error && <p className="muted">⚠ {error}</p>}
          {!loading && !error && packs.length === 0 && <p className="muted">No modpacks found.</p>}
          <div className="tiles">
            {packs.map((h) => (
              <PackTile key={h.id} hit={h} onCreate={() => onCreate(source, h.id, h.name, h.icon)} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function blankInstance(version: string, count: number, ram: number): InstanceConfig {
  return { id: crypto.randomUUID(), name: `Instance ${count + 1}`, version, loader: "vanilla", maxRamMb: ram };
}

/** The Play tab — a list of launchable instances (versions / modpacks). */
export function InstancesPanel() {
  const {
    versions,
    settings,
    instances,
    busy,
    progress,
    systemRamMb,
    playInstance,
    createFromPack,
    saveInstanceCfg,
    deleteInstanceCfg,
    openInstanceFolder,
    openContent,
    openInventory,
    openUpgrade,
    openBackups,
  } = useLauncher();
  const [editing, setEditing] = useState<InstanceConfig | null>(null);

  const fallbackVersion = settings.lastVersion || versions[0] || "1.21.4";

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Your instances</div>
        <button
          className="btn-play"
          style={{ padding: "11px 20px", fontSize: 14 }}
          onClick={() => setEditing(blankInstance(fallbackVersion, instances.length, recommendRam(systemRamMb)))}
        >
          <Icon.plus size={17} /> New instance
        </button>
      </div>

      {busy && progress && (
        <div className="surface" style={{ padding: "14px 18px", borderRadius: 16 }}>
          <div className="row" style={{ justifyContent: "space-between", marginBottom: 8 }}>
            <span style={{ fontWeight: 600 }}>{progress.stage}</span>
            <span className="sub" style={{ color: "var(--text-mute)" }}>
              {progress.total > 0
                ? `${(progress.done / 1048576).toFixed(0)} / ${(progress.total / 1048576).toFixed(0)} MiB`
                : ""}
            </span>
          </div>
          <Progress value={progress.fraction} />
        </div>
      )}

      {editing && (
        <InstanceEditor
          key={editing.id}
          value={editing}
          versions={versions}
          maxRam={ramMax(systemRamMb)}
          isNew={!instances.some((i) => i.id === editing.id)}
          onCancel={() => setEditing(null)}
          onSave={async (cfg) => {
            await saveInstanceCfg(cfg);
            setEditing(null);
          }}
          onCreatePack={(source, pid, title, icon) => {
            createFromPack(source, pid, title, icon);
            setEditing(null);
          }}
        />
      )}

      {instances.length === 0 && !editing && (
        <p className="muted">
          No instances yet. Create one — each keeps its own version, mods, worlds, and settings,
          isolated from the others.
        </p>
      )}

      <div className="col" style={{ gap: 2 }}>
        {instances.map((it) => (
          <div className="lrow" key={it.id}>
            <PackAvatar icon={it.icon} />
            <div className="grow">
              <div className="name">{it.name}</div>
              <div className="sub">
                {it.version} · {loaderLabel(it.loader)} · {(it.maxRamMb / 1024).toFixed(0)} GB
              </div>
            </div>
            <button className="btn-play" style={{ padding: "9px 20px", fontSize: 14 }} disabled={busy} onClick={() => playInstance(it.id)}>
              <Icon.play size={15} /> Play
            </button>
            <button className="btn ghost" onClick={() => setEditing(it)}>
              Edit
            </button>
            <RowMenu
              items={[
                {
                  label: "Content",
                  icon: <Icon.mods size={15} />,
                  onClick: () =>
                    openContent({
                      kind: "instance",
                      id: it.id,
                      name: it.name,
                      version: it.version,
                      loader: it.loader === "vanilla" ? null : it.loader,
                    }),
                },
                {
                  label: "Edit inventory",
                  icon: <Icon.chest size={15} />,
                  onClick: () => openInventory({ kind: "instance", id: it.id, name: it.name, version: it.version, loader: null }),
                },
                {
                  label: "World backups",
                  icon: <Icon.host size={15} />,
                  onClick: () => openBackups({ kind: "instance", id: it.id, name: it.name, version: it.version, loader: null }),
                },
                {
                  label: "Upgrade version",
                  icon: <Icon.upgrade size={15} />,
                  onClick: () => openUpgrade({ kind: "instance", id: it.id, name: it.name, version: it.version, loader: it.loader }),
                },
                { label: "Open folder", icon: <Icon.folder size={15} />, onClick: () => openInstanceFolder(it.id) },
                { label: "Delete", icon: <Icon.trash size={15} />, onClick: () => deleteInstanceCfg(it.id), danger: true },
              ]}
            />
          </div>
        ))}
      </div>
    </div>
  );
}

function InstanceEditor({
  value,
  versions,
  maxRam,
  isNew,
  onSave,
  onCancel,
  onCreatePack,
}: {
  value: InstanceConfig;
  versions: string[];
  maxRam: number;
  isNew: boolean;
  onSave: (cfg: InstanceConfig) => void;
  onCancel: () => void;
  onCreatePack: (source: string, projectId: string, title: string, icon: string | null) => void;
}) {
  const [cfg, setCfg] = useState<InstanceConfig>(value);
  const [mode, setMode] = useState<"blank" | "modpack">("blank");
  const set = (p: Partial<InstanceConfig>) => setCfg((c) => ({ ...c, ...p }));

  return (
    <div className="surface" style={{ padding: 20, borderRadius: 20 }}>
      {isNew && (
        <div className="seg" style={{ marginBottom: 16 }}>
          <button className={mode === "blank" ? "on" : ""} onClick={() => setMode("blank")}>
            Blank
          </button>
          <button className={mode === "modpack" ? "on" : ""} onClick={() => setMode("modpack")}>
            From modpack
          </button>
        </div>
      )}

      {isNew && mode === "modpack" ? (
        <ModpackBrowse onCreate={onCreatePack} onCancel={onCancel} />
      ) : (
        <>
      <div className="row wrap" style={{ gap: 16, alignItems: "flex-end" }}>
        <Field label="Name">
          <input className="input" value={cfg.name} onChange={(e) => set({ name: e.target.value })} />
        </Field>
        <Field label="Version">
          <Select value={cfg.version} onChange={(v) => set({ version: v })} minWidth={130} options={versions.map((v) => ({ value: v, label: v }))} />
        </Field>
        <Field label="Loader">
          <Select value={cfg.loader} onChange={(v) => set({ loader: v })} minWidth={130} options={INSTANCE_LOADERS.map((l) => ({ value: l.id, label: l.name }))} />
        </Field>
        <Field label={`Memory · ${(cfg.maxRamMb / 1024).toFixed(1)} GB`}>
          <input
            type="range"
            min={1024}
            max={maxRam}
            step={512}
            value={Math.min(cfg.maxRamMb, maxRam)}
            onChange={(e) => set({ maxRamMb: Number(e.target.value) })}
            style={{ width: 180 }}
          />
        </Field>
        <Field label="Icon (image URL — optional)">
          <div className="row" style={{ gap: 8 }}>
            <PackAvatar icon={cfg.icon} />
            <input
              className="input"
              value={cfg.icon ?? ""}
              onChange={(e) => set({ icon: e.target.value || null })}
              placeholder="https://…/icon.png"
              style={{ minWidth: 240 }}
            />
          </div>
        </Field>
      </div>
      <div className="row" style={{ marginTop: 18 }}>
        <button className="btn-play" style={{ padding: "11px 22px", fontSize: 14 }} onClick={() => onSave(cfg)}>
          <Icon.check size={16} /> Save instance
        </button>
        <button className="btn ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>
        </>
      )}
    </div>
  );
}

/* ------------------------------ Servers -------------------------------- */

function blankServer(version: string, count: number, ram: number): ServerConfig {
  return {
    id: crypto.randomUUID(),
    name: `Server ${count + 1}`,
    description: "",
    version,
    port: 25565 + count,
    maxPlayers: 20,
    maxRamMb: ram,
    loader: "vanilla",
  };
}

export function MinecraftServers() {
  const {
    versions,
    settings,
    servers,
    serverStatuses,
    systemRamMb,
    saveServerCfg,
    deleteServerCfg,
    startServer,
    stopServer,
    openConsole,
    openServerFolder,
    openContent,
    openInventory,
    openUpgrade,
    createServerFromPack,
    showToast,
    openBackups,
  } = useLauncher();
  const [editing, setEditing] = useState<ServerConfig | null>(null);

  const fallbackVersion = settings.lastVersion || versions[0] || "1.21.4";

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Your servers</div>
        <button
          className="btn-play"
          style={{ padding: "11px 20px", fontSize: 14 }}
          onClick={() => setEditing(blankServer(fallbackVersion, servers.length, recommendRam(systemRamMb)))}
        >
          <Icon.plus size={17} /> New server
        </button>
      </div>

      {editing && (
        <ServerEditor
          key={editing.id}
          value={editing}
          versions={versions}
          maxRam={ramMax(systemRamMb)}
          isNew={!servers.some((s) => s.id === editing.id)}
          onCancel={() => setEditing(null)}
          onSave={async (cfg) => {
            await saveServerCfg(cfg);
            setEditing(null);
          }}
          onCreatePack={(source, pid, title, icon) => {
            createServerFromPack(source, pid, title, icon);
            setEditing(null);
          }}
        />
      )}

      {servers.length === 0 && !editing && (
        <p className="muted">
          No servers yet. Create one and Aurora downloads the server, writes server.properties, and
          runs it — the dashboard opens in-app with a live console.
        </p>
      )}

      <div className="col" style={{ gap: 2 }}>
        {servers.map((s) => {
          const st = serverStatuses[s.id];
          const running = !!st?.running;
          return (
            <div key={s.id}>
            <div className="lrow">
              {s.icon ? (
                <PackAvatar icon={s.icon} />
              ) : (
                <div className="avatar">
                  <Icon.host size={19} />
                </div>
              )}
              <div className="grow">
                <div className="name">
                  {s.name} {running && <Pill tone="ok">{st!.players}/{s.maxPlayers} online</Pill>}
                </div>
                <div className="sub">
                  {s.version} ·{" "}
                  {s.loader === "fabric" ? "Fabric" : s.loader === "forge" ? "Forge" : "Vanilla"} ·
                  port {s.port} · {(s.maxRamMb / 1024).toFixed(0)} GB
                </div>
              </div>
              {running ? (
                <>
                  <button className="btn" onClick={() => openConsole(s.id)}>
                    <Icon.terminal size={15} /> Dashboard
                  </button>
                  <button className="btn danger" onClick={() => stopServer(s.id)}>
                    <Icon.stop size={13} /> Stop
                  </button>
                </>
              ) : (
                <>
                  <button className="btn" onClick={() => startServer(s.id)}>
                    <Icon.play size={14} /> Start
                  </button>
                  <button className="btn ghost" onClick={() => setEditing(s)}>
                    Edit
                  </button>
                </>
              )}
              <RowMenu
                items={[
                  {
                    label: s.loader === "paper" ? "Plugins" : "Content",
                    icon: <Icon.mods size={15} />,
                    onClick: () =>
                      openContent({
                        kind: "server",
                        id: s.id,
                        name: s.name,
                        version: s.version,
                        loader: s.loader && s.loader !== "vanilla" ? s.loader : null,
                      }),
                  },
                  {
                    label: "Edit inventory",
                    icon: <Icon.chest size={15} />,
                    onClick: () => openInventory({ kind: "server", id: s.id, name: s.name, version: s.version, loader: null }),
                  },
                  {
                    label: "World backups",
                    icon: <Icon.host size={15} />,
                    onClick: () => openBackups({ kind: "server", id: s.id, name: s.name, version: s.version, loader: null }),
                  },
                  ...(!running
                    ? [
                        {
                          label: "Upgrade version",
                          icon: <Icon.upgrade size={15} />,
                          onClick: () => openUpgrade({ kind: "server", id: s.id, name: s.name, version: s.version, loader: s.loader ?? null }),
                        },
                      ]
                    : []),
                  { label: "Open folder", icon: <Icon.folder size={15} />, onClick: () => openServerFolder(s.id) },
                  ...(!running
                    ? [{ label: "Delete", icon: <Icon.trash size={15} />, onClick: () => deleteServerCfg(s.id), danger: true }]
                    : []),
                ]}
              />
            </div>
            {running && <HostAddress port={s.port} onCopy={showToast} />}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function ServerEditor({
  value,
  versions,
  maxRam,
  isNew,
  onSave,
  onCancel,
  onCreatePack,
}: {
  value: ServerConfig;
  versions: string[];
  maxRam: number;
  isNew: boolean;
  onSave: (cfg: ServerConfig) => void;
  onCancel: () => void;
  onCreatePack: (source: string, projectId: string, title: string, icon: string | null) => void;
}) {
  const [cfg, setCfg] = useState<ServerConfig>(value);
  const [mode, setMode] = useState<"blank" | "modpack">("blank");
  const set = (p: Partial<ServerConfig>) => setCfg((c) => ({ ...c, ...p }));

  return (
    <div className="surface" style={{ padding: 20, borderRadius: 20 }}>
      {isNew && (
        <div className="seg" style={{ marginBottom: 16 }}>
          <button className={mode === "blank" ? "on" : ""} onClick={() => setMode("blank")}>
            Blank
          </button>
          <button className={mode === "modpack" ? "on" : ""} onClick={() => setMode("modpack")}>
            From modpack
          </button>
        </div>
      )}

      {isNew && mode === "modpack" ? (
        <ModpackBrowse onCreate={onCreatePack} onCancel={onCancel} />
      ) : (
        <>
      <div className="row wrap" style={{ gap: 16, alignItems: "flex-end" }}>
        <Field label="Name">
          <input className="input" value={cfg.name} onChange={(e) => set({ name: e.target.value })} />
        </Field>
        <Field label="Version">
          <Select value={cfg.version} onChange={(v) => set({ version: v })} minWidth={130} options={versions.map((v) => ({ value: v, label: v }))} />
        </Field>
        <Field label="Type">
          <Select
            value={cfg.loader || "vanilla"}
            onChange={(v) => set({ loader: v })}
            minWidth={150}
            options={[
              { value: "vanilla", label: "Vanilla" },
              { value: "paper", label: "Paper (plugins)" },
              { value: "fabric", label: "Fabric (mods)" },
              { value: "forge", label: "Forge (mods)" },
            ]}
          />
        </Field>
        <Field label="Port">
          <input className="input" type="number" value={cfg.port} onChange={(e) => set({ port: Number(e.target.value) })} style={{ width: 110 }} />
        </Field>
        <Field label="Max players">
          <input className="input" type="number" value={cfg.maxPlayers} onChange={(e) => set({ maxPlayers: Number(e.target.value) })} style={{ width: 110 }} />
        </Field>
        <Field label="Icon (image URL — optional)">
          <div className="row" style={{ gap: 8 }}>
            <PackAvatar icon={cfg.icon} />
            <input
              className="input"
              value={cfg.icon ?? ""}
              onChange={(e) => set({ icon: e.target.value || null })}
              placeholder="https://…/icon.png"
              style={{ minWidth: 220 }}
            />
          </div>
        </Field>
      </div>
      <div className="row wrap" style={{ gap: 16, alignItems: "flex-start", marginTop: 14 }}>
        <Field label="Description (MOTD)">
          <MotdEditor value={cfg.description} onChange={(d) => set({ description: d })} />
        </Field>
        <Field label={`Max RAM · ${(cfg.maxRamMb / 1024).toFixed(1)} GB`}>
          <input type="range" min={1024} max={maxRam} step={512} value={Math.min(cfg.maxRamMb, maxRam)} onChange={(e) => set({ maxRamMb: Number(e.target.value) })} style={{ width: 200 }} />
        </Field>
      </div>
      <div className="row" style={{ marginTop: 18 }}>
        <button className="btn-play" style={{ padding: "11px 22px", fontSize: 14 }} onClick={() => onSave(cfg)}>
          <Icon.check size={16} /> Save server
        </button>
        <button className="btn ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>
        </>
      )}
    </div>
  );
}
