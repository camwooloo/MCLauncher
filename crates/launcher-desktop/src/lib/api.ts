// Bridge to the Rust backend. When running inside Tauri we call real commands;
// in a plain browser (used for design previews) we return representative mock
// data so the UI renders fully without a backend.

import type {
  Account,
  AccountStore,
  GamesStatus,
  PathsInfo,
  Settings,
} from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function listen<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

// ---- Window controls ----
export async function windowAction(action: "minimize" | "toggleMaximize" | "close") {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  if (action === "minimize") await w.minimize();
  else if (action === "toggleMaximize") await w.toggleMaximize();
  else await w.close();
}

// ---- Commands (with browser mocks) ----

export const appVersion = () => (isTauri ? call<string>("app_version") : Promise.resolve("0.1.0"));

export const openUrl = (url: string) =>
  isTauri ? call<void>("open_url", { url }) : Promise.resolve(void window.open(url, "_blank"));

export const systemMemoryMb = (): Promise<number> =>
  isTauri ? call("system_memory_mb") : Promise.resolve(32768);

export const pathsInfo = (): Promise<PathsInfo> =>
  isTauri
    ? call("paths_info")
    : Promise.resolve({
        gameDir: "C:\\Users\\you\\AppData\\Roaming\\.minecraft",
        dataDir: "C:\\Users\\you\\AppData\\Roaming\\AuroraLauncher",
      });

export const getSettings = (): Promise<Settings> =>
  isTauri
    ? call("get_settings")
    : Promise.resolve({
        maxMemoryMb: 4096,
        lastLoader: "vanilla",
        lastVersion: "1.21.4",
        theme: "dark",
        uiStyle: "aurora",
        background: "liquid",
      });

export const saveSettings = (settings: Settings) =>
  isTauri ? call<void>("save_settings", { settings }) : Promise.resolve();

export const setLaunchAtLogin = (enabled: boolean) =>
  isTauri ? call<void>("set_launch_at_login", { enabled }) : Promise.resolve();

export const minecraftVersions = (): Promise<string[]> =>
  isTauri
    ? call("minecraft_versions")
    : Promise.resolve([
        "1.21.4",
        "1.21.3",
        "1.21.1",
        "1.20.6",
        "1.20.4",
        "1.20.1",
        "1.19.4",
        "1.18.2",
        "1.16.5",
        "1.12.2",
      ]);

const mockStore: AccountStore = {
  accounts: [
    {
      username: "AuroraDev",
      uuid: "9a1f0c5e7b2d4a3c8e6f1b09d4a25c11",
      access_token: "mock",
      xuid: "",
      user_type: "msa",
      refresh_token: "",
    },
  ],
  active_uuid: "9a1f0c5e7b2d4a3c8e6f1b09d4a25c11",
};

export const listAccounts = (): Promise<AccountStore> =>
  isTauri ? call("list_accounts") : Promise.resolve(structuredClone(mockStore));

export const addOfflineAccount = (username: string): Promise<Account> =>
  isTauri
    ? call("add_offline_account", { username })
    : Promise.resolve({
        username,
        uuid: Math.random().toString(16).slice(2).padEnd(32, "0").slice(0, 32),
        access_token: "0",
        xuid: "",
        user_type: "legacy",
      });

export const setActiveAccount = (uuid: string) =>
  isTauri ? call<void>("set_active_account", { uuid }) : Promise.resolve();

export const removeAccount = (uuid: string) =>
  isTauri ? call<void>("remove_account", { uuid }) : Promise.resolve();

export const microsoftLogin = (): Promise<Account> =>
  isTauri
    ? call("microsoft_login")
    : Promise.reject(new Error("Microsoft login is only available in the desktop app"));

/** Fallback device-code sign-in (visit a URL + type a short code). */
export const microsoftLoginCode = (): Promise<Account> =>
  isTauri
    ? call("microsoft_login_code")
    : Promise.reject(new Error("Microsoft login is only available in the desktop app"));

