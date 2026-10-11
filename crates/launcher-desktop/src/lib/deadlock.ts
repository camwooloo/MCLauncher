// Deadlock mod manager bridge: GameBanana store, the mod library, hero
// skins and the Extras page. In a plain browser (design previews) it serves
// mock data, and library changes persist for the session so toggles work.

import { useEffect, useSyncExternalStore } from "react";

import { call, isTauri, listen } from "./api";
import type { DlCategory, DlExtras, DlLibrary, DlLibraryMod, DlModDetail, DlSort, DlStoreMod, DlStorePage } from "./types";

/** Deadlock's heroes (matches launcher-core's gamebanana::HEROES). */
export const HEROES = [
  "Abrams",
  "Apollo",
  "Bebop",
  "Billy",
  "Calico",
  "Celeste",
  "Doorman",
  "Drifter",
  "Dynamo",
  "Graves",
  "Grey Talon",
  "Haze",
  "Holliday",
  "Infernus",
  "Ivy",
  "Kelvin",
  "Lady Geist",
  "Lash",
  "McGinnis",
  "Mina",
  "Mirage",
  "Mo & Krill",
  "Paige",
  "Paradox",
  "Pocket",
  "Rem",
  "Seven",
  "Shiv",
  "Silver",
  "Sinclair",
  "Venator",
  "Victor",
  "Vindicta",
  "Viscous",
  "Vyper",
  "Warden",
  "Wraith",
  "Yamato",
];

export const effectiveHero = (m: DlLibraryMod) => m.heroOverride ?? m.hero;
export const isSkin = (m: { category: string | null; hero: string | null }) => !!m.hero || /skin|model/i.test(m.category ?? "");

/* --------------------------------- mocks --------------------------------- */

const now = () => Math.floor(Date.now() / 1000);

/** Placeholder cover art for previews: a tinted poster with the mod's hero. */
function mockArt(seed: number, label: string): string {
  const hue = (seed * 47) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="530" height="300" viewBox="0 0 530 300">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue},55%,42%)"/><stop offset="1" stop-color="hsl(${(hue + 40) % 360},45%,12%)"/></linearGradient>
<radialGradient id="r" cx="0.7" cy="0.35" r="0.6"><stop offset="0" stop-color="hsla(${(hue + 20) % 360},90%,75%,0.55)"/><stop offset="1" stop-color="transparent"/></radialGradient></defs>
<rect width="530" height="300" fill="url(#g)"/><rect width="530" height="300" fill="url(#r)"/>
<g fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="2"><circle cx="380" cy="120" r="70"/><circle cx="380" cy="120" r="110"/></g>
<text x="28" y="150" font-family="Georgia,serif" font-size="50" font-weight="700" fill="rgba(255,255,255,0.82)" letter-spacing="2">${label.toUpperCase()}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

const MOCK_STORE: DlStoreMod[] = (
  [
    ["Jacket (Billy Overhaul)", "amperesevere", "Skins", "Billy", 831084],
    ["Reze over Calico", "rezedegen", "Skins", "Calico", 536471],
    ["Formal Wraith (Icons)", "ticoslay", "Skins", "Wraith", 412300],
    ["Manhattan Cafe as Pocket", "Manos", "Skins", "Pocket", 340185],
    ["Bottom Heavy Paige", "FlamingRug", "Skins", "Paige", 324711],
    ["Shadow (Cid Kagenou)", "MoltiOG", "Skins", "Apollo", 298004],
    ["Yamato remodel", "thiagolima13D", "Skins", "Yamato", 254112],
    ["Spiderman Lash", "Beyonx", "Skins", "Lash", 233500],
    ["Hank Wimbleton Drifter", "Artidery", "Skins", "Drifter", 201345],
    ["Frozone Kelvin", "P1g", "Skins", "Kelvin", 188002],
    ["Rice Shower Vindicta", "N8FanAccount", "Skins", "Vindicta", 176210],
    ["Bondrewd Paradox", "TasteOpotato", "Skins", "Paradox", 150231],
    ["Kore Graves", "DragonRollZ", "Skins", "Graves", 143100],
    ["Toasted Style Sisyphus", "Tropitortoise", "Skins", "Seven", 98000],
    ["Phalaenopsis as Mina", "Manos", "Skins", "Mina", 84008],
    ["Hoglin & Piglin", "Hubababa_", "Skins", "Mo & Krill", 76000],
    ["Deadlock Toasted", "toastyghostey", "Gameplay Modifications", null, 690100],
    ["OptiLock FPS Configs", "dacooderr", "Gameplay Modifications", null, 594964],
    ["Minimal HUD", "souls", "HUD", null, 412000],
    ["Clean Crosshair Pack", "lumen", "HUD", null, 288400],
    ["Scottish Ratking", "db123infane", "Other/Misc", null, 61282],
    ["Ratking Flag", "Kotifi", "Other/Misc", null, 41173],
    ["Gigi Murin VO (Drifter Voice)", "wooloopt2", "Sounds", "Drifter", 30199],
    ["Fortunate Son for Rat King", "ShockHub", "Sounds", null, 22186],
    ["Squidward over Celeste", "Tropitortoise", "Sounds", "Celeste", 18126],
    ["Neco Arc Announcer", "nekoarc", "Sounds", null, 120004],
    ["Snicker's Shop Music", "candyman", "Sounds", null, 88010],
    ["Smooth Zipline", "flow", "Gameplay Modifications", null, 70002],
  ] as [string, string, string, string | null, number][]
).map(([name, author, category, hero, downloads], i) => ({
  id: 600000 + i * 137,
  name,
  author,
  image: category === "Sounds" ? null : mockArt(i + 3, hero ?? name.split(" ")[0]),
  category,
  hero,
  likes: Math.round(downloads / 9000) + (i % 7),
  views: downloads * 3,
  downloads,
  nsfw: false,
  added: now() - (i + 1) * 86400 * 2,
  updated: now() - (i % 9) * 86400,
  url: `https://gamebanana.com/mods/${600000 + i * 137}`,
  audio: category === "Sounds",
}));

