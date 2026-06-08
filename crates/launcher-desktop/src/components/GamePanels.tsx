import { useState } from "react";

import { useLauncher, loadServers, saveServers } from "../store";
import type { ServerEntry } from "../lib/types";
import { Field, Pill, Icon } from "./ui";

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

/* ----------------------------- Skyrim ---------------------------------- */

export function SkyrimPlay() {
  const { games, launchSkyrim, refreshGames } = useLauncher();
  const sky = games?.skyrim;

  return (
    <div className="hero">
      <div className="eyebrow">The Elder Scrolls V</div>
      <h1 className="title">Skyrim Special Edition</h1>
      <p className="subtitle">{sky?.installed ? sky.install_dir : "Detecting your Steam install…"}</p>

      <div className="action-bar surface">
        <div className="row wrap">
          <Pill tone={sky?.installed ? "ok" : "warn"}>{sky?.installed ? "Installed" : "Not found"}</Pill>
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
        <p className="muted">
          SKSE and Skyrim Together Reborn are detected from your install folder. Install them with
          their own installers, then refresh to enable the matching buttons.
        </p>
      </div>
    </div>
  );
}

export function SkyrimCoop() {
  const { games, launchSkyrim, showToast } = useLauncher();
  const sky = games?.skyrim;
  const [list, setList] = useState<ServerEntry[]>(() => loadServers("skyrim"));
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");

  if (!sky?.installed) return <NotInstalled title="Skyrim" />;

  const persist = (next: ServerEntry[]) => {
    setList(next);
    saveServers("skyrim", next);
  };

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
      <p className="muted">
        Connect to a saved address from the in-game co-op menu after launching Skyrim Together.
      </p>
    </div>
  );
}

export function SkyrimMods() {
  const { games } = useLauncher();
  const sky = games?.skyrim;
  if (!sky?.installed) return <NotInstalled title="Skyrim" />;
  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Modding status</div>
      </div>
      <div className="row wrap">
        <Pill tone={sky.has_skse ? "ok" : "warn"}>SKSE64 {sky.has_skse ? "installed" : "missing"}</Pill>
        <Pill tone={sky.has_skyrim_together ? "ok" : "warn"}>
          Skyrim Together {sky.has_skyrim_together ? "installed" : "missing"}
        </Pill>
      </div>
      <p className="muted">
        Install location: <code>{sky.install_dir}</code>. Use a mod manager (MO2 / Vortex) for load
        order; Aurora handles launching with the right loader.
      </p>
    </div>
  );
}

/* --------------------------- Elden Ring -------------------------------- */

export function EldenRingPlay() {
  const { games, launchEldenRing, refreshGames } = useLauncher();
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
        </div>
        <p className="muted">
          Seamless Co-op launches with anti-cheat disabled — never use it on official servers.
          Official play goes through Steam so EAC and online services start normally.
        </p>
      </div>
    </div>
  );
}

export function EldenRingCoop() {
  const { games, launchEldenRing, setEldenRingPassword } = useLauncher();
  const er = games?.eldenRing;
  const [pw, setPw] = useState(er?.coop_password ?? "");

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
        <p className="muted">
          Seamless Co-op isn't installed. Add the mod to your Elden Ring <code>Game</code> folder,
          then refresh on the Play tab.
        </p>
      )}
    </div>
  );
}
