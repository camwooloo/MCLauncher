import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { LauncherProvider, useLauncher } from "./store";
import { windowAction, checkAppUpdate, applyAppUpdate, type UpdateInfo, type PlayRecord } from "./lib/api";
import type { GameKey } from "./lib/types";
import { Icon, AuroraMark, Markdown } from "./components/ui";
import { AccountMenu } from "./components/AccountMenu";
import { ServerDashboard } from "./components/ServerConsole";
import { InstancesPanel, MinecraftServers, UpgradeModal } from "./components/MinecraftPanels";
import { SkinsPanel, ContentOverlay } from "./components/ContentPanel";
import { InventoryEditor } from "./components/InventoryEditor";
import { BackupsModal } from "./components/BackupsModal";
import { ConfigEditor } from "./components/ConfigEditor";
import {
  HomePanel,
  SkyrimPlay,
  SkyrimCoop,
  SkyrimMods,
  EldenRingPlay,
  EldenRingCoop,
  EldenRingMods,
  CyberpunkPlay,
  CyberpunkCoop,
  CyberpunkMods,
} from "./components/GamePanels";
import { AccountsPanel, SettingsPanel } from "./components/SystemPanels";
import { NetworkPanel } from "./components/NetworkPanel";

type Section = GameKey | "home" | "accounts" | "settings" | "network";

const GAME_TABS: Record<GameKey, string[]> = {
  minecraft: ["Play", "Servers", "Skins"],
  skyrim: ["Play", "Co-op", "Mods"],
  eldenring: ["Play", "Co-op", "Mods"],
  cyberpunk: ["Play", "Co-op", "Mods"],
};

const SECTION_TITLE: Record<string, string> = { home: "Home", accounts: "Accounts", settings: "Settings", network: "Aurora Net" };

