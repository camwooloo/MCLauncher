import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import * as api from "./lib/api";
import type {
  AccountStore,
  ContentTarget,
  GamesStatus,
  InstanceConfig,
  LoginPrompt,
  PathsInfo,
  ProgressSnapshot,
  ServerConfig,
  ServerStatus,
  Settings,
  StoredAccount,
} from "./lib/types";

interface Launcher {
  versions: string[];
  settings: Settings;
  settingsLoaded: boolean;
  store: AccountStore;
  games: GamesStatus | null;
  paths: PathsInfo | null;

  busy: boolean;
  progress: ProgressSnapshot | null;
  loginPrompt: LoginPrompt | null;
  loginError: string | null;
  toast: string | null;
  instances: InstanceConfig[];
  servers: ServerConfig[];
  serverStatuses: Record<string, ServerStatus>;
  consoleServerId: string | null;
  contentTarget: ContentTarget | null;
  inventoryTarget: ContentTarget | null;
  backupTarget: ContentTarget | null;
  configTarget: ContentTarget | null;
  upgradeTarget: ContentTarget | null;
  systemRamMb: number;

  activeAccount: () => StoredAccount | undefined;
  showToast: (msg: string) => void;
  patchSettings: (s: Partial<Settings>) => void;
  saveSettings: () => Promise<void>;
  persistSettings: (s: Partial<Settings>) => void;

  refreshInstances: () => Promise<void>;
  saveInstanceCfg: (cfg: InstanceConfig) => Promise<void>;
  deleteInstanceCfg: (id: string) => Promise<void>;
  playInstance: (id: string, server?: string) => Promise<void>;
  createFromPack: (source: string, projectId: string, title: string, icon: string | null) => Promise<void>;
  createServerFromPack: (source: string, projectId: string, title: string, icon: string | null) => Promise<void>;
  openInstanceFolder: (id: string) => Promise<void>;
  openServerFolder: (id: string) => Promise<void>;

  refreshServers: () => Promise<void>;
  saveServerCfg: (cfg: ServerConfig) => Promise<void>;
  deleteServerCfg: (id: string) => Promise<void>;
  startServer: (id: string) => Promise<void>;
  stopServer: (id: string) => Promise<void>;
  openConsole: (id: string) => void;
  closeConsole: () => void;
  openContent: (target: ContentTarget) => void;
  closeContent: () => void;
  openInventory: (target: ContentTarget) => void;
  closeInventory: () => void;
  openBackups: (target: ContentTarget) => void;
  closeBackups: () => void;
  openConfigEditor: (target: ContentTarget) => void;
  closeConfigEditor: () => void;
  crashTarget: { id: string; name: string } | null;
  openCrash: (id: string, name: string) => void;
  closeCrash: () => void;
  openUpgrade: (target: ContentTarget) => void;
  closeUpgrade: () => void;
  upgrade: (kind: "instance" | "server", id: string, newVersion: string) => Promise<void>;

  refreshAccounts: () => Promise<void>;
  refreshGames: () => Promise<void>;

  play: (opts: { loader: string; version: string; server?: string }) => Promise<void>;
  addOffline: (name: string) => Promise<void>;
  microsoftLogin: () => Promise<void>;
  microsoftLoginCode: () => Promise<void>;
  setActive: (uuid: string) => Promise<void>;
  removeAccount: (uuid: string) => Promise<void>;

  launchSkyrim: (mode: string) => Promise<void>;
  launchEldenRing: (mode: string) => Promise<void>;
  launchCyberpunk: (mode: string) => Promise<void>;
  setEldenRingPassword: (pw: string) => Promise<void>;
  installTool: (tool: string) => Promise<void>;
  installTogether: () => Promise<void>;
}

const Ctx = createContext<Launcher | null>(null);
export const useLauncher = () => {
  const v = useContext(Ctx);
  if (!v) throw new Error("useLauncher outside provider");
  return v;
};

