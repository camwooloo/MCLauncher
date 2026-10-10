import { useEffect, useRef, useState, type ReactNode } from "react";

import * as api from "../lib/api";
import { useLauncher } from "../store";
import launcherIcon from "../assets/kawma/launcher.png";
import { Popover } from "./Popover";

const TEX_BASE =
  "https://raw.githubusercontent.com/InventivetalentDev/minecraft-assets/1.21.1/assets/minecraft/textures";

/** Pixel-art icon for a Minecraft item id; tries item→block texture, then a
 *  letter fallback (covers blocks and any non-vanilla / modpack item). */
export function ItemIcon({ id, size = 34 }: { id: string; size?: number }) {
  const [stage, setStage] = useState(0);
  useEffect(() => setStage(0), [id]);

  const vanilla = !id.includes(":") || id.startsWith("minecraft:");
  const name = id.includes(":") ? id.split(":")[1] : id;

  if (!vanilla || stage >= 2 || !name) {
    return (
      <div
        style={{
          width: size,
          height: size,
          display: "grid",
          placeItems: "center",
          fontFamily: "var(--font-display)",
          fontWeight: 700,
          fontSize: size * 0.42,
          color: "var(--accent)",
          background: "rgba(var(--accent-rgb),0.12)",
          borderRadius: 8,
        }}
      >
        {name ? name[0].toUpperCase() : "?"}
      </div>
    );
  }
  const src = stage === 0 ? `${TEX_BASE}/item/${name}.png` : `${TEX_BASE}/block/${name}.png`;
  return (
    <img
      src={src}
      width={size}
      height={size}
      alt={id}
      onError={() => setStage((s) => s + 1)}
      style={{ imageRendering: "pixelated", objectFit: "contain" }}
    />
  );
}

/** A frosted-glass card. */
export function Glass({
  children,
  className = "",
  style,
}: {
  children: ReactNode;
  className?: string;
  style?: React.CSSProperties;
}) {
  return (
    <div className={`glass card ${className}`} style={style}>
      {children}
    </div>
  );
}

export function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="field">
      <label>{label}</label>
      {children}
    </div>
  );
}

export function Progress({ value }: { value: number }) {
  return (
    <div className="progress">
      <span style={{ width: `${Math.round(value * 100)}%` }} />
    </div>
  );
}

export function Pill({
  children,
  tone = "default",
}: {
  children: ReactNode;
  tone?: "default" | "ok" | "warn";
}) {
  return (
    <span className={`pill ${tone === "ok" ? "ok" : tone === "warn" ? "warn" : ""}`}>
      {(tone === "ok" || tone === "warn") && <span className="dot" />}
      {children}
    </span>
  );
}

/* ---- Icons (inline SVG, currentColor) -------------------------------- */

type IconProps = { size?: number };
const svg = (size: number, path: ReactNode, extra?: object) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.7}
    strokeLinecap="round"
    strokeLinejoin="round"
    {...extra}
  >
    {path}
  </svg>
);

