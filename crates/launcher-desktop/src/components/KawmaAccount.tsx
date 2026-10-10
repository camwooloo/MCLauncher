import { useEffect, useRef, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { KawmaProfile, StoredAccount } from "../lib/types";
import { Avatar, Icon, initials } from "./ui";
import studioIcon from "../assets/kawma/studio.png";
import { Popover } from "./Popover";
import { readLocalBest, UNLOCK_AT } from "./StarCatcher";

/* Brand marks, filled, to sit beside the launcher's line icons. */
type LogoProps = { size?: number };
const TwitchLogo = ({ size = 16 }: LogoProps) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
    <path d="M11.571 4.714h1.715v5.143H11.57zm4.715 0H18v5.143h-1.714zM6 0 1.714 4.286v15.428h5.143V24l4.286-4.286h3.428L22.286 12V0zm14.571 11.143-3.428 3.428h-3.429l-3 3v-3H6.857V1.714h13.714Z" />
  </svg>
);
const DiscordLogo = ({ size = 16 }: LogoProps) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
    <path d="M20.317 4.37a19.79 19.79 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.865-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.74 19.74 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.1 13.1 0 0 1-1.872-.892.077.077 0 0 1-.008-.128c.126-.094.252-.192.372-.291a.074.074 0 0 1 .078-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.009c.12.099.246.198.373.292a.077.077 0 0 1-.006.127 12.3 12.3 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.84 19.84 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z" />
  </svg>
);
const KickLogo = ({ size = 16 }: LogoProps) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
    <path d="M3 3h6v4.5h2V5.25h2.25V3H21v6h-2.25v2.25H16.5v1.5h2.25V15H21v6h-7.75v-2.25H11V16.5H9V21H3V3Z" />
  </svg>
);
const StarIcon = ({ size = 16 }: LogoProps) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
    <path d="M12 2.6l2.83 5.73 6.33.92-4.58 4.46 1.08 6.3L12 17.03l-5.66 2.98 1.08-6.3L2.84 9.25l6.33-.92z" />
  </svg>
);
const LOGOS = { twitch: TwitchLogo, discord: DiscordLogo, kick: KickLogo };
type Platform = keyof typeof LOGOS;
const PLATFORM_NAME: Record<Platform, string> = { twitch: "Twitch", discord: "Discord", kick: "Kick" };

/** One linked platform in the account menu: its logo in brand colour, and the name. */
function LinkRow({ platform, name, second }: { platform: Platform; name: string; second?: boolean }) {
  const Logo = LOGOS[platform];
  return (
    <div className={`kawma-link ${platform}`} title={second ? `Second ${PLATFORM_NAME[platform]} account, managed on kawma.app` : undefined}>
      <span className="kawma-link-logo">
        <Logo size={15} />
      </span>
      <span className="kawma-link-text">
        <span className="kawma-link-platform">{PLATFORM_NAME[platform]}</span>
        <span className="kawma-link-name">{name}</span>
      </span>
      {second && <span className="kawma-link-tag">Second</span>}
    </div>
  );
}

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

