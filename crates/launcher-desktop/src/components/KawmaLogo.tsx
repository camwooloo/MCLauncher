import { useEffect, useId, useState } from "react";

/** The rail's animated Kawma Launcher logo: the purple Kawma planet from
 *  `launcher.svg`, drawn inline so it can gently come alive. It floats, the
 *  eyes blink now and then, the sparkle twinkles and a glint orbits the ring.
 *
 *  All motion is CSS (`.kawma-logo` in deck.css), so it costs next to nothing.
 *  It holds still on the static look when the window is hidden, when the OS
 *  asks for reduced motion, or when the launcher's "Static (no motion)"
 *  background is chosen (`data-anim="off"`). `bounce` replays a short playful
 *  hop-and-spin, e.g. when the logo is clicked. */
export function KawmaLogo({ size = 38, bounce = 0 }: { size?: number; bounce?: number }) {
  const uid = useId().replace(/:/g, "");
  const id = (n: string) => `kl${uid}-${n}`;
  const [hidden, setHidden] = useState(() => typeof document !== "undefined" && document.visibilityState === "hidden");
  const [boing, setBoing] = useState(false);

  useEffect(() => {
    const onVis = () => setHidden(document.visibilityState === "hidden");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);

  // Restart the bounce each time `bounce` ticks up.
  useEffect(() => {
    if (!bounce) return;
    setBoing(false);
    const r = requestAnimationFrame(() => setBoing(true));
    return () => cancelAnimationFrame(r);
  }, [bounce]);

  const ring = (
    <ellipse cx="256" cy="256" rx="214" ry="66" fill="none" stroke={`url(#${id("ring")})`} strokeWidth="34" />
  );
  // A short bright dash that travels round the ring. Drawn twice, behind and
  // in front of the planet, so it passes behind it like the ring does.
  const glint = (
    <ellipse
      className="kl-glint"
      cx="256"
      cy="256"
      rx="214"
      ry="66"
      pathLength={1000}
      fill="none"
      stroke="#fffaf2"
      strokeWidth="12"
      strokeLinecap="round"
      strokeDasharray="46 954"
    />
  );

  return (
    <span
      className={`kawma-logo ${hidden ? "paused" : ""} ${boing ? "boing" : ""}`}
      style={{ width: size, height: size }}
      onAnimationEnd={(e) => {
        if (e.animationName === "kl-boing") setBoing(false);
      }}
    >
      <svg width={size} height={size} viewBox="0 0 512 512" role="img" aria-label="Kawma Launcher">
        <defs>
          <radialGradient id={id("body")} cx="0.36" cy="0.3" r="0.8">
            <stop offset="0" stopColor="#d4b0ff" />
            <stop offset="0.38" stopColor="#9a4dff" />
            <stop offset="0.78" stopColor="#6420e0" />
            <stop offset="1" stopColor="#4a14b4" />
          </radialGradient>
          <linearGradient id={id("ring")} x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stopColor="#ff7a45" />
            <stop offset="0.55" stopColor="#ffb36b" />
            <stop offset="1" stopColor="#ffe2b8" />
          </linearGradient>
          <filter id={id("glow")} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="6" result="b" />
            <feMerge>
              <feMergeNode in="b" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
          <clipPath id={id("front")}>
            <rect x="-200" y="256" width="912" height="400" />
          </clipPath>
        </defs>
        <g>
          <g transform="rotate(-20 256 256)">
            {ring}
            {glint}
          </g>
          <circle cx="256" cy="256" r="122" fill={`url(#${id("body")})`} />
          <circle cx="256" cy="256" r="122" fill="none" stroke="#ffd9a8" strokeOpacity="0.75" strokeWidth="5" />
          <ellipse cx="214" cy="196" rx="34" ry="18" transform="rotate(-30 214 196)" fill="#ffffff" opacity="0.28" />
          <g fill="#fff3e0" filter={`url(#${id("glow")})`}>
            <g className="kl-eyes">
              <rect x="214" y="222" width="28" height="66" rx="14" />
              <rect x="270" y="222" width="28" height="66" rx="14" />
            </g>
          </g>
          <g transform="rotate(-20 256 256)" clipPath={`url(#${id("front")})`}>
            {ring}
            {glint}
            <path
              d="M 120 306 Q 210 330 300 320"
              fill="none"
              stroke="#fff6ea"
              strokeOpacity="0.55"
              strokeWidth="10"
              strokeLinecap="round"
            />
          </g>
        </g>
        <g filter={`url(#${id("glow")})`}>
          <path
            className="kl-sparkle"
            d="M 372 64 L 382 92 L 410 102 L 382 112 L 372 140 L 362 112 L 334 102 L 362 92 Z"
            fill="#fff0d8"
          />
        </g>
      </svg>
    </span>
  );
}
