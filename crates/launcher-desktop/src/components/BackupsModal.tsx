import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import { Icon } from "./ui";

function fmtSize(b: number): string {
  if (b >= 1e6) return `${(b / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(b / 1e3))} KB`;
}
function fmtWhen(sec: number): string {
  if (!sec) return "";
  const d = Math.floor(Date.now() / 1000) - sec;
  if (d < 60) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)}m ago`;
  if (d < 86400) return `${Math.floor(d / 3600)}h ago`;
  return `${Math.floor(d / 86400)}d ago`;
}

/** Back up / restore worlds for an instance or server (rendered globally;
 *  shown when `backupTarget` is set). */
export function BackupsModal() {
  const { backupTarget, closeBackups, showToast } = useLauncher();
  const [list, setList] = useState<api.BackupInfo[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const t = backupTarget;

  useEffect(() => {
    if (t) api.listBackups(t.kind, t.id).then(setList).catch(() => {});
  }, [t?.kind, t?.id]);

  if (!t) return null;
  const refresh = () => api.listBackups(t.kind, t.id).then(setList).catch(() => {});

  const create = async () => {
    setBusy("create");
    try {
      await api.createBackup(t.kind, t.id);
      showToast("Backup created");
      await refresh();
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(null);
    }
  };
  const restore = async (file: string) => {
    setBusy(file);
    try {
      await api.restoreBackup(t.kind, t.id, file);
      showToast("World restored from backup");
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(null);
      setConfirm(null);
    }
  };
  const del = async (file: string) => {
    try {
      await api.deleteBackup(t.kind, t.id, file);
      await refresh();
    } catch (e) {
      showToast(`${e}`);
    }
  };

  return (
    <div className="dash-overlay" onClick={closeBackups}>
      <div className="update-modal surface" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="eyebrow">World backups</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 20 }}>{t.name}</div>
          </div>
          <button className="btn ghost" onClick={closeBackups}>
            <Icon.close size={16} /> Close
          </button>
        </div>

        <button className="btn-play" disabled={busy !== null} onClick={create}>
          <Icon.chest size={16} /> {busy === "create" ? "Backing up…" : "Back up worlds now"}
        </button>

        <div className="col" style={{ gap: 2, maxHeight: "44vh", overflowY: "auto" }}>
          {list.length === 0 && <p className="muted">No backups yet — create one before risky changes or version upgrades.</p>}
          {list.map((b) => (
            <div className="lrow" key={b.file}>
              <div className="avatar"><Icon.chest size={18} /></div>
              <div className="grow">
                <div className="name">{fmtWhen(b.created)}</div>
                <div className="sub">{fmtSize(b.size)}</div>
              </div>
              {confirm === b.file ? (
                <>
                  <span className="muted" style={{ fontSize: 12 }}>Overwrite current world?</span>
                  <button className="btn danger" disabled={busy !== null} onClick={() => restore(b.file)}>
                    {busy === b.file ? "Restoring…" : "Confirm"}
                  </button>
                  <button className="btn ghost" onClick={() => setConfirm(null)}>Cancel</button>
                </>
              ) : (
                <>
                  <button className="btn" onClick={() => setConfirm(b.file)}>
                    <Icon.upgrade size={14} /> Restore
                  </button>
                  <button className="btn danger ghost" onClick={() => del(b.file)}>
                    <Icon.trash size={14} />
                  </button>
                </>
              )}
            </div>
          ))}
        </div>
        <p className="muted" style={{ fontSize: 12 }}>
          Stop the server before restoring. Backups are stored in your launcher data folder.
        </p>
      </div>
    </div>
  );
}
