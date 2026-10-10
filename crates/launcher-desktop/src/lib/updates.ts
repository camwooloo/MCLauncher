// Launcher self-update state, shared by the titlebar's UpdatePill and the
// "Check for updates" row in Settings, so a manual check lights up the pill.

import { useSyncExternalStore } from "react";

import { applyAppUpdate, checkAppUpdate, type UpdateInfo } from "./api";

export interface UpdateState {
  /** The newer build the last check offered; null when up to date (or not checked yet). */
  info: UpdateInfo | null;
  checking: boolean;
  /** The last check's or install's error, shown inline in Settings. */
  error: string | null;
  /** When a check last succeeded (ms since epoch). */
  lastChecked: number | null;
  installing: boolean;
}

let state: UpdateState = { info: null, checking: false, error: null, lastChecked: null, installing: false };
const listeners = new Set<() => void>();

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

let inflight: Promise<UpdateInfo | null> | null = null;

/** Ask kawma.app for a newer build. Concurrent calls share one request. */
export function checkForUpdates(): Promise<UpdateInfo | null> {
  if (inflight) return inflight;
  set({ checking: true, error: null });
  inflight = checkAppUpdate()
    .then(
      (info) => {
        set({ info, checking: false, lastChecked: Date.now() });
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

let startupChecked = false;
/** The check the launcher runs once at startup (errors stay quiet there). */
export function checkAtStartup() {
  if (startupChecked) return;
  startupChecked = true;
  checkForUpdates().catch(() => {});
}

/** Download, verify and run the offered installer (the launcher then restarts). */
export async function installUpdate(): Promise<void> {
  set({ installing: true, error: null });
  try {
    await applyAppUpdate();
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
