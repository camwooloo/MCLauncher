import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import { Field, Pill, Icon, Select, Avatar, Markdown } from "./ui";

/** "What's new" — recent releases + their patch notes, from GitHub. */
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
  } = useLauncher();
  const [name, setName] = useState("");

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Sign in</div>
        <button className="btn-play" style={{ padding: "11px 22px", fontSize: 14 }} disabled={busy} onClick={microsoftLogin}>
          <Icon.user size={17} /> Sign in with Microsoft
        </button>
      </div>
      <p className="muted" style={{ marginTop: -4 }}>
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
        <div className="sect-title">Your accounts</div>
      </div>
      <div className="col" style={{ gap: 2 }}>
        {store.accounts.length === 0 && <p className="muted">No accounts yet.</p>}
        {store.accounts.map((a) => {
          const active = a.uuid === store.active_uuid;
          return (
            <div className="lrow" key={a.uuid}>
              <Avatar account={a} size={38} />
              <div className="grow">
                <div className="name">
                  {a.username} {active && <Pill tone="ok">active</Pill>}
                </div>
                <div className="sub">{a.user_type === "msa" ? "Microsoft account" : "Offline account"}</div>
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
  );
}

/** A labelled On/Off toggle matching the launcher's segmented control. */
function Toggle({
  title,
  note,
  value,
  onChange,
}: {
  title: string;
  note: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="row" style={{ justifyContent: "space-between" }}>
      <div>
        <div style={{ fontWeight: 600 }}>{title}</div>
        <div className="muted">{note}</div>
      </div>
      <div className="seg">
        <button className={value ? "on" : ""} onClick={() => onChange(true)}>
          On
        </button>
        <button className={!value ? "on" : ""} onClick={() => onChange(false)}>
          Off
        </button>
      </div>
    </div>
  );
}

export function SettingsPanel() {
  const { settings, persistSettings, paths, systemRamMb } = useLauncher();
  const ramMax = Math.max(2048, Math.floor(systemRamMb / 512) * 512);
  const mcRam = Math.min(settings.maxMemoryMb || 4096, ramMax);

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
            <div style={{ fontWeight: 600 }}>Style</div>
            <div className="muted">The launcher's overall look.</div>
          </div>
          <Select
            value={settings.uiStyle ?? "aurora"}
            onChange={(v) => persistSettings({ uiStyle: v })}
            minWidth={180}
            options={[
              { value: "aurora", label: "Aurora (default)" },
              { value: "liquidglass", label: "Liquid Glass" },
              { value: "compact", label: "Compact (dense)" },
              { value: "cozy", label: "Cozy (big & round)" },
              { value: "classic", label: "Classic (sidebar)" },
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
            <div className="muted">Which screen Aurora shows when it starts.</div>
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
              { value: "network", label: "Aurora Net" },
              { value: "settings", label: "Settings" },
            ]}
          />
        </div>
        <div className="divide" />
        <Toggle
          title="Launch Aurora when Windows starts"
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
            <div className="muted">Show "Playing … via Aurora Launcher" on your Discord profile.</div>
          </div>
          <div className="seg">
            <button
              className={settings.discordRpc !== false ? "on" : ""}
              onClick={() => persistSettings({ discordRpc: true })}
            >
              On
            </button>
            <button
              className={settings.discordRpc === false ? "on" : ""}
              onClick={() => persistSettings({ discordRpc: false })}
            >
              Off
            </button>
          </div>
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
