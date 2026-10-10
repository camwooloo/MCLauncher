import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";

import { useLauncher } from "../store";
import { starsBest, starsSubmit, type StarsResult } from "../lib/api";
import { KawmaLogo } from "./KawmaLogo";

/* Star catcher: the rail logo's easter egg. Hold the logo for HOLD_MS and a
   20-second game opens over the stage: steer the Kawma planet along the
   bottom and catch falling stars (gold ones are worth 3). The best score is
   saved on kawma.app when signed in and the feature is released for the
   account; otherwise it stays in localStorage. */

export const HOLD_MS = 900;
const ROUND_S = 20;
const LOCAL_KEY = "kawma:stars-best";
/** Catch this many in one round to unlock Starfield in Kawma Studio (server side). */
export const UNLOCK_AT = 30;

export function readLocalBest(): number {
  try {
    const n = Number(localStorage.getItem(LOCAL_KEY));
    return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
  } catch {
    return 0;
  }
}
function writeLocalBest(n: number) {
  try {
    if (n > readLocalBest()) localStorage.setItem(LOCAL_KEY, String(n));
  } catch {
    /* storage unavailable: the best just isn't kept */
  }
}

// --- Hold to play ------------------------------------------------------------

/** Click-or-hold for a button. A press shorter than HOLD_MS (or one that
 *  drags away) is an ordinary `onTap`; holding the pointer, Space or Enter for
 *  HOLD_MS calls `onHold` instead and swallows the click. Never starts inside
 *  a window drag region. */
export function useHoldToPlay(onTap: () => void, onHold: () => void) {
  const [holding, setHolding] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const fired = useRef(false);
  const press = useRef<{ id: number; x: number; y: number } | null>(null);
  const key = useRef<string | null>(null);
  const cb = useRef({ onTap, onHold });
  cb.current = { onTap, onHold };

  const stop = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    setHolding(false);
  }, []);
  const begin = useCallback(() => {
    fired.current = false;
    window.clearTimeout(timer.current);
    setHolding(true);
    timer.current = window.setTimeout(() => {
      timer.current = undefined;
      fired.current = true;
      setHolding(false);
      cb.current.onHold();
    }, HOLD_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const props = {
    onPointerDown(e: PointerEvent<HTMLElement>) {
      if (e.pointerType === "mouse" && e.button !== 0) return;
      if ((e.target as Element).closest("[data-tauri-drag-region]")) return;
      press.current = { id: e.pointerId, x: e.clientX, y: e.clientY };
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        /* capture is a nicety */
      }
      begin();
    },
    onPointerMove(e: PointerEvent<HTMLElement>) {
      const p = press.current;
      if (!p || p.id !== e.pointerId || !timer.current) return;
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) > 12) stop();
    },
    onPointerUp(e: PointerEvent<HTMLElement>) {
      if (press.current?.id !== e.pointerId) return;
      press.current = null;
      if (timer.current) stop(); // released early: the click that follows is a tap
    },
    onPointerCancel() {
      press.current = null;
      stop();
    },
    onContextMenu(e: MouseEvent<HTMLElement>) {
      // A long touch shouldn't open the context menu over the game.
      if (timer.current || fired.current) e.preventDefault();
    },
    onClick() {
      if (fired.current) {
        fired.current = false;
        return;
      }
      cb.current.onTap();
    },
    onKeyDown(e: KeyboardEvent<HTMLElement>) {
      if (e.key !== " " && e.key !== "Enter") return;
      e.preventDefault(); // we decide between tap and hold on key up
      if (e.repeat || key.current) return;
      key.current = e.key;
      begin();
    },
    onKeyUp(e: KeyboardEvent<HTMLElement>) {
      if (e.key !== key.current) return;
      e.preventDefault();
      key.current = null;
      if (timer.current) {
        stop();
        cb.current.onTap();
      } else {
        fired.current = false;
      }
    },
    onBlur() {
      key.current = null;
      if (timer.current) stop();
    },
  };
  return { holding, props };
}

/** The ring that fills round the logo while it's held. */
export function HoldRing({ holding }: { holding: boolean }) {
  return (
    <svg className={`hold-ring ${holding ? "on" : ""}`} viewBox="0 0 50 50" aria-hidden>
      <circle cx="25" cy="25" r="23" pathLength={100} />
    </svg>
  );
}

// --- The game -------------------------------------------------------------------