export const Icon = {
  home: ({ size = 24 }: IconProps) =>
    svg(size, <><path d="M4 11l8-7 8 7" /><path d="M6 9.5V20h12V9.5" /><path d="M10 20v-5.5h4V20" /></>),
  // Grass block — Minecraft.
  minecraft: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M12 3 4 7.5v9L12 21l8-4.5v-9z" />
        <path d="M4 7.5 12 12l8-4.5" />
        <path d="M12 12v9" />
        <path d="M6.2 8.8 12 12l5.8-3.2" opacity="0.55" />
      </>
    ),
  // Horned Nord helmet — Skyrim.
  skyrim: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M5 14a7 7 0 0 1 14 0v2H5z" />
        <path d="M5.5 13C2.5 11.5 2.2 7.5 3 5c2.2 1.8 3.2 4 3.2 6.4" />
        <path d="M18.5 13c3-1.5 3.3-5.5 2.5-8-2.2 1.8-3.2 4-3.2 6.4" />
        <path d="M12 9.5v4.5" />
      </>
    ),
  // Glitched hex-chip — Cyberpunk 2077.
  cyberpunk: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M7 4h13l-3 5 3 5-3 6H7" />
        <path d="M7 4 4 9l3 5-3 6" opacity="0.6" />
        <path d="M9.5 12h7" />
        <path d="M9.5 8.5h4M13 15.5h3.5" opacity="0.6" />
      </>
    ),
  // Ring crowned by the Erdtree — Elden Ring.
  elden: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <circle cx="12" cy="13" r="7.5" />
        <path d="M12 13V3" />
        <path d="M12 6.5 9 4M12 6.5 15 4M12 9 8.5 6.5M12 9l3.5-2.5" />
      </>
    ),
  play: ({ size = 24 }: IconProps) => svg(size, <path d="M7 4v16l13-8z" fill="currentColor" stroke="none" />),
  user: ({ size = 24 }: IconProps) =>
    svg(size, <><circle cx="12" cy="8" r="4" /><path d="M4 21c0-4 4-6 8-6s8 2 8 6" /></>),
  gear: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
        <circle cx="12" cy="12" r="3" />
      </>
    ),
  server: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <rect x="3" y="4" width="18" height="7" rx="2" />
        <rect x="3" y="13" width="18" height="7" rx="2" />
        <path d="M7 7.5h.01M7 16.5h.01" />
      </>
    ),
  mods: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z" />
        <path d="M12 12l8-4.5M12 12v9M12 12L4 7.5" />
      </>
    ),
  coop: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <circle cx="8" cy="9" r="3" />
        <circle cx="16" cy="9" r="3" />
        <path d="M2 20c0-3 3-5 6-5M22 20c0-3-3-5-6-5" />
      </>
    ),
  plus: ({ size = 24 }: IconProps) => svg(size, <path d="M12 5v14M5 12h14" />),
  refresh: ({ size = 24 }: IconProps) =>
    svg(size, <><path d="M21 12a9 9 0 1 1-3-6.7" /><path d="M21 4v4h-4" /></>),
  link: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M10 14a4 4 0 0 0 6 .5l2-2a4 4 0 0 0-6-6l-1 1" />
        <path d="M14 10a4 4 0 0 0-6-.5l-2 2a4 4 0 0 0 6 6l1-1" />
      </>
    ),
  copy: ({ size = 24 }: IconProps) =>
    svg(size, <><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V5a2 2 0 0 1 2-2h10" /></>),
  trash: ({ size = 24 }: IconProps) =>
    svg(size, <><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></>),
  check: ({ size = 24 }: IconProps) => svg(size, <path d="M5 13l4 4L19 7" />),
  min: ({ size = 24 }: IconProps) => svg(size, <path d="M5 12h14" />),
  max: ({ size = 24 }: IconProps) => svg(size, <rect x="5" y="5" width="14" height="14" rx="2" />),
  close: ({ size = 24 }: IconProps) => svg(size, <path d="M6 6l12 12M18 6L6 18" />),
  folder: ({ size = 24 }: IconProps) =>
    svg(size, <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />),
  chevron: ({ size = 24 }: IconProps) => svg(size, <path d="M6 9l6 6 6-6" />),
  stop: ({ size = 24 }: IconProps) => svg(size, <rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" stroke="none" />),
  terminal: ({ size = 24 }: IconProps) => svg(size, <><path d="M5 8l4 4-4 4" /><path d="M13 16h6" /></>),
  sun: ({ size = 24 }: IconProps) =>
    svg(size, <><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M19 5l-1.5 1.5M6.5 17.5L5 19" /></>),
  moon: ({ size = 24 }: IconProps) => svg(size, <path d="M21 12.8A8 8 0 1 1 11.2 3a6 6 0 0 0 9.8 9.8z" />),
  sliders: ({ size = 24 }: IconProps) =>
    svg(size, <><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle cx="10" cy="17" r="2" /></>),
  sparkles: ({ size = 24 }: IconProps) =>
    svg(size, <><path d="M12 3l1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6z" /><path d="M19 14l.8 2.2L22 17l-2.2.8L19 20l-.8-2.2L16 17l2.2-.8z" /></>),
  dots: ({ size = 24 }: IconProps) =>
    svg(size, <><circle cx="5" cy="12" r="1.6" fill="currentColor" stroke="none" /><circle cx="12" cy="12" r="1.6" fill="currentColor" stroke="none" /><circle cx="19" cy="12" r="1.6" fill="currentColor" stroke="none" /></>),
  upgrade: ({ size = 24 }: IconProps) =>
    svg(size, <><path d="M12 20V8" /><path d="M6 12l6-6 6 6" /><path d="M5 4h14" /></>),
  chest: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <rect x="3" y="7" width="18" height="13" rx="2" />
        <path d="M3 12h18M10 12v3h4v-3" />
        <path d="M5 7V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v2" />
      </>
    ),
  host: ({ size = 24 }: IconProps) =>
    svg(size, <><rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path d="M7 7.5h.01M7 16.5h.01M17 7.5h2M17 16.5h2" /></>),
  gamepad: ({ size = 24 }: IconProps) =>
    svg(
      size,
      <>
        <path d="M7 7h10a5 5 0 0 1 4.9 6l-.7 3.3a2.4 2.4 0 0 1-4.1 1.1L15 15H9l-2.1 2.4a2.4 2.4 0 0 1-4.1-1.1L2.1 13A5 5 0 0 1 7 7z" />
        <path d="M7 10v4M5 12h4" />
        <path d="M15.5 11h.01M17.5 13h.01" />
      </>
    ),
  shirt: ({ size = 24 }: IconProps) =>
    svg(size, <path d="M8 3 3 6l2 4 2-1v12h10V9l2 1 2-4-5-3a4 4 0 0 1-8 0z" />),
  globe: ({ size = 24 }: IconProps) =>
    svg(size, <><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></>),
  clock: ({ size = 24 }: IconProps) =>
    svg(size, <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>),
};

export function initials(name: string) {
  return name.slice(0, 2).toUpperCase();
}

/** Render the face (head + hat overlay) of a 64×64 skin texture onto a canvas,
 *  for previewing any skin URL in the gallery. Drawn at native pixels then
 *  scaled up crisp — no external render service needed. */
export function SkinFace({ url, size = 72 }: { url: string; size?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      ctx.clearRect(0, 0, size, size);
      ctx.imageSmoothingEnabled = false;
      // Head front face at (8,8) and the hat/overlay layer at (40,8), each 8×8.
      ctx.drawImage(img, 8, 8, 8, 8, 0, 0, size, size);
      ctx.drawImage(img, 40, 8, 8, 8, 0, 0, size, size);
    };
    img.src = url;
  }, [url, size]);
  return (
    <canvas
      ref={ref}
      width={size}
      height={size}
      style={{ width: size, height: size, imageRendering: "pixelated", display: "block" }}
    />
  );
}