export function LauncherProvider({ children }: { children: ReactNode }) {
  const [versions, setVersions] = useState<string[]>([]);
  const [settings, setSettings] = useState<Settings>({
    maxMemoryMb: 4096,
    lastLoader: "vanilla",
    lastVersion: "",
    theme: "dark",
    uiStyle: "aurora",
    background: "liquid",
    defaultView: "home",
  });
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [store, setStore] = useState<AccountStore>({ accounts: [], active_uuid: null });
  const [instances, setInstances] = useState<InstanceConfig[]>([]);
  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [serverStatuses, setServerStatuses] = useState<Record<string, ServerStatus>>({});
  const [consoleServerId, setConsoleServerId] = useState<string | null>(null);
  const [contentTarget, setContentTarget] = useState<ContentTarget | null>(null);
  const [inventoryTarget, setInventoryTarget] = useState<ContentTarget | null>(null);
  const [backupTarget, setBackupTarget] = useState<ContentTarget | null>(null);
  const [configTarget, setConfigTarget] = useState<ContentTarget | null>(null);
  const [upgradeTarget, setUpgradeTarget] = useState<ContentTarget | null>(null);
  const [systemRamMb, setSystemRamMb] = useState(16384);
  const [games, setGames] = useState<GamesStatus | null>(null);
  const [paths, setPaths] = useState<PathsInfo | null>(null);

  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ProgressSnapshot | null>(null);
  const [loginPrompt, setLoginPrompt] = useState<LoginPrompt | null>(null);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<number | null>(null);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 4200);
  }, []);

  // Initial load + event subscriptions.
  useEffect(() => {
    (async () => {
      setSettings(await api.getSettings());
      setSettingsLoaded(true);
      setStore(await api.listAccounts());
      setPaths(await api.pathsInfo());
      api.minecraftVersions().then(setVersions).catch(() => {});
      api.detectGames().then(setGames).catch(() => {});
      api.systemMemoryMb().then(setSystemRamMb).catch(() => {});
      api.listInstances().then(setInstances).catch(() => {});
      const serverList = await api.listServers().catch(() => [] as ServerConfig[]);
      setServers(serverList);
      const running = await api.serversStatus().catch(() => []);
      setServerStatuses(Object.fromEntries(running.map((s) => [s.id, s])));
      // Auto-start servers flagged to launch with Aurora (if not already up).
      const runningIds = new Set(running.map((s) => s.id));
      for (const s of serverList) {
        if (s.autoStart && !runningIds.has(s.id)) {
          api.serverStart(s.id).catch(() => {});
        }
      }
    })();

    const mergeStatus = (s: ServerStatus) =>
      setServerStatuses((prev) => {
        const next = { ...prev };
        if (s.running) next[s.id] = s;
        else delete next[s.id];
        return next;
      });

    const unlisteners: Promise<() => void>[] = [
      api.listen<ProgressSnapshot>("mc-progress", setProgress),
      api.listen<LoginPrompt>("login-prompt", setLoginPrompt),
      api.listen<{ message: string }>("login-error", (p) => {
        setLoginPrompt(null);
        setLoginError(p.message);
      }),
      api.listen<{ username: string }>("login-ok", () => setLoginError(null)),
      api.listen<Record<string, never>>("login-opened", () =>
        showToast("Check your browser to finish signing in")
      ),
      api.listen<{ message: string }>("mc-done", (p) => showToast(p.message)),
      api.listen<{ message: string }>("mc-error", (p) => showToast(`Error: ${p.message}`)),
      api.listen<ServerStatus>("server-status", mergeStatus),
    ];
    return () => {
      unlisteners.forEach((u) => u.then((fn) => fn()).catch(() => {}));
    };
  }, [showToast]);

  // Apply theme + motion preferences to <html> live.
  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", settings.theme || "dark");
    root.setAttribute("data-style", settings.uiStyle || "aurora");
    root.setAttribute("data-bg", settings.background || "pulsing");
    root.setAttribute("data-anim", settings.background === "static" ? "off" : "on");
  }, [settings.theme, settings.background, settings.uiStyle]);

  const activeAccount = useCallback(
    () => store.accounts.find((a) => a.uuid === store.active_uuid) ?? store.accounts[0],
    [store]
  );

  const patchSettings = useCallback((s: Partial<Settings>) => {
    setSettings((prev) => ({ ...prev, ...s }));
  }, []);

  const saveSettings = useCallback(async () => {
    await api.saveSettings(settings);
  }, [settings]);

  // Patch + persist immediately (for toggles/theme).
  const persistSettings = useCallback((s: Partial<Settings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...s };
      api.saveSettings(next).catch(() => {});
      return next;
    });
  }, []);

  const refreshInstances = useCallback(async () => {
    setInstances(await api.listInstances());
  }, []);
  const saveInstanceCfg = useCallback(
    async (cfg: InstanceConfig) => {
      await api.saveInstance(cfg);
      await refreshInstances();
    },
    [refreshInstances]
  );
  const deleteInstanceCfg = useCallback(
    async (id: string) => {
      await api.deleteInstance(id);
      await refreshInstances();
      showToast("Instance removed");
    },
    [refreshInstances, showToast]
  );
  const playInstance = useCallback(
    async (id: string, server?: string) => {
      setBusy(true);
      setProgress({ stage: "Preparing", total: 0, done: 0, fraction: 0 });
      const launchAt = Math.floor(Date.now() / 1000);
      try {
        const msg = await api.instancePlay(id, server ?? null);
        showToast(msg);
        // Watch for an early crash; if one lands, surface the analyzer.
        const name = instances.find((i) => i.id === id)?.name ?? "Instance";
        [7000, 16000, 28000].forEach((delay) =>
          setTimeout(() => {
            api
              .analyzeCrash(id)
              .then((c) => {
                if (c.found && c.when >= launchAt - 2 && c.culpritFile) setCrashTarget({ id, name });
              })
              .catch(() => {});
          }, delay)
        );
        // Remember a joined session so Home can offer "Rejoin".
        if (server) {
          const inst = instances.find((i) => i.id === id);
          try {
            localStorage.setItem(
              "aurora:lastSession",
              JSON.stringify({ id, name: inst?.name ?? "server", address: server })
            );
          } catch {
            /* ignore */
          }
        }
      } catch (e) {
        showToast(`Error: ${e}`);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [showToast, instances]
  );
  const createFromPack = useCallback(
    async (source: string, projectId: string, title: string, icon: string | null) => {
      setBusy(true);
      setProgress({ stage: "Preparing", total: 0, done: 0, fraction: 0 });
      try {
        await api.createInstanceFromPack(source, projectId, title, icon);
        await refreshInstances();
        showToast(`Installed ${title}`);
      } catch (e) {
        showToast(`Error: ${e}`);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [refreshInstances, showToast]
  );
  const openInstanceFolder = useCallback(async (id: string) => {
    await api.openInstanceFolder(id);
  }, []);
  const openServerFolder = useCallback(async (id: string) => {
    await api.openServerFolder(id);
  }, []);

  const refreshServers = useCallback(async () => {
    setServers(await api.listServers());
  }, []);
  const createServerFromPack = useCallback(
    async (source: string, projectId: string, title: string, icon: string | null) => {
      setBusy(true);
      setProgress({ stage: "Preparing", total: 0, done: 0, fraction: 0 });
      try {
        await api.createServerFromPack(source, projectId, title, icon);
        await refreshServers();
        showToast(`Installed ${title}`);
      } catch (e) {
        showToast(`Error: ${e}`);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [refreshServers, showToast]
  );
  const saveServerCfg = useCallback(
    async (cfg: ServerConfig) => {
      await api.saveServer(cfg);
      await refreshServers();
    },
    [refreshServers]
  );
  const deleteServerCfg = useCallback(
    async (id: string) => {
      await api.deleteServer(id);
      await refreshServers();
      showToast("Server deleted");
    },
    [refreshServers, showToast]
  );
  const startServer = useCallback(
    async (id: string) => {
      setConsoleServerId(id); // open the dashboard immediately to show progress/logs
      try {
        await api.serverStart(id);
      } catch (e) {
        showToast(`Error: ${e}`);
      }
    },
    [showToast]
  );
  const stopServer = useCallback(
    async (id: string) => {
      try {
        await api.serverStop(id);
      } catch (e) {
        showToast(`Error: ${e}`);
      }
    },
    [showToast]
  );
  const openConsole = useCallback((id: string) => setConsoleServerId(id), []);
  const closeConsole = useCallback(() => setConsoleServerId(null), []);
  const openContent = useCallback((t: ContentTarget) => setContentTarget(t), []);
  const closeContent = useCallback(() => setContentTarget(null), []);
  const openInventory = useCallback((t: ContentTarget) => setInventoryTarget(t), []);
  const closeInventory = useCallback(() => setInventoryTarget(null), []);
  const openBackups = useCallback((t: ContentTarget) => setBackupTarget(t), []);
  const closeBackups = useCallback(() => setBackupTarget(null), []);
  const openConfigEditor = useCallback((t: ContentTarget) => setConfigTarget(t), []);
  const closeConfigEditor = useCallback(() => setConfigTarget(null), []);
  const [crashTarget, setCrashTarget] = useState<{ id: string; name: string } | null>(null);
  const openCrash = useCallback((id: string, name: string) => setCrashTarget({ id, name }), []);
  const closeCrash = useCallback(() => setCrashTarget(null), []);
  const openUpgrade = useCallback((t: ContentTarget) => setUpgradeTarget(t), []);
  const closeUpgrade = useCallback(() => setUpgradeTarget(null), []);
  const upgrade = useCallback(
    async (kind: "instance" | "server", id: string, newVersion: string) => {
      setBusy(true);
      setProgress({ stage: `Upgrading to ${newVersion}`, total: 0, done: 0, fraction: 0 });
      try {
        // 0) auto-backup worlds before a version change (best-effort safety net)
        try {
          await api.createBackup(kind, id);
        } catch {
          /* no worlds yet / nothing to back up */
        }
        // 1) bump the config version so content resolves against the new version
        if (kind === "instance") {
          const it = instances.find((i) => i.id === id);
          if (it) await api.saveInstance({ ...it, version: newVersion });
        } else {
          const s = servers.find((x) => x.id === id);
          if (s) await api.saveServer({ ...s, version: newVersion });
        }
        // 2) update every installed mod/content to the new version
        const results = await api.checkUpdates(kind, id, newVersion);
        let updated = 0;
        let incompatible = 0;
        for (const r of results) {
          if (r.status === "update") {
            try {
              await api.applyUpdate(kind, id, r.item.projectId, newVersion);
              updated++;
            } catch {
              incompatible++;
            }
          } else if (r.status === "incompatible") {
            incompatible++;
          }
        }
        await refreshInstances();
        await refreshServers();
        const tail = results.length ? ` · ${updated} mod(s) updated${incompatible ? `, ${incompatible} need attention` : ""}` : "";
        showToast(`Upgraded to ${newVersion}${tail}`);
      } catch (e) {
        showToast(`Error: ${e}`);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [instances, servers, refreshInstances, refreshServers, showToast]
  );

  const refreshAccounts = useCallback(async () => {
    setStore(await api.listAccounts());
  }, []);
  const refreshGames = useCallback(async () => {
    setGames(await api.detectGames());
  }, []);

  const play = useCallback(
    async ({ loader, version, server }: { loader: string; version: string; server?: string }) => {
      const account = activeAccount();
      if (!account) {
        showToast("Add an account first");
        return;
      }
      patchSettings({ lastLoader: loader, lastVersion: version });
      api.saveSettings({ ...settings, lastLoader: loader, lastVersion: version }).catch(() => {});
      setBusy(true);
      setProgress({ stage: "Preparing", total: 0, done: 0, fraction: 0 });
      try {
        const msg = await api.playMinecraft({
          version,
          loader,
          account,
          memoryMb: settings.maxMemoryMb,
          server: server ?? null,
        });
        showToast(msg);
      } catch (e) {
        showToast(`Error: ${e}`);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [activeAccount, patchSettings, settings, showToast]
  );

  const addOffline = useCallback(
    async (name: string) => {
      if (!name.trim()) return;
      await api.addOfflineAccount(name.trim());
      await refreshAccounts();
      showToast(`Added offline account “${name.trim()}”`);
    },
    [refreshAccounts, showToast]
  );

  const runLogin = useCallback(
    async (fn: () => Promise<{ username: string }>) => {
      setBusy(true);
      setLoginPrompt(null);
      setLoginError(null);
      try {
        const acct = await fn();
        await refreshAccounts();
        showToast(`Signed in as ${acct.username}`);
      } catch (e) {
        showToast(`Login failed: ${e}`);
      } finally {
        setBusy(false);
        setLoginPrompt(null);
      }
    },
    [refreshAccounts, showToast]
  );

  // Default "no-code" flow: opens the browser, captures the redirect.
  const microsoftLogin = useCallback(() => runLogin(api.microsoftLogin), [runLogin]);
  // Fallback flow: shows a short code to type at microsoft.com/link.
  const microsoftLoginCode = useCallback(() => runLogin(api.microsoftLoginCode), [runLogin]);

  const setActive = useCallback(
    async (uuid: string) => {
      await api.setActiveAccount(uuid);
      await refreshAccounts();
    },
    [refreshAccounts]
  );
  const removeAccount = useCallback(
    async (uuid: string) => {
      await api.removeAccount(uuid);
      await refreshAccounts();
    },
    [refreshAccounts]
  );

  const launchSkyrim = useCallback(
    async (mode: string) => {
      try {
        await api.launchSkyrim(mode);
        showToast("Launching Skyrim…");
      } catch (e) {
        showToast(`Error: ${e}`);
      }
    },
    [showToast]
  );
  const launchEldenRing = useCallback(
    async (mode: string) => {
      try {
        await api.launchEldenRing(mode);
        showToast(mode === "seamless" ? "Launching Seamless Co-op…" : "Launching Elden Ring…");
      } catch (e) {
        showToast(`Error: ${e}`);
      }
    },
    [showToast]
  );
  const setEldenRingPassword = useCallback(
    async (pw: string) => {
      try {
        await api.setEldenRingPassword(pw);
        await refreshGames();
        showToast("Co-op password saved");
      } catch (e) {
        showToast(`Error: ${e}`);
      }
    },
    [refreshGames, showToast]
  );
  const launchCyberpunk = useCallback(
    async (mode: string) => {
      try {
        await api.launchCyberpunk(mode);
        showToast(mode === "mp" ? "Launching CyberpunkMP…" : "Launching Cyberpunk 2077…");
      } catch (e) {
        showToast(`Error: ${e}`);
      }
    },
    [showToast]
  );
  /** One-click install of a game tool (Seamless, SKSE, ME2, CET, CyberpunkMP). */
  const installTool = useCallback(
    async (tool: string) => {
      setBusy(true);
      setProgress({ stage: "Downloading", total: 0, done: 0, fraction: 0 });
      try {
        const msg = await api.installGameTool(tool);
        await refreshGames();
        showToast(msg);
      } catch (e) {
        showToast(`Error: ${e}`);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [refreshGames, showToast]
  );
  /** Guided Skyrim Together install (zip from the user's Downloads). */
  const installTogether = useCallback(async () => {
    setBusy(true);
    try {
      const msg = await api.installSkyrimTogether();
      await refreshGames();
      showToast(msg);
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(false);
    }
  }, [refreshGames, showToast]);

  const value = useMemo<Launcher>(
    () => ({
      versions,
      settings,
      settingsLoaded,
      store,
      games,
      paths,
      busy,
      progress,
      loginPrompt,
      loginError,
      toast,
      instances,
      servers,
      serverStatuses,
      consoleServerId,
      contentTarget,
      inventoryTarget,
      backupTarget,
      configTarget,
      upgradeTarget,
      systemRamMb,
      activeAccount,
      showToast,
      patchSettings,
      saveSettings,
      persistSettings,
      refreshInstances,
      saveInstanceCfg,
      deleteInstanceCfg,
      playInstance,
      createFromPack,
      createServerFromPack,
      openInstanceFolder,
      openServerFolder,
      refreshServers,
      saveServerCfg,
      deleteServerCfg,
      startServer,
      stopServer,
      openConsole,
      closeConsole,
      openContent,
      closeContent,
      openInventory,
      closeInventory,
      openBackups,
      closeBackups,
      openConfigEditor,
      closeConfigEditor,
      crashTarget,
      openCrash,
      closeCrash,
      openUpgrade,
      closeUpgrade,
      upgrade,
      refreshAccounts,
      refreshGames,
      play,
      addOffline,
      microsoftLogin,
      microsoftLoginCode,
      setActive,
      removeAccount,
      launchSkyrim,
      launchEldenRing,
      launchCyberpunk,
      setEldenRingPassword,
      installTool,
      installTogether,
    }),
    [
      versions,
      settings,
      settingsLoaded,
      store,
      games,
      paths,
      busy,
      progress,
      loginPrompt,
      loginError,
      toast,
      instances,
      servers,
      serverStatuses,
      consoleServerId,
      contentTarget,
      inventoryTarget,
      backupTarget,
      configTarget,
      upgradeTarget,
      systemRamMb,
      activeAccount,
      showToast,
      patchSettings,
      saveSettings,
      persistSettings,
      refreshInstances,
      saveInstanceCfg,
      deleteInstanceCfg,
      playInstance,
      createFromPack,
      createServerFromPack,
      openInstanceFolder,
      openServerFolder,
      refreshServers,
      saveServerCfg,
      deleteServerCfg,
      startServer,
      stopServer,
      openConsole,
      closeConsole,
      openContent,
      closeContent,
      openInventory,
      closeInventory,
      openBackups,
      closeBackups,
      openConfigEditor,
      closeConfigEditor,
      crashTarget,
      openCrash,
      closeCrash,
      openUpgrade,
      closeUpgrade,
      upgrade,
      refreshAccounts,
      refreshGames,
      play,
      addOffline,
      microsoftLogin,
      microsoftLoginCode,
      setActive,
      removeAccount,
      launchSkyrim,
      launchEldenRing,
      launchCyberpunk,
      setEldenRingPassword,
      installTool,
      installTogether,
    ]
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

/** Persisted server lists (per game) via localStorage. */
export function loadServers(key: string) {
  try {
    return JSON.parse(localStorage.getItem(`servers:${key}`) || "[]");
  } catch {
    return [];
  }
}
export function saveServers(key: string, list: unknown) {
  localStorage.setItem(`servers:${key}`, JSON.stringify(list));
}