/* Sliding "liquid" tab indicator. */
function TabBar({
  tabs,
  active,
  onSelect,
}: {
  tabs: string[];
  active: string;
  onSelect: (t: string) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [ind, setInd] = useState({ left: 0, width: 0, ready: false });
  useLayoutEffect(() => {
    const el = ref.current?.querySelector(".tab.active") as HTMLElement | null;
    if (el) setInd({ left: el.offsetLeft, width: el.offsetWidth, ready: true });
  }, [active, tabs]);
  return (
    <div className="tabbar" ref={ref}>
      <div
        className="tab-ind"
        style={{ left: 0, transform: `translateX(${ind.left}px)`, width: ind.width, opacity: ind.ready ? 1 : 0 }}
      />
      {tabs.map((t) => (
        <button key={t} className={`tab ${t === active ? "active" : ""}`} onClick={() => onSelect(t)}>
          {t}
        </button>
      ))}
    </div>
  );
}

/* Title-bar pill: shows running servers; hover lists them to open a dashboard. */
function ServerPill() {
  const { serverStatuses, openConsole } = useLauncher();
  const [open, setOpen] = useState(false);
  const list = Object.values(serverStatuses);
  if (list.length === 0) return null;
  const totalPlayers = list.reduce((a, s) => a + s.players, 0);

  return (
    <div style={{ position: "relative" }} onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)}>
      <button className="server-ind" onClick={() => list[0] && openConsole(list[0].id)}>
        <span className="live" /> {totalPlayers} online · {list.length} server{list.length > 1 ? "s" : ""}
      </button>
      {open && (
        <div className="acct-menu surface" style={{ width: 290, top: 44 }}>
          {list.map((s) => (
            <button key={s.id} className="menu-item" onClick={() => openConsole(s.id)}>
              <span className="av" style={{ width: 28, height: 28 }}>
                <Icon.minecraft size={15} />
              </span>
              <span style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: 13.5 }}>{s.name}</div>
                <div style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
                  Minecraft · {s.players}/{s.maxPlayers} · :{s.port}
                </div>
              </span>
              <Icon.terminal size={15} />
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/* Title-bar pill: appears when a newer release exists; opens patch notes + 1-click update. */
function UpdatePill() {
  const { showToast } = useLauncher();
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    checkAppUpdate()
      .then((i) => setInfo(i))
      .catch(() => {});
  }, []);

  if (!info) return null;

  const update = async () => {
    setBusy(true);
    try {
      await applyAppUpdate(info.downloadUrl);
      showToast("Downloading update — the installer will open, then Aurora restarts…");
    } catch (e) {
      showToast(`${e}`);
      setBusy(false);
    }
  };

  return (
    <>
      <button className="update-ind" onClick={() => setOpen(true)} title={`Update to v${info.version}`}>
        <Icon.upgrade size={15} /> Update available
      </button>
      {open && (
        <div className="dash-overlay" onClick={() => setOpen(false)}>
          <div className="update-modal surface" onClick={(e) => e.stopPropagation()}>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
              <div>
                <div className="eyebrow">Update available</div>
                <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 22 }}>
                  Aurora Launcher v{info.version}
                </div>
                <div className="sub" style={{ color: "var(--text-mute)" }}>You're on v{info.current}</div>
              </div>
              <button className="btn ghost" onClick={() => setOpen(false)}>
                <Icon.close size={16} /> Close
              </button>
            </div>
            <div className="patch-notes">
              <Markdown source={info.notes.trim() || "No release notes provided."} />
            </div>
            <div className="row" style={{ justifyContent: "flex-end", gap: 10, marginTop: 4 }}>
              <button className="btn ghost" onClick={() => setOpen(false)}>
                Later
              </button>
              <button className="btn-play" disabled={busy} onClick={update}>
                <Icon.upgrade size={16} /> {busy ? "Updating…" : "Update now"}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function Shell() {
  const { toast, consoleServerId, closeConsole, contentTarget, closeContent, inventoryTarget, closeInventory, configTarget, closeConfigEditor, playInstance, settings, settingsLoaded } =
    useLauncher();
  const [activeGame, setActiveGame] = useState<GameKey>("minecraft");
  const [section, setSection] = useState<Section>("home");
  const [tabs, setTabs] = useState<Record<GameKey, string>>({
    minecraft: "Play",
    skyrim: "Play",
    eldenring: "Play",
    cyberpunk: "Play",
  });

  // Apply the saved "open to" view once, after settings have loaded from disk.
  const appliedDefault = useRef(false);
  useEffect(() => {
    if (!settingsLoaded || appliedDefault.current) return;
    appliedDefault.current = true;
    const view = settings.defaultView || "home";
    const [sec, tab] = view.split(":");
    const games: GameKey[] = ["minecraft", "skyrim", "eldenring", "cyberpunk"];
    if ((games as string[]).includes(sec)) {
      setActiveGame(sec as GameKey);
      setSection(sec as Section);
      if (tab && GAME_TABS[sec as GameKey].includes(tab)) {
        setTabs((prev) => ({ ...prev, [sec as GameKey]: tab }));
      }
    } else if (sec && sec !== "home") {
      setSection(sec as Section);
    }
  }, [settingsLoaded, settings.defaultView]);

  const isGame =
    section === "minecraft" || section === "skyrim" || section === "eldenring" || section === "cyberpunk";

  // Games carry their own accent; everything else (home, settings, accounts)
  // wears the launcher's aurora identity.
  useEffect(() => {
    document.documentElement.setAttribute("data-game", isGame ? section : "aurora");
  }, [section, isGame]);

  // Liquid-glass pointer lensing: expose the cursor position to CSS so glass
  // controls brighten under it (used by the ::after radial in liquidglass).
  useEffect(() => {
    const SEL = ".btn, .btn-play, .select-btn, .lrow, .game-tile, .rail-btn, .tab, .seg button, .acct-chip";
    const onMove = (e: PointerEvent) => {
      const el = (e.target as HTMLElement | null)?.closest?.(SEL) as HTMLElement | null;
      if (!el) return;
      const r = el.getBoundingClientRect();
      el.style.setProperty("--mx", `${(((e.clientX - r.left) / r.width) * 100).toFixed(1)}%`);
      el.style.setProperty("--my", `${(((e.clientY - r.top) / r.height) * 100).toFixed(1)}%`);
    };
    document.addEventListener("pointermove", onMove, { passive: true });
    return () => document.removeEventListener("pointermove", onMove);
  }, []);

  const selectGame = (g: GameKey) => {
    setActiveGame(g);
    setSection(g);
  };
  // Home "Continue": instances launch straight away; games open their section.
  const continuePlay = (r: PlayRecord) => {
    if (r.kind === "instance") {
      selectGame("minecraft");
      void playInstance(r.key.replace(/^instance:/, ""));
    } else {
      selectGame(r.kind as GameKey);
    }
  };
  const rejoin = (s: { id: string; name: string; address: string }) => {
    selectGame("minecraft");
    void playInstance(s.id, s.address);
  };
  const setTab = (t: string) => setTabs((prev) => ({ ...prev, [activeGame]: t }));
  const currentTab = isGame ? tabs[section as GameKey] : "";
  // Accounts are Minecraft/Microsoft — only surface the chip there.
  const showAccount = section === "minecraft" || section === "accounts";

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <AuroraMark size={24} />
          Aurora
          <span className="brand-sub">Launcher</span>
        </div>
        <div className="spacer" data-tauri-drag-region />
        <UpdatePill />
        <ServerPill />
        <button className="win-btn" onClick={() => windowAction("minimize")}>
          <Icon.min size={16} />
        </button>
        <button className="win-btn" onClick={() => windowAction("toggleMaximize")}>
          <Icon.max size={13} />
        </button>
        <button className="win-btn close" onClick={() => windowAction("close")}>
          <Icon.close size={15} />
        </button>
      </div>

      <div className="shell">
        <nav className="rail">
          <RailBtn label="Home" active={section === "home"} onClick={() => setSection("home")}>
            <Icon.home size={24} />
          </RailBtn>
          <RailBtn label="MC" active={section === "minecraft"} onClick={() => selectGame("minecraft")}>
            <Icon.minecraft size={24} />
          </RailBtn>
          <RailBtn label="Skyrim" active={section === "skyrim"} onClick={() => selectGame("skyrim")}>
            <Icon.skyrim size={24} />
          </RailBtn>
          <RailBtn label="Elden" active={section === "eldenring"} onClick={() => selectGame("eldenring")}>
            <Icon.elden size={24} />
          </RailBtn>
          <RailBtn label="2077" active={section === "cyberpunk"} onClick={() => selectGame("cyberpunk")}>
            <Icon.cyberpunk size={24} />
          </RailBtn>
          <div className="rail-spacer" />
          <button
            className={`rail-btn rail-mini ${section === "network" ? "active" : ""}`}
            onClick={() => setSection("network")}
            title="Aurora Net — play together with no port forwarding"
          >
            <Icon.coop size={20} />
          </button>
          <button
            className={`rail-btn rail-mini ${section === "settings" ? "active" : ""}`}
            onClick={() => setSection("settings")}
            title="Settings"
          >
            <Icon.gear size={19} />
          </button>
        </nav>

        <main className="content">
          <div className="topbar">
            {isGame ? (
              <TabBar tabs={GAME_TABS[section as GameKey]} active={currentTab} onSelect={setTab} />
            ) : (
              <div className="sect-title" style={{ fontSize: 20 }}>
                {SECTION_TITLE[section]}
              </div>
            )}
            {showAccount ? <AccountMenu onManage={() => setSection("accounts")} /> : <span />}
          </div>

          <div className="panel-scroll">
            <div className="view" key={`${section}:${currentTab}`}>
              <Panel section={section} tab={currentTab} onSelectGame={selectGame} onContinue={continuePlay} onRejoin={rejoin} />
            </div>
          </div>
        </main>
      </div>

      {consoleServerId && <ServerDashboard id={consoleServerId} onClose={closeConsole} />}
      {contentTarget && <ContentOverlay target={contentTarget} onClose={closeContent} />}
      {inventoryTarget && <InventoryEditor target={inventoryTarget} onClose={closeInventory} />}
      <BackupsModal />
      {configTarget && <ConfigEditor target={configTarget} onClose={closeConfigEditor} />}
      <UpgradeModal />
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

function RailBtn({
  children,
  label,
  active,
  onClick,
}: {
  children: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`rail-btn ${active ? "active" : ""}`} onClick={onClick} title={label}>
      {children}
      <span className="label">{label}</span>
    </button>
  );
}

function Panel({
  section,
  tab,
  onSelectGame,
  onContinue,
  onRejoin,
}: {
  section: Section;
  tab: string;
  onSelectGame: (g: GameKey) => void;
  onContinue: (r: PlayRecord) => void;
  onRejoin: (s: { id: string; name: string; address: string }) => void;
}) {
  switch (section) {
    case "minecraft":
      if (tab === "Servers") return <MinecraftServers />;
      if (tab === "Skins") return <SkinsPanel />;
      return <InstancesPanel />;
    case "skyrim":
      if (tab === "Co-op") return <SkyrimCoop />;
      if (tab === "Mods") return <SkyrimMods />;
      return <SkyrimPlay />;
    case "eldenring":
      if (tab === "Co-op") return <EldenRingCoop />;
      if (tab === "Mods") return <EldenRingMods />;
      return <EldenRingPlay />;
    case "cyberpunk":
      if (tab === "Co-op") return <CyberpunkCoop />;
      if (tab === "Mods") return <CyberpunkMods />;
      return <CyberpunkPlay />;
    case "home":
      return <HomePanel onSelect={onSelectGame} onContinue={onContinue} onRejoin={onRejoin} />;
    case "accounts":
      return <AccountsPanel />;
    case "settings":
      return <SettingsPanel />;
    case "network":
      return <NetworkPanel />;
  }
}

function Aurora() {
  return (
    <>
      <div className="aurora">
        <i className="b1" />
        <i className="b2" />
        <i className="b3" />
        <i className="b4" />
      </div>
      <div className="grain" />
    </>
  );
}

export default function App() {
  return (
    <>
      <Aurora />
      <LauncherProvider>
        <Shell />
      </LauncherProvider>
    </>
  );
}