/** Render URL for a player's skin head (face), by UUID — what other launchers
 *  show. mc-heads accepts the dashless UUID we store and renders the account's
 *  *current* skin. */
export function skinHeadUrl(uuid: string, size = 64) {
  return `https://mc-heads.net/avatar/${uuid}/${size}`;
}

/** A player avatar: the live skin-head portrait for Microsoft accounts, with a
 *  graceful fall back to monogram initials (offline accounts, or if the head
 *  service is unreachable). */
export function Avatar({
  account,
  size = 32,
}: {
  account: { username: string; uuid: string; user_type: string };
  size?: number;
}) {
  const isMsa = account.user_type === "msa";
  const [failed, setFailed] = useState(false);
  const showHead = isMsa && !failed;
  return (
    <span
      className="av"
      style={{ width: size, height: size, fontSize: size * 0.38, borderRadius: Math.round(size * 0.3), padding: 0 }}
    >
      {showHead ? (
        <img
          src={skinHeadUrl(account.uuid, Math.max(64, Math.round(size * 2)))}
          alt={account.username}
          width={size}
          height={size}
          style={{ width: "100%", height: "100%", objectFit: "cover", imageRendering: "pixelated" }}
          onError={() => setFailed(true)}
        />
      ) : (
        initials(account.username)
      )}
    </span>
  );
}

/** The Kawma Launcher mark (static): the purple Kawma planet on a
 *  transparent background. The rail's animated version is KawmaLogo. */
export function KawmaMark({ size = 24 }: { size?: number }) {
  return <img className="kawma-mark" src={launcherIcon} width={size} height={size} alt="Kawma Launcher" draggable={false} />;
}

