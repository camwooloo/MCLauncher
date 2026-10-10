import { useEffect, useMemo, useState } from "react";

import { useLauncher, loadServers } from "../store";
import * as api from "../lib/api";
import type { GameKey, GamesStatus } from "../lib/types";
import { Icon } from "./ui";
import { GameArt, gameIconSrc } from "./GameArt";

type IconFn = (p: { size?: number }) => JSX.Element;

/** What the player picked on a game's home page: the Minecraft instance and
 *  each game's launch mode. The hero's Play button uses it too. */
export type Choice = Partial<Record<"instance" | GameKey, string>>;

const CHOICE_KEY = "aurora:homeChoice";
export function loadChoice(): Choice {
  try {
    return JSON.parse(localStorage.getItem(CHOICE_KEY) || "{}");
  } catch {
    return {};
  }
}
export function saveChoice(c: Choice) {
  try {
    localStorage.setItem(CHOICE_KEY, JSON.stringify(c));
  } catch {
    /* ignore */
  }
}

/* Optional real artwork for the library cards: drop `<game>-<card>.jpg|png|webp`
   (e.g. `minecraft-mods.jpg`) into src/assets/art. Without one the card uses
   the game's own icon, blown up and blurred, as its backdrop. */
const CARD_ART: Record<string, string> = Object.fromEntries(
  Object.entries(import.meta.glob("../assets/art/*.{png,jpg,jpeg,webp}", { eager: true, import: "default" })).map(
    ([path, url]) => [path.replace(/^.*\/|\.[^.]+$/g, ""), url as string]
  )
);

interface LaunchMode {
  id: string;
  label: string;
  sub: string;
  ready: boolean;
}

/** Launch modes per game, in the order the hero falls back through them. */
export function launchModes(game: Exclude<GameKey, "minecraft">, g: GamesStatus | null): LaunchMode[] {
  switch (game) {
    case "skyrim": {
      const s = g?.skyrim;
      return [
        { id: "vanilla", label: "Vanilla", sub: "The official game", ready: !!s?.installed },
        { id: "skse", label: "SKSE", sub: "Script extender for mods", ready: !!s?.has_skse },
        { id: "together", label: "Skyrim Together", sub: "Co-op with friends", ready: !!s?.has_skyrim_together },
      ];
    }
    case "eldenring": {
      const e = g?.eldenRing;
      return [
        { id: "vanilla", label: "Official", sub: "EasyAntiCheat on", ready: !!e?.installed },
        { id: "seamless", label: "Seamless Co-op", sub: "Anti-cheat off", ready: !!e?.has_seamless_coop },
        { id: "modded", label: "Mod Engine 2", sub: "Anti-cheat off", ready: !!e?.has_mod_engine },
      ];
    }
    case "cyberpunk": {
      const c = g?.cyberpunk;
      return [
        { id: "vanilla", label: "Official", sub: "Through Steam", ready: !!c?.installed },
        { id: "skip-launcher", label: "Skip launcher", sub: "Straight into the game", ready: !!c?.installed },
        { id: "mp", label: "CyberpunkMP", sub: "Experimental co-op", ready: !!c?.has_mp },
      ];
    }
  }
}

/** The launch mode Play uses: the one picked, if it is ready, else Vanilla. */
export function pickedMode(game: Exclude<GameKey, "minecraft">, g: GamesStatus | null, choice: Choice): string {
  const modes = launchModes(game, g);
  return modes.find((m) => m.id === choice[game] && m.ready)?.id ?? "vanilla";
}

/* ------------------------------ helpers -------------------------------- */

