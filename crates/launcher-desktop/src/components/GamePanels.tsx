import { useState } from "react";

import { useLauncher, loadServers, saveServers } from "../store";
import * as api from "../lib/api";
import type { GameKey, ServerEntry } from "../lib/types";
import { Field, Pill, Icon } from "./ui";

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
export function HomePanel({ onSelect }: { onSelect: (g: GameKey) => void }) {
  return (
    <div className="hero">
      <div className="eyebrow">Welcome to</div>
      <h1 className="title">Aurora</h1>
      <p className="subtitle">One launcher for playing, modding and hosting — together.</p>

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
function CoopServers({ game, hint }: { game: string; hint: string }) {
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

export function SkyrimCoop() {
  const { games, launchSkyrim } = useLauncher();
  const sky = games?.skyrim;
  if (!sky?.installed) return <NotInstalled title="Skyrim" />;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Skyrim Together — servers</div>
        <button
          className="btn-play"
          style={{ padding: "11px 22px", fontSize: 14 }}
          disabled={!sky.has_skyrim_together}
          onClick={() => launchSkyrim("together")}
        >
          <Icon.coop size={17} /> Launch
        </button>
      </div>
      {(!sky.has_skyrim_together || !sky.has_address_library) && <TogetherSetup />}
      <CoopServers
        game="skyrim"
        hint="Connect to a saved address from the in-game co-op menu after launching Skyrim Together."
      />
    </div>
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
            <Icon.upgrade size={15} /> Install SKSE64
          </button>
        )}
      </div>
      {(!sky.has_skyrim_together || !sky.has_address_library) && <TogetherSetup />}
      <p className="muted" style={{ marginTop: 10 }}>
        SKSE is the foundation most Skyrim mods need — Aurora installs it straight from the official
        release. For big load orders (graphics overhauls etc.) use a mod manager (MO2 / Vortex) on
        top; Aurora launches through the right loader either way.
      </p>
    </div>
  );
}

/* --------------------------- Elden Ring -------------------------------- */

export function EldenRingPlay() {
  const { games, launchEldenRing, refreshGames, installTool, busy } = useLauncher();
  const er = games?.eldenRing;

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
    </div>
  );
}

export function EldenRingMods() {
  const { games, launchEldenRing, installTool, busy } = useLauncher();
  const er = games?.eldenRing;
  if (!er?.installed) return <NotInstalled title="Elden Ring" />;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Mod Engine 2</div>
        {er.mods_dir && (
          <button className="btn ghost" onClick={() => api.openPath(er.mods_dir!)}>
            <Icon.folder size={15} /> Open mods folder
          </button>
        )}
      </div>
      <div className="row wrap">
        <Pill tone={er.has_mod_engine ? "ok" : "warn"}>
          Mod Engine 2 {er.has_mod_engine ? "installed" : "missing"}
        </Pill>
      </div>

      {er.has_mod_engine ? (
        <>
          <div className="row wrap" style={{ marginTop: 8 }}>
            <button
              className="btn-play"
              style={{ padding: "11px 22px", fontSize: 14 }}
              onClick={() => launchEldenRing("modded")}
            >
              <Icon.play size={16} /> Launch Modded
            </button>
          </div>
          <p className="muted" style={{ marginTop: 10 }}>
            Drop mods (graphics overhauls, reshades, gameplay packs — usually from Nexus Mods) into
            the <code>mod</code> folder above, then Launch Modded. Mod Engine injects them without
            touching your game files, and runs with EAC off — offline/co-op only.
          </p>
        </>
      ) : (
        <>
          <div className="row wrap" style={{ marginTop: 8 }}>
            <button className="btn" disabled={busy} onClick={() => installTool("modengine2")}>
              <Icon.upgrade size={15} /> Install Mod Engine 2
            </button>
          </div>
          <p className="muted" style={{ marginTop: 10 }}>
            Mod Engine 2 is the standard Elden Ring mod loader — one click installs it from the
            official release, then you get a mods folder and a Launch Modded button.
          </p>
        </>
      )}
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
