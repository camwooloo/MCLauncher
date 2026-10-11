import { useState } from "react";

import type { GameKey } from "../lib/types";
import { Icon } from "./ui";

/* Official game icons shipped with the launcher: drop `<game>.png|jpg|webp|svg`
   into src/assets/games and it is picked up at build time. */
export const BUNDLED_ART: Record<string, string> = Object.fromEntries(
  Object.entries(import.meta.glob("../assets/games/*.{png,jpg,jpeg,webp,svg}", { eager: true, import: "default" })).map(
    ([path, url]) => [path.replace(/^.*\/|\.[^.]+$/g, ""), url as string]
  )
);

const GLYPH: Record<GameKey, (p: { size?: number }) => JSX.Element> = {
  minecraft: Icon.minecraft,
  skyrim: Icon.skyrim,
  eldenring: Icon.elden,
  cyberpunk: Icon.cyberpunk,
  deadlock: Icon.deadlock,
};

/** The best icon we have for a game: bundled, else Steam's cached one. */
export const gameIconSrc = (game: GameKey, icons: Record<string, string>): string | undefined =>
  BUNDLED_ART[game] ?? icons[game];

/* A game's real icon: the one bundled with the app, else the icon Steam has
   cached on this PC, else our own glyph. */
export function GameArt({ game, icons, size }: { game: GameKey; icons: Record<string, string>; size: number }) {
  const [broken, setBroken] = useState(false);
  const src = gameIconSrc(game, icons);
  if (src && !broken) return <img className="game-art" src={src} alt="" width={size} height={size} onError={() => setBroken(true)} />;
  const G = GLYPH[game];
  return <G size={Math.round(size * 0.46)} />;
}
