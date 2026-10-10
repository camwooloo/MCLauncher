import { useLauncher } from "../store";
import { installUpdate, useAppUpdate } from "../lib/updates";
import { Icon } from "./ui";

const RING = 2 * Math.PI * 15;

/** Under the Kawma avatar in the right rail: shows only when a launcher update
 *  is out. One click downloads it (with a progress ring) if needed, then the
 *  launcher closes, updates and reopens. */
export function UpdateButton() {
  const { showToast } = useLauncher();
  const { info, downloading, progress, installing } = useAppUpdate();
  if (!info) return null;

  const pct = progress == null ? null : Math.round(progress * 100);
  const label = installing
    ? `Updating to v${info.version}…`
    : downloading
      ? `Downloading v${info.version}${pct == null ? "…" : ` · ${pct}%`}`
      : `Update to v${info.version}`;
  const update = () => {
    if (installing) return;
    if (!downloading) showToast(`Updating Kawma Launcher to v${info.version}. It reopens by itself.`);
    installUpdate().catch((e) => showToast(`${e}`));
  };
  const spinning = installing || (downloading && progress == null);

  return (
    <button
      className={`rail-update ${downloading ? "loading" : ""} ${installing ? "installing" : ""}`}
      onClick={update}
      title={label}
      aria-label={label}
      aria-busy={downloading || installing}
    >
      <Icon.upgrade size={15} />
      {(downloading || installing) && (
        <svg className={`rail-update-ring ${spinning ? "spin" : ""}`} viewBox="0 0 34 34" aria-hidden>
          <circle cx="17" cy="17" r="15" className="track" />
          <circle
            cx="17"
            cy="17"
            r="15"
            className="bar"
            strokeDasharray={RING}
            strokeDashoffset={spinning ? RING * 0.7 : RING * (1 - (progress ?? 0))}
          />
        </svg>
      )}
      {!downloading && !installing && <span className="rail-update-dot" />}
    </button>
  );
}