export interface PlayArgs {
  version: string;
  loader: string;
  account: Account;
  memoryMb: number;
  server?: string | null;
}
export const playMinecraft = (args: PlayArgs): Promise<string> =>
  isTauri
    ? call("play_minecraft", { args })
    : Promise.reject(new Error("Launching is only available in the desktop app"));

export const detectGames = (): Promise<GamesStatus> =>
  isTauri
    ? call("detect_games")
    : Promise.resolve({
        skyrim: {
          installed: true,
          install_dir: "D:\\SteamLibrary\\steamapps\\common\\Skyrim Special Edition",
          has_skse: false,
          has_skyrim_together: false,
          has_address_library: false,
          skyrim_together_path: null,
        },
        eldenRing: {
          installed: true,
          install_dir: "D:\\SteamLibrary\\steamapps\\common\\ELDEN RING",
          game_dir: "D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game",
          has_seamless_coop: false,
          seamless_launcher_path: null,
          coop_password: "aurora",
          has_mod_engine: false,
          mods_dir: null,
          ultrawide_installed: false,
          ultrawide_enabled: false,
        },
        cyberpunk: {
          installed: true,
          install_dir: "D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077",
          has_cet: false,
          has_mp: false,
          mp_path: null,
          mods_dir: null,
        },
      });

export const launchSkyrim = (mode: string): Promise<number> =>
  isTauri ? call("launch_skyrim", { mode }) : Promise.resolve(1234);

export const launchEldenRing = (mode: string): Promise<number> =>
  isTauri ? call("launch_elden_ring", { mode }) : Promise.resolve(1234);

export const launchCyberpunk = (mode: string): Promise<number> =>
  isTauri ? call("launch_cyberpunk", { mode }) : Promise.resolve(1234);

export const setEldenRingPassword = (password: string) =>
  isTauri ? call<void>("set_elden_ring_password", { password }) : Promise.resolve();

/** One-click install of a game tool from its official GitHub release. */
export const installGameTool = (tool: string): Promise<string> =>
  isTauri
    ? call("install_game_tool", { tool })
    : new Promise((r) => setTimeout(() => r(`${tool} installed (mock)`), 600));

export const installSkyrimTogether = (path?: string): Promise<string> =>
  isTauri
    ? call("install_skyrim_together", { path: path ?? null })
    : Promise.resolve("Skyrim Together installed (mock)");

export const openTogetherPage = () =>
  isTauri ? call<void>("open_together_page") : Promise.resolve();

export const installAddressLibrary = (path?: string): Promise<string> =>
  isTauri
    ? call("install_address_library", { path: path ?? null })
    : Promise.resolve("Address Library installed (mock)");

export const openAddressLibraryPage = () =>
  isTauri ? call<void>("open_address_library_page") : Promise.resolve();

export const installSeamlessUpdate = (path?: string): Promise<string> =>
  isTauri
    ? call("install_seamless_update", { path: path ?? null })
    : Promise.resolve("Seamless updated (mock)");

export const openSeamlessPage = () =>
  isTauri ? call<void>("open_seamless_page") : Promise.resolve();

export const openEldenringUltrawidePage = () =>
  isTauri ? call<void>("open_eldenring_ultrawide_page") : Promise.resolve();

export const installEldenringUltrawide = (path?: string): Promise<string> =>
  isTauri ? call("install_eldenring_ultrawide", { path: path ?? null }) : Promise.reject(new Error("Desktop only"));

export const setEldenringUltrawide = (enabled: boolean) =>
  isTauri ? call<void>("set_eldenring_ultrawide", { enabled }) : Promise.resolve();

export const openPath = (path: string) =>
  isTauri ? call<void>("open_path", { path }) : Promise.resolve();

// ---- Server hosting (multi-server) ----
import type { ServerConfig, ServerStatus } from "./types";

