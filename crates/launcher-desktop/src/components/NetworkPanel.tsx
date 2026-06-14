import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { ServerConfig, InstanceConfig } from "../lib/types";
import { Field, Icon, Pill, Select } from "./ui";

/** Derive a modpack reference from an instance created from a pack (its id is
 *  `<source>-<projectId>`). Returns null for hand-made instances. */
function packFromInstance(inst: InstanceConfig): api.PackRef | null {
  const m = /^(modrinth|curseforge|ftb|technic)-(.+)$/.exec(inst.id);
  if (!m) return null;
  return { source: m[1], projectId: m[2], title: inst.name, icon: inst.icon ?? null };
}

/** Aurora Net — built-in Tailscale VPN so friends can connect with no port
 *  forwarding. Phase 1 (setup/connect), Phase 2 (join), Phase 3 (host/share). */
export function NetworkPanel() {
  const { showToast, playInstance } = useLauncher();
  const [status, setStatus] = useState<api.VpnStatus | null>(null);
  const [hasToken, setHasToken] = useState(false);
  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [instances, setInstances] = useState<InstanceConfig[]>([]);
  const [peers, setPeers] = useState<api.Peer[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const [joinCode, setJoinCode] = useState("");
  const [joined, setJoined] = useState<api.JoinPayload | null>(null);

  const [token, setToken] = useState("");
  const [shareName, setShareName] = useState("");
  const [sharePort, setSharePort] = useState(25565);
  const [shareGame, setShareGame] = useState("minecraft");
  const [sharePackId, setSharePackId] = useState("");
  const [lockAccess, setLockAccess] = useState(true);
  const [shareCode, setShareCode] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const st = await api.vpnStatus();
      setStatus(st);
      setHasToken((await api.vpnConfig()).hasToken);
      if (st.running) setPeers(await api.vpnPeers());
    } catch {
      /* ignore */
    }
  };

  useEffect(() => {
    void refresh();
    api.listServers().then(setServers).catch(() => {});
    api.listInstances().then(setInstances).catch(() => {});
  }, []);

  // Instances that came from a modpack can be bundled into a Minecraft invite.
  const packInstances = instances.filter((i) => packFromInstance(i));

  const copy = (text: string, what: string) => {
    void navigator.clipboard?.writeText(text);
    showToast(`${what} copied`);
  };

  const run = async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(null);
    }
  };

  const install = () =>
    run("install", async () => {
      await api.vpnInstall();
      showToast("Aurora Net installed");
      await refresh();
    });

  const connect = () =>
    run("connect", async () => {
      const url = await api.vpnLogin();
      if (url) {
        await api.openUrl(url);
        showToast("Approve the sign-in in your browser…");
        // Poll a few times for the connection to come up.
        for (let i = 0; i < 20; i++) {
          await new Promise((r) => setTimeout(r, 1500));
          const s = await api.vpnStatus();
          setStatus(s);
          if (s.running) {
            showToast("Connected to Aurora Net");
            break;
          }
        }
      } else {
        showToast("Connected to Aurora Net");
      }
      await refresh();
    });

  const disconnect = () =>
    run("disconnect", async () => {
      await api.vpnDisconnect();
      await refresh();
    });

  const join = () =>
    run("join", async () => {
      const payload = await api.vpnJoin(joinCode);
      setJoined(payload);
      setJoinCode("");
      await refresh();
      showToast(`Joined ${payload.name}`);
      // One-click: if the invite carries a modpack, build a matching instance.
      if (payload.pack) {
        const have = instances.some((i) => i.id === `${payload.pack!.source}-${payload.pack!.projectId}`);
        if (!have) {
          showToast(`Installing ${payload.pack.title}…`);
          try {
            await api.createInstanceFromPack(
              payload.pack.source,
              payload.pack.projectId,
              payload.pack.title,
              payload.pack.icon ?? null
            );
            api.listInstances().then(setInstances).catch(() => {});
            showToast(`${payload.pack.title} ready — launch it from Minecraft`);
          } catch (e) {
            showToast(`Couldn't install the modpack: ${e}`);
          }
        }
      }
    });

  const saveToken = () =>
    run("token", async () => {
      await api.vpnSetToken(token);
      setToken("");
      setHasToken(true);
      showToast("Tailscale token saved");
    });

  const share = () =>
    run("share", async () => {
      const inst = packInstances.find((i) => i.id === sharePackId);
      const pack = shareGame === "minecraft" && inst ? packFromInstance(inst) : null;
      const code = await api.vpnShare({
        name: shareName || "My server",
        port: sharePort,
        game: shareGame,
        configureAccess: lockAccess,
        pack,
      });
      setShareCode(code);
      copy(code, "Join code");
    });

  const installed = status?.installed ?? false;
  const running = status?.running ?? false;

  return (
    <div className="sect">
      <div className="sect-head">
        <div className="sect-title">Connection</div>
        <button className="btn ghost" onClick={() => void refresh()}>
          <Icon.refresh size={15} /> Refresh
        </button>
      </div>
      <p className="muted">
        Play together with no port forwarding — Aurora puts you and your friends on a private,
        encrypted network (powered by Tailscale). Works for Minecraft servers, Skyrim Together,
        Elden Ring co-op and more.
      </p>

      {/* Connection status */}
      <div className="surface" style={{ padding: "14px 16px", marginTop: 6 }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div className="row" style={{ gap: 10, alignItems: "center" }}>
            <span className={`net-dot ${running ? "on" : ""}`} />
            <div>
              <div style={{ fontWeight: 700 }}>
                {!installed ? "Not set up" : running ? "Connected" : "Disconnected"}
              </div>
              <div className="muted" style={{ fontSize: 12.5 }}>
                {running && status?.ip ? (
                  <>
                    Your address: <b>{status.ip}</b>
                  </>
                ) : installed ? (
                  "Connect to join or host games"
                ) : (
                  "A one-time setup gets you connected"
                )}
              </div>
            </div>
          </div>
          <div className="row" style={{ gap: 8 }}>
            {!installed && (
              <button className="btn-play" disabled={busy !== null} onClick={install}>
                <Icon.host size={16} /> {busy === "install" ? "Installing…" : "Set up Aurora Net"}
              </button>
            )}
            {installed && !running && (
              <button className="btn-play" disabled={busy !== null} onClick={connect}>
                <Icon.coop size={16} /> {busy === "connect" ? "Connecting…" : "Connect"}
              </button>
            )}
            {installed && running && status?.ip && (
              <>
                <button className="btn" onClick={() => copy(status.ip!, "Your address")}>
                  <Icon.copy size={15} /> Copy address
                </button>
                <button className="btn ghost" disabled={busy !== null} onClick={disconnect}>
                  Disconnect
                </button>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Friends on Aurora Net */}
      {running && peers.length > 0 && (
        <>
          <div className="sect-head" style={{ marginTop: 20 }}>
            <div className="sect-title">Friends on Aurora Net</div>
            <Pill tone="ok">{peers.filter((p) => p.online).length} online</Pill>
          </div>
          <div className="col" style={{ gap: 2 }}>
            {peers.map((p) => (
              <div className="lrow" key={p.ip ?? p.name}>
                <span className={`net-dot ${p.online ? "on" : ""}`} style={{ marginLeft: 4 }} />
                <div className="grow" style={{ marginLeft: 10 }}>
                  <div className="name">
                    {p.name} {p.me && <Pill>you</Pill>}
                  </div>
                  <div className="sub">{p.ip ?? "—"} · {p.online ? "online" : "offline"}</div>
                </div>
                {p.ip && !p.me && (
                  <button className="btn ghost" onClick={() => copy(p.ip!, "Address")}>
                    <Icon.copy size={14} /> Copy IP
                  </button>
                )}
              </div>
            ))}
          </div>
        </>
      )}

      {/* Join a friend */}
      <div className="sect-head" style={{ marginTop: 20 }}>
        <div className="sect-title">Join a friend</div>
      </div>
      <p className="muted">Paste the join code a friend gave you. You'll connect to their game.</p>
      <div className="row wrap" style={{ alignItems: "flex-end" }}>
        <Field label="Join code">
          <input
            className="input"
            style={{ minWidth: 360 }}
            value={joinCode}
            onChange={(e) => setJoinCode(e.target.value)}
            placeholder="aurora-net:…"
          />
        </Field>
        <button className="btn-play" disabled={busy !== null || !joinCode.trim()} onClick={join}>
          <Icon.coop size={16} /> {busy === "join" ? "Joining…" : "Join"}
        </button>
      </div>
      {joined && (
        <div className="surface" style={{ padding: "12px 16px", marginTop: 8 }}>
          <div style={{ fontWeight: 700 }}>
            Connected to {joined.name} <Pill tone="ok">ready</Pill>
          </div>
          <p className="muted" style={{ margin: "6px 0 0" }}>
            {joined.game === "minecraft" ? (
              <>
                In Minecraft, add a server with address <b>{joined.ip}:{joined.port}</b>.
              </>
            ) : (
              <>
                Use host address <b>{joined.ip}</b> (port {joined.port}) in the game's co-op screen.
              </>
            )}
          </p>
          <div className="row" style={{ marginTop: 8, gap: 8 }}>
            {joined.game === "minecraft" && joined.pack && (
              <button
                className="btn-play"
                onClick={() => void playInstance(`${joined.pack!.source}-${joined.pack!.projectId}`, `${joined.ip}:${joined.port}`)}
              >
                <Icon.play size={15} /> Launch &amp; join
              </button>
            )}
            <button className="btn" onClick={() => copy(`${joined.ip}:${joined.port}`, "Address")}>
              <Icon.copy size={15} /> Copy address
            </button>
          </div>
        </div>
      )}

      {/* Host a server */}
      <div className="sect-head" style={{ marginTop: 22 }}>
        <div className="sect-title">Host a game</div>
      </div>
      {!hasToken ? (
        <>
          <p className="muted">
            To create join codes, connect Aurora to your Tailscale account once. Create an access
            token at{" "}
            <button className="linklike" onClick={() => void api.openUrl("https://login.tailscale.com/admin/settings/keys")}>
              login.tailscale.com → Settings → Keys
            </button>{" "}
            (an <b>API access token</b>) and paste it here. Stored only on this PC.
          </p>
          <div className="row wrap" style={{ alignItems: "flex-end" }}>
            <Field label="Tailscale API access token">
              <input
                className="input"
                style={{ minWidth: 360 }}
                type="password"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="tskey-api-…"
              />
            </Field>
            <button className="btn" disabled={busy !== null || !token.trim()} onClick={saveToken}>
              <Icon.check size={15} /> {busy === "token" ? "Saving…" : "Save token"}
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="muted">
            Generate a one-time join code for a server and send it to friends. They click Join and
            connect — and (if locked) can reach <i>only</i> that server, nothing else on your network.
          </p>
          <div className="row wrap" style={{ alignItems: "flex-end" }}>
            {servers.length > 0 && (
              <Field label="Pick a server">
                <Select
                  minWidth={220}
                  value={shareName ? `${shareName}|${sharePort}` : "__custom__"}
                  onChange={(v) => {
                    if (v === "__custom__") {
                      setShareName("");
                      return;
                    }
                    const s = servers.find((x) => `${x.name}|${x.port}` === v);
                    if (s) {
                      setShareName(s.name);
                      setSharePort(s.port);
                      setShareGame("minecraft");
                    }
                  }}
                  options={[
                    { value: "__custom__", label: "Custom…" },
                    ...servers.map((s) => ({ value: `${s.name}|${s.port}`, label: `${s.name} (MC :${s.port})` })),
                  ]}
                />
              </Field>
            )}
            <Field label="Name">
              <input className="input" value={shareName} onChange={(e) => setShareName(e.target.value)} placeholder="My server" />
            </Field>
            <Field label="Port">
              <input
                className="input"
                style={{ width: 100 }}
                type="number"
                value={sharePort}
                onChange={(e) => setSharePort(Number(e.target.value) || 0)}
              />
            </Field>
            <Field label="Game">
              <Select
                minWidth={170}
                value={shareGame}
                onChange={setShareGame}
                options={[
                  { value: "minecraft", label: "Minecraft" },
                  { value: "skyrim", label: "Skyrim Together" },
                  { value: "eldenring", label: "Elden Ring co-op" },
                  { value: "cyberpunk", label: "Cyberpunk MP" },
                ]}
              />
            </Field>
            {shareGame === "minecraft" && packInstances.length > 0 && (
              <Field label="Bundle modpack (one-click join)">
                <Select
                  minWidth={220}
                  value={sharePackId}
                  onChange={setSharePackId}
                  options={[
                    { value: "", label: "None" },
                    ...packInstances.map((i) => ({ value: i.id, label: i.name })),
                  ]}
                />
              </Field>
            )}
          </div>
          {shareGame === "minecraft" && sharePackId && (
            <p className="muted" style={{ fontSize: 12, marginTop: 8 }}>
              Friends who use this code will auto-install <b>{packInstances.find((i) => i.id === sharePackId)?.name}</b> and join — no manual setup.
            </p>
          )}
          <label className="row" style={{ gap: 8, marginTop: 10, alignItems: "center", cursor: "pointer" }}>
            <input type="checkbox" checked={lockAccess} onChange={(e) => setLockAccess(e.target.checked)} />
            <span>Lock guests to only this server (sets your Tailscale access rules)</span>
          </label>
          <button
            className="btn-play"
            style={{ marginTop: 12 }}
            disabled={busy !== null || !running}
            onClick={share}
            title={running ? "" : "Connect to Aurora Net first"}
          >
            <Icon.link size={16} /> {busy === "share" ? "Creating…" : "Create join code"}
          </button>
          {shareCode && (
            <div className="surface" style={{ padding: "12px 16px", marginTop: 10 }}>
              <div style={{ fontWeight: 700, marginBottom: 6 }}>Send this to your friends</div>
              <div className="codebox" style={{ wordBreak: "break-all", userSelect: "all" }}>
                aurora-net:{shareCode}
              </div>
              <button className="btn" style={{ marginTop: 8 }} onClick={() => copy(`aurora-net:${shareCode}`, "Join code")}>
                <Icon.copy size={15} /> Copy join code
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
