// Launcher self-update state, shared by the update button under the Kawma
// avatar in the right rail and Settings → Updates.
//
// Automatic updates work like Kawma Studio's: check on launch and every four
// hours, download in the background, and install once the launcher has been
// in the background for a minute. Never during a game launched from here, a
// hosted server, or a download/install. The install is silent and the
// launcher reopens by itself (see src-tauri/src/updates.rs).

import { useEffect, useRef, useSyncExternalStore } from "react";

import {
  checkAppUpdate,
  downloadAppUpdate,
  installAppUpdate,
  updateBlocker,
  workInProgress,
  type UpdateInfo,
} from "./api";

export interface UpdateState {
  /** The newer build the last check offered; null when up to date (or not checked yet). */
  info: UpdateInfo | null;
  checking: boolean;
  downloading: boolean;
  /** Download progress 0..1 (null when the size isn't known). */
  progress: number | null;
  /** Downloaded and verified: "Restart to update" installs it straight away. */
  ready: boolean;
  installing: boolean;
  /** The last check's, download's or install's error, shown in Settings. */
  error: string | null;
  /** When a check last succeeded (ms since epoch). */
  lastChecked: number | null;
  /** What an automatic install is waiting for ("A hosted server is running"). */
  waitingFor: string | null;
}

let state: UpdateState = {
  info: null,
  checking: false,
  downloading: false,
  progress: null,
  ready: false,
  installing: false,
  error: null,
  lastChecked: null,
  waitingFor: null,
};
const listeners = new Set<() => void>();

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

/** Automatic updates on (Settings → Updates); kept in sync by useAutoUpdates. */
let automatic = true;

const CHECK_EVERY = 4 * 60 * 60 * 1000;
const BACKGROUND_FOR = 60 * 1000;

let inflight: Promise<UpdateInfo | null> | null = null;
let lastAttempt = 0;

/** Ask kawma.app for a newer build. Concurrent calls share one request.
 *  With automatic updates on, a new build starts downloading straight away. */
export function checkForUpdates(): Promise<UpdateInfo | null> {
  if (inflight) return inflight;
  lastAttempt = Date.now();
  set({ checking: true, error: null });
  inflight = checkAppUpdate()
    .then(
      (info) => {
        set({ info, ready: !!info?.ready, checking: false, lastChecked: Date.now() });
        if (info && !info.ready && automatic) downloadUpdate().catch(() => {});
        return info;
      },
      (e) => {
        set({ checking: false, error: `${e}` });
        throw e;
      },
    )
    .finally(() => {
      inflight = null;
    });
  return inflight;
}

let downloading: Promise<void> | null = null;

/** Download and verify the offered build (kept until it's installed). */
export function downloadUpdate(): Promise<void> {
  if (downloading) return downloading;
  set({ downloading: true, progress: 0, error: null });
  downloading = downloadAppUpdate((p) => set({ progress: p.total ? Math.min(1, p.received / p.total) : null }))
    .then(
      () => set({ downloading: false, progress: null, ready: true }),
      (e) => {
        set({ downloading: false, progress: null, error: `${e}` });
        throw e;
      },
    )
    .finally(() => {
      downloading = null;
    });
  return downloading;
}

/** Install now: download first if needed, then the launcher closes, updates
 *  and reopens. Rejects (and stays open) if something must finish first. */
export async function installUpdate(): Promise<void> {
  if (state.installing) return;
  try {
    if (!state.ready) await downloadUpdate();
    set({ installing: true, error: null });
    await installAppUpdate(false);
  } catch (e) {
    set({ installing: false, error: `${e}` });
    throw e;
  }
}

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
};

export function useAppUpdate(): UpdateState {
  return useSyncExternalStore(subscribe, () => state);
}

let started = false;

/**
 * Runs the automatic updater for the life of the window. `enabled` is the
 * "Automatic updates" setting; `busy` says what the launcher itself is in the
 * middle of (a launch, an install), if anything. With it off, the launcher
 * still checks once at launch so the update button can appear.
 */
export function useAutoUpdates(enabled: boolean, ready: boolean, busy: () => string | null) {
  const busyRef = useRef(busy);
  busyRef.current = busy;
  useEffect(() => {
    automatic = enabled;
  }, [enabled]);

  // One check at launch, once settings are loaded.
  useEffect(() => {
    if (!ready || started) return;
    started = true;
    checkForUpdates().catch(() => {});
  }, [ready]);

  // Then every four hours.
  useEffect(() => {
    if (!ready || !enabled) return;
    const t = setInterval(() => {
      if (Date.now() - lastAttempt >= CHECK_EVERY) checkForUpdates().catch(() => {});
    }, 60 * 1000);
    return () => clearInterval(t);
  }, [ready, enabled]);

  // Install once the launcher has sat in the background (hidden, minimised or
  // just not focused, with no clicks or keys) for a minute.
  useEffect(() => {
    if (!ready || !enabled) return;
    const inBackground = () => document.hidden || !document.hasFocus();
    let since = inBackground() ? Date.now() : 0;
    const activity = () => {
      since = 0;
    };
    const visibility = () => {
      since = inBackground() ? Date.now() : 0;
    };
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("focus", activity);
    window.addEventListener("blur", visibility);
    window.addEventListener("pointerdown", activity, true);
    window.addEventListener("keydown", activity, true);
    window.addEventListener("wheel", activity, { capture: true, passive: true });
    let attempted: string | null = null;
    let ticking = false;
    const tick = async () => {
      const { info, ready: downloaded, installing } = state;
      if (!info || !downloaded || installing || state.downloading || ticking) return;
      if (!inBackground()) {
        since = 0;
        return;
      }
      if (!since) since = Date.now();
      if (Date.now() - since < BACKGROUND_FOR || attempted === info.version) return;
      ticking = true;
      try {
        const local = busyRef.current() ?? (workInProgress() > 0 ? "A download or install is running" : null);
        const blocked = local ?? (await updateBlocker().catch(() => "Couldn't check what's running"));
        if (state.waitingFor !== blocked) set({ waitingFor: blocked });
        if (blocked) return;
        attempted = info.version;
        set({ installing: true, error: null });
        await installAppUpdate(true);
      } catch (e) {
        const msg = `${e}`;
        // Something started in the meantime: wait and try again later.
        if (msg.endsWith("so the update will wait.")) {
          attempted = null;
          set({ installing: false, waitingFor: msg.replace(/, so the update will wait\.$/, "") });
        } else {
          set({ installing: false, error: msg });
        }
      } finally {
        ticking = false;
      }
    };
    const t = setInterval(() => void tick(), 5000);
    return () => {
      clearInterval(t);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("focus", activity);
      window.removeEventListener("blur", visibility);
      window.removeEventListener("pointerdown", activity, true);
      window.removeEventListener("keydown", activity, true);
      window.removeEventListener("wheel", activity, true);
    };
  }, [ready, enabled]);
}
