import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { LauncherProvider, useLauncher } from "./store";
import { windowAction } from "./lib/api";
import type { GameKey } from "./lib/types";
import { Icon } from "./components/ui";
import { AccountMenu } from "./components/AccountMenu";
import { ServerDashboard } from "./components/ServerConsole";
import { InstancesPanel, MinecraftServers, UpgradeModal } from "./components/MinecraftPanels";
import { SkinsPanel, ContentOverlay } from "./components/ContentPanel";
import { InventoryEditor } from "./components/InventoryEditor";
import {
  SkyrimPlay,
  SkyrimCoop,
  SkyrimMods,
  EldenRingPlay,
  EldenRingCoop,
} from "./components/GamePanels";
import { AccountsPanel, SettingsPanel } from "./components/SystemPanels";

type Section = GameKey | "accounts" | "settings";

const GAME_TABS: Record<GameKey, string[]> = {
  minecraft: ["Play", "Servers", "Skins"],
  skyrim: ["Play", "Co-op", "Mods"],
  eldenring: ["Play", "Co-op"],
};

const SECTION_TITLE: Record<string, string> = { accounts: "Accounts", settings: "Settings" };

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

function Shell() {
  const { toast, consoleServerId, closeConsole, contentTarget, closeContent, inventoryTarget, closeInventory } =
    useLauncher();
  const [activeGame, setActiveGame] = useState<GameKey>("minecraft");
  const [section, setSection] = useState<Section>("minecraft");
  const [tabs, setTabs] = useState<Record<GameKey, string>>({
    minecraft: "Play",
    skyrim: "Play",
    eldenring: "Play",
  });

  useEffect(() => {
    document.documentElement.setAttribute("data-game", activeGame);
  }, [activeGame]);

  const selectGame = (g: GameKey) => {
    setActiveGame(g);
    setSection(g);
  };
  const setTab = (t: string) => setTabs((prev) => ({ ...prev, [activeGame]: t }));

  const isGame = section === "minecraft" || section === "skyrim" || section === "eldenring";
  const currentTab = isGame ? tabs[section as GameKey] : "";
  // Accounts are Minecraft/Microsoft — only surface the chip there.
  const showAccount = section === "minecraft" || section === "accounts";

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <span className="brand-mark" />
          Aurora
          <span className="brand-sub">Launcher</span>
        </div>
        <div className="spacer" data-tauri-drag-region />
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
          <RailBtn label="MC" active={section === "minecraft"} onClick={() => selectGame("minecraft")}>
            <Icon.minecraft size={24} />
          </RailBtn>
          <RailBtn label="Skyrim" active={section === "skyrim"} onClick={() => selectGame("skyrim")}>
            <Icon.skyrim size={24} />
          </RailBtn>
          <RailBtn label="Elden" active={section === "eldenring"} onClick={() => selectGame("eldenring")}>
            <Icon.elden size={24} />
          </RailBtn>
          <div className="rail-spacer" />
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
              <Panel section={section} tab={currentTab} />
            </div>
          </div>
        </main>
      </div>

      {consoleServerId && <ServerDashboard id={consoleServerId} onClose={closeConsole} />}
      {contentTarget && <ContentOverlay target={contentTarget} onClose={closeContent} />}
      {inventoryTarget && <InventoryEditor target={inventoryTarget} onClose={closeInventory} />}
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

function Panel({ section, tab }: { section: Section; tab: string }) {
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
      return <EldenRingPlay />;
    case "accounts":
      return <AccountsPanel />;
    case "settings":
      return <SettingsPanel />;
  }
}

function Aurora() {
  return (
    <>
      <div className="aurora">
        <i className="b1" />
        <i className="b2" />
        <i className="b3" />
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
