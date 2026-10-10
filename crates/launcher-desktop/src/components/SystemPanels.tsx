import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import { Field, Pill, Icon, Select, Switch, Avatar, Markdown } from "./ui";
import { KawmaAvatar, KawmaSignIn, isLinked, kawmaLinks } from "./KawmaAccount";

/** "What's new" — released builds + their patch notes, from kawma.app. */
function WhatsNew() {
  const [data, setData] = useState<api.ReleasesResult | null>(null);
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    api
      .listReleases()
      .then((d) => {
        setData(d);
        setOpen(d.releases[0]?.version ?? null); // expand the newest by default
      })
      .catch(() => setData({ current: "", releases: [] }));
  }, []);

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">What's new</div>
        {data?.current && <Pill>v{data.current}</Pill>}
      </div>
      {!data ? (
        <p className="muted">Loading release notes…</p>
      ) : data.releases.length === 0 ? (
        <p className="muted">Couldn't load release notes (offline?).</p>
      ) : (
        <div className="col" style={{ gap: 8 }}>
          {data.releases.map((r) => {
            const expanded = open === r.version;
            return (
              <div key={r.version} className="surface" style={{ padding: 0, overflow: "hidden" }}>
                <button
                  className="release-head"
                  onClick={() => setOpen(expanded ? null : r.version)}
                >
                  <span className={`select-chev ${expanded ? "up" : ""}`}>
                    <Icon.chevron size={14} />
                  </span>
                  <span style={{ fontWeight: 700 }}>v{r.version}</span>
                  {r.version === data.current && <Pill tone="ok">current</Pill>}
                  <span className="grow" />
                  <span className="muted" style={{ fontSize: 12 }}>{r.date}</span>
                </button>
                {expanded && (
                  <div className="patch-notes" style={{ margin: "0 14px 14px" }}>
                    <Markdown source={r.notes.trim() || "No notes."} />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** The Kawma account and the Minecraft accounts linked to it. */
export function AccountsPanel() {
  const {
    store,
    addOffline,
    microsoftLogin,
    microsoftLoginCode,
    setActive,
    removeAccount,
    loginPrompt,
    loginError,
    busy,
    kawma,
  } = useLauncher();
  const [name, setName] = useState("");

  return (
    <div>
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Kawma account</div>
        {kawma && <Pill tone="ok">Signed in</Pill>}
      </div>
      <div className="lrow">
        <KawmaAvatar profile={kawma} size={42} />
        <div className="grow">
          <div className="name">{kawma ? kawma.displayName : "Not signed in"}</div>
          <div className="sub">
            {kawma
              ? kawmaLinks(kawma) || "Kawma account"
              : "Optional. Sign in with Twitch or Discord to link your Minecraft accounts to Kawma. Everything here works without it."}
          </div>
        </div>
      </div>
      {!kawma && <KawmaSignIn />}
    </div>

    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Link a Minecraft account</div>
        <button className="btn-play" style={{ padding: "11px 22px", fontSize: 14 }} disabled={busy} onClick={microsoftLogin}>
          <Icon.user size={17} /> Sign in with Microsoft
        </button>
      </div>
      <p className="muted" style={{ marginTop: -4 }}>
        {kawma ? "Microsoft accounts you add are linked to your Kawma account. " : ""}
        Opens your browser — just sign in and approve, no code to copy.{" "}
        <button className="linklike" disabled={busy} onClick={microsoftLoginCode}>
          Use a sign-in code instead
        </button>
      </p>

      <div className="row wrap" style={{ alignItems: "flex-end" }}>
        <Field label="Offline username">
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Steve" />
        </Field>
        <button
          className="btn"
          onClick={() => {
            addOffline(name);
            setName("");
          }}
        >
          <Icon.plus size={16} /> Add offline
        </button>
      </div>

      {loginError && (
        <div
          className="surface"
          style={{ marginTop: 6, padding: "12px 16px", borderColor: "rgba(255,120,120,0.5)" }}
        >
          <div style={{ fontWeight: 600, color: "#ff8585" }}>Sign-in failed</div>
          <p className="muted" style={{ margin: "4px 0 0" }}>{loginError}</p>
          <p className="muted" style={{ margin: "6px 0 0", fontSize: 12 }}>
            Make sure the account owns Minecraft: Java Edition and has an Xbox profile, then try again.
          </p>
        </div>
      )}

      {loginPrompt && (
        <div className="action-bar surface" style={{ marginTop: 6 }}>
          <div>
            <div className="sect-title">Finish signing in</div>
            <p className="muted" style={{ margin: "6px 0 0" }}>{loginPrompt.message}</p>
          </div>
          <div className="row" style={{ gap: 14 }}>
            <div className="codebox">{loginPrompt.userCode}</div>
            <button className="btn" onClick={() => api.openUrl(loginPrompt.verificationUri)}>
              <Icon.link size={16} /> Open page
            </button>
          </div>
        </div>
      )}

      <div className="sect-head" style={{ marginTop: 18 }}>
        <div className="sect-title">{kawma ? "Linked Minecraft accounts" : "Minecraft accounts"}</div>
      </div>
      <div className="col" style={{ gap: 2 }}>
        {store.accounts.length === 0 && <p className="muted">No Minecraft accounts yet.</p>}
        {store.accounts.map((a) => {
          const active = a.uuid === store.active_uuid;
          return (
            <div className="lrow" key={a.uuid}>
              <Avatar account={a} size={38} />
              <div className="grow">
                <div className="name">
                  {a.username} {active && <Pill tone="ok">active</Pill>}
                </div>
                <div className="sub">
                  {a.user_type === "msa"
                    ? isLinked(kawma, a)
                      ? "Microsoft account · linked to Kawma"
                      : "Microsoft account"
                    : "Offline account · stays on this PC"}
                </div>
              </div>
              {!active && (
                <button className="btn ghost" onClick={() => setActive(a.uuid)}>
                  Use
                </button>
              )}
              <button className="btn danger ghost" onClick={() => removeAccount(a.uuid)}>
                <Icon.trash size={15} />
              </button>
            </div>
          );
        })}
      </div>
    </div>
    </div>
  );
}

/** A labelled on/off switch row. */
function Toggle({
  title,
  note,
  value,
  onChange,
  disabled,
  hint,
}: {
  title: string;
  note: string;
  value: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  /** Why the switch is unavailable. */
  hint?: string | null;
}) {
  return (
    <div className="row" style={{ justifyContent: "space-between" }}>
      <div>
        <div style={{ fontWeight: 600 }}>{title}</div>
        <div className="muted">{note}</div>
        {hint && <div className="muted" style={{ fontSize: 12, marginTop: 2, color: "var(--accent)" }}>{hint}</div>}
      </div>
      <div style={disabled ? { opacity: 0.45, pointerEvents: "none" } : undefined} aria-disabled={disabled}>
        <Switch value={value && !disabled} onChange={onChange} label={title} />
      </div>
    </div>
  );
}

/** Why a Kawma feature switch can't be used yet, or null when it can. */
function kawmaGate(signedIn: boolean, released: boolean) {
  if (!signedIn) return "Sign in to Kawma (top of the right rail) to turn this on.";
  if (!released) return "Coming soon for your account.";
  return null;
}

export function SettingsPanel() {
  const { settings, persistSettings, paths, systemRamMb, showToast, kawma, kawmaReleased } = useLauncher();
  const nowPlayingGate = kawmaGate(!!kawma, kawmaReleased("launcher-now-playing"));
  const communityGate = kawmaGate(!!kawma, kawmaReleased("launcher-community"));
  const ramMax = Math.max(2048, Math.floor(systemRamMb / 512) * 512);
  const mcRam = Math.min(settings.maxMemoryMb || 4096, ramMax);
  const [netBusy, setNetBusy] = useState(false);
  const allowRemote = async () => {
    setNetBusy(true);
    try {
      const applied = await api.repairAuroraNet();
      showToast(applied ? "This PC is now reachable on your network" : "Network access already set up");
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setNetBusy(false);
    }
  };

  return (
    <div>
      {/* Appearance */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Appearance</div>
        </div>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Theme</div>
            <div className="muted">Dark or light glass.</div>
          </div>
          <div className="seg">
            <button
              className={settings.theme === "dark" ? "on" : ""}
              onClick={() => persistSettings({ theme: "dark" })}
            >
              <Icon.moon size={15} /> Dark
            </button>
            <button
              className={settings.theme === "light" ? "on" : ""}
              onClick={() => persistSettings({ theme: "light" })}
            >
              <Icon.sun size={15} /> Light
            </button>
          </div>
        </div>
        <div className="divide" />
        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Window</div>
            <div className="muted">One edge-to-edge surface, or a rounded card floating in the window.</div>
          </div>
          <Select
            value={settings.windowFrame ?? "flush"}
            onChange={(v) => persistSettings({ windowFrame: v })}
            minWidth={180}
            options={[
              { value: "flush", label: "Flush (default)" },
              { value: "floating", label: "Floating card" },
            ]}
          />
        </div>
        <div className="divide" />
        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Background</div>
            <div className="muted">How the backdrop behaves.</div>
          </div>
          <Select
            value={settings.background ?? "liquid"}
            onChange={(v) => persistSettings({ background: v })}
            minWidth={180}
            options={[
              { value: "pulsing", label: "Aurora (ribbons)" },
              { value: "liquid", label: "Liquid metal" },
              { value: "mesh", label: "Gradient mesh" },
              { value: "grid", label: "Synthwave grid" },
              { value: "stars", label: "Starfield" },
              { value: "waves", label: "Waves" },
              { value: "glow", label: "Glow" },
              { value: "dots", label: "Dot grid" },
              { value: "static", label: "Static (no motion)" },
            ]}
          />
        </div>
      </div>

      {/* Startup */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Startup</div>
        </div>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Open to</div>
            <div className="muted">Which screen Kawma shows when it starts.</div>
          </div>
          <Select
            value={settings.defaultView ?? "home"}
            onChange={(v) => persistSettings({ defaultView: v })}
            minWidth={220}
            options={[
              { value: "home", label: "Home" },
              { value: "minecraft", label: "Minecraft — Play" },
              { value: "minecraft:Servers", label: "Minecraft — Servers" },
              { value: "minecraft:Skins", label: "Minecraft — Skins" },
              { value: "skyrim", label: "Skyrim — Play" },
              { value: "skyrim:Co-op", label: "Skyrim — Co-op" },
              { value: "eldenring", label: "Elden Ring — Play" },
              { value: "eldenring:Co-op", label: "Elden Ring — Co-op" },
              { value: "cyberpunk", label: "Cyberpunk — Play" },
              { value: "network", label: "Kawma Net" },
              { value: "settings", label: "Settings" },
            ]}
          />
        </div>
        <div className="divide" />
        <Toggle
          title="Launch Kawma when Windows starts"
          note="Great for hosting — your auto-start servers come online with your PC."
          value={settings.launchAtLogin === true}
          onChange={(v) => {
            persistSettings({ launchAtLogin: v });
            api.setLaunchAtLogin(v).catch(() => {});
          }}
        />
        <div className="divide" />
        <Toggle
          title="Start minimized to tray"
          note="Open hidden in the system tray instead of showing the window."
          value={settings.startMinimized === true}
          onChange={(v) => persistSettings({ startMinimized: v })}
        />
        <div className="divide" />
        <Toggle
          title="Close button minimizes to tray"
          note="Keeps hosted servers running when you close the window. Turn off to quit fully (stops servers)."
          value={settings.closeToTray !== false}
          onChange={(v) => persistSettings({ closeToTray: v })}
        />
      </div>

      {/* Minecraft */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Minecraft</div>
        </div>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Default memory · {(mcRam / 1024).toFixed(1)} GB</div>
            <div className="muted">
              Starting RAM for new instances &amp; servers. Tune per item when you create it.
            </div>
          </div>
          <input
            type="range"
            min={1024}
            max={ramMax}
            step={512}
            value={mcRam}
            onChange={(e) => persistSettings({ maxMemoryMb: Number(e.target.value) })}
            style={{ width: 220 }}
          />
        </div>
      </div>

      {/* Integrations */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Integrations</div>
        </div>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Discord Rich Presence</div>
            <div className="muted">Show "Playing … via Kawma Launcher" on your Discord profile.</div>
          </div>
          <Switch
            value={settings.discordRpc !== false}
            onChange={(v) => persistSettings({ discordRpc: v })}
            label="Discord Rich Presence"
          />
        </div>
      </div>

      {/* Kawma — optional, off by default, needs the Kawma sign-in */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Kawma</div>
          {!kawma && <Pill>Signed out</Pill>}
        </div>
        <Toggle
          title="Now Playing: share what you're playing with Kawma"
          note="When you launch a game here, Kawma shows it on your stream and in Kawma Studio. Cleared when you stop playing. Needs your Kawma sign-in."
          value={settings.nowPlaying === true}
          onChange={(v) => persistSettings({ nowPlaying: v })}
          disabled={!!nowPlayingGate}
          hint={nowPlayingGate}
        />
        <div className="divide" />
        <Toggle
          title="Play with your community"
          note="When you share a Minecraft server to the internet, post it to your Kawma community on Twitch and Discord. With the whitelist on, viewers ask with !server (Twitch) or /server (Discord) and Kawma whitelists them for you. You can still choose per share. Needs your Kawma sign-in."
          value={settings.community === true}
          onChange={(v) => persistSettings({ community: v })}
          disabled={!!communityGate}
          hint={communityGate}
        />
      </div>

      {/* Network — LAN remote control */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Network</div>
        </div>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontWeight: 600 }}>Remote control on this network</div>
            <div className="muted">
              Let other PCs running Kawma on your network find and control this one (manage its servers
              from your main PC). Run this once on <b>each</b> PC — it allows Kawma through the firewall.
            </div>
          </div>
          <button className="btn" disabled={netBusy} onClick={allowRemote}>
            <Icon.coop size={15} /> {netBusy ? "Allowing…" : "Allow on network"}
          </button>
        </div>
      </div>

      {/* Locations */}
      <div className="sect">
        <div className="sect-head">
          <div className="sect-title">Locations</div>
        </div>
        <div className="lrow">
          <div className="avatar"><Icon.folder size={18} /></div>
          <div className="grow">
            <div className="name">Game directory</div>
            <div className="sub">{paths?.gameDir ?? "…"}</div>
          </div>
        </div>
        <div className="lrow">
          <div className="avatar"><Icon.folder size={18} /></div>
          <div className="grow">
            <div className="name">Launcher data</div>
            <div className="sub">{paths?.dataDir ?? "…"}</div>
          </div>
        </div>
      </div>

      <WhatsNew />
    </div>
  );
}