const mockServers: ServerConfig[] = [
  {
    id: "demo-survival",
    name: "Survival SMP",
    description: "Friends survival world",
    version: "1.21.4",
    port: 25565,
    maxPlayers: 10,
    maxRamMb: 4096,
    loader: "vanilla",
  },
];

export const listServers = (): Promise<ServerConfig[]> =>
  isTauri ? call("list_servers") : Promise.resolve(structuredClone(mockServers));

export const saveServer = (config: ServerConfig) =>
  isTauri ? call<void>("save_server", { config }) : Promise.resolve();

export const deleteServer = (id: string) =>
  isTauri ? call<void>("delete_server", { id }) : Promise.resolve();

export const serversStatus = (): Promise<ServerStatus[]> =>
  isTauri ? call("servers_status") : Promise.resolve([]);

export const serverStart = (id: string) =>
  isTauri
    ? call<void>("server_start", { id })
    : Promise.reject(new Error("Hosting is only available in the desktop app"));

export const serverStop = (id: string) =>
  isTauri ? call<void>("server_stop", { id }) : Promise.resolve();

export const serverCommand = (id: string, line: string) =>
  isTauri ? call<void>("server_command", { id, line }) : Promise.resolve();

// Replay buffered console output so reopening the dashboard keeps history.
export const serverLogHistory = (id: string): Promise<{ line: string; err: boolean }[]> =>
  isTauri ? call("server_log_history", { id }) : Promise.resolve([]);

export const openServerConsole = (id: string) =>
  isTauri ? call<void>("open_server_console", { id }) : Promise.resolve();

// ---- Modrinth content + updates ----
import type { InstalledItem, SearchHit, UpdateResult } from "./types";

const mockHits: SearchHit[] = [
  { project_id: "P7dR8mSH", slug: "fabric-api", title: "Fabric API", description: "Core library for Fabric mods.", author: "modmuss50", downloads: 28000000, icon_url: null, project_type: "mod" },
  { project_id: "AANobbMI", slug: "sodium", title: "Sodium", description: "Modern rendering engine & performance.", author: "jellysquid3", downloads: 19000000, icon_url: null, project_type: "mod" },
  { project_id: "gvQqBUqZ", slug: "lithium", title: "Lithium", description: "General-purpose optimization mod.", author: "jellysquid3", downloads: 12000000, icon_url: null, project_type: "mod" },
];

export const modrinthSearch = (
  query: string,
  kind: string,
  gameVersion?: string,
  loader?: string
): Promise<SearchHit[]> =>
  isTauri
    ? call("modrinth_search", { query, kind, gameVersion, loader })
    : Promise.resolve(mockHits.filter((h) => h.project_type === kind || kind === "mod"));

export const contentInstall = (
  targetKind: string,
  targetId: string,
  projectId: string,
  projectType: string,
  title: string
): Promise<InstalledItem> =>
  isTauri
    ? call("content_install", { targetKind, targetId, projectId, projectType, title })
    : Promise.reject(new Error("Installing is only available in the desktop app"));

export const listInstalled = (targetKind: string, targetId: string): Promise<InstalledItem[]> =>
  isTauri ? call("list_installed", { targetKind, targetId }) : Promise.resolve([]);

export const contentRemove = (targetKind: string, targetId: string, projectId: string) =>
  isTauri ? call<void>("content_remove", { targetKind, targetId, projectId }) : Promise.resolve();

export const checkUpdates = (
  targetKind: string,
  targetId: string,
  targetVersion: string
): Promise<UpdateResult[]> =>
  isTauri ? call("check_updates", { targetKind, targetId, targetVersion }) : Promise.resolve([]);

export const applyUpdate = (
  targetKind: string,
  targetId: string,
  projectId: string,
  targetVersion: string
): Promise<InstalledItem> =>
  isTauri
    ? call("apply_update", { targetKind, targetId, projectId, targetVersion })
    : Promise.reject(new Error("Updating is only available in the desktop app"));

// ---- Inventory editor ----
import type { ItemSlot, PlayerRef } from "./types";

