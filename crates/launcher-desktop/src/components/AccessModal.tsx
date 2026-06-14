import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import { Icon } from "./ui";

/** Manage a server's whitelist and operators — add by Minecraft username
 *  (resolved to a UUID via Mojang), remove with a click. */
export function AccessModal({ id, name, onClose }: { id: string; name: string; onClose: () => void }) {
  const { showToast } = useLauncher();
  const [data, setData] = useState<api.ServerAccess>({ whitelist: [], ops: [] });
  const [adding, setAdding] = useState<{ whitelist: string; ops: string }>({ whitelist: "", ops: "" });
  const [busy, setBusy] = useState(false);

  const refresh = () => api.serverAccess(id).then(setData).catch(() => {});
  useEffect(() => {
    refresh();
  }, [id]);

  const add = async (list: "whitelist" | "ops") => {
    const who = adding[list].trim();
    if (!who) return;
    setBusy(true);
    try {
      await api.accessAdd(id, list, who);
      setAdding((a) => ({ ...a, [list]: "" }));
      await refresh();
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(false);
    }
  };
  const remove = async (list: "whitelist" | "ops", uuid: string) => {
    try {
      await api.accessRemove(id, list, uuid);
      await refresh();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  const section = (list: "whitelist" | "ops", title: string, hint: string) => (
    <div style={{ flex: 1, minWidth: 0 }}>
      <div className="sect-title" style={{ marginBottom: 4 }}>{title}</div>
      <p className="muted" style={{ margin: "0 0 8px", fontSize: 12 }}>{hint}</p>
      <div className="row" style={{ gap: 8 }}>
        <input
          className="input"
          style={{ flex: 1 }}
          placeholder="Minecraft username"
          value={adding[list]}
          onChange={(e) => setAdding((a) => ({ ...a, [list]: e.target.value }))}
          onKeyDown={(e) => e.key === "Enter" && add(list)}
        />
        <button className="btn" disabled={busy || !adding[list].trim()} onClick={() => add(list)}>
          <Icon.plus size={15} /> Add
        </button>
      </div>
      <div className="col" style={{ gap: 2, marginTop: 8 }}>
        {data[list].length === 0 && <p className="muted" style={{ fontSize: 12.5 }}>Nobody yet.</p>}
        {data[list].map((m) => (
          <div className="lrow" key={m.uuid || m.name}>
            <img
              src={`https://mc-heads.net/avatar/${m.uuid || m.name}/28`}
              width={28}
              height={28}
              style={{ borderRadius: 7 }}
              alt=""
            />
            <div className="grow"><div className="name" style={{ fontSize: 13.5 }}>{m.name}</div></div>
            <button className="btn danger ghost" onClick={() => remove(list, m.uuid)}>
              <Icon.trash size={14} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );

  return (
    <div className="dash-overlay" onClick={onClose}>
      <div className="update-modal surface" onClick={(e) => e.stopPropagation()} style={{ width: "min(720px, 94vw)" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="eyebrow">Players & ops</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 20 }}>{name}</div>
          </div>
          <button className="btn ghost" onClick={onClose}>
            <Icon.close size={16} /> Close
          </button>
        </div>
        <div className="row" style={{ gap: 24, alignItems: "flex-start" }}>
          {section("whitelist", "Whitelist", "Only these players can join (if white-list is on).")}
          {section("ops", "Operators", "Server admins — full command access.")}
        </div>
      </div>
    </div>
  );
}