/** Inline markdown: **bold**, `code`, [text](url). */
function renderInline(text: string, base: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(\*\*([^*]+)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    if (m[2] !== undefined) out.push(<strong key={`${base}b${i}`}>{m[2]}</strong>);
    else if (m[3] !== undefined) out.push(<code key={`${base}c${i}`} className="md-code">{m[3]}</code>);
    else if (m[4] !== undefined)
      out.push(
        <a key={`${base}l${i}`} href={m[5]} target="_blank" rel="noreferrer">
          {m[4]}
        </a>
      );
    last = re.lastIndex;
    i++;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** A small, dependency-free Markdown renderer — enough for release notes
 *  (headings, **bold**, `code`, links, and bullet lists). */
export function Markdown({ source }: { source: string }) {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let list: ReactNode[] = [];
  let key = 0;
  const flush = () => {
    if (list.length) {
      blocks.push(
        <ul key={`ul${key++}`} className="md-ul">
          {list}
        </ul>
      );
      list = [];
    }
  };
  for (const raw of lines) {
    const t = raw.trim();
    if (!t) {
      flush();
      continue;
    }
    const h = /^(#{1,6})\s+(.*)$/.exec(t);
    const li = /^[-*]\s+(.*)$/.exec(t);
    if (h) {
      flush();
      const lvl = Math.min(h[1].length, 4);
      blocks.push(
        <div key={`h${key}`} className={`md-h md-h${lvl}`}>
          {renderInline(h[2], `h${key++}`)}
        </div>
      );
    } else if (li) {
      list.push(<li key={`li${key}`}>{renderInline(li[1], `li${key++}`)}</li>);
    } else {
      flush();
      blocks.push(
        <p key={`p${key}`} className="md-p">
          {renderInline(t, `p${key++}`)}
        </p>
      );
    }
  }
  flush();
  return <div className="md">{blocks}</div>;
}

/** Shows the address(es) friends use to reach a server hosted on this PC —
 *  the Kawma Net IP (preferred, no port forwarding) and/or the LAN IP. Never
 *  shows localhost, which only works for the host themselves. */
/** One "how friends connect" method row. */
function AddrRow({ tag, note, addr, onCopy, accent }: { tag: string; note: string; addr: string; onCopy: () => void; accent?: boolean }) {
  return (
    <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 10 }}>
      <div style={{ minWidth: 0 }}>
        <div className="row" style={{ gap: 6, alignItems: "center" }}>
          <Pill tone={accent ? "ok" : "default"}>{tag}</Pill>
          <b style={{ fontVariantNumeric: "tabular-nums" }}>{addr}</b>
        </div>
        <div className="muted" style={{ fontSize: 11.5 }}>{note}</div>
      </div>
      <button className="btn ghost" onClick={onCopy}>
        <Icon.copy size={14} /> Copy
      </button>
    </div>
  );
}

/** How friends connect to a hosted server — three methods:
 *  a public bore tunnel (any launcher, no port-forward), Kawma Net (private
 *  VPN), and direct/port-forward. `tunnelKey` scopes the tunnel; `internetTunnel`
 *  is false for UDP games (Skyrim Together) where bore (TCP) can't help. */
export function HostAddress({
  port,
  onCopy,
  tunnelKey,
  internetTunnel = true,
}: {
  port: number;
  onCopy?: (msg: string) => void;
  tunnelKey?: string;
  internetTunnel?: boolean;
}) {
  const key = tunnelKey ?? `port-${port}`;
  const { settings, kawma, kawmaReleased } = useLauncher();
  // "Post to my Kawma community" for this share (Minecraft servers shared to
  // the internet), defaulting to the Settings choice.
  const canPost = internetTunnel && !!tunnelKey && !!kawma && kawmaReleased("launcher-community");
  const [postToCommunity, setPostToCommunity] = useState(settings.community === true);
  useEffect(() => setPostToCommunity(settings.community === true), [settings.community]);
  const [a, setA] = useState<api.HostAddresses | null>(null);
  const [pub, setPub] = useState<string | null>(null);
  const [tunnel, setTunnel] = useState<api.TunnelInfo | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = () => {
    api.hostAddresses().then(setA).catch(() => {});
    api.tunnelStatus().then((m) => setTunnel(m[key] ?? null)).catch(() => {});
  };
  useEffect(() => {
    refresh();
    api.publicIp().then(setPub).catch(() => {});
    const t = setInterval(refresh, 5000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  if (!a) return null;
  const copy = (addr: string) => {
    void navigator.clipboard?.writeText(addr);
    onCopy?.("Address copied");
  };
  const share = async () => {
    setBusy(true);
    try {
      setTunnel(await api.tunnelStart(key, port, canPost ? postToCommunity : false));
      onCopy?.("Sharing to the internet — anyone can join with the address");
    } catch (e) {
      onCopy?.(`${e}`);
    } finally {
      setBusy(false);
    }
  };
  const unshare = async () => {
    setBusy(true);
    try {
      await api.tunnelStop(key);
      setTunnel(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="surface" style={{ padding: "12px 16px", marginTop: 10, display: "grid", gap: 10 }}>
      <div style={{ fontWeight: 700 }}>Friends connect to</div>

      {/* 1 · Internet tunnel (bore) — any launcher, no port forwarding. */}
      {internetTunnel &&
        (tunnel ? (
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 10 }}>
            <div>
              <div className="row" style={{ gap: 6, alignItems: "center" }}>
                <Pill tone="ok">Internet</Pill>
                <b style={{ fontVariantNumeric: "tabular-nums" }}>{tunnel.host}:{tunnel.port}</b>
              </div>
              <div className="muted" style={{ fontSize: 11.5 }}>Works from any launcher — no VPN, no port forwarding.</div>
            </div>
            <div className="row" style={{ gap: 6 }}>
              <button className="btn ghost" onClick={() => copy(`${tunnel.host}:${tunnel.port}`)}>
                <Icon.copy size={14} /> Copy
              </button>
              <button className="btn danger ghost" disabled={busy} onClick={unshare}>
                <Icon.stop size={13} /> Stop
              </button>
            </div>
          </div>
        ) : (
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 10 }}>
            <div>
              <div style={{ fontWeight: 600 }}>Share to the internet</div>
              <div className="muted" style={{ fontSize: 11.5 }}>One click — anyone on any launcher can join, no port forwarding.</div>
            </div>
            <button className="btn-play" style={{ padding: "8px 16px", fontSize: 13 }} disabled={busy} onClick={share}>
              <Icon.host size={14} /> {busy ? "Starting…" : "Share"}
            </button>
          </div>
        ))}
      {canPost && !tunnel && (
        <label className="row" style={{ gap: 8, cursor: "pointer", alignItems: "center", fontSize: 12.5 }}>
          <input type="checkbox" checked={postToCommunity} onChange={(e) => setPostToCommunity(e.target.checked)} />
          <span>
            <span style={{ fontWeight: 600 }}>Post to my Kawma community</span>
            <span className="muted"> · announce it on your Twitch and Discord; viewers ask to join with !server</span>
          </span>
        </label>
      )}

      {/* 2 · Kawma Net (private VPN). */}
      {a.aurora && (
        <AddrRow tag="Kawma Net" note="Private, encrypted — friends need Kawma Net too." addr={`${a.aurora}:${port}`} accent onCopy={() => copy(`${a.aurora}:${port}`)} />
      )}

      {/* 3 · Direct / port-forward. */}
      {a.lan && (
        <AddrRow tag="Same Wi-Fi" note="Same network only — no setup." addr={`${a.lan}:${port}`} onCopy={() => copy(`${a.lan}:${port}`)} />
      )}
      {pub && (
        <AddrRow tag="Port forward" note={`Forward port ${port} on your router, then friends use this.`} addr={`${pub}:${port}`} onCopy={() => copy(`${pub}:${port}`)} />
      )}

      {internetTunnel && (
        <p className="muted" style={{ margin: 0, fontSize: 11 }}>
          Tip: an internet-shared server is reachable by anyone with the address — keep the whitelist on.
        </p>
      )}
    </div>
  );
}

export interface SelectOption {
  value: string;
  label: string;
}

/** A styled dropdown that matches the launcher (custom popover, not native). */
/** An on/off switch. */
export function Switch({ value, onChange, label }: { value: boolean; onChange: (v: boolean) => void; label?: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      aria-label={label}
      className={`switch ${value ? "on" : ""}`}
      onClick={() => onChange(!value)}
    />
  );
}

export function Select({
  value,
  onChange,
  options,
  minWidth,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  options: SelectOption[];
  minWidth?: number;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const current = options.find((o) => o.value === value);
  return (
    <div className="select-wrap" style={{ position: "relative", minWidth }}>
      <button
        ref={btnRef}
        type="button"
        className="select-btn"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        style={{ minWidth }}
      >
        <span className="select-label">{current?.label ?? value}</span>
        <span className={`select-chev ${open ? "up" : ""}`}>
          <Icon.chevron size={14} />
        </span>
      </button>
      <Popover
        anchor={btnRef}
        open={open}
        onClose={() => setOpen(false)}
        placement="bottom-start"
        offset={8}
        matchWidth
        className="row-menu surface select-menu"
      >
        {options.map((o) => (
          <button
            key={o.value}
            type="button"
            className={`row-menu-item ${o.value === value ? "sel" : ""}`}
            onClick={() => {
              onChange(o.value);
              setOpen(false);
            }}
          >
            <span className="select-opt">{o.label}</span>
            {o.value === value && <Icon.check size={14} />}
          </button>
        ))}
      </Popover>
    </div>
  );
}