export const listWorlds = (targetKind: string, targetId: string): Promise<string[]> =>
  isTauri ? call("list_worlds", { targetKind, targetId }) : Promise.resolve(["world", "New World"]);

export const listPlayers = (targetKind: string, targetId: string, world: string): Promise<PlayerRef[]> =>
  isTauri
    ? call("list_players", { targetKind, targetId, world })
    : Promise.resolve([{ label: "Singleplayer", source: "host" }]);

export const getInventory = (
  targetKind: string,
  targetId: string,
  world: string,
  source: string
): Promise<ItemSlot[]> =>
  isTauri
    ? call("get_inventory", { targetKind, targetId, world, source })
    : Promise.resolve([
        { slot: 0, id: "minecraft:diamond_sword", count: 1, enchantments: [{ id: "minecraft:sharpness", lvl: 5 }] },
        { slot: 1, id: "minecraft:golden_apple", count: 16, enchantments: [] },
      ]);

export const saveInventory = (
  targetKind: string,
  targetId: string,
  world: string,
  source: string,
  items: ItemSlot[]
) => (isTauri ? call<void>("save_inventory", { targetKind, targetId, world, source, items }) : Promise.resolve());

export const setSkin = (variant: string, png: number[]) =>
  isTauri
    ? call<void>("set_skin", { variant, png })
    : Promise.reject(new Error("Skins are only available in the desktop app"));

export const setSkinFromUrl = (variant: string, url: string) =>
  isTauri
    ? call<void>("set_skin_from_url", { variant, url })
    : Promise.reject(new Error("Skins are only available in the desktop app"));

// ---- Play stats (Home: recently played / playtime) ----
export interface PlayRecord {
  key: string;
  name: string;
  kind: string; // "instance" | "skyrim" | "eldenring" | "cyberpunk"
  icon: string | null;
  lastPlayed: number; // unix seconds
  totalSeconds: number;
  launches: number;
}
const mockStats: PlayRecord[] = [
  { key: "instance:modrinth-1KVo5zza", name: "Fabulously Optimized", kind: "instance", icon: "https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp", lastPlayed: Math.floor(Date.now() / 1000) - 3600, totalSeconds: 3 * 3600 + 25 * 60, launches: 12 },
  { key: "game:skyrim", name: "Skyrim Special Edition", kind: "skyrim", icon: null, lastPlayed: Math.floor(Date.now() / 1000) - 90000, totalSeconds: 8 * 3600, launches: 5 },
  { key: "game:cyberpunk", name: "Cyberpunk 2077", kind: "cyberpunk", icon: null, lastPlayed: Math.floor(Date.now() / 1000) - 400000, totalSeconds: 12 * 3600, launches: 9 },
];
export const playStats = (): Promise<PlayRecord[]> =>
  isTauri ? call<PlayRecord[]>("play_stats") : Promise.resolve(mockStats);

// ---- Host addresses (what friends connect to) ----
export interface HostAddresses {
  lan: string | null;
  aurora: string | null;
}
export const hostAddresses = (): Promise<HostAddresses> =>
  isTauri ? call<HostAddresses>("host_addresses") : Promise.resolve({ lan: "192.168.1.42", aurora: "100.101.102.103" });

// ---- Skyrim Together hosting ----
export interface TogetherServerConfig {
  available: boolean;
  serverName: string;
  password: string;
  maxPlayers: number;
  port: number;
  pvp: boolean;
  deathSystem: boolean;
  xpSync: boolean;
  itemDrops: boolean;
  autoPartyJoin: boolean;
  difficulty: number;
}
const mockTogether: TogetherServerConfig = {
  available: true,
  serverName: "Aurora Together Server",
  password: "",
  maxPlayers: 8,
  port: 10578,
  pvp: false,
  deathSystem: true,
  xpSync: true,
  itemDrops: false,
  autoPartyJoin: true,
  difficulty: 4,
};
export const skyrimServerConfig = (): Promise<TogetherServerConfig> =>
  isTauri ? call<TogetherServerConfig>("skyrim_server_config") : Promise.resolve(mockTogether);