interface Star {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  gold: boolean;
  rot: number;
  spin: number;
  wob: number;
}
interface Pop {
  x: number;
  y: number;
  t: number;
  gold: boolean;
}

interface Result {
  score: number;
  best: number;
  newBest: boolean;
  unlocked: boolean;
  justUnlocked: boolean;
  /** kawma.app keeps the score (signed in and released). */
  saved: boolean;
  saving: boolean;
}

const rand = (a: number, b: number) => a + Math.random() * (b - a);

function sparkle(ctx: CanvasRenderingContext2D, r: number) {
  // The four-point sparkle from the logo.
  const i = r * 0.3;
  ctx.beginPath();
  ctx.moveTo(0, -r);
  ctx.lineTo(i * 0.5, -i * 0.5);
  ctx.lineTo(r, 0);
  ctx.lineTo(i * 0.5, i * 0.5);
  ctx.lineTo(0, r);
  ctx.lineTo(-i * 0.5, i * 0.5);
  ctx.lineTo(-r, 0);
  ctx.lineTo(-i * 0.5, -i * 0.5);
  ctx.closePath();
}

export function StarCatcher({ onClose }: { onClose: () => void }) {
  const { kawma } = useLauncher();
  const signedIn = !!kawma;
  const [round, setRound] = useState(0);
  const [result, setResult] = useState<Result | null>(null);
  const [best, setBest] = useState(readLocalBest);
  const [server, setServer] = useState<StarsResult | null>(null);
  const [size, setSize] = useState(72);

  const rootRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const playerRef = useRef<HTMLDivElement>(null);
  const scoreRef = useRef<HTMLSpanElement>(null);
  const timeRef = useRef<HTMLSpanElement>(null);
  const againRef = useRef<HTMLButtonElement>(null);
  const bestRef = useRef(best);
  bestRef.current = best;
  const serverRef = useRef(server);
  serverRef.current = server;
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  // The saved best, when kawma.app keeps it for this account.
  useEffect(() => {
    if (!signedIn) return;
    let live = true;
    starsBest()
      .then((r) => {
        if (!live || !r) return;
        setServer(r);
        setBest(r.best);
      })
      .catch(() => {
        /* keep the local best */
      });
    return () => {
      live = false;
    };
  }, [signedIn]);

  // Escape closes at any time; it never reaches the app underneath.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      closeRef.current();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const finish = useCallback(
    (score: number) => {
      const prev = bestRef.current;
      const top = Math.max(prev, score);
      writeLocalBest(score);
      setBest(top);
      const known = serverRef.current;
      setResult({
        score,
        best: top,
        newBest: score > prev && score > 0,
        unlocked: known?.unlocked ?? false,
        justUnlocked: false,
        saved: !!known,
        saving: signedIn,
      });
      if (!signedIn) return;
      starsSubmit(score)
        .then((r) => {
          if (!r) {
            setResult((x) => x && { ...x, saving: false, saved: false });
            return;
          }
          setServer({ best: r.best, unlocked: r.unlocked });
          setBest(Math.max(r.best, score));
          setResult(
            (x) =>
              x && {
                ...x,
                best: Math.max(r.best, score),
                newBest: r.newBest ?? x.newBest,
                unlocked: r.unlocked,
                justUnlocked: !!r.justUnlocked,
                saved: true,
                saving: false,
              },
          );
        })
        .catch(() => setResult((x) => x && { ...x, saving: false }));
    },
    [signedIn],
  );
  const finishRef = useRef(finish);
  finishRef.current = finish;

  // One round: canvas + rAF, steering by pointer, arrows and A/D.
  useEffect(() => {
    const root = rootRef.current!;
    const canvas = canvasRef.current!;
    const ctx = canvas.getContext("2d")!;
    let W = 0;
    let H = 0;
    let P = 72;
    const fit = () => {
      const r = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      W = r.width;
      H = r.height;
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      P = Math.round(Math.max(56, Math.min(92, H * 0.15)));
      setSize(P);
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(canvas);

    let x = W / 2;
    let vx = 0;
    let target: number | null = null;
    const keys = { left: false, right: false };
    let stars: Star[] = [];
    let pops: Pop[] = [];
    let score = 0;
    let elapsed = 0;
    let spawn = 0.3;
    let shownTime = -1;
    let over = false;
    let raf = 0;
    let last: number | null = null;
    if (scoreRef.current) scoreRef.current.textContent = "0";

    const onMove = (e: globalThis.PointerEvent) => {
      const r = canvas.getBoundingClientRect();
      target = e.clientX - r.left;
    };
    const setKey = (e: globalThis.KeyboardEvent, down: boolean) => {
      const k = e.key.toLowerCase();
      if (k === "arrowleft" || k === "a") keys.left = down;
      else if (k === "arrowright" || k === "d") keys.right = down;
      else return;
      e.preventDefault();
      if (down) target = null;
    };
    const onDown = (e: globalThis.KeyboardEvent) => setKey(e, true);
    const onUp = (e: globalThis.KeyboardEvent) => setKey(e, false);
    root.addEventListener("pointermove", onMove);
    root.addEventListener("pointerdown", onMove);
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);

    const playerY = () => H - P * 0.62;

    const step = (dt: number) => {
      elapsed += dt;
      const p = Math.min(1, elapsed / ROUND_S);
      const scale = Math.max(0.8, H / 560);

      // Steer: keys move at a steady pace, the pointer pulls smoothly.
      const prevX = x;
      const dir = (keys.right ? 1 : 0) - (keys.left ? 1 : 0);
      if (dir) x += dir * 640 * scale * dt;
      else if (target !== null) x += (target - x) * Math.min(1, dt * 14);
      x = Math.max(P * 0.4, Math.min(W - P * 0.4, x));
      vx = vx * 0.8 + ((x - prevX) / Math.max(dt, 1e-3)) * 0.2;

      // Spawn: faster and quicker as the round goes on.
      spawn -= dt;
      if (spawn <= 0) {
        spawn = rand(0.62, 0.9) * (1 - p * 0.6);
        const gold = Math.random() < 0.08;
        stars.push({
          x: rand(24, W - 24),
          y: -20,
          vx: rand(-28, 28),
          vy: rand(130, 230) * (1 + p * 1.3) * scale,
          r: gold ? 13 : rand(8, 11),
          gold,
          rot: rand(0, Math.PI),
          spin: rand(-1.4, 1.4),
          wob: rand(0, Math.PI * 2),
        });
      }

      const py = playerY();
      const kept: Star[] = [];
      for (const s of stars) {
        s.y += s.vy * dt;
        s.x += (s.vx + Math.sin(elapsed * 2.2 + s.wob) * 16) * dt;
        s.rot += s.spin * dt;
        const dx = s.x - x;
        const dy = s.y - py;
        if (Math.abs(dx) < P * 0.44 + s.r * 0.5 && dy > -P * 0.38 && dy < P * 0.22) {
          score += s.gold ? 3 : 1;
          pops.push({ x: s.x, y: s.y, t: 0, gold: s.gold });
          if (scoreRef.current) scoreRef.current.textContent = String(score);
          continue;
        }
        if (s.y < H + 24) kept.push(s);
      }
      stars = kept;
      pops = pops.filter((q) => (q.t += dt) < 0.7);

      const left = Math.max(0, Math.ceil(ROUND_S - elapsed));
      if (left !== shownTime && timeRef.current) {
        shownTime = left;
        timeRef.current.textContent = `${left}s`;
      }
      if (elapsed >= ROUND_S) over = true;
    };

    const draw = () => {
      ctx.clearRect(0, 0, W, H);
      for (const s of stars) {
        ctx.save();
        ctx.translate(s.x, s.y);
        ctx.rotate(s.rot);
        ctx.shadowColor = s.gold ? "rgba(255, 196, 70, 0.9)" : "rgba(255, 236, 210, 0.7)";
        ctx.shadowBlur = s.gold ? 16 : 10;
        ctx.fillStyle = s.gold ? "#ffc94a" : "#fff0d8";
        sparkle(ctx, s.r * 1.5);
        ctx.fill();
        ctx.restore();
      }
      ctx.shadowBlur = 0;
      for (const q of pops) {
        const k = q.t / 0.7;
        const a = 1 - k;
        // Sparkle pop: a soft ring and eight motes drifting out.
        ctx.strokeStyle = q.gold ? `rgba(255, 201, 74, ${a * 0.8})` : `rgba(255, 240, 216, ${a * 0.7})`;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(q.x, q.y, 6 + k * 26, 0, Math.PI * 2);
        ctx.stroke();
        ctx.fillStyle = q.gold ? `rgba(255, 201, 74, ${a})` : `rgba(255, 240, 216, ${a})`;
        for (let i = 0; i < 8; i++) {
          const ang = (i / 8) * Math.PI * 2;
          const d = 8 + k * 30;
          ctx.beginPath();
          ctx.arc(q.x + Math.cos(ang) * d, q.y + Math.sin(ang) * d, 2.2 * a + 0.4, 0, Math.PI * 2);
          ctx.fill();
        }
        ctx.font = "700 16px Geist, system-ui, sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(q.gold ? "+3" : "+1", q.x, q.y - 18 - k * 28);
      }
      const el = playerRef.current;
      if (el) {
        const tilt = Math.max(-14, Math.min(14, vx * 0.02));
        el.style.transform = `translate(${x - P / 2}px, ${playerY() - P / 2}px) rotate(${tilt}deg)`;
      }
    };

    const frame = (now: number) => {
      raf = 0;
      if (last !== null) {
        // Real time drives the clock; physics runs in small steps so a slow
        // frame never lets a star tunnel through the planet.
        let dt = Math.min(0.25, (now - last) / 1000);
        while (dt > 0 && !over) {
          const h = Math.min(dt, 1 / 60);
          step(h);
          dt -= h;
        }
      }
      last = now;
      draw();
      if (over) {
        finishRef.current(score);
        return;
      }
      raf = requestAnimationFrame(frame);
    };
    const run = () => {
      if (!raf && !over && document.visibilityState !== "hidden") {
        last = null;
        raf = requestAnimationFrame(frame);
      }
    };
    // Pause while the window is hidden; the clock stops with it.
    const onVis = () => {
      if (document.visibilityState === "hidden") {
        cancelAnimationFrame(raf);
        raf = 0;
      } else run();
    };
    document.addEventListener("visibilitychange", onVis);
    run();

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      document.removeEventListener("visibilitychange", onVis);
      root.removeEventListener("pointermove", onMove);
      root.removeEventListener("pointerdown", onMove);
      window.removeEventListener("keydown", onDown);
      window.removeEventListener("keyup", onUp);
    };
  }, [round]);

  useEffect(() => {
    if (result) againRef.current?.focus();
    else rootRef.current?.focus();
  }, [result]);

  const again = () => {
    setResult(null);
    setRound((n) => n + 1);
  };

  return (
    <div className="stars" ref={rootRef} tabIndex={-1} role="dialog" aria-modal="true" aria-label="Star catcher">
      <div className="stars-hud">
        <span className="stars-chip">
          <b ref={scoreRef}>0</b> stars
        </span>
        <span className="stars-chip">
          <b ref={timeRef}>{ROUND_S}s</b> left
        </span>
        <span className="stars-chip dim">
          Best <b>{best}</b>
        </span>
        <span className="stars-spacer" />
        <button className="stars-x" onClick={onClose} title="Close (Esc)" aria-label="Close Star catcher">
          ×
        </button>
      </div>
      <canvas ref={canvasRef} className="stars-canvas" aria-hidden />
      <div className="stars-player" ref={playerRef} style={{ width: size, height: size }} aria-hidden>
        <KawmaLogo size={size} />
      </div>
      {!result && (
        <div className="stars-intro" key={round} aria-hidden>
          Catch the falling stars
          <small>Mouse, ← → or A / D · gold stars are worth 3</small>
        </div>
      )}
      {result && (
        <div className="stars-end">
          <div className="stars-card" aria-live="polite">
            <div className="stars-kicker">Star catcher</div>
            {result.newBest && <div className="stars-newbest">New best!</div>}
            <div className="stars-score">{result.score}</div>
            <div className="stars-sub">
              {result.score === 1 ? "star" : "stars"} caught · best {result.best}
              {result.saving && " · saving…"}
            </div>
            {result.justUnlocked ? (
              <div className="stars-unlock">✦ Starfield unlocked in Kawma Studio</div>
            ) : result.unlocked ? (
              <div className="stars-badge">✦ Starfield unlocked</div>
            ) : result.saved ? (
              <div className="stars-hint">Catch {UNLOCK_AT} to unlock something in Kawma Studio</div>
            ) : !signedIn ? (
              <div className="stars-hint">Sign in to Kawma to save your score</div>
            ) : null}
            <div className="stars-actions">
              <button className="btn" ref={againRef} onClick={again}>
                Play again
              </button>
              <button className="btn ghost" onClick={onClose}>
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
