import { useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import { Icon } from "./ui";

/** Shown when an instance crashed: reads the crash report, names the likely
 *  culprit mod, and offers to disable it and relaunch. */
export function CrashModal() {
  const { crashTarget, closeCrash, showToast, playInstance } = useLauncher();
  const [info, setInfo] = useState<api.CrashInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const t = crashTarget;

  useEffect(() => {
    if (t) {
      setInfo(null);
      api.analyzeCrash(t.id).then(setInfo).catch(() => {});
    }
  }, [t?.id]);

  if (!t) return null;

  const disable = async () => {
    if (!info?.culpritFile) return;
    setBusy(true);
    try {
      await api.disableMod(t.id, info.culpritFile);
      showToast(`Disabled ${info.culpritName ?? "the mod"} — relaunching…`);
      closeCrash();
      void playInstance(t.id);
    } catch (e) {
      showToast(`${e}`);
      setBusy(false);
    }
  };

  return (
    <div className="dash-overlay" onClick={closeCrash}>
      <div className="update-modal surface" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="eyebrow">Crash detected</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 20 }}>{t.name} crashed</div>
          </div>
          <button className="btn ghost" onClick={closeCrash}>
            <Icon.close size={16} /> Close
          </button>
        </div>

        {!info ? (
          <p className="muted">Reading the crash report…</p>
        ) : !info.found ? (
          <p className="muted">No crash report found for this instance.</p>
        ) : (
          <>
            <div className="patch-notes" style={{ maxHeight: "30vh" }}>{info.title}</div>
            {info.culpritName ? (
              <p className="muted">
                Most likely cause: <b>{info.culpritName}</b>
                {info.culpritFile ? <> (<code className="md-code">{info.culpritFile}</code>)</> : null}. You can
                disable it and try again — it isn't needed to play.
              </p>
            ) : (
              <p className="muted">Couldn't pinpoint a single mod — open the full report to dig in.</p>
            )}
            <div className="row" style={{ justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
              <button className="btn ghost" onClick={() => api.openPath(info.reportPath)}>
                <Icon.terminal size={15} /> Open report
              </button>
              {info.culpritFile && (
                <button className="btn-play" disabled={busy} onClick={disable}>
                  <Icon.check size={16} /> {busy ? "Working…" : `Disable ${info.culpritName ?? "mod"} & relaunch`}
                </button>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