export const saveSkyrimServerConfig = (config: TogetherServerConfig): Promise<void> =>
  isTauri ? call<void>("save_skyrim_server_config", { config }) : Promise.resolve();
export const startSkyrimServer = (): Promise<number> =>
  isTauri ? call<number>("start_skyrim_server") : Promise.resolve(0);

// ---- Self-update ----
export interface UpdateInfo {
  version: string;
  current: string;
  notes: string;
  downloadUrl: string;
}
export const checkAppUpdate = (): Promise<UpdateInfo | null> =>
  isTauri ? call<UpdateInfo | null>("check_app_update") : Promise.resolve(null);
export const applyAppUpdate = (downloadUrl: string): Promise<void> =>
  isTauri ? call<void>("apply_app_update", { downloadUrl }) : Promise.resolve();

export interface ReleaseInfo {
  version: string;
  name: string;
  notes: string;
  date: string;
}
export interface ReleasesResult {
  current: string;
  releases: ReleaseInfo[];
}
const mockReleases: ReleasesResult = {
  current: "0.3.2",
  releases: [
    { version: "0.3.2", name: "Aurora Launcher v0.3.2", date: "2026-06-14", notes: "- Styled Aurora Net dropdowns\n- Built-in updater + patch notes" },
    { version: "0.3.0", name: "Aurora Launcher v0.3.0 — Aurora Net", date: "2026-06-14", notes: "- Aurora Net: built-in VPN for no-port-forward co-op" },
  ],
};
export const listReleases = (): Promise<ReleasesResult> =>
  isTauri ? call<ReleasesResult>("list_releases") : Promise.resolve(mockReleases);

// ---- Aurora Net (built-in Tailscale VPN) ----
export interface VpnStatus {
  installed: boolean;
  running: boolean;
  loggedIn: boolean;
  ip: string | null;
  hostname: string | null;
}
export interface JoinPayload {
  v: number;
  key: string;
  ip: string;
  port: number;
  name: string;
  game: string;
  pack?: PackRef | null;
}

const mockVpn: VpnStatus = { installed: false, running: false, loggedIn: false, ip: null, hostname: null };

export const vpnStatus = (): Promise<VpnStatus> =>
  isTauri ? call<VpnStatus>("vpn_status") : Promise.resolve(mockVpn);
export const vpnInstall = (): Promise<void> =>
  isTauri ? call<void>("vpn_install") : Promise.resolve();
export const vpnLogin = (): Promise<string | null> =>
  isTauri ? call<string | null>("vpn_login") : Promise.resolve(null);
export const vpnDisconnect = (): Promise<void> =>
  isTauri ? call<void>("vpn_disconnect") : Promise.resolve();
export const vpnConfig = (): Promise<{ hasToken: boolean }> =>
  isTauri ? call<{ hasToken: boolean }>("vpn_config") : Promise.resolve({ hasToken: false });
export const vpnSetToken = (token: string): Promise<void> =>
  isTauri ? call<void>("vpn_set_token", { token }) : Promise.resolve();
export const vpnJoin = (code: string): Promise<JoinPayload> =>
  isTauri
    ? call<JoinPayload>("vpn_join", { code })
    : Promise.reject(new Error("Aurora Net is only available in the desktop app"));
export interface PackRef {
  source: string;
  projectId: string;
  title: string;
  icon?: string | null;
}
export const vpnShare = (args: {
  name: string;
  port: number;
  game: string;
  configureAccess: boolean;
  pack?: PackRef | null;
}): Promise<string> =>
  isTauri
    ? call<string>("vpn_share", { args })
    : Promise.reject(new Error("Aurora Net is only available in the desktop app"));