function playtime(sec: number): string {
  if (sec < 60) return sec > 0 ? "<1m" : "0m";
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return h > 0 ? `${h} hr, ${m} min` : `${m} min`;
}
function ago(sec: number): string {
  if (!sec) return "";
  const d = Math.floor(Date.now() / 1000) - sec;
  if (d < 90) return "Just now";
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} hr ago`;
  if (d < 7 * 86400) return `${Math.floor(d / 86400)}d ago`;
  return `${Math.floor(d / (7 * 86400))}w ago`;
}
const recordKind = (game: GameKey) => (game === "minecraft" ? "instance" : game);

/* ------------------------------ pieces --------------------------------- */

function LibCard({
  game,
  slug,
  title,
  icon: CardIcon,
  art,
  text,
  action,
  onClick,
}: {
  game: GameKey;
  slug: string;
  title: string;
  icon: IconFn;
  art?: string;
  text: string;
  action: string;
  onClick: () => void;
}) {
  const photo = CARD_ART[`${game}-${slug}`];
  return (
    <div className="lib-card">
      <div className={`lib-art ${photo ? "photo" : ""}`} data-card={slug}>
        {photo ? <img src={photo} alt="" /> : art && <img className="lib-art-blur" src={art} alt="" />}
        <span className="lib-art-title">
          <span className="lib-art-ico">
            <CardIcon size={22} />
          </span>
          {title}
        </span>
      </div>
      <div className="lib-body">
        <div className="lib-text">{text}</div>
        <button className="pill-btn" onClick={onClick}>
          {action}
        </button>
      </div>
    </div>
  );
}

interface SessionRow {
  key: string;
  name: string;
  sub: string;
  on: boolean;
  icon: React.ReactNode;
  onClick?: () => void;
  selected?: boolean;
}

function SessionCard({
  title,
  count,
  rows,
  empty,
  foot,
  pickLabel,
  pickValue,
  options,
  onPick,
}: {
  title: string;
  count: string;
  rows: SessionRow[];
  empty: React.ReactNode;
  foot: string;
  pickLabel: string;
  pickValue: string;
  options: { value: string; label: string; disabled?: boolean }[];
  onPick: (v: string) => void;
}) {
  return (
    <div className="sess-card">
      <div className="sess-head">
        <span>{title}</span>
        <span>{count}</span>
      </div>
      <div className="sess-rows">
        {rows.length === 0 && <div className="sess-empty">{empty}</div>}
        {rows.map((r) => (
          <button
            key={r.key}
            className={`sess-row ${r.selected ? "selected" : ""}`}
            onClick={r.onClick}
            disabled={!r.onClick}
          >
            <span className="sess-ico">{r.icon}</span>
            <span className="sess-txt">
              <span className="sess-name">{r.name}</span>
              <span className="sess-sub">{r.sub}</span>
            </span>
            <span className={`sess-dot ${r.on ? "on" : ""}`} />
          </button>
        ))}
      </div>
      <div className="sess-foot">{foot}</div>
      <label className="sess-pick">
        <span className="sess-pick-label">{pickLabel}</span>
        <span className="sess-pick-row">
          <Icon.chevron size={15} />
          <select value={pickValue} onChange={(e) => onPick(e.target.value)}>
            {options.map((o) => (
              <option key={o.value} value={o.value} disabled={o.disabled}>
                {o.label}
              </option>
            ))}
          </select>
        </span>
      </label>
    </div>
  );
}

/** Seven columns, oldest on the left, today highlighted. */
function WeekActivity({ game, icons, records }: { game: GameKey; icons: Record<string, string>; records: api.PlayRecord[] }) {
  const today = Math.floor(Date.now() / 86_400_000);
  const days = Array.from({ length: 7 }, (_, i) => today - 6 + i);
  const secs = days.map((d) => records.reduce((a, r) => a + (r.days?.[String(d)] ?? 0), 0));
  const max = Math.max(...secs, 1);
  const total = secs.reduce((a, b) => a + b, 0);
  return (
    <div className="dash-card week">
      <span className="week-ico">
        <GameArt game={game} icons={icons} size={46} />
      </span>
      <div className="week-cols">
        {days.map((d, i) => (
          <div key={d} className="week-col" title={secs[i] ? playtime(secs[i]) : "Not played"}>
            <div className="week-track">
              <div
                className={`week-bar ${i === 6 ? "today" : ""}`}
                style={{ height: secs[i] ? `${Math.max(10, (secs[i] / max) * 100)}%` : 0 }}
              />
            </div>
            <span className="week-day">{"SMTWTFS"[new Date(d * 86_400_000).getUTCDay()]}</span>
          </div>
        ))}
      </div>
      <div className="week-total">{total ? `${playtime(total)} this week` : "Not played this week"}</div>
    </div>
  );
}

interface Tile {
  key: string;
  label: string;
  sub: string;
  ready: boolean;
  selected?: boolean;
  art?: string | null;
  onClick: () => void;
}

function TileShelf({ tiles, empty }: { tiles: Tile[]; empty: string }) {
  return (
    <div className="dash-card shelf">
      {tiles.length === 0 && <div className="sess-empty">{empty}</div>}
      {tiles.map((t) => (
        <button key={t.key} className={`shelf-tile ${t.ready ? "" : "missing"} ${t.selected ? "selected" : ""}`} onClick={t.onClick} title={t.label}>
          <span className="shelf-art">
            {t.art && <img src={t.art} alt="" />}
            <span className="shelf-name">{t.label}</span>
          </span>
          <span className="shelf-sub">{t.sub}</span>
        </button>
      ))}
    </div>
  );
}

/** A play record's own icon (e.g. a modpack's), falling back to the game's. */
function RecordIcon({ src, game, icons }: { src: string | null; game: GameKey; icons: Record<string, string> }) {
  const [broken, setBroken] = useState(false);
  if (src && !broken) return <img src={src} alt="" onError={() => setBroken(true)} />;
  return <GameArt game={game} icons={icons} size={40} />;
}

function RecentList({ game, icons, records }: { game: GameKey; icons: Record<string, string>; records: api.PlayRecord[] }) {
  return (
    <div className="dash-card recent-list">
      {records.length === 0 && <div className="sess-empty">Nothing played yet. Hit Play and it shows up here.</div>}
      {records.slice(0, 3).map((r) => (
        <div key={r.key} className="act-row">
          <span className="act-ico">
            <RecordIcon src={r.icon} game={game} icons={icons} />
          </span>
          <span className="sess-txt">
            <span className="sess-name">{r.name}</span>
            <span className="sess-sub">
              {r.launches} launch{r.launches === 1 ? "" : "es"} · {playtime(r.totalSeconds)} played
            </span>
          </span>
          <span className="act-when">{ago(r.lastPlayed)}</span>
        </div>
      ))}
    </div>
  );
}

/* ------------------------------ the page -------------------------------- */

/** A game's home page: library cards, the current session, then activity,
 *  what's installed and recent play. Laid out like a dashboard. */
export function GameHome({
  game,
  icons,
  choice,
  onChoice,
  onOpen,
}: {
  game: GameKey;
  icons: Record<string, string>;
  choice: Choice;
  onChoice: (c: Choice) => void;
  onOpen: (tab: string) => void;
}) {
  const L = useLauncher();
  const [records, setRecords] = useState<api.PlayRecord[]>([]);
  const [modCount, setModCount] = useState<number | null>(null);

  useEffect(() => {
    api
      .playStats()
      .then((r) => setRecords(r.filter((x) => x.kind === recordKind(game)).sort((a, b) => b.lastPlayed - a.lastPlayed)))
      .catch(() => {});
  }, [game, L.busy]);

  const art = gameIconSrc(game, icons);
  const set = (k: keyof Choice, v: string) => onChoice({ ...choice, [k]: v });

  // Minecraft: total mods across instances for the Mods card.
  const instKey = L.instances.map((i) => i.id).join(",");
  useEffect(() => {
    if (game !== "minecraft") return;
    Promise.all(L.instances.map((i) => api.listInstalled("instance", i.id).catch(() => [])))
      .then((lists) => setModCount(lists.reduce((a, l) => a + l.filter((x) => x.projectType === "mod").length, 0)))
      .catch(() => setModCount(null));
  }, [game, instKey]);

  const page = useMemo(() => {
    if (game === "minecraft") {
      const lastId = records[0]?.key.replace(/^instance:/, "");
      const inst = L.instances.find((i) => i.id === choice.instance) ?? L.instances.find((i) => i.id === lastId) ?? L.instances[0];
      const running = Object.values(L.serverStatuses).filter((s) => s.running);
      return {
        cards: [
          {
            slug: "mods",
            title: "Mods",
            icon: Icon.mods,
            text: modCount === null ? "Mods" : `${modCount} mod${modCount === 1 ? "" : "s"} installed`,
            action: "View mods",
            onClick: () =>
              inst
                ? L.openContent({ kind: "instance", id: inst.id, name: inst.name, version: inst.version, loader: inst.loader === "vanilla" ? null : inst.loader })
                : onOpen("Instances"),
          },
          {
            slug: "servers",
            title: "Servers",
            icon: Icon.server,
            text: `${L.servers.length} server${L.servers.length === 1 ? "" : "s"} created`,
            action: "View servers",
            onClick: () => onOpen("Servers"),
          },
        ],
        session: {
          title: "Your servers",
          count: `${running.length} online`,
          rows: L.servers.slice(0, 3).map((s) => {
            const st = L.serverStatuses[s.id];
            return {
              key: s.id,
              name: s.name,
              sub: st?.running ? `Online · ${st.players}/${st.maxPlayers} players` : `Offline · ${s.version}`,
              on: !!st?.running,
              icon: s.icon ? <img src={s.icon} alt="" /> : <GameArt game="minecraft" icons={icons} size={40} />,
              onClick: () => (st?.running ? L.openConsole(s.id) : onOpen("Servers")),
            };
          }),
          empty: (
            <>
              No servers yet.{" "}
              <button className="link-btn" onClick={() => onOpen("Servers")}>
                Create one
              </button>
            </>
          ),
          foot: `Kawma Net · ${L.netPeers.length} PC${L.netPeers.length === 1 ? "" : "s"} nearby`,
          pickLabel: "Selected instance",
          pickValue: inst?.id ?? "",
          options: L.instances.length ? L.instances.map((i) => ({ value: i.id, label: i.name })) : [{ value: "", label: "No instances yet" }],
          onPick: (v: string) => set("instance", v),
        },
        shelfTitle: "Installed builds",
        tiles: L.instances.map<Tile>((i) => ({
          key: i.id,
          label: i.loader === "vanilla" ? "Vanilla" : i.loader.charAt(0).toUpperCase() + i.loader.slice(1),
          sub: i.version,
          ready: true,
          selected: i.id === inst?.id,
          art: i.icon ?? art,
          onClick: () => set("instance", i.id),
        })),
        shelfEmpty: "No instances yet.",
      };
    }

    const g = game;
    const modes = launchModes(g, L.games);
    const mode = pickedMode(g, L.games, choice);
    const info = g === "skyrim" ? L.games?.skyrim : g === "eldenring" ? L.games?.eldenRing : L.games?.cyberpunk;
    const source = info?.installed ? ("source" in info && info.source === "epic" ? "Epic Games" : "Steam") : null;
    const launch = (id: string) =>
      void (g === "skyrim" ? L.launchSkyrim(id) : g === "eldenring" ? L.launchEldenRing(id) : L.launchCyberpunk(id));
    const coop = (loadServers(g) as { name: string; address: string }[]).length;

    const cards = {
      skyrim: [
        { slug: "mods", title: "Mods", icon: Icon.mods, text: L.games?.skyrim.has_skse ? "SKSE ready for mods" : "SKSE not installed", action: "View mods", tab: "Mods" },
        { slug: "coop", title: "Co-op", icon: Icon.coop, text: L.games?.skyrim.has_skyrim_together ? "Skyrim Together ready" : "Co-op not set up", action: "Play together", tab: "Co-op" },
      ],
      eldenring: [
        { slug: "coop", title: "Co-op", icon: Icon.coop, text: L.games?.eldenRing.has_seamless_coop ? "Seamless Co-op ready" : "Co-op not set up", action: "Play together", tab: "Co-op" },
        { slug: "cheats", title: "Cheats", icon: Icon.sparkles, text: "Offline tweaks and trainers", action: "View cheats", tab: "Cheats" },
      ],
      cyberpunk: [
        { slug: "coop", title: "Co-op", icon: Icon.coop, text: L.games?.cyberpunk.has_mp ? "CyberpunkMP ready" : "Co-op not set up", action: "Play together", tab: "Co-op" },
        { slug: "mods", title: "Mods", icon: Icon.mods, text: L.games?.cyberpunk.has_cet ? "Cyber Engine Tweaks ready" : "CET not installed", action: "View mods", tab: "Mods" },
      ],
    }[g].map((c) => ({ ...c, onClick: () => onOpen(c.tab) }));

    const tools: { key: string; label: string; ready: boolean }[] = {
      skyrim: [
        { key: "skse", label: "SKSE64", ready: !!L.games?.skyrim.has_skse },
        { key: "addr", label: "Address Library", ready: !!L.games?.skyrim.has_address_library },
        { key: "str", label: "Skyrim Together", ready: !!L.games?.skyrim.has_skyrim_together },
      ],
      eldenring: [
        { key: "seamless", label: "Seamless Co-op", ready: !!L.games?.eldenRing.has_seamless_coop },
        { key: "me2", label: "Mod Engine 2", ready: !!L.games?.eldenRing.has_mod_engine },
        { key: "uw", label: "Ultrawide", ready: !!L.games?.eldenRing.ultrawide_installed },
      ],
      cyberpunk: [
        { key: "cet", label: "CET", ready: !!L.games?.cyberpunk.has_cet },
        { key: "mp", label: "CyberpunkMP", ready: !!L.games?.cyberpunk.has_mp },
      ],
    }[g];

    return {
      cards,
      session: {
        title: "Launch modes",
        count: `${modes.filter((m) => m.ready).length} ready`,
        rows: modes.map((m) => ({
          key: m.id,
          name: m.label,
          sub: m.ready ? m.sub : "Not installed",
          on: m.ready,
          selected: m.id === mode,
          icon: <GameArt game={g} icons={icons} size={40} />,
          onClick: m.ready ? () => launch(m.id) : () => onOpen("Setup"),
        })),
        empty: null,
        foot: source ? `Installed via ${source}${coop ? ` · ${coop} co-op server${coop === 1 ? "" : "s"} saved` : ""}` : "Not found on this PC",
        pickLabel: "Play button launches",
        pickValue: mode,
        options: modes.map((m) => ({ value: m.id, label: m.ready ? m.label : `${m.label} (not installed)`, disabled: !m.ready })),
        onPick: (v: string) => set(g, v),
      },
      shelfTitle: "Installed tools",
      tiles: tools.map<Tile>((t) => ({
        key: t.key,
        label: t.label,
        sub: t.ready ? "Ready" : "Set up",
        ready: t.ready,
        art,
        onClick: () => onOpen("Setup"),
      })),
      shelfEmpty: "",
    };
  }, [game, L, choice, records, modCount, art, icons]);

  return (
    <div className="ghome">
      <div className="ghome-top">
        <section>
          <div className="dash-label">Your library</div>
          <div className="lib-shelf">
            {page.cards.map((c) => (
              <LibCard key={c.slug} game={game} art={art} {...c} />
            ))}
          </div>
        </section>
        <section>
          <div className="dash-label">Current session</div>
          <SessionCard {...page.session} />
        </section>
      </div>

      <div className="ghome-bottom">
        <section>
          <div className="dash-label">Play activity</div>
          <WeekActivity game={game} icons={icons} records={records} />
        </section>
        <section>
          <div className="dash-label">{page.shelfTitle}</div>
          <TileShelf tiles={page.tiles} empty={page.shelfEmpty} />
        </section>
        <section>
          <div className="dash-label">Recent activity</div>
          <RecentList game={game} icons={icons} records={records} />
        </section>
      </div>
    </div>
  );
}