const MOCK_CATEGORIES: DlCategory[] = ["Gameplay Modifications", "HUD", "Other/Misc", "Skins", "Sounds"].map((name, i) => ({
  id: 3000 + i,
  name,
  count: MOCK_STORE.filter((m) => m.category === name).length,
  icon: null,
}));

function libFromStore(m: DlStoreMod, enabled: boolean, extra: Partial<DlLibraryMod> = {}): DlLibraryMod {
  return {
    id: `gb-${m.id}`,
    gamebananaId: m.id,
    name: m.name,
    author: m.author,
    image: m.image,
    category: m.category,
    hero: m.hero,
    heroOverride: null,
    nsfw: m.nsfw,
    enabled,
    files: [{ name: "01_pak01_dir.vpk", label: "pak01_dir.vpk", size: 48_000_000, enabled: true }],
    updated: m.updated,
    installedAt: now() - 86400,
    fileId: m.id + 1,
    updateAvailable: false,
    ...extra,
  };
}

let mockLibrary: DlLibraryMod[] = [
  libFromStore(MOCK_STORE[6], true),
  libFromStore(MOCK_STORE[2], true),
  libFromStore(MOCK_STORE[16], true, { updateAvailable: true }),
  libFromStore(MOCK_STORE[9], false),
  libFromStore(MOCK_STORE[15], false),
  libFromStore(MOCK_STORE[7], true),
  libFromStore(MOCK_STORE[8], false),
  libFromStore(MOCK_STORE[10], true),
  libFromStore(MOCK_STORE[11], false),
  libFromStore(MOCK_STORE[12], true),
  libFromStore(MOCK_STORE[5], true),
  libFromStore(MOCK_STORE[17], true, {
    files: [
      { name: "01_pak01_dir.vpk", label: "Low/pak01_dir.vpk", size: 2_000_000, enabled: false },
      { name: "02_pak01_dir.vpk", label: "Medium/pak01_dir.vpk", size: 2_000_000, enabled: true },
      { name: "03_pak01_dir.vpk", label: "Ultra/pak01_dir.vpk", size: 2_000_000, enabled: false },
    ],
  }),
];
let mockUnmanaged = ["pak42_dir.vpk", "pak43_dir.vpk"];
let mockExtras: DlExtras = {
  autoexec: "// Kawma Launcher autoexec\nfps_max 240\nengine_no_focus_sleep 0\n",
  autoexecPath: "D:\\SteamLibrary\\steamapps\\common\\Deadlock\\game\\citadel\\cfg\\autoexec.cfg",
  launchOptions: "-novid",
  effectiveArgs: "+exec autoexec -novid",
  modsActive: true,
  gameinfoFound: true,
  backupExists: true,
  installDir: "D:\\SteamLibrary\\steamapps\\common\\Deadlock",
  addonsDir: "D:\\SteamLibrary\\steamapps\\common\\Deadlock\\game\\citadel\\addons",
  libraryDir: "C:\\Users\\you\\AppData\\Roaming\\AuroraLauncher\\deadlock\\mods",
};