export interface Peer {
  name: string;
  ip: string | null;
  online: boolean;
  me: boolean;
}
const mockPeers: Peer[] = [
  { name: "your-pc", ip: "100.101.102.103", online: true, me: true },
  { name: "cams-laptop", ip: "100.64.0.7", online: true, me: false },
  { name: "alex-desktop", ip: "100.64.0.9", online: false, me: false },
];
export const vpnPeers = (): Promise<Peer[]> =>
  isTauri ? call<Peer[]>("vpn_peers") : Promise.resolve(mockPeers);

// One-time Windows Firewall allow-rule so friends can reach servers you host
// over Aurora Net. Returns true if it applied (false = already set up).
export const repairAuroraNet = (): Promise<boolean> =>
  isTauri ? call<boolean>("repair_aurora_net") : Promise.resolve(false);

// Reusable "friend code" — share once, any friend can join your network.
export const vpnFriendCode = (regenerate = false): Promise<string> =>
  isTauri ? call<string>("vpn_friend_code", { regenerate }) : Promise.resolve("aurora-net-demo-code");

// Guided Skyrim mod install: merges a downloaded Data-layout zip into the game.
export const installSkyrimMod = (keywords: string[], name: string, path?: string): Promise<string> =>
  isTauri
    ? call<string>("install_skyrim_mod", { keywords, name, path: path ?? null })
    : Promise.reject(new Error("Installing is only available in the desktop app"));

// ---- Skyrim mod catalog (curated + live Nexus metadata) ----
export interface CatalogMod {
  nexusId: number;
  name: string;
  category: string;
  summary: string;
  strCompatible: boolean;
  installable: boolean;
  keywords: string[];
  note: string;
  nexusUrl: string;
  imageUrl?: string | null;
  downloads?: number | null;
  endorsements?: number | null;
  author?: string | null;
}

const mockCatalog: CatalogMod[] = [
  { nexusId: 34179, name: "Skyland AIO", category: "Graphics", summary: "All-in-one 2K landscape, architecture and clutter texture overhaul.", strCompatible: true, installable: false, keywords: ["skyland"], note: "Big texture pack — use a mod manager.", nexusUrl: "#", imageUrl: null, downloads: 4200000, endorsements: 120000 },
  { nexusId: 12125, name: "Obsidian Weathers and Seasons", category: "Weather & Lighting", summary: "Cinematic weather with moody storms, auroras and fog.", strCompatible: true, installable: true, keywords: ["obsidian"], note: "", nexusUrl: "#", imageUrl: null, downloads: 1800000, endorsements: 45000 },
  { nexusId: 12604, name: "SkyUI", category: "Interface", summary: "Searchable inventory menus and the MCM mod-config menu. Needs SKSE.", strCompatible: true, installable: true, keywords: ["skyui"], note: "", nexusUrl: "#", imageUrl: null, downloads: 9000000, endorsements: 300000 },
  { nexusId: 1137, name: "Ordinator - Perks of Skyrim", category: "Gameplay", summary: "Reworks every perk tree with 400+ new perks.", strCompatible: false, installable: true, keywords: ["ordinator"], note: "Can desync in co-op.", nexusUrl: "#", imageUrl: null, downloads: 3500000, endorsements: 110000 },
];

export const nexusConfig = (): Promise<{ hasKey: boolean }> =>
  isTauri ? call("nexus_config") : Promise.resolve({ hasKey: false });

export const nexusSetKey = (key: string): Promise<void> =>
  isTauri ? call<void>("nexus_set_key", { key }) : Promise.resolve();

export const skyrimCatalog = (): Promise<CatalogMod[]> =>
  isTauri ? call("skyrim_catalog") : Promise.resolve(mockCatalog);

export interface ModDetail {
  name: string;
  summary: string;
  description: string;
  images: string[];
  downloads: number;
  endorsements: number;
  version?: string | null;
  author?: string | null;
  updated?: string | null;
  adult: boolean;
}

