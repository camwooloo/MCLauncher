import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

/** Where a popover sits relative to its anchor before it is kept on screen.
 *  - `bottom-start` / `bottom-end`: below, lined up with the anchor's left / right edge.
 *  - `left-start`: to the anchor's left, tops lined up (menus from the right rail). */
export type PopoverPlacement = "bottom-start" | "bottom-end" | "left-start";

/** Gap kept between a popover and the window edge. */
const EDGE = 8;

type Pos = { top: number; left: number; maxHeight: number; side: "top" | "bottom" | "left" | "right" };

/** Work out where a popover of `menu`'s natural size goes so it stays inside
 *  the window: it flips above the anchor when there's more room there, is
 *  clamped horizontally, and gets a max height so a tall menu scrolls inside. */
function place(anchor: DOMRect, menu: HTMLElement, placement: PopoverPlacement, offset: number): Pos {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  // Natural size, ignoring any max-height we set last time (content scrolls).
  const frameY = menu.offsetHeight - menu.clientHeight;
  const natH = menu.scrollHeight + frameY;
  const w = Math.min(menu.offsetWidth, vw - EDGE * 2);
  const clampX = (x: number) => Math.max(EDGE, Math.min(x, vw - w - EDGE));

  if (placement === "left-start") {
    let left = anchor.left - offset - w;
    let side: Pos["side"] = "left";
    if (left < EDGE && anchor.right + offset + w <= vw - EDGE) {
      left = anchor.right + offset;
      side = "right";
    }
    const maxHeight = vh - EDGE * 2;
    const top = Math.max(EDGE, Math.min(anchor.top, vh - EDGE - Math.min(natH, maxHeight)));
    return { top, left: clampX(left), maxHeight, side };
  }

  const below = vh - anchor.bottom - offset - EDGE;
  const above = anchor.top - offset - EDGE;
  const left = clampX(placement === "bottom-start" ? anchor.left : anchor.right - w);
  if (natH <= below || below >= above) {
    return { top: anchor.bottom + offset, left, maxHeight: Math.max(below, 0), side: "bottom" };
  }
  const maxHeight = Math.max(above, 0);
  return { top: anchor.top - offset - Math.min(natH, maxHeight), left, maxHeight, side: "top" };
}

/** A dropdown / menu rendered in a portal at fixed coordinates next to
 *  `anchor`, kept inside the window (flips up, clamps sideways, scrolls when
 *  taller than the window). Every menu in the launcher goes through this, so
 *  none can run off the bottom of the window or get trapped behind a sibling
 *  with its own stacking context.
 *
 *  Pass `onClose` to get a click-outside backdrop and Escape to close; hover
 *  menus leave it out and pass `onMouseEnter` / `onMouseLeave` instead. */
export function Popover({
  anchor,
  open,
  onClose,
  placement = "bottom-end",
  offset = 6,
  matchWidth = false,
  className = "",
  style,
  onMouseEnter,
  onMouseLeave,
  children,
}: {
  anchor: RefObject<HTMLElement>;
  open: boolean;
  onClose?: () => void;
  placement?: PopoverPlacement;
  offset?: number;
  /** Be at least as wide as the anchor (selects). */
  matchWidth?: boolean;
  className?: string;
  style?: CSSProperties;
  onMouseEnter?: () => void;
  onMouseLeave?: () => void;
  children: ReactNode;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<Pos | null>(null);
  const [minW, setMinW] = useState<number | undefined>(undefined);

  const update = useCallback(() => {
    const a = anchor.current;
    const m = menuRef.current;
    if (!a || !m) return;
    const r = a.getBoundingClientRect();
    if (matchWidth) setMinW((w) => (w === r.width ? w : r.width));
    const next = place(r, m, placement, offset);
    setPos((p) =>
      p && p.top === next.top && p.left === next.left && p.maxHeight === next.maxHeight && p.side === next.side ? p : next
    );
  }, [anchor, placement, offset, matchWidth]);

  useLayoutEffect(() => {
    if (!open) {
      setPos(null);
      return;
    }
    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    // Content can change while open (accounts load, a sign-in finishes).
    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(() => update()) : null;
    if (menuRef.current) ro?.observe(menuRef.current);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
      ro?.disconnect();
    };
  }, [open, update]);

  useEffect(() => {
    if (!open || !onClose) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  // Inside .app so the shell's `.app …` styles still apply to the menu.
  const host = document.querySelector(".app") ?? document.body;
  return createPortal(
    <>
      {onClose && <div className="popover-backdrop" onClick={onClose} />}
      <div
        ref={menuRef}
        className={`popover ${className}`}
        data-side={pos?.side ?? "bottom"}
        onMouseEnter={onMouseEnter}
        onMouseLeave={onMouseLeave}
        style={{
          ...style,
          position: "fixed",
          top: pos?.top ?? 0,
          left: pos?.left ?? 0,
          right: "auto",
          bottom: "auto",
          maxHeight: pos?.maxHeight,
          maxWidth: `calc(100vw - ${EDGE * 2}px)`,
          minWidth: minW !== undefined ? Math.max(minW, Number(style?.minWidth) || 0) : style?.minWidth,
          overflowY: "auto",
          overscrollBehavior: "contain",
          visibility: pos ? undefined : "hidden",
        }}
      >
        {children}
      </div>
    </>,
    host
  );
}
