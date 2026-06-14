// Mirrors of the Rust types sent across the Tauri bridge.

export interface Account {
  username: string;
  uuid: string;
  access_token: string;
  xuid: string;
  user_type: string; // "msa" | "legacy"
}

export interface StoredAccount extends Account {
  refresh_token: string;
}

export interface AccountStore {
  accounts: StoredAccount[];
  active_uuid: string | null;
}

export interface Settings {
  maxMemoryMb: number;
  lastLoader: string;
  lastVersion: string;
  theme: "dark" | "light";
  uiStyle: string; // "aurora" | "liquidglass" | "minimal" | "midnight" | "frost"
  background: string; // pulsing | liquid | mesh | grid | stars | waves | glow | dots | static
  discordRpc?: boolean;
  defaultView?: string; // "home" | "network" | "settings" | "<game>" | "<game>:<tab>"
  launchAtLogin?: boolean;
  startMinimized?: boolean;
  closeToTray?: boolean;
  onboarded?: boolean;
}

export interface PackHit {
  id: string;
  name: string;
  summary: string;
  icon: string | null;
  downloads: number;
}

export interface InstanceConfig {
  id: string;
  name: string;
  version: string;
  loader: string; // "vanilla" | "fabric" | "quilt"
  maxRamMb: number;
  icon?: string | null;
}

export interface ServerConfig {
  id: string;
  name: string;
  description: string;
  version: string;
  port: number;
  maxPlayers: number;
  maxRamMb: number;
  loader?: string | null;
  icon?: string | null;
  autoStart?: boolean;
}

export interface ServerStatus {
  id: string;
  name: string;
  version: string;
  running: boolean;
  players: number;
  maxPlayers: number;
  port: number;
  memoryMb: number;
}

export interface ServerLog {
  id: string;
  line: string;
  err: boolean;
}

export interface PathsInfo {
  gameDir: string;
  dataDir: string;
}

export interface SkyrimInfo {
  installed: boolean;
  install_dir: string | null;
  source?: string | null;
  has_skse: boolean;
  has_skyrim_together: boolean;
  has_address_library: boolean;
  skyrim_together_path: string | null;
}

export interface EldenRingInfo {
  installed: boolean;
  install_dir: string | null;
  game_dir: string | null;
  has_seamless_coop: boolean;
  seamless_launcher_path: string | null;
  coop_password: string | null;
  has_mod_engine: boolean;
  mods_dir: string | null;
}

export interface CyberpunkInfo {
  installed: boolean;
  install_dir: string | null;
  source?: string | null;
  has_cet: boolean;
  has_mp: boolean;
  mp_path: string | null;
  mods_dir: string | null;
}

export interface GamesStatus {
  skyrim: SkyrimInfo;
  eldenRing: EldenRingInfo;
  cyberpunk: CyberpunkInfo;
}

export interface ProgressSnapshot {
  stage: string;
  total: number;
  done: number;
  fraction: number;
}

export interface LoginPrompt {
  userCode: string;
  verificationUri: string;
  message: string;
}

export interface SearchHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  icon_url: string | null;
  project_type: string;
}

export interface InstalledItem {
  projectId: string;
  projectType: string;
  title: string;
  versionId: string;
  versionNumber: string;
  fileName: string;
  gameVersion: string;
  loader: string | null;
}

export interface UpdateResult {
  item: InstalledItem;
  status: "update" | "current" | "incompatible";
  newVersionNumber: string | null;
}

export interface PlayerRef {
  label: string;
  source: string;
}

export interface Enchant {
  id: string;
  lvl: number;
}

export interface ItemSlot {
  slot: number;
  id: string;
  count: number;
  enchantments: Enchant[];
}

export interface ContentTarget {
  kind: "instance" | "server";
  id: string;
  name: string;
  version: string;
  loader: string | null;
}

export type GameKey = "minecraft" | "skyrim" | "eldenring" | "cyberpunk";

export interface ServerEntry {
  id: string;
  name: string;
  address: string;
}
