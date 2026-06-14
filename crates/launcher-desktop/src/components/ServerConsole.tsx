import { useEffect, useRef, useState } from "react";

import * as api from "../lib/api";
import type { ServerLog, ServerStatus } from "../lib/types";
import { Icon, HostAddress } from "./ui";
import { useLauncher } from "../store";

function Stat({ label, value, accent }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="surface" style={{ padding: "12px 16px", borderRadius: 16, minWidth: 92 }}>
      <div style={{ fontSize: 11, color: "var(--text-mute)", letterSpacing: 0.4 }}>{label}</div>
      <div
        style={{
          fontFamily: "var(--font-display)",
          fontWeight: 700,
          fontSize: 19,
          color: accent ? "var(--accent)" : "var(--text)",
        }}
      >
        {value}
      </div>
    </div>
  );
}

/** In-app dashboard overlay for one hosted server. */
export function ServerDashboard({ id, onClose }: { id: string; onClose: () => void }) {
  const { showToast } = useLauncher();
  const [lines, setLines] = useState<ServerLog[]>([]);
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [cmd, setCmd] = useState("");
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setLines([]);
    const unlisteners = [
      api.listen<ServerLog>("server-log", (p) => {
        if (p.id === id) setLines((l) => [...l.slice(-800), p]);
      }),
      api.listen<ServerStatus>("server-status", (s) => {
        if (s.id === id) setStatus(s);
      }),
    ];
    api
      .serversStatus()
      .then((list) => {
        const s = list.find((x) => x.id === id);
        if (s) setStatus(s);
      })
      .catch(() => {});
    return () => unlisteners.forEach((u) => u.then((f) => f()).catch(() => {}));
  }, [id]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  const running = status?.running ?? false;
  const mem = status?.memoryMb
    ? status.memoryMb >= 1024
      ? `${(status.memoryMb / 1024).toFixed(1)} GB`
      : `${status.memoryMb} MB`
    : "—";

  const send = () => {
    if (!cmd.trim()) return;
    api.serverCommand(id, cmd.trim());
    setLines((l) => [...l, { id, line: `> ${cmd.trim()}`, err: false }]);
    setCmd("");
  };

  const cls = (l: ServerLog) =>
    l.err || /ERROR|severe|exception|FAILED/i.test(l.line)
      ? "err"
      : /WARN/i.test(l.line)
      ? "warn"
      : /joined the game|left the game|Done \(/i.test(l.line)
      ? "join"
      : "";

  return (
    <div className="dash-overlay">
      <div className="dash">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div className="eyebrow">Server dashboard</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 24 }}>
              {status?.name || "Server"}
            </div>
          </div>
          <button className="btn ghost" onClick={onClose}>
            <Icon.close size={16} /> Close
          </button>
        </div>

        <div className="row wrap" style={{ gap: 12 }}>
          <Stat label="Status" value={running ? "Running" : "Stopped"} accent={running} />
          <Stat label="Players" value={`${status?.players ?? 0} / ${status?.maxPlayers ?? 0}`} />
          <Stat label="Memory" value={mem} accent />
          <Stat label="Port" value={status?.port ? String(status.port) : "—"} />
          <Stat label="Version" value={status?.version || "—"} />
        </div>

        {running && status?.port && <HostAddress port={status.port} onCopy={showToast} />}

        <div className="console">
          {lines.length === 0 && <div style={{ opacity: 0.5 }}>Waiting for server output…</div>}
          {lines.map((l, i) => (
            <div key={i} className={cls(l)}>
              {l.line}
            </div>
          ))}
          <div ref={endRef} />
        </div>

        <div className="row">
          <input
            className="input"
            style={{ flex: 1 }}
            placeholder={running ? "Type a server command (e.g. say hello)…" : "Server is stopped"}
            value={cmd}
            disabled={!running}
            onChange={(e) => setCmd(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && send()}
          />
          <button className="btn" disabled={!running} onClick={send}>
            <Icon.terminal size={16} /> Send
          </button>
          <button className="btn danger" disabled={!running} onClick={() => api.serverStop(id)}>
            <Icon.stop size={14} /> Stop
          </button>
        </div>
      </div>
    </div>
  );
}
