import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { KawmaProfile, StoredAccount } from "../lib/types";
import { Avatar, Icon, initials } from "./ui";
import studioIcon from "../assets/kawma/studio.png";

/** The Kawma account's picture (Twitch/Discord avatar), else its initials,
 *  else a neutral "sign in" face when signed out. */
export function KawmaAvatar({ profile, size = 40 }: { profile: KawmaProfile | null; size?: number }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [profile?.avatar]);
  return (
    <span className={`av kawma-av ${profile ? "" : "out"}`} style={{ width: size, height: size, fontSize: size * 0.38, padding: 0 }}>
      {profile?.avatar && !failed ? (
        <img src={profile.avatar} alt="" width={size} height={size} onError={() => setFailed(true)} />
      ) : profile ? (
        initials(profile.displayName)
      ) : (
        <Icon.user size={Math.round(size * 0.5)} />
      )}
    </span>
  );
}

/** "Twitch · name · Discord · name" for the signed-in profile. */
export function kawmaLinks(p: KawmaProfile) {
  return [p.twitch && `Twitch · ${p.twitch}`, p.discord && `Discord · ${p.discord}`].filter(Boolean).join("   ");
}

/** Is this Minecraft account linked to the Kawma account? (Microsoft accounts only.) */
export const isLinked = (p: KawmaProfile | null, a: StoredAccount) =>
  !!p && a.user_type === "msa" && p.minecraft.some((m) => m.uuid === a.uuid.replace(/-/g, "").toLowerCase());

/** Sign in with Twitch / Discord, or the "finish in your browser" state. */
export function KawmaSignIn({ compact = false }: { compact?: boolean }) {
  const { kawmaLogin, kawmaError, kawmaSignIn, kawmaCancel } = useLauncher();
  if (kawmaLogin) {
    return (
      <div className="kawma-pending">
        <div style={{ fontWeight: 600, fontSize: 13.5 }}>
          Finish signing in with {kawmaLogin.provider === "twitch" ? "Twitch" : "Discord"} in your browser
        </div>
        <div className="muted" style={{ fontSize: 12 }}>
          Check the code matches: <b className="kawma-code">{kawmaLogin.code}</b>
        </div>
        <button className="btn ghost" style={{ alignSelf: "flex-start" }} onClick={kawmaCancel}>
          Cancel
        </button>
      </div>
    );
  }
  return (
    <div className={compact ? "col" : "row wrap"} style={{ gap: compact ? 2 : 10 }}>
      {compact ? (
        <>
          <button className="menu-item" onClick={() => void kawmaSignIn("twitch")}>
            <span className="kawma-provider twitch" /> Sign in with Twitch
          </button>
          <button className="menu-item" onClick={() => void kawmaSignIn("discord")}>
            <span className="kawma-provider discord" /> Sign in with Discord
          </button>
        </>
      ) : (
        <>
          <button className="btn" onClick={() => void kawmaSignIn("twitch")}>
            <span className="kawma-provider twitch" /> Sign in with Twitch
          </button>
          <button className="btn" onClick={() => void kawmaSignIn("discord")}>
            <span className="kawma-provider discord" /> Sign in with Discord
          </button>
        </>
      )}
      {kawmaError && (
        <div style={{ color: "#ff8585", fontSize: 12, padding: compact ? "4px 12px" : 0 }}>{kawmaError}</div>
      )}
    </div>
  );
}

/** Top of the right rail: the Kawma account, with its linked Minecraft accounts. */
export function KawmaAccountButton({ active, onManage }: { active: boolean; onManage: () => void }) {
  const { kawma, store, setActive, kawmaSignOut, kawmaLogin } = useLauncher();
  const [open, setOpen] = useState(false);
  // Keep the menu open while a sign-in finishes so the person sees it land.
  useEffect(() => {
    if (kawmaLogin) setOpen(true);
  }, [kawmaLogin]);

  return (
    <div style={{ position: "relative" }}>
      <button
        className={`game-me ${active || open ? "active" : ""}`}
        onClick={() => setOpen((o) => !o)}
        title={kawma ? `${kawma.displayName} · Kawma account` : "Sign in to Kawma"}
        aria-label="Kawma account"
      >
        <KawmaAvatar profile={kawma} size={44} />
        <span className={`game-me-dot ${kawma ? "on" : ""}`} />
      </button>

      {open && (
        <>
          <div className="backdrop" onClick={() => setOpen(false)} />
          <div className="acct-menu surface kawma-menu">
            <div className="kawma-head">
              <KawmaAvatar profile={kawma} size={42} />
              <div style={{ minWidth: 0 }}>
                <div style={{ fontWeight: 700, fontSize: 15 }}>{kawma ? kawma.displayName : "Kawma account"}</div>
                <div className="muted" style={{ fontSize: 11.5 }}>
                  {kawma ? kawmaLinks(kawma) || "Kawma account" : "Optional · link your Minecraft accounts"}
                </div>
              </div>
            </div>

            {!kawma && <KawmaSignIn compact />}

            <div className="menu-label">{kawma ? "Linked Minecraft accounts" : "Minecraft accounts"}</div>
            {store.accounts.length === 0 && <div className="muted" style={{ fontSize: 12, padding: "2px 12px 6px" }}>None yet.</div>}
            {store.accounts.map((a) => (
              <button
                key={a.uuid}
                className="menu-item"
                onClick={() => {
                  void setActive(a.uuid);
                  setOpen(false);
                }}
              >
                <Avatar account={a} size={28} />
                <span style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, fontSize: 13.5 }}>{a.username}</div>
                  <div style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
                    {a.user_type === "msa" ? (isLinked(kawma, a) ? "Microsoft · linked" : "Microsoft") : "Offline"}
                  </div>
                </span>
                {a.uuid === store.active_uuid && <Icon.check size={16} />}
              </button>
            ))}
            <button
              className="menu-item"
              onClick={() => {
                onManage();
                setOpen(false);
              }}
            >
              <Icon.link size={17} /> Manage Minecraft accounts
            </button>

            {kawma && (
              <>
                <div className="divide" style={{ margin: "6px 4px" }} />
                <button
                  className="menu-item"
                  onClick={() => {
                    void kawmaSignOut();
                    setOpen(false);
                  }}
                >
                  <Icon.close size={16} /> Sign out of Kawma
                </button>
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
}

/** Bottom of the right rail: open Kawma Studio, or get it when it isn't installed. */
export function KawmaStudioTile() {
  const { showToast } = useLauncher();
  const [status, setStatus] = useState<api.StudioStatus | null>(null);
  useEffect(() => {
    const check = () => api.kawmaStudioStatus().then(setStatus).catch(() => {});
    check();
    // Re-check when the launcher comes back to the front (it may have just been installed).
    window.addEventListener("focus", check);
    return () => window.removeEventListener("focus", check);
  }, []);
  const installed = status?.installed === true;
  const label = installed ? "Kawma Studio" : "Get Kawma Studio";
  const open = async () => {
    try {
      const launched = await api.openKawmaStudio();
      showToast(launched ? "Opening Kawma Studio…" : "Opening kawma.app/studio in your browser");
    } catch (e) {
      showToast(`${e}`);
    }
  };
  return (
    <button className="fn-btn studio-btn" onClick={() => void open()} title={label} aria-label={label}>
      <img src={studioIcon} alt="" width={26} height={26} />
      {status && !installed && <span className="studio-plus">+</span>}
    </button>
  );
}
