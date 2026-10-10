import { useRef, useState } from "react";

import { useLauncher } from "../store";
import { Avatar, Icon } from "./ui";
import { Popover } from "./Popover";

/** The Minecraft account chip on the Minecraft page: switch between the
 *  Minecraft accounts linked to your Kawma account, or link another. */
export function AccountMenu({ onManage }: { onManage: () => void }) {
  const { store, activeAccount, microsoftLogin, setActive } = useLauncher();
  const [open, setOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const acct = activeAccount();

  return (
    <div style={{ position: "relative" }}>
      <button ref={btnRef} className="acct-chip" onClick={() => setOpen((o) => !o)}>
        {acct ? <Avatar account={acct} size={30} /> : <span className="av">?</span>}
        <span className="nm">{acct ? acct.username : "Sign in"}</span>
        <Icon.chevron size={15} />
      </button>

      <Popover anchor={btnRef} open={open} onClose={() => setOpen(false)} placement="bottom-end" offset={8} className="acct-menu surface">
        {store.accounts.length > 0 && (
          <>
            {store.accounts.map((a) => (
              <button
                key={a.uuid}
                className="menu-item"
                onClick={() => {
                  setActive(a.uuid);
                  setOpen(false);
                }}
              >
                <Avatar account={a} size={28} />
                <span className="grow" style={{ flex: 1 }}>
                  <div style={{ fontWeight: 600, fontSize: 13.5 }}>{a.username}</div>
                  <div style={{ color: "var(--text-mute)", fontSize: 11.5 }}>
                    {a.user_type === "msa" ? "Microsoft" : "Offline"}
                  </div>
                </span>
                {a.uuid === store.active_uuid && <Icon.check size={16} />}
              </button>
            ))}
            <div className="divide" style={{ margin: "6px 4px" }} />
          </>
        )}
        <button
          className="menu-item"
          onClick={() => {
            microsoftLogin();
            setOpen(false);
          }}
        >
          <Icon.user size={17} /> Link a Microsoft account
        </button>
        <button
          className="menu-item"
          onClick={() => {
            onManage();
            setOpen(false);
          }}
        >
          <Icon.gear size={17} /> Manage Minecraft accounts
        </button>
      </Popover>
    </div>
  );
}
