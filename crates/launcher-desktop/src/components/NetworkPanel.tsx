import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { InstanceConfig } from "../lib/types";
import { Field, Icon, Pill } from "./ui";

const FRIEND_COLORS = ["#34d399", "#22d3ee", "#a78bfa", "#f59e0b", "#f472b6", "#60a5fa", "#f87171", "#2dd4bf"];
function friendColor(s: string): string {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return FRIEND_COLORS[h % FRIEND_COLORS.length];
}
function friendInitials(s: string): string {
  return s.replace(/[^a-zA-Z0-9]/g, "").slice(0, 2).toUpperCase() || "?";
}
/** Round avatar with initials, tinted from the name. */
function FriendAvatar({ name }: { name: string }) {
  const c = friendColor(name);
  return (
    <div
      className="friend-av"
      style={{ background: `${c}22`, border: `1.5px solid ${c}`, color: c }}
    >
      {friendInitials(name)}
    </div>
  );
}

/** Aurora Net — built-in Tailscale VPN so friends can connect with no port
 *  forwarding. Phase 1 (setup/connect), Phase 2 (join), Phase 3 (host/share). */
export function NetworkPanel() {
  const { showToast, playInstance } = useLauncher();
  const [status, setStatus] = useState<api.VpnStatus | null>(null);
  const [hasToken, setHasToken] = useState(false);
  const [instances, setInstances] = useState<InstanceConfig[]>([]);
  const [peers, setPeers] = useState<api.Peer[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const [joinCode, setJoinCode] = useState("");
  const [joined, setJoined] = useState<api.JoinPayload | null>(null);

  const [token, setToken] = useState("");
  const [friendCode, setFriendCode] = useState<string | null>(null);

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
    api.listInstances().then(setInstances).catch(() => {});
  }, []);

  // Live-refresh the friends list while connected, so a friend joining (or
  // going offline) shows up within a few seconds without a manual refresh.
  useEffect(() => {
    if (!status?.running) return;
    const t = setInterval(() => {
      api.vpnPeers().then(setPeers).catch(() => {});
    }, 4000);
    return () => clearInterval(t);
  }, [status?.running]);

  // Once connected with a token, load (or mint) the persistent friend code.
  useEffect(() => {
    if (status?.running && hasToken && friendCode === null) {
      api.vpnFriendCode(false).then(setFriendCode).catch(() => {});
    }
  }, [status?.running, hasToken, friendCode]);

  const me = peers.find((p) => p.me);
  const friends = peers.filter((p) => !p.me);
  const friendsOnline = friends.filter((p) => p.online).length;

  const scrollTo = (id: string) =>
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });

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

  const repairFirewall = () =>
    run("repair", async () => {
      const applied = await api.repairAuroraNet();
      showToast(applied ? "Aurora Net is allowed through the firewall" : "Firewall was already set up");
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
      // Mint the friend code right away so it's ready to share.
      try {
        setFriendCode(await api.vpnFriendCode(false));
      } catch {
        /* needs to be connected; the section will offer it once online */
      }
    });

  const makeFriendCode = (regenerate: boolean) =>
    run("friendcode", async () => {
      const code = await api.vpnFriendCode(regenerate);
      setFriendCode(code);
      if (regenerate) showToast("New friend code created");
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
                <button
                  className="btn"
                  disabled={busy !== null}
                  onClick={repairFirewall}
                  title="Allow servers you host through the Windows Firewall on Aurora Net (one-time)"
                >
                  <Icon.host size={15} /> {busy === "repair" ? "Fixing…" : "Fix hosting"}
                </button>
                <button className="btn ghost" disabled={busy !== null} onClick={disconnect}>
                  Disconnect
                </button>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Friends — the proof you're on the same network. Always shown when
          connected, with a teaching empty state when it's just you. */}
      {running && (
        <>
          <div className="sect-head" style={{ marginTop: 22 }}>
            <div className="sect-title">Friends</div>
            <Pill tone={friendsOnline > 0 ? "ok" : "default"}>
              {friends.length === 0 ? "just you" : `${friendsOnline}/${friends.length} online`}
            </Pill>
          </div>

          {friends.length === 0 ? (
            <div className="surface" style={{ padding: 18 }}>
              <div style={{ fontWeight: 700, marginBottom: 4 }}>It's just you on Aurora Net</div>
              <p className="muted" style={{ marginTop: 0 }}>
                To play together, <b>one</b> of you hosts and the others join the same network — then
                everyone shows up here. Swapping <code className="md-code">100.x</code> addresses won't
                work on its own; you have to be on the <i>same</i> network.
              </p>
              <div className="row wrap" style={{ gap: 10, marginTop: 10 }}>
                <button className="btn-play" onClick={() => scrollTo("aurora-host")}>
                  <Icon.host size={15} /> I'm hosting — create an invite
                </button>
                <button className="btn" onClick={() => scrollTo("aurora-join")}>
                  <Icon.coop size={15} /> I'm joining — paste an invite
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="surface" style={{ padding: "10px 14px", marginBottom: 8, fontSize: 13 }}>
                ✓ You're connected with <b>{friends.length}</b> {friends.length === 1 ? "friend" : "friends"} on
                Aurora Net. Use a friend's address in-game (or hand them yours) to play.
              </div>
              <div className="col" style={{ gap: 6 }}>
                {me && (
                  <div className="friend-row surface">
                    <FriendAvatar name={me.name} />
                    <span className={`net-dot ${me.online ? "on" : ""}`} />
                    <div className="grow">
                      <div className="name">
                        {me.name} <Pill>you</Pill>
                      </div>
                      <div className="sub">{me.ip ?? "—"} · this PC</div>
                    </div>
                    {me.ip && (
                      <button className="btn ghost" onClick={() => copy(me.ip!, "Your address")}>
                        <Icon.copy size={14} /> Copy
                      </button>
                    )}
                  </div>
                )}
                {friends.map((p) => (
                  <div className="friend-row surface" key={p.ip ?? p.name}>
                    <FriendAvatar name={p.name} />
                    <span className={`net-dot ${p.online ? "on" : ""}`} />
                    <div className="grow">
                      <div className="name">{p.name}</div>
                      <div className="sub">
                        {p.ip ?? "—"} · {p.online ? "online" : "offline"}
                      </div>
                    </div>
                    {p.ip && (
                      <button className="btn ghost" disabled={!p.online} onClick={() => copy(p.ip!, "Address")}>
                        <Icon.copy size={14} /> Copy IP
                      </button>
                    )}
                  </div>
                ))}
              </div>
            </>
          )}
        </>
      )}

      {/* Join a friend — paste their friend code to get onto their network */}
      <div className="sect-head" id="aurora-join" style={{ marginTop: 20 }}>
        <div className="sect-title">Join a friend</div>
      </div>
      <p className="muted">
        Got a friend code from someone? Paste it to join their network. You'll show up in each other's
        Friends list — then connect to their address in whichever game's co-op tab.
      </p>
      <div className="row wrap" style={{ alignItems: "flex-end" }}>
        <Field label="Friend code">
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
            You're on {joined.name}'s network <Pill tone="ok">connected</Pill>
          </div>
          <p className="muted" style={{ margin: "6px 0 0" }}>
            {joined.game === "network" || !joined.port ? (
              <>
                You'll see {joined.name} in your Friends list above. Open the game you both want to play
                and connect to their address (shown in that game's co-op tab).
              </>
            ) : joined.game === "minecraft" ? (
              <>In Minecraft, add a server with address <b>{joined.ip}:{joined.port}</b>.</>
            ) : (
              <>Use host address <b>{joined.ip}</b> (port {joined.port}) in the game's co-op screen.</>
            )}
          </p>
          {joined.ip && (
            <div className="row" style={{ marginTop: 8, gap: 8 }}>
              {joined.game === "minecraft" && joined.pack && (
                <button
                  className="btn-play"
                  onClick={() => void playInstance(`${joined.pack!.source}-${joined.pack!.projectId}`, `${joined.ip}:${joined.port}`)}
                >
                  <Icon.play size={15} /> Launch &amp; join
                </button>
              )}
              <button
                className="btn"
                onClick={() => copy(joined.port ? `${joined.ip}:${joined.port}` : joined.ip, "Address")}
              >
                <Icon.copy size={15} /> Copy address
              </button>
            </div>
          )}
        </div>
      )}

      {/* Your friend code — share once, friends join your network */}
      <div className="sect-head" id="aurora-host" style={{ marginTop: 22 }}>
        <div className="sect-title">Your friend code</div>
      </div>
      {!hasToken ? (
        <>
          <p className="muted">
            One-time setup so Aurora can make your friend code: connect it to your Tailscale account.
            Create an <b>API access token</b> at{" "}
            <button className="linklike" onClick={() => void api.openUrl("https://login.tailscale.com/admin/settings/keys")}>
              login.tailscale.com → Settings → Keys
            </button>{" "}
            and paste it here. Stored only on this PC.
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
      ) : !running ? (
        <p className="muted">Connect to Aurora Net above, then your friend code appears here.</p>
      ) : (
        <>
          <p className="muted">
            Share this one code with your friends. They paste it under <b>Join a friend</b> and they're
            on your network — for good. The same code works for everyone, so you only need to send it
            once.
          </p>
          {friendCode ? (
            <div className="surface" style={{ padding: "12px 16px", marginTop: 4 }}>
              <div className="codebox" style={{ wordBreak: "break-all", userSelect: "all" }}>
                aurora-net:{friendCode}
              </div>
              <div className="row" style={{ marginTop: 8, gap: 8 }}>
                <button className="btn-play" onClick={() => copy(`aurora-net:${friendCode}`, "Friend code")}>
                  <Icon.copy size={15} /> Copy friend code
                </button>
                <button className="btn ghost" disabled={busy !== null} onClick={() => makeFriendCode(true)}>
                  <Icon.refresh size={15} /> {busy === "friendcode" ? "Working…" : "New code"}
                </button>
              </div>
            </div>
          ) : (
            <button
              className="btn-play"
              style={{ marginTop: 4 }}
              disabled={busy !== null}
              onClick={() => makeFriendCode(false)}
            >
              <Icon.link size={16} /> {busy === "friendcode" ? "Creating…" : "Create friend code"}
            </button>
          )}
          <p className="muted" style={{ fontSize: 12, marginTop: 10 }}>
            Servers you host are allowed through your firewall automatically (one admin prompt the first
            time). Trouble connecting? Hit <b>Fix hosting</b> at the top.
          </p>
        </>
      )}
    </div>
  );
}