const mockDetail: ModDetail = {
  name: "Sample Mod",
  summary: "A short summary of what this mod does.",
  description:
    "This is the full description.\n\nIt spans multiple paragraphs and explains features, requirements and installation notes in plain text.",
  images: [
    "https://picsum.photos/seed/skyrim1/900/506",
    "https://picsum.photos/seed/skyrim2/900/506",
    "https://picsum.photos/seed/skyrim3/900/506",
  ],
  downloads: 4200000,
  endorsements: 120000,
  version: "3.0.1",
  author: "ModAuthor",
  updated: "2024-09-01",
  adult: false,
};

export const skyrimModDetail = (nexusId: number): Promise<ModDetail> =>
  isTauri ? call("skyrim_mod_detail", { nexusId }) : Promise.resolve(mockDetail);

// ---- Built-in config / code editor ----
const mockConfigFiles = ["config/sodium-options.json", "config/fabric/indigo.json", "server.properties", "config/example.yaml"];
export const listConfigFiles = (kind: string, id: string): Promise<string[]> =>
  isTauri ? call<string[]>("list_config_files", { kind, id }) : Promise.resolve(mockConfigFiles);
export const readConfigFile = (kind: string, id: string, path: string): Promise<string> =>
  isTauri
    ? call<string>("read_config_file", { kind, id, path })
    : Promise.resolve(`# ${path}\nexample: true\nquality: high\nnested:\n  - a\n  - b\n`);
export const writeConfigFile = (kind: string, id: string, path: string, content: string): Promise<void> =>
  isTauri ? call<void>("write_config_file", { kind, id, path, content }) : Promise.resolve();

// ---- World backups ----
export interface BackupInfo {
  file: string;
  size: number;
  created: number;
}
export const listBackups = (kind: string, id: string): Promise<BackupInfo[]> =>
  isTauri ? call<BackupInfo[]>("list_backups", { kind, id }) : Promise.resolve([]);
export const createBackup = (kind: string, id: string): Promise<BackupInfo> =>
  isTauri
    ? call<BackupInfo>("create_backup", { kind, id })
    : Promise.resolve({ file: "backup-0.zip", size: 0, created: Math.floor(Date.now() / 1000) });
export const restoreBackup = (kind: string, id: string, file: string): Promise<void> =>
  isTauri ? call<void>("restore_backup", { kind, id, file }) : Promise.resolve();
export const deleteBackup = (kind: string, id: string, file: string): Promise<void> =>
  isTauri ? call<void>("delete_backup", { kind, id, file }) : Promise.resolve();

// ---- Instances ----
import type { InstanceConfig } from "./types";

const mockInstances: InstanceConfig[] = [
  { id: "demo-vanilla", name: "Vanilla 1.21.4", version: "1.21.4", loader: "vanilla", maxRamMb: 4096 },
  { id: "demo-fabric", name: "Fabric Optimized", version: "1.21.1", loader: "fabric", maxRamMb: 6144 },
];

export const listInstances = (): Promise<InstanceConfig[]> =>
  isTauri ? call("list_instances") : Promise.resolve(structuredClone(mockInstances));

export const saveInstance = (config: InstanceConfig) =>
  isTauri ? call<void>("save_instance", { config }) : Promise.resolve();

export const deleteInstance = (id: string) =>
  isTauri ? call<void>("delete_instance", { id }) : Promise.resolve();

export const popularModpacks = (): Promise<SearchHit[]> =>
  isTauri
    ? call("popular_modpacks")
    : Promise.resolve([
        { project_id: "fabulously-optimized", slug: "fo", title: "Fabulously Optimized", description: "Maximum FPS, minimal fuss.", author: "—", downloads: 9000000, icon_url: null, project_type: "modpack" },
        { project_id: "create", slug: "create", title: "Create: Above and Beyond", description: "Tech + automation.", author: "—", downloads: 2000000, icon_url: null, project_type: "modpack" },
      ]);

