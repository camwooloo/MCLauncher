import { useState, type ReactNode } from "react";

import { useLauncher } from "../store";
import { Icon } from "./ui";

/* ------------------------------ Slide art ------------------------------ */
/* Each visual is pure CSS/SVG so it animates instantly with no assets. */

function WelcomeArt() {
  return (
    <div className="onb-art">
      <div className="onb-aurora" />
      <div className="onb-logo">
        <span className="onb-logo-a">Aurora</span>
      </div>
    </div>
  );
}

const GAMES = [
  { key: "minecraft", label: "Minecraft", rgb: "108, 198, 116" },
  { key: "skyrim", label: "Skyrim", rgb: "150, 170, 200" },
  { key: "eldenring", label: "Elden Ring", rgb: "212, 175, 95" },
  { key: "cyberpunk", label: "Cyberpunk", rgb: "245, 224, 80" },
];

function GamesArt() {
  return (
    <div className="onb-art">
      <div className="onb-games">
        {GAMES.map((g, i) => (
          <div
            key={g.key}
            className="onb-game"
            style={{ ["--g" as string]: g.rgb, animationDelay: `${i * 0.12}s` }}
          >
            <div className="onb-game-dot" />
            <span>{g.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function NetArt() {
  return (
    <div className="onb-art">
      <div className="onb-net">
        <div className="onb-node host">
          <Icon.host size={22} />
          <span>You</span>
        </div>
        <div className="onb-link">
          <span className="onb-pulse" />
        </div>
        <div className="onb-node friend" style={{ animationDelay: "0.15s" }}>
          <Icon.user size={22} />
          <span>Friends</span>
        </div>
      </div>
      <div className="onb-shield">
        <Icon.check size={14} /> No port forwarding
      </div>
    </div>
  );
}

function ThemeArt() {
  const swatches = ["mesh", "grid", "stars", "waves", "glow", "dots"];
  return (
    <div className="onb-art">
      <div className="onb-swatches">
        {swatches.map((s, i) => (
          <div key={s} className={`onb-swatch sw-${s}`} style={{ animationDelay: `${i * 0.08}s` }} />
        ))}
      </div>
    </div>
  );
}

type Slide = { art: ReactNode; title: string; line: string };

const SLIDES: Slide[] = [
  { art: <WelcomeArt />, title: "Welcome to Aurora", line: "Play, mod and host — beautifully, together." },
  { art: <GamesArt />, title: "Every game, one launcher", line: "Minecraft, Skyrim, Elden Ring & Cyberpunk." },
  { art: <NetArt />, title: "Play together, instantly", line: "Host a server, share a link — friends just join." },
  { art: <ThemeArt />, title: "Make it yours", line: "Themes, backgrounds and layouts to match your vibe." },
];

/** First-run, animated, low-text welcome with a Skip on every step. */
export function Onboarding() {
  const { settings, settingsLoaded, persistSettings } = useLauncher();
  const [i, setI] = useState(0);
  const [leaving, setLeaving] = useState(false);

  if (!settingsLoaded || settings.onboarded) return null;

  const finish = () => {
    setLeaving(true);
    setTimeout(() => persistSettings({ onboarded: true }), 320);
  };

  const last = i === SLIDES.length - 1;
  const slide = SLIDES[i];

  return (
    <div className={`onb-overlay ${leaving ? "leaving" : ""}`}>
      <button className="onb-skip" onClick={finish}>
        Skip
      </button>

      <div className="onb-card">
        <div className="onb-stage" key={i}>
          {slide.art}
          <div className="onb-text">
            <div className="onb-title">{slide.title}</div>
            <div className="onb-line">{slide.line}</div>
          </div>
        </div>

        <div className="onb-controls">
          <div className="onb-dots">
            {SLIDES.map((_, d) => (
              <button
                key={d}
                className={`onb-dot ${d === i ? "on" : ""}`}
                onClick={() => setI(d)}
                aria-label={`Slide ${d + 1}`}
              />
            ))}
          </div>
          <div className="row" style={{ gap: 8 }}>
            {i > 0 && (
              <button className="btn ghost" onClick={() => setI(i - 1)}>
                Back
              </button>
            )}
            <button className="btn-play" style={{ padding: "11px 24px", fontSize: 14 }} onClick={() => (last ? finish() : setI(i + 1))}>
              {last ? "Get started" : "Next"} <Icon.chevron size={15} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