function mockState(): DlLibrary {
  const placed = mockLibrary.filter((m) => m.enabled).reduce((a, m) => a + m.files.filter((f) => f.enabled).length, 0);
  return { mods: mockLibrary.map((m) => ({ ...m })), unmanaged: [...mockUnmanaged], slotsUsed: placed + mockUnmanaged.length, syncError: null };
}

const delay = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms));

/* ---------------------------------- api ---------------------------------- */

export function store(sort: DlSort, search: string, category: number | null, page: number): Promise<DlStorePage> {
  if (isTauri) return call("deadlock_store", { sort, search, category, page });
  const q = search.trim().toLowerCase();
  const cat = MOCK_CATEGORIES.find((c) => c.id === category)?.name;
  let list = MOCK_STORE.filter((m) => (!q || `${m.name} ${m.author} ${m.hero ?? ""}`.toLowerCase().includes(q)) && (!cat || m.category === cat));
  list = [...list].sort((a, b) =>
    sort === "newest" ? b.added - a.added : sort === "updated" ? b.updated - a.updated : sort === "viewed" ? b.views - a.views : b.likes - a.likes,
  );
  const per = 30;
  return delay({ mods: list.slice((page - 1) * per, page * per), page, total: list.length, complete: page * per >= list.length });
}

export const trending = (): Promise<DlStoreMod[]> =>
  isTauri ? call("deadlock_trending") : delay([...MOCK_STORE].sort((a, b) => b.downloads - a.downloads));

export const categories = (): Promise<DlCategory[]> => (isTauri ? call("deadlock_categories") : delay(MOCK_CATEGORIES, 50));

export function modDetail(id: number): Promise<DlModDetail> {
  if (isTauri) return call("deadlock_mod_detail", { id });
  const m = MOCK_STORE.find((x) => x.id === id) ?? MOCK_STORE[0];
  return delay({
    ...m,
    description: `${m.name} by ${m.author}.\n\nDrop-in replacement, no extra steps needed. Works with the latest Deadlock update.`,
    images: m.image ? [m.image, mockArt(m.id % 97, "Preview")] : [],
    files: [
      {
        id: m.id + 1,
        name: `${m.name.replace(/\W+/g, "_").toLowerCase()}.zip`,
        size: 48_312_000,
        url: "",
        added: m.updated,
        downloads: Math.round(m.downloads * 0.8),
        description: "Main file",
      },
      {
        id: m.id + 2,
        name: `${m.name.replace(/\W+/g, "_").toLowerCase()}_alt.zip`,
        size: 47_900_000,
        url: "",
        added: m.updated - 86400 * 30,
        downloads: Math.round(m.downloads * 0.2),
        description: "Alternative colours",
      },
    ],
  });
}

export function install(id: number, fileId: number | null): Promise<DlLibrary> {
  if (isTauri) return call("deadlock_install", { id, fileId });
  const m = MOCK_STORE.find((x) => x.id === id)!;
  const entry = libFromStore(m, true);
  const hero = entry.hero;
  if (hero) mockLibrary = mockLibrary.map((x) => (effectiveHero(x) === hero ? { ...x, enabled: false } : x));
  mockLibrary = [...mockLibrary.filter((x) => x.id !== entry.id), entry];
  return delay(mockState(), 1400);
}

export const library = (): Promise<DlLibrary> => (isTauri ? call("deadlock_library") : delay(mockState(), 60));

function mockEdit(f: () => void): Promise<DlLibrary> {
  f();
  return delay(mockState(), 80);
}

export const setEnabled = (id: string, enabled: boolean): Promise<DlLibrary> =>
  isTauri
    ? call("deadlock_set_enabled", { id, enabled })
    : mockEdit(() => (mockLibrary = mockLibrary.map((m) => (m.id === id ? { ...m, enabled } : m))));

export const setFileEnabled = (id: string, file: string, enabled: boolean): Promise<DlLibrary> =>
  isTauri
    ? call("deadlock_set_file_enabled", { id, file, enabled })
    : mockEdit(
        () =>
          (mockLibrary = mockLibrary.map((m) => (m.id === id ? { ...m, files: m.files.map((f) => (f.name === file ? { ...f, enabled } : f)) } : m))),
      );

export const setHero = (id: string, hero: string | null): Promise<DlLibrary> =>
  isTauri
    ? call("deadlock_set_hero", { id, hero })
    : mockEdit(() => (mockLibrary = mockLibrary.map((m) => (m.id === id ? { ...m, heroOverride: hero } : m))));