export const createInstanceFromModpack = (projectId: string, title: string): Promise<InstanceConfig> =>
  isTauri
    ? call("create_instance_from_modpack", { projectId, title })
    : Promise.reject(new Error("Modpack install is only available in the desktop app"));

// ---- Multi-source modpacks (Modrinth / FTB / CurseForge / Technic) ----
import type { PackHit } from "./types";

export const packSearch = (source: string, query: string): Promise<PackHit[]> =>
  isTauri
    ? call("pack_search", { source, query })
    : Promise.resolve([
        { id: "atm9", name: "All the Mods 9", summary: "The kitchen-sink classic.", icon: null, downloads: 5000000 },
        { id: "ftb-skyfactory", name: "FTB SkyFactory", summary: "Skyblock, but huge.", icon: null, downloads: 800000 },
      ]);

export const createInstanceFromPack = (
  source: string,
  projectId: string,
  title: string,
  icon: string | null
): Promise<InstanceConfig> =>
  isTauri
    ? call("create_instance_from_pack", { source, projectId, title, icon })
    : Promise.reject(new Error("Modpack install is only available in the desktop app"));

export const createServerFromPack = (
  source: string,
  projectId: string,
  title: string,
  icon: string | null
): Promise<unknown> =>
  isTauri
    ? call("create_server_from_pack", { source, projectId, title, icon })
    : Promise.reject(new Error("Modpack install is only available in the desktop app"));

export const instancePlay = (id: string, server?: string | null): Promise<string> =>
  isTauri
    ? call("instance_play", { id, server: server ?? null })
    : Promise.reject(new Error("Launching is only available in the desktop app"));

// ---- Server whitelist / ops ----
export interface AccessMember { name: string; uuid: string }
export interface ServerAccess { whitelist: AccessMember[]; ops: AccessMember[] }
export const serverAccess = (id: string): Promise<ServerAccess> =>
  isTauri ? call<ServerAccess>("server_access", { id }) : Promise.resolve({ whitelist: [], ops: [] });
export const accessAdd = (id: string, list: "whitelist" | "ops", name: string): Promise<AccessMember> =>
  isTauri ? call<AccessMember>("access_add", { id, list, name }) : Promise.resolve({ name, uuid: "0000" });
export const accessRemove = (id: string, list: "whitelist" | "ops", uuid: string): Promise<void> =>
  isTauri ? call<void>("access_remove", { id, list, uuid }) : Promise.resolve();

// ---- Crash analyzer ----
export interface CrashInfo {
  found: boolean;
  when: number;
  title: string;
  culpritName: string | null;
  culpritFile: string | null;
  reportPath: string;
}
export const analyzeCrash = (id: string): Promise<CrashInfo> =>
  isTauri
    ? call<CrashInfo>("analyze_crash", { id })
    : Promise.resolve({ found: true, when: Math.floor(Date.now() / 1000), title: "Failed to initialize Controlify", culpritName: "Controlify", culpritFile: "controlify-3.0.0+lts+1.21.5-fabric.jar", reportPath: "crash-reports/latest.txt" });
export const disableMod = (id: string, file: string): Promise<void> =>
  isTauri ? call<void>("disable_mod", { id, file }) : Promise.resolve();

// ---- Import / export instances ----
export const exportInstance = (id: string): Promise<string> =>
  isTauri ? call<string>("export_instance", { id }) : Promise.resolve("");
export const importMrpack = (name: string, bytes: number[]): Promise<InstanceConfig> =>
  isTauri
    ? call<InstanceConfig>("import_mrpack", { name, bytes })
    : Promise.reject(new Error("Import is only available in the desktop app"));

export const openInstanceFolder = (id: string) =>
  isTauri ? call<void>("open_instance_folder", { id }) : Promise.resolve();

export const openServerFolder = (id: string) =>
  isTauri ? call<void>("open_server_folder", { id }) : Promise.resolve();

/** The current window's label (used to detect the dashboard window). */
export async function currentWindowLabel(): Promise<string> {
  if (!isTauri) return "main";
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}
