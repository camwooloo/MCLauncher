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
  uiStyle: "aurora" | "liquidglass";
  background: "static" | "pulsing";
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
  has_skse: boolean;
  has_skyrim_together: boolean;
  skyrim_together_path: string | null;
}

export interface EldenRingInfo {
  installed: boolean;
  install_dir: string | null;
  game_dir: string | null;
  has_seamless_coop: boolean;
  seamless_launcher_path: string | null;
  coop_password: string | null;
}

export interface GamesStatus {
  skyrim: SkyrimInfo;
  eldenRing: EldenRingInfo;
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

export type GameKey = "minecraft" | "skyrim" | "eldenring";

export interface ServerEntry {
  id: string;
  name: string;
  address: string;
}
