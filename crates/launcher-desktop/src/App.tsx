import { useEffect, useRef, useState } from "react";

import { LauncherProvider, useLauncher } from "./store";
import { windowAction, checkAppUpdate, applyAppUpdate, playStats, type UpdateInfo, type PlayRecord } from "./lib/api";
import type { GameKey } from "./lib/types";
import { Icon, AuroraMark, Markdown, Avatar } from "./components/ui";
import { AccountMenu } from "./components/AccountMenu";
import { ServerDashboard } from "./components/ServerConsole";
import { InstancesPanel, MinecraftServers, UpgradeModal } from "./components/MinecraftPanels";
import { SkinsPanel, ContentOverlay } from "./components/ContentPanel";
import { InventoryEditor } from "./components/InventoryEditor";
import { BackupsModal } from "./components/BackupsModal";
import { ConfigEditor } from "./components/ConfigEditor";
import { CrashModal } from "./components/CrashModal";
import { Onboarding } from "./components/Onboarding";
import {
  HomePanel,
  SkyrimPlay,
  SkyrimCoop,
  SkyrimMods,
  EldenRingPlay,
  EldenRingCoop,
  EldenRingCheats,
  CyberpunkPlay,
  CyberpunkCoop,
  CyberpunkMods,
} from "./components/GamePanels";
import { AccountsPanel, SettingsPanel } from "./components/SystemPanels";
import { NetworkPanel } from "./components/NetworkPanel";

type Section = GameKey | "home" | "accounts" | "settings" | "network";
type IconFn = (p: { size?: number }) => JSX.Element;

const GAMES: GameKey[] = ["minecraft", "skyrim", "eldenring", "cyberpunk"];

/* Each game's identity in the shell: name, maker line and rail icon. */
const GAME_META: Record<GameKey, { name: string; short: string; maker: string; icon: IconFn }> = {
  minecraft: { name: "Minecraft", short: "MC", maker: "Mojang Studios", icon: Icon.minecraft },
  skyrim: { name: "Skyrim", short: "SK", maker: "The Elder Scrolls V", icon: Icon.skyrim },
  eldenring: { name: "Elden Ring", short: "ER", maker: "FromSoftware", icon: Icon.elden },
  cyberpunk: { name: "Cyberpunk 2077", short: "CP", maker: "CD Projekt Red", icon: Icon.cyberpunk },
};

/* The functions each game exposes on the left rail. The first is the primary one. */
const GAME_FUNCS: Record<GameKey, { tab: string; icon: IconFn }[]> = {
  minecraft: [
    { tab: "Play", icon: Icon.gamepad },
    { tab: "Servers", icon: Icon.server },
    { tab: "Skins", icon: Icon.shirt },
  ],
  skyrim: [
    { tab: "Play", icon: Icon.gamepad },
    { tab: "Co-op", icon: Icon.coop },
    { tab: "Mods", icon: Icon.mods },
  ],
  eldenring: [
    { tab: "Play", icon: Icon.gamepad },
    { tab: "Co-op", icon: Icon.coop },
    { tab: "Cheats", icon: Icon.sparkles },
  ],
  cyberpunk: [
    { tab: "Play", icon: Icon.gamepad },
    { tab: "Co-op", icon: Icon.coop },
    { tab: "Mods", icon: Icon.mods },
  ],
};

const SECTION_TITLE: Record<string, string> = { home: "Aurora", accounts: "Accounts", settings: "Settings", network: "Aurora Net" };
const SECTION_KICKER: Record<string, string> = {
  home: "Welcome back",
  accounts: "Minecraft accounts",
  settings: "Launcher",
  network: "Play together, no port forwarding",
};

interface HeroStat {
  icon: IconFn;
  label: string;
  value: string;
}
interface HeroInfo {
  title: string;
  kicker: string;
  play?: { label: string; disabled: boolean; onClick: () => void };
  stats: HeroStat[];
}

const ready = (b: boolean | undefined) => (b ? "Ready" : "Not installed");