/** "Twitch · name   Discord · name   Kick · name" for the signed-in profile. */
export function kawmaLinks(p: KawmaProfile) {
  return [p.twitch && `Twitch · ${p.twitch}`, p.discord && `Discord · ${p.discord}`, p.kick && `Kick · ${p.kick}`]
    .filter(Boolean)
    .join("   ");
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
            <span className="kawma-provider twitch"><TwitchLogo size={12} /></span> Sign in with Twitch
          </button>
          <button className="menu-item" onClick={() => void kawmaSignIn("discord")}>
            <span className="kawma-provider discord"><DiscordLogo size={12} /></span> Sign in with Discord
          </button>
        </>
      ) : (
        <>
          <button className="btn" onClick={() => void kawmaSignIn("twitch")}>
            <span className="kawma-provider twitch"><TwitchLogo size={12} /></span> Sign in with Twitch
          </button>
          <button className="btn" onClick={() => void kawmaSignIn("discord")}>
            <span className="kawma-provider discord"><DiscordLogo size={12} /></span> Sign in with Discord
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
  const btnRef = useRef<HTMLButtonElement>(null);
  // Keep the menu open while a sign-in finishes so the person sees it land.
  useEffect(() => {
    if (kawmaLogin) setOpen(true);
  }, [kawmaLogin]);

  // Star catcher best: what kawma.app keeps for the account (refreshed when the
  // menu opens, a round may have just finished), else the best on this PC.
  const [serverStars, setServerStars] = useState<{ best: number; unlocked: boolean } | null>(null);
  const kawmaId = kawma?.id ?? null;
  useEffect(() => {
    setServerStars(null);
  }, [kawmaId]);
  useEffect(() => {
    if (!open || !kawmaId) return;
    let live = true;
    api
      .starsBest()
      .then((r) => {
        if (live && r) setServerStars({ best: r.best, unlocked: r.unlocked });
      })
      .catch(() => {
        /* keep what the profile says */
      });
    return () => {
      live = false;
    };
  }, [open, kawmaId]);
  const kept = serverStars ?? kawma?.stars ?? null;
  const localBest = open ? readLocalBest() : 0;
  const stars = kept ? { best: kept.best, unlocked: kept.unlocked } : localBest > 0 ? { best: localBest, unlocked: false } : null;

  return (
    <div style={{ position: "relative" }}>
      <button
        ref={btnRef}
        className={`game-me ${active || open ? "active" : ""}`}
        onClick={() => setOpen((o) => !o)}
        title={kawma ? `${kawma.displayName} · Kawma account` : "Sign in to Kawma"}
        aria-label="Kawma account"
      >
        <KawmaAvatar profile={kawma} size={44} />
        <span className={`game-me-dot ${kawma ? "on" : ""}`} />
      </button>

      <Popover anchor={btnRef} open={open} onClose={() => setOpen(false)} placement="left-start" offset={18} className="acct-menu surface kawma-menu">
        <div className="kawma-head">
          <KawmaAvatar profile={kawma} size={54} />
          <div style={{ minWidth: 0 }}>
            <div className="kawma-eyebrow">{kawma ? "Kawma account" : "One Kawma account"}</div>
            <div className="kawma-name">{kawma ? kawma.displayName : "Sign in to Kawma"}</div>
            {!kawma && <div className="muted" style={{ fontSize: 11.5 }}>Optional · link your Minecraft accounts</div>}
          </div>
        </div>

        {kawma && (kawma.twitch || kawma.discord || kawma.kick || kawma.extraTwitch || kawma.extraDiscord) && (
          <div className="kawma-links">
            {kawma.twitch && <LinkRow platform="twitch" name={kawma.twitch} />}
            {kawma.kick && <LinkRow platform="kick" name={kawma.kick} />}
            {kawma.discord && <LinkRow platform="discord" name={kawma.discord} />}
            {kawma.extraTwitch && <LinkRow platform="twitch" name={kawma.extraTwitch} second />}
            {kawma.extraDiscord && <LinkRow platform="discord" name={kawma.extraDiscord} second />}
          </div>
        )}

        {!kawma && <KawmaSignIn compact />}

        {stars && (
          <div className="kawma-stars">
            <span className="kawma-stars-ico">
              <StarIcon size={15} />
            </span>
            <span style={{ minWidth: 0 }}>
              <div>
                Star catcher best · <b>{stars.best}</b>
              </div>
              <div className="kawma-stars-hint">
                {stars.unlocked
                  ? "Unlocked Starfield in Studio"
                  : kawma
                    ? `Catch ${UNLOCK_AT} in one round to unlock Starfield in Studio`
                    : "Sign in to keep it on your Kawma account"}
              </div>
            </span>
          </div>
        )}

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
      </Popover>
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
