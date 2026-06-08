import { useEffect, useState, type ReactNode } from "react";

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
