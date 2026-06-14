import { useEffect, useRef, useState, type ReactNode } from "react";

import * as api from "../lib/api";

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
        <circle cx="12" cy="12" r="3.2" />
        <path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2" />
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

/** The Aurora logo: northern-lights ribbons over a night sky, with a star. */
export function AuroraMark({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-label="Aurora">
      <defs>
        <linearGradient id="am-sky" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#171034" />
          <stop offset="1" stopColor="#081226" />
        </linearGradient>
        <linearGradient id="am-r1" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="#b794f6" stopOpacity="0" />
          <stop offset="0.35" stopColor="#b794f6" />
          <stop offset="1" stopColor="#7c5cf6" stopOpacity="0.25" />
        </linearGradient>
        <linearGradient id="am-r2" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="#34d399" stopOpacity="0" />
          <stop offset="0.4" stopColor="#34d399" />
          <stop offset="1" stopColor="#22d3ee" stopOpacity="0.3" />
        </linearGradient>
      </defs>
      <rect x="2" y="2" width="60" height="60" rx="16" fill="url(#am-sky)" />
      <rect x="2.5" y="2.5" width="59" height="59" rx="15.5" fill="none" stroke="rgba(255,255,255,0.18)" strokeWidth="1" />
      {/* aurora ribbons */}
      <path d="M8 50 C 22 44, 28 28, 56 14" stroke="url(#am-r1)" strokeWidth="9" strokeLinecap="round" fill="none" opacity="0.95" />
      <path d="M8 56 C 26 52, 36 38, 58 26" stroke="url(#am-r2)" strokeWidth="6.5" strokeLinecap="round" fill="none" opacity="0.9" />
      <path d="M10 44 C 22 38, 30 24, 50 13" stroke="rgba(255,255,255,0.55)" strokeWidth="1.6" strokeLinecap="round" fill="none" />
      {/* star */}
      <path d="M48 42 l1.6 3.4 3.4 1.6 -3.4 1.6 -1.6 3.4 -1.6 -3.4 -3.4 -1.6 3.4 -1.6 z" fill="#fff" opacity="0.9" />
      <circle cx="16" cy="14" r="1.3" fill="#fff" opacity="0.7" />
      <circle cx="26" cy="10" r="0.9" fill="#fff" opacity="0.5" />
    </svg>
  );
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

/** A small, dependency-free Markdown renderer — enough for GitHub release notes
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
 *  the Aurora Net IP (preferred, no port forwarding) and/or the LAN IP. Never
 *  shows localhost, which only works for the host themselves. */
export function HostAddress({ port, onCopy }: { port: number; onCopy?: (msg: string) => void }) {
  const [a, setA] = useState<api.HostAddresses | null>(null);
  useEffect(() => {
    api.hostAddresses().then(setA).catch(() => {});
  }, []);
  if (!a) return null;
  const copy = (addr: string) => {
    void navigator.clipboard?.writeText(addr);
    onCopy?.("Address copied");
  };
  const Row = ({ ip, tag, note }: { ip: string; tag?: string; note: string }) => (
    <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 10 }}>
      <div>
        <b style={{ fontVariantNumeric: "tabular-nums" }}>{ip}:{port}</b>{" "}
        {tag && <Pill tone="ok">{tag}</Pill>} <span className="muted" style={{ fontSize: 12 }}>· {note}</span>
      </div>
      <button className="btn ghost" onClick={() => copy(`${ip}:${port}`)}>
        <Icon.copy size={14} /> Copy
      </button>
    </div>
  );
  return (
    <div className="surface" style={{ padding: "12px 16px", marginTop: 10, display: "grid", gap: 8 }}>
      <div style={{ fontWeight: 700 }}>Friends connect to</div>
      {a.aurora && <Row ip={a.aurora} tag="Aurora Net" note="works anywhere, no port forwarding" />}
      {a.lan && <Row ip={a.lan} note="same Wi-Fi / network" />}
      {!a.aurora && !a.lan && (
        <p className="muted" style={{ margin: 0 }}>
          Couldn't detect your IP. Open <b>Aurora Net</b> and connect, then friends can reach you with no setup.
        </p>
      )}
      {!a.aurora && a.lan && (
        <p className="muted" style={{ margin: 0, fontSize: 12 }}>
          That's a local address. For friends elsewhere, open <b>Aurora Net</b> (no port forwarding) or forward this port.
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
  const current = options.find((o) => o.value === value);
  return (
    <div className="select-wrap" style={{ position: "relative", minWidth }}>
      <button
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
      {open && (
        <>
          <div onClick={() => setOpen(false)} style={{ position: "fixed", inset: 0, zIndex: 60 }} />
          <div
            className="row-menu surface select-menu"
            style={{ position: "absolute", left: 0, minWidth: "100%", top: "calc(100% + 6px)", zIndex: 61, maxHeight: 300, overflowY: "auto" }}
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
                {o.label}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
