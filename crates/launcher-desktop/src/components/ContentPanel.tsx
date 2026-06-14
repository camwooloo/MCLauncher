import { useCallback, useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { ContentTarget, InstalledItem, SearchHit, UpdateResult } from "../lib/types";
import { Field, Pill, Icon, SkinFace } from "./ui";
import { SKIN_GALLERY, type GallerySkin } from "../lib/skins";

function fmt(n: number) {
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(0)}K`;
  return String(n);
}

function typesFor(target: ContentTarget): { id: string; label: string }[] {
  if (target.kind === "server") {
    if (target.loader === "paper") return [{ id: "plugin", label: "Plugins" }];
    return target.loader ? [{ id: "mod", label: "Mods" }] : [];
  }
  const all = [
    { id: "mod", label: "Mods" },
    { id: "shader", label: "Shaders" },
    { id: "resourcepack", label: "Resource Packs" },
    { id: "modpack", label: "Modpacks" },
  ];
  return target.loader ? all : all.filter((t) => t.id !== "mod");
}

/** In-app overlay: content scoped to one instance or server. */
export function ContentOverlay({ target, onClose }: { target: ContentTarget; onClose: () => void }) {
  const { showToast } = useLauncher();
  const [mode, setMode] = useState<"browse" | "installed">("browse");
  const types = typesFor(target);

  return (
    <div className="dash-overlay" onClick={onClose}>
      <div className="dash" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div className="eyebrow">Content · {target.kind === "server" ? "Server" : "Instance"}</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 22 }}>
              {target.name}
            </div>
            <div className="sub" style={{ color: "var(--text-mute)" }}>
              Minecraft {target.version}
              {target.loader ? ` · ${target.loader}` : ""}
            </div>
          </div>
          <div className="row">
            <div className="seg">
              <button className={mode === "browse" ? "on" : ""} onClick={() => setMode("browse")}>
                Browse
              </button>
              <button className={mode === "installed" ? "on" : ""} onClick={() => setMode("installed")}>
                Installed
              </button>
            </div>
            <button className="btn ghost" onClick={onClose}>
              <Icon.close size={16} /> Close
            </button>
          </div>
        </div>

        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", paddingRight: 6 }}>
          {types.length === 0 ? (
            <p className="muted">
              A vanilla server has no installable content. Set this server to Paper (plugins) or
              Fabric/Forge (mods) to add content.
            </p>
          ) : mode === "browse" ? (
            <Browse target={target} types={types} showToast={showToast} />
          ) : (
            <Installed target={target} showToast={showToast} />
          )}
        </div>
      </div>
    </div>
  );
}

function Browse({
  target,
  types,
  showToast,
}: {
  target: ContentTarget;
  types: { id: string; label: string }[];
  showToast: (m: string) => void;
}) {
  const [kind, setKind] = useState(types[0]?.id ?? "mod");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);

  const run = useCallback(async () => {
    setLoading(true);
    try {
      const r = await api.modrinthSearch(
        query,
        kind,
        target.version,
        kind === "mod" ? target.loader ?? undefined : undefined
      );
      setHits(r);
    } catch (e) {
      showToast(`Search failed: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [query, kind, target.version, target.loader, showToast]);

  useEffect(() => {
    run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind]);

  const install = async (h: SearchHit) => {
    setInstalling(h.project_id);
    try {
      await api.contentInstall(target.kind, target.id, h.project_id, kind, h.title);
      showToast(`Installed ${h.title}`);
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setInstalling(null);
    }
  };

  return (
    <>
      <div className="row wrap" style={{ alignItems: "flex-end", marginBottom: 14 }}>
        <div className="seg">
          {types.map((t) => (
            <button key={t.id} className={kind === t.id ? "on" : ""} onClick={() => setKind(t.id)}>
              {t.label}
            </button>
          ))}
        </div>
        <Field label="Search">
          <input
            className="input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && run()}
            placeholder={`Search…`}
            style={{ minWidth: 240 }}
          />
        </Field>
        <button className="btn" onClick={run}>
          <Icon.refresh size={15} /> Search
        </button>
      </div>

      <div className="tiles">
        {loading && <p className="muted">Searching…</p>}
        {!loading && hits.length === 0 && <p className="muted">No results.</p>}
        {hits.map((h) => (
          <div className="tile" key={h.project_id}>
            {h.icon_url ? (
              <img className="thumb" src={h.icon_url} alt="" />
            ) : (
              <div className="thumb" style={{ display: "grid", placeItems: "center" }}>
                <span style={{ fontFamily: "var(--font-display)", fontSize: 26, opacity: 0.5 }}>
                  {h.title.slice(0, 1)}
                </span>
              </div>
            )}
            <div className="body">
              <div style={{ fontWeight: 600, fontSize: 14 }}>{h.title}</div>
              <div className="muted" style={{ fontSize: 12, margin: "4px 0 10px", maxHeight: 34, overflow: "hidden" }}>
                {h.description}
              </div>
              <div className="row" style={{ justifyContent: "space-between" }}>
                <span className="sub" style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
                  ↓ {fmt(h.downloads)}
                </span>
                <button
                  className="btn"
                  style={{ padding: "7px 12px", fontSize: 12.5 }}
                  disabled={installing === h.project_id}
                  onClick={() => install(h)}
                >
                  {installing === h.project_id ? "…" : <><Icon.plus size={13} /> Install</>}
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </>
  );
}

function Installed({ target, showToast }: { target: ContentTarget; showToast: (m: string) => void }) {
  const [items, setItems] = useState<InstalledItem[]>([]);
  const [results, setResults] = useState<Record<string, UpdateResult>>({});
  const [checking, setChecking] = useState(false);

  const reload = useCallback(async () => setItems(await api.listInstalled(target.kind, target.id)), [target]);
  useEffect(() => {
    reload();
  }, [reload]);

  const check = async () => {
    setChecking(true);
    try {
      const r = await api.checkUpdates(target.kind, target.id, target.version);
      setResults(Object.fromEntries(r.map((x) => [x.item.projectId, x])));
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setChecking(false);
    }
  };

  const update = async (it: InstalledItem) => {
    try {
      await api.applyUpdate(target.kind, target.id, it.projectId, target.version);
      showToast(`Updated ${it.title}`);
      await reload();
      await check();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  const remove = async (it: InstalledItem) => {
    await api.contentRemove(target.kind, target.id, it.projectId);
    setResults((r) => {
      const n = { ...r };
      delete n[it.projectId];
      return n;
    });
    await reload();
  };

  const updatable = Object.values(results).filter((r) => r.status === "update");
  const incompatible = Object.values(results).filter((r) => r.status === "incompatible");

  return (
    <>
      <div className="row wrap" style={{ marginBottom: 12 }}>
        <button className="btn-play" style={{ padding: "10px 18px", fontSize: 14 }} disabled={checking} onClick={check}>
          <Icon.refresh size={16} /> {checking ? "Checking…" : `Check updates for ${target.version}`}
        </button>
        {Object.keys(results).length > 0 && (
          <>
            <Pill tone="ok">{updatable.length} update{updatable.length === 1 ? "" : "s"}</Pill>
            {incompatible.length > 0 && <Pill tone="warn">{incompatible.length} not on {target.version}</Pill>}
          </>
        )}
      </div>

      <div className="col" style={{ gap: 2 }}>
        {items.length === 0 && <p className="muted">Nothing installed yet — add some from Browse.</p>}
        {items.map((it) => {
          const r = results[it.projectId];
          return (
            <div className="lrow" key={it.projectId}>
              <div className="avatar">
                <Icon.mods size={18} />
              </div>
              <div className="grow">
                <div className="name">
                  {it.title}{" "}
                  {r?.status === "update" && <Pill tone="ok">→ {r.newVersionNumber}</Pill>}
                  {r?.status === "current" && <Pill tone="ok">up to date</Pill>}
                  {r?.status === "incompatible" && <Pill tone="warn">no {target.version} build</Pill>}
                </div>
                <div className="sub">
                  {it.projectType} · {it.versionNumber}
                </div>
              </div>
              {r?.status === "update" && (
                <button className="btn" onClick={() => update(it)}>
                  <Icon.refresh size={14} /> Update
                </button>
              )}
              <button className="btn danger ghost" onClick={() => remove(it)}>
                <Icon.trash size={15} />
              </button>
            </div>
          );
        })}
      </div>
    </>
  );
}

export function SkinsPanel() {
  const { activeAccount, showToast } = useLauncher();
  const [variant, setVariant] = useState<"classic" | "slim">("classic");
  const [busy, setBusy] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [urlInput, setUrlInput] = useState("");
  const account = activeAccount();
  const online = account?.user_type === "msa";

  const applyUrl = async (url: string, key: string) => {
    setBusy(key);
    try {
      await api.setSkinFromUrl(variant, url);
      showToast("Skin applied — restart your game to see it");
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(null);
    }
  };

  const applyGallery = (s: GallerySkin) => {
    setSelected(s.id);
    setVariant(s.variant);
    void applyUrl(s.url, s.id);
  };

  const onFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (!f) return;
    setBusy("upload");
    try {
      const buf = await f.arrayBuffer();
      await api.setSkin(variant, Array.from(new Uint8Array(buf)));
      showToast("Skin applied — restart your game to see it");
    } catch (err) {
      showToast(`${err}`);
    } finally {
      setBusy(null);
      e.target.value = "";
    }
  };

  if (!online) {
    return (
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Skins</div>
        </div>
        <p className="muted">Sign in with a Microsoft account (top-right) to change your skin.</p>
      </div>
    );
  }

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Skins</div>
        <Field label="Model">
          <div className="seg">
            <button className={variant === "classic" ? "on" : ""} onClick={() => setVariant("classic")}>
              Classic
            </button>
            <button className={variant === "slim" ? "on" : ""} onClick={() => setVariant("slim")}>
              Slim
            </button>
          </div>
        </Field>
      </div>
      <p className="muted">Pick a skin for {account?.username} — it applies instantly.</p>

      <div className="skin-grid">
        {SKIN_GALLERY.map((s) => (
          <button
            key={s.id}
            className={`skin-card${selected === s.id ? " sel" : ""}`}
            disabled={busy !== null}
            onClick={() => applyGallery(s)}
            title={`Apply ${s.name} (${s.variant})`}
          >
            <SkinFace url={s.url} size={72} />
            <span className="skin-name">{busy === s.id ? "Applying…" : s.name}</span>
          </button>
        ))}
      </div>

      <div className="sect-head" style={{ marginTop: 20 }}>
        <div className="sect-title">Use your own</div>
      </div>
      <p className="muted">
        Paste a skin link from any site (e.g. NameMC, The Skindex) or upload a 64×64 PNG. The chosen
        model (Classic / Slim) above is used.
      </p>
      <div className="row wrap" style={{ alignItems: "flex-end" }}>
        <Field label="Skin image URL">
          <input
            className="input"
            style={{ minWidth: 320 }}
            value={urlInput}
            onChange={(e) => setUrlInput(e.target.value)}
            placeholder="https://…/skin.png"
          />
        </Field>
        <button
          className="btn"
          disabled={busy !== null || !urlInput.trim()}
          onClick={() => applyUrl(urlInput.trim(), "url")}
        >
          <Icon.check size={16} /> {busy === "url" ? "Applying…" : "Apply URL"}
        </button>
        <Field label="Upload a .png">
          <input className="input" type="file" accept="image/png" onChange={onFile} disabled={busy !== null} />
        </Field>
      </div>
    </div>
  );
}
