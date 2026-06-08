import { useRef } from "react";

// Minecraft § color codes → hex.
const COLORS: { code: string; hex: string; name: string }[] = [
  { code: "0", hex: "#000000", name: "Black" },
  { code: "1", hex: "#0000AA", name: "Dark Blue" },
  { code: "2", hex: "#00AA00", name: "Dark Green" },
  { code: "3", hex: "#00AAAA", name: "Dark Aqua" },
  { code: "4", hex: "#AA0000", name: "Dark Red" },
  { code: "5", hex: "#AA00AA", name: "Dark Purple" },
  { code: "6", hex: "#FFAA00", name: "Gold" },
  { code: "7", hex: "#AAAAAA", name: "Gray" },
  { code: "8", hex: "#555555", name: "Dark Gray" },
  { code: "9", hex: "#5555FF", name: "Blue" },
  { code: "a", hex: "#55FF55", name: "Green" },
  { code: "b", hex: "#55FFFF", name: "Aqua" },
  { code: "c", hex: "#FF5555", name: "Red" },
  { code: "d", hex: "#FF55FF", name: "Light Purple" },
  { code: "e", hex: "#FFFF55", name: "Yellow" },
  { code: "f", hex: "#FFFFFF", name: "White" },
];
const FORMATS: { code: string; label: string; title: string }[] = [
  { code: "l", label: "B", title: "Bold" },
  { code: "o", label: "I", title: "Italic" },
  { code: "n", label: "U", title: "Underline" },
  { code: "m", label: "S", title: "Strikethrough" },
  { code: "k", label: "?", title: "Obfuscated" },
  { code: "r", label: "⟲", title: "Reset" },
];
const HEX: Record<string, string> = Object.fromEntries(COLORS.map((c) => [c.code, c.hex]));

interface Seg {
  text: string;
  color?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
}

/** Parse a §-coded string into styled segments for preview. */
function parse(s: string): Seg[] {
  const segs: Seg[] = [];
  let cur: Seg = { text: "" };
  const push = () => {
    if (cur.text) segs.push({ ...cur });
  };
  for (let i = 0; i < s.length; i++) {
    if (s[i] === "§" && i + 1 < s.length) {
      const code = s[++i].toLowerCase();
      push();
      if (code in HEX) cur = { text: "", color: HEX[code] };
      else if (code === "l") cur = { ...cur, text: "", bold: true };
      else if (code === "o") cur = { ...cur, text: "", italic: true };
      else if (code === "n") cur = { ...cur, text: "", underline: true };
      else if (code === "m") cur = { ...cur, text: "", strike: true };
      else if (code === "r") cur = { text: "" };
      else cur = { ...cur, text: "" };
    } else {
      cur.text += s[i];
    }
  }
  push();
  return segs;
}

export function MotdEditor({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const ref = useRef<HTMLInputElement>(null);

  const insert = (code: string) => {
    const el = ref.current;
    const token = "§" + code;
    if (!el) {
      onChange(value + token);
      return;
    }
    const start = el.selectionStart ?? value.length;
    const end = el.selectionEnd ?? value.length;
    const next = value.slice(0, start) + token + value.slice(end);
    onChange(next);
    requestAnimationFrame(() => {
      el.focus();
      const pos = start + token.length;
      el.setSelectionRange(pos, pos);
    });
  };

  const segs = parse(value || "");

  return (
    <div className="col" style={{ gap: 10 }}>
      <input
        ref={ref}
        className="input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="A Minecraft Server"
        style={{ minWidth: 320, fontFamily: "ui-monospace, monospace" }}
      />
      <div className="row wrap" style={{ gap: 6 }}>
        {COLORS.map((c) => (
          <button
            key={c.code}
            title={c.name}
            onClick={() => insert(c.code)}
            style={{
              width: 22,
              height: 22,
              borderRadius: 6,
              background: c.hex,
              border: "1px solid rgba(255,255,255,0.25)",
              cursor: "pointer",
            }}
          />
        ))}
        <span style={{ width: 8 }} />
        {FORMATS.map((f) => (
          <button
            key={f.code}
            className="btn ghost"
            title={f.title}
            onClick={() => insert(f.code)}
            style={{
              padding: "2px 9px",
              fontWeight: f.code === "l" ? 800 : 600,
              fontStyle: f.code === "o" ? "italic" : "normal",
              textDecoration:
                f.code === "n" ? "underline" : f.code === "m" ? "line-through" : "none",
            }}
          >
            {f.label}
          </button>
        ))}
      </div>
      <div
        style={{
          padding: "10px 14px",
          borderRadius: 12,
          background: "#0e1016",
          border: "1px solid var(--stroke)",
          fontFamily: "ui-monospace, monospace",
          minHeight: 22,
          fontSize: 14,
        }}
      >
        {segs.length === 0 ? (
          <span style={{ color: "var(--text-mute)" }}>preview…</span>
        ) : (
          segs.map((s, i) => (
            <span
              key={i}
              style={{
                color: s.color ?? "#FFFFFF",
                fontWeight: s.bold ? 700 : 400,
                fontStyle: s.italic ? "italic" : "normal",
                textDecoration: `${s.underline ? "underline " : ""}${s.strike ? "line-through" : ""}`.trim() || "none",
              }}
            >
              {s.text}
            </span>
          ))
        )}
      </div>
    </div>
  );
}