export const setActiveSkin = (hero: string, id: string | null): Promise<DlLibrary> =>
  isTauri
    ? call("deadlock_set_active_skin", { hero, id })
    : mockEdit(() => (mockLibrary = mockLibrary.map((m) => (effectiveHero(m) === hero ? { ...m, enabled: m.id === id } : m))));

export const reorder = (id: string, delta: number): Promise<DlLibrary> =>
  isTauri
    ? call("deadlock_reorder", { id, delta })
    : mockEdit(() => {
        const i = mockLibrary.findIndex((m) => m.id === id);
        const j = Math.max(0, Math.min(mockLibrary.length - 1, i + delta));
        const next = [...mockLibrary];
        const [m] = next.splice(i, 1);
        next.splice(j, 0, m);
        mockLibrary = next;
      });

export const remove = (id: string): Promise<DlLibrary> =>
  isTauri ? call("deadlock_remove", { id }) : mockEdit(() => (mockLibrary = mockLibrary.filter((m) => m.id !== id)));

export const checkUpdates = (): Promise<DlLibrary> => (isTauri ? call("deadlock_check_updates") : delay(mockState(), 900));

export const adopt = (): Promise<DlLibrary> =>
  isTauri
    ? call("deadlock_adopt")
    : mockEdit(() => {
        mockLibrary = [
          ...mockLibrary,
          ...mockUnmanaged.map((n, i) => ({
            ...libFromStore(MOCK_STORE[20 + i], true),
            id: `local-${i}`,
            gamebananaId: null,
            name: `Local mod (${n})`,
            author: "Added by hand",
            image: null,
            category: null,
            hero: null,
          })),
        ];
        mockUnmanaged = [];
      });

export const launch = (modded: boolean): Promise<number> => (isTauri ? call("deadlock_launch", { modded }) : Promise.resolve(0));

export const extras = (): Promise<DlExtras> => (isTauri ? call("deadlock_extras") : delay({ ...mockExtras }, 60));

export const saveAutoexec = (text: string): Promise<DlExtras> =>
  isTauri ? call("deadlock_save_autoexec", { text }) : delay((mockExtras = { ...mockExtras, autoexec: text }));

export const setLaunchOptions = (options: string): Promise<DlExtras> =>
  isTauri
    ? call("deadlock_set_launch_options", { options })
    : delay((mockExtras = { ...mockExtras, launchOptions: options.trim(), effectiveArgs: `+exec autoexec ${options.trim()}`.trim() }));

export const resetGameinfo = (): Promise<string> => (isTauri ? call("deadlock_reset_gameinfo") : delay("gameinfo.gi restored from the backup"));

export const setModsActive = (active: boolean): Promise<DlExtras> =>
  isTauri ? call("deadlock_set_mods_active", { active }) : delay((mockExtras = { ...mockExtras, modsActive: active }));

export const clearAddons = (): Promise<string> => (isTauri ? call("deadlock_clear_addons") : delay("Removed 9 addon files from the game"));

/* ------------------------- shared library state -------------------------- */

// One copy of the library for every Deadlock page and the hero header, so a
// toggle on one page shows everywhere.
let current: DlLibrary | null = null;
let loading = false;
const subs = new Set<() => void>();
const emit = () => subs.forEach((f) => f());

export function setLibrary(next: DlLibrary) {
  current = next;
  emit();
}

export function reloadLibrary() {
  if (loading) return;
  loading = true;
  library()
    .then(setLibrary)
    .catch(() => {})
    .finally(() => (loading = false));
}

/** The Deadlock library, loaded on first use. */
export function useDeadlockLibrary(): DlLibrary | null {
  const lib = useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => subs.delete(f);
    },
    () => current,
  );
  useEffect(() => {
    if (!current) reloadLibrary();
  }, []);
  return lib;
}

/** Download progress per GameBanana id (0..1), from `deadlock-progress`. */
const progress = new Map<number, number>();
const progressSubs = new Set<() => void>();
let progressListening = false;
export function useInstallProgress(id: number): number | undefined {
  useEffect(() => {
    if (progressListening) return;
    progressListening = true;
    void listen<{ id: number; fraction: number }>("deadlock-progress", (p) => {
      progress.set(p.id, p.fraction);
      progressSubs.forEach((f) => f());
    });
  }, []);
  return useSyncExternalStore(
    (f) => {
      progressSubs.add(f);
      return () => progressSubs.delete(f);
    },
    () => progress.get(id),
  );
}

/* --------------------------------- format -------------------------------- */

export function fmtCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 10_000) return `${Math.round(n / 1000)}k`;
  return n.toLocaleString();
}

export function fmtDate(sec: number): string {
  if (!sec) return "";
  return new Date(sec * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

export function fmtSize(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}
