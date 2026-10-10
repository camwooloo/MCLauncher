import { useEffect, useLayoutEffect, useRef } from "react";

/* One selection marker per rail that slides to the selected item instead of
   the highlight jumping. Items opt in with `data-rail-item` and carry
   `.active` when selected; the box (the returned `box` ref) must be
   positioned. The marker is drawn by CSS (`.rail-ind`), sized and moved here. */

/** Offset of `el` inside `box`, ignoring transforms (hover/press scaling). */
function offsetWithin(el: HTMLElement, box: HTMLElement) {
  let x = 0;
  let y = 0;
  let n: HTMLElement | null = el;
  while (n && n !== box) {
    x += n.offsetLeft;
    y += n.offsetTop;
    n = n.offsetParent as HTMLElement | null;
  }
  return n === box ? { x, y } : null;
}

export function useRailIndicator<B extends HTMLElement>() {
  const box = useRef<B>(null);
  const ind = useRef<HTMLSpanElement>(null);
  const last = useRef("");
  const lastItem = useRef<{ el: HTMLElement; g: string } | null>(null);
  const shown = useRef(false);

  const place = useRef<(instant?: boolean) => void>(() => {});
  place.current = (instant = false) => {
    const b = box.current;
    const m = ind.current;
    if (!b || !m) return;
    const item = b.querySelector<HTMLElement>("[data-rail-item].active");
    const pos = item && offsetWithin(item, b);
    if (!item || !pos) {
      if (shown.current) m.classList.remove("on");
      shown.current = false;
      last.current = "";
      return;
    }
    // The item's accent (game dots carry one); read only when the item changes.
    if (lastItem.current?.el !== item) lastItem.current = { el: item, g: getComputedStyle(item).getPropertyValue("--g").trim() };
    const g = lastItem.current.g;
    const key = `${pos.x},${pos.y},${item.offsetWidth},${item.offsetHeight},${g}`;
    if (shown.current && key === last.current) return;
    last.current = key;
    // Appearing (or the rail resizing): put it in place without a slide; a new
    // marker then fades in.
    const jump = instant || !shown.current;
    if (jump) m.style.transition = "none";
    m.style.width = `${item.offsetWidth}px`;
    m.style.height = `${item.offsetHeight}px`;
    m.style.transform = `translate3d(${pos.x}px, ${pos.y}px, 0)`;
    if (g) m.style.setProperty("--g", g);
    if (jump) {
      void m.offsetWidth;
      m.style.transition = "";
    }
    m.classList.add("on");
    shown.current = true;
  };

  // Every render: cheap when nothing moved, and catches any selection change.
  useLayoutEffect(() => place.current());

  // The rail can change size (narrow windows); follow it without a slide.
  useEffect(() => {
    const b = box.current;
    if (!b || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      last.current = "";
      place.current(true);
    });
    ro.observe(b);
    return () => ro.disconnect();
  }, []);

  return { box, ind };
}