/* Title-bar pill: shows running servers; hover lists them to open a dashboard. */
function ServerPill() {
  const { serverStatuses, openConsole } = useLauncher();
  const [open, setOpen] = useState(false);
  const closeTimer = useRef<number | null>(null);
  const list = Object.values(serverStatuses);
  if (list.length === 0) return null;
  const totalPlayers = list.reduce((a, s) => a + s.players, 0);

  // Keep the menu open while moving between the pill and the dropdown — a short
  // grace period bridges the gap between them so it doesn't close mid-move.
  const show = () => {
    if (closeTimer.current) window.clearTimeout(closeTimer.current);
    setOpen(true);
  };
  const hide = () => {
    closeTimer.current = window.setTimeout(() => setOpen(false), 240);
  };

  return (
    <div style={{ position: "relative" }} onMouseEnter={show} onMouseLeave={hide}>
      <button className="server-ind" onClick={() => list[0] && openConsole(list[0].id)}>
        <span className="live" /> {totalPlayers} online · {list.length} server{list.length > 1 ? "s" : ""}
      </button>
      {open && (
        <div className="acct-menu surface" style={{ width: 290, top: 40 }} onMouseEnter={show} onMouseLeave={hide}>
          {list.map((s) => (
            <button key={s.id} className="menu-item" onClick={() => openConsole(s.id)}>
              <span className="av" style={{ width: 28, height: 28 }}>
                <Icon.minecraft size={15} />
              </span>
              <span style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: 13.5 }}>{s.name}</div>
                <div style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
                  {s.version} · {s.players}/{s.maxPlayers} · :{s.port}
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

/* Title-bar pill: other Aurora PCs on the LAN; pick one to control it. */
function RemotePill() {
  const { netPeers, remoteTarget, connecting, connectRemote, disconnectRemote } = useLauncher();
  const [open, setOpen] = useState(false);
  const closeTimer = useRef<number | null>(null);
  if (netPeers.length === 0 && !remoteTarget) return null;
  const show = () => {
    if (closeTimer.current) window.clearTimeout(closeTimer.current);
    setOpen(true);
  };
  const hide = () => {
    closeTimer.current = window.setTimeout(() => setOpen(false), 240);
  };
  const label = remoteTarget ? `Controlling ${remoteTarget.name}` : `${netPeers.length} PC${netPeers.length > 1 ? "s" : ""}`;
  return (
    <div style={{ position: "relative" }} onMouseEnter={show} onMouseLeave={hide}>
      <button className={`server-ind ${remoteTarget ? "remote-on" : ""}`} onClick={() => setOpen((o) => !o)}>
        <Icon.coop size={14} /> {label}
      </button>
      {open && (
        <div className="acct-menu surface" style={{ width: 280, top: 40 }} onMouseEnter={show} onMouseLeave={hide}>
          <button className={`menu-item ${!remoteTarget ? "on" : ""}`} onClick={() => { void disconnectRemote(); setOpen(false); }}>
            <span className="av" style={{ width: 28, height: 28 }}><Icon.host size={15} /></span>
            <span style={{ flex: 1 }}>
              <div style={{ fontWeight: 600, fontSize: 13.5 }}>This PC</div>
              <div style={{ color: "var(--text-mute)", fontSize: 11.5 }}>{remoteTarget ? "Return here" : "You're here"}</div>
            </span>
            {!remoteTarget && <Icon.check size={15} />}
          </button>
          {netPeers.map((p) => (
            <button key={p.id} className={`menu-item ${remoteTarget?.id === p.id ? "on" : ""}`} onClick={() => { void connectRemote(p); setOpen(false); }}>
              <span className="av" style={{ width: 28, height: 28 }}><Icon.coop size={15} /></span>
              <span style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: 13.5 }}>{p.name}</div>
                <div style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
                  {p.ip}{connecting?.id === p.id ? " · connecting…" : ""}
                </div>
              </span>
              {remoteTarget?.id === p.id && <Icon.check size={15} />}
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


/* What the hero shows for the current section: title, the Play pill and status chips. */
function useHero(section: Section, lastInstanceId: string | null, onContinue: (r: PlayRecord) => void, recent: PlayRecord | null): HeroInfo {
  const { games, instances, servers, serverStatuses, busy, playInstance, launchSkyrim, launchEldenRing, launchCyberpunk, activeAccount, netPeers } =
    useLauncher();
  const running = Object.values(serverStatuses).filter((s) => s.running).length;

  switch (section) {
    case "minecraft": {
      const inst = instances.find((i) => i.id === lastInstanceId) ?? instances[0];
      const acct = activeAccount();
      return {
        title: "Minecraft",
        kicker: GAME_META.minecraft.maker,
        play: { label: "Play", disabled: busy || !inst, onClick: () => inst && void playInstance(inst.id) },
        stats: [
          { icon: Icon.clock, label: "Selected instance", value: inst ? (inst.name.includes(inst.version) ? inst.name : `${inst.name} · ${inst.version}`) : "None yet" },
          { icon: Icon.mods, label: "Instances", value: `${instances.length} installed` },
          { icon: Icon.server, label: "Servers", value: `${running} of ${servers.length} running` },
          { icon: Icon.user, label: "Account", value: acct ? acct.username : "Signed out" },
        ],
      };
    }
    case "skyrim": {
      const g = games?.skyrim;
      return {
        title: "Skyrim Special Edition",
        kicker: GAME_META.skyrim.maker,
        play: { label: "Play", disabled: !g?.installed, onClick: () => void launchSkyrim("vanilla") },
        stats: [
          { icon: Icon.folder, label: "Install", value: g?.installed ? (g.source === "epic" ? "Epic Games" : "Steam") : "Not found" },
          { icon: Icon.mods, label: "SKSE", value: ready(g?.has_skse) },
          { icon: Icon.coop, label: "Skyrim Together", value: ready(g?.has_skyrim_together) },
        ],
      };
    }
    case "eldenring": {
      const g = games?.eldenRing;
      return {
        title: "Elden Ring",
        kicker: GAME_META.eldenring.maker,
        play: { label: "Play", disabled: !g?.installed, onClick: () => void launchEldenRing("vanilla") },
        stats: [
          { icon: Icon.folder, label: "Install", value: g?.installed ? "Steam" : "Not found" },
          { icon: Icon.coop, label: "Seamless Co-op", value: ready(g?.has_seamless_coop) },
          { icon: Icon.mods, label: "Mod Engine 2", value: ready(g?.has_mod_engine) },
        ],
      };
    }
    case "cyberpunk": {
      const g = games?.cyberpunk;
      return {
        title: "Cyberpunk 2077",
        kicker: GAME_META.cyberpunk.maker,
        play: { label: "Play", disabled: !g?.installed, onClick: () => void launchCyberpunk("vanilla") },
        stats: [
          { icon: Icon.folder, label: "Install", value: g?.installed ? (g.source === "epic" ? "Epic Games" : "Steam") : "Not found" },
          { icon: Icon.coop, label: "CyberpunkMP", value: ready(g?.has_mp) },
          { icon: Icon.terminal, label: "Cyber Engine Tweaks", value: ready(g?.has_cet) },
        ],
      };
    }
    case "home": {
      const installed = 1 + [games?.skyrim, games?.eldenRing, games?.cyberpunk].filter((g) => g?.installed).length;
      return {
        title: "Aurora",
        kicker: SECTION_KICKER.home,
        play: recent ? { label: "Continue", disabled: busy, onClick: () => onContinue(recent) } : undefined,
        stats: [
          ...(recent ? [{ icon: Icon.clock, label: "Last played", value: recent.name }] : []),
          { icon: Icon.gamepad, label: "Library", value: `${installed} games` },
          { icon: Icon.server, label: "Servers", value: `${running} running` },
          { icon: Icon.globe, label: "Nearby PCs", value: `${netPeers.length} online` },
        ],
      };
    }
    default:
      return { title: SECTION_TITLE[section], kicker: SECTION_KICKER[section], stats: [] };
  }
}

function Shell() {
  const { toast, consoleServerId, closeConsole, contentTarget, closeContent, inventoryTarget, closeInventory, configTarget, closeConfigEditor, playInstance, settings, settingsLoaded, connecting, remoteTarget, disconnectRemote, activeAccount, busy } =
    useLauncher();
  const [activeGame, setActiveGame] = useState<GameKey>("minecraft");
  const [section, setSection] = useState<Section>("home");
  const [tabs, setTabs] = useState<Record<GameKey, string>>({
    minecraft: "Play",
    skyrim: "Play",
    eldenring: "Play",
    cyberpunk: "Play",
  });
  const [recents, setRecents] = useState<PlayRecord[]>([]);

  // Play history drives the hero: the Minecraft "selected instance" is the one
  // played most recently, and Home's Continue picks up the last game.
  useEffect(() => {
    if (busy) return;
    playStats()
      .then((r) => setRecents([...r].sort((a, b) => b.lastPlayed - a.lastPlayed)))
      .catch(() => {});
  }, [busy]);
  const lastInstanceId = recents.find((r) => r.kind === "instance")?.key.replace(/^instance:/, "") ?? null;

  // Apply the saved "open to" view once, after settings have loaded from disk.
  const appliedDefault = useRef(false);
  useEffect(() => {
    if (!settingsLoaded || appliedDefault.current) return;
    appliedDefault.current = true;
    const view = settings.defaultView || "home";
    const [sec, tab] = view.split(":");
    if ((GAMES as string[]).includes(sec)) {
      setActiveGame(sec as GameKey);
      setSection(sec as Section);
      if (tab && GAME_FUNCS[sec as GameKey].some((f) => f.tab === tab)) {
        setTabs((prev) => ({ ...prev, [sec as GameKey]: tab }));
      }
    } else if (sec && sec !== "home") {
      setSection(sec as Section);
    }
  }, [settingsLoaded, settings.defaultView]);

  const isGame = (GAMES as string[]).includes(section);

  // Games carry their own accent; everything else (home, settings, accounts)
  // wears the launcher's aurora identity.
  useEffect(() => {
    document.documentElement.setAttribute("data-game", isGame ? section : "aurora");
  }, [section, isGame]);

  // Liquid-glass pointer lensing: expose the cursor position to CSS so glass
  // controls brighten under it (used by the ::after radial in liquidglass).
  useEffect(() => {
    const SEL = ".btn, .btn-play, .select-btn, .lrow, .game-tile, .rail-btn, .tab, .seg button, .acct-chip, .fn-btn, .game-dot";
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
  const openFunc = (t: string) => {
    setTabs((prev) => ({ ...prev, [activeGame]: t }));
    setSection(activeGame);
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
  const currentTab = isGame ? tabs[section as GameKey] : "";
  // Accounts are Minecraft/Microsoft — only surface the chip there.
  const showAccount = section === "minecraft" || section === "accounts";
  const hero = useHero(section, lastInstanceId, continuePlay, recents[0] ?? null);
  const acct = activeAccount();
  const funcs = GAME_FUNCS[activeGame];
  const [primary, ...rest] = funcs;
  const fnActive = (t: string) => section === activeGame && currentTab === t;
  const SectionIcon = isGame ? GAME_META[section as GameKey].icon : null;

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <AuroraMark size={22} />
          Aurora
          <span className="brand-sub">Launcher</span>
        </div>
        <div className="spacer" data-tauri-drag-region />
        <UpdatePill />
        <ServerPill />
        <RemotePill />
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

      <div className="deck">
        <div className="stage-art" data-art={isGame ? section : "aurora"} aria-hidden />
        {/* Left: what you can do with the selected game. */}
        <nav className="fn-rail" aria-label={`${GAME_META[activeGame].name} functions`}>
          <button className="fn-mark" data-g={activeGame} onClick={() => selectGame(activeGame)} title={GAME_META[activeGame].name}>
            {(() => {
              const G = GAME_META[activeGame].icon;
              return <G size={24} />;
            })()}
          </button>
          <div className="fn-spacer" />
          <FnBtn primary label={primary.tab} active={fnActive(primary.tab)} onClick={() => openFunc(primary.tab)}>
            <primary.icon size={26} />
          </FnBtn>
          <div className="fn-group">
            {rest.map((f) => (
              <FnBtn key={f.tab} label={f.tab} active={fnActive(f.tab)} onClick={() => openFunc(f.tab)}>
                <f.icon size={21} />
              </FnBtn>
            ))}
          </div>
          <div className="fn-spacer" />
          <div className="rail-foot">{GAME_META[activeGame].short}</div>
        </nav>

        {/* Centre: hero + the selected function. */}
        <main className="stage">
          <div className="stage-scroll">
            <header className="hero3">
              <div className="hero3-top">
                <span className="kicker">{hero.kicker}</span>
                <div className="row" style={{ gap: 12, alignItems: "center" }}>
                  {showAccount && <AccountMenu onManage={() => setSection("accounts")} />}
                  {SectionIcon && (
                    <span className="hero3-badge">
                      <span>{GAME_META[section as GameKey].name}</span>
                      <span className="hero3-badge-ico">
                        <SectionIcon size={20} />
                      </span>
                    </span>
                  )}
                </div>
              </div>
              <h1 className="hero3-title">{hero.title}</h1>
              {(hero.play || hero.stats.length > 0) && (
                <div className="hero3-actions">
                  {hero.play && (
                    <button className="pill-play" disabled={hero.play.disabled} onClick={hero.play.onClick}>
                      <span className="pill-play-ico">
                        <Icon.play size={13} />
                      </span>
                      {hero.play.label}
                    </button>
                  )}
                  {hero.stats.map((s) => (
                    <div className="stat" key={s.label}>
                      <s.icon size={20} />
                      <div>
                        <div className="stat-label">{s.label}</div>
                        <div className="stat-value">{s.value}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </header>

            <div className="view" key={`${section}:${currentTab}`}>
              <Panel section={section} tab={currentTab} onSelectGame={selectGame} onContinue={continuePlay} onRejoin={rejoin} />
            </div>
          </div>
        </main>

        {/* Right: your games, plus launcher-wide places (account, network, settings). */}
        <aside className="game-rail" aria-label="Games">
          <button
            className={`game-me ${section === "accounts" ? "active" : ""}`}
            onClick={() => setSection("accounts")}
            title={acct ? acct.username : "Sign in"}
          >
            {acct ? <Avatar account={acct} size={44} /> : <Icon.user size={22} />}
            <span className={`game-me-dot ${acct ? "on" : ""}`} />
          </button>
          <div className="fn-spacer" />
          <div className="game-group">
            <GameDot label="Home" active={section === "home"} onClick={() => setSection("home")}>
              <AuroraMark size={22} />
            </GameDot>
            {GAMES.map((g) => {
              const G = GAME_META[g].icon;
              return (
                <GameDot key={g} game={g} label={GAME_META[g].name} active={section === g} onClick={() => selectGame(g)}>
                  <G size={22} />
                </GameDot>
              );
            })}
          </div>
          <div className="fn-spacer" />
          <div className="fn-group">
            <FnBtn label="Aurora Net" active={section === "network"} onClick={() => setSection("network")}>
              <Icon.globe size={21} />
            </FnBtn>
            <FnBtn label="Settings" active={section === "settings"} onClick={() => setSection("settings")}>
              <Icon.gear size={21} />
            </FnBtn>
          </div>
          <div className="rail-foot">Aurora</div>
        </aside>
      </div>

      {consoleServerId && <ServerDashboard id={consoleServerId} onClose={closeConsole} />}
      {contentTarget && <ContentOverlay target={contentTarget} onClose={closeContent} />}
      {inventoryTarget && <InventoryEditor target={inventoryTarget} onClose={closeInventory} />}
      <BackupsModal />
      {configTarget && <ConfigEditor target={configTarget} onClose={closeConfigEditor} />}
      <CrashModal />
      <UpgradeModal />
      <Onboarding />

      {/* Remote control: "warp" transition + a persistent banner while controlling. */}
      {connecting && (
        <div className="remote-warp">
          <div className="remote-warp-ring" />
          <div className="remote-warp-name">Connecting to {connecting.name}…</div>
        </div>
      )}
      {remoteTarget && !connecting && (
        <button className="remote-banner" onClick={() => void disconnectRemote()}>
          <span className="net-dot on" /> Controlling <b>{remoteTarget.name}</b> — click to return to this PC
        </button>
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

function FnBtn({
  children,
  label,
  active,
  primary,
  onClick,
}: {
  children: React.ReactNode;
  label: string;
  active: boolean;
  primary?: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`fn-btn ${primary ? "primary" : ""} ${active ? "active" : ""}`} onClick={onClick} title={label} aria-label={label}>
      {children}
    </button>
  );
}

function GameDot({
  children,
  label,
  game,
  active,
  onClick,
}: {
  children: React.ReactNode;
  label: string;
  game?: GameKey;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`game-dot ${active ? "active" : ""}`} data-g={game ?? "aurora"} onClick={onClick} title={label} aria-label={label}>
      {children}
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
      if (tab === "Cheats") return <EldenRingCheats />;
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
