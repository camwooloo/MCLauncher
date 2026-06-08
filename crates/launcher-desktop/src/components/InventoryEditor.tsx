import { useCallback, useEffect, useState } from "react";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { ContentTarget, Enchant, ItemSlot, PlayerRef } from "../lib/types";
import { ENCHANTMENTS, VANILLA_ITEMS } from "../lib/mcdata";
import { Field, Icon, ItemIcon, Select } from "./ui";

const ARMOR_SLOTS = [103, 102, 101, 100]; // head, chest, legs, feet
const ARMOR_LABELS = ["Head", "Chest", "Legs", "Feet"];
const OFFHAND = -106;
const MAIN = Array.from({ length: 27 }, (_, i) => 9 + i);
const HOTBAR = Array.from({ length: 9 }, (_, i) => i);

/** Item picker as an icon grid; type any id for modpack items. */
function ItemPickerGrid({ onPick, onClose }: { onPick: (id: string) => void; onClose: () => void }) {
  const [q, setQ] = useState("");
  const query = q.trim().toLowerCase();
  const matches = (query ? VANILLA_ITEMS.filter((i) => i.includes(query)) : VANILLA_ITEMS).slice(0, 240);

  return (
    <div className="dash-overlay" style={{ zIndex: 95 }} onClick={onClose}>
      <div className="dash" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 20 }}>Choose an item</div>
          <button className="btn ghost" onClick={onClose}>
            <Icon.close size={16} /> Close
          </button>
        </div>
        <div className="row" style={{ alignItems: "flex-end" }}>
          <Field label="Search or type any id (incl. modpack items)">
            <input
              className="input"
              autoFocus
              value={q}
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && q.includes(":") && onPick(q.trim())}
              placeholder="diamond, create:cogwheel…"
              style={{ minWidth: 340 }}
            />
          </Field>
          {q.includes(":") && (
            <button className="btn" onClick={() => onPick(q.trim())}>
              <Icon.plus size={15} /> Use “{q.trim()}”
            </button>
          )}
        </div>
        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", paddingRight: 6 }}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
            {matches.map((id) => (
              <button
                key={id}
                className="slot"
                title={id.replace("minecraft:", "")}
                onClick={() => onPick(id)}
              >
                <ItemIcon id={id} size={34} />
              </button>
            ))}
          </div>
          {matches.length === 0 && <p className="muted">No vanilla match — type a full id and press Enter.</p>}
        </div>
      </div>
    </div>
  );
}

function EnchantPanel({ enchants, onChange }: { enchants: Enchant[]; onChange: (e: Enchant[]) => void }) {
  const add = () => {
    const def = ENCHANTMENTS.find((e) => !enchants.some((x) => x.id === e.id)) ?? ENCHANTMENTS[0];
    onChange([...enchants, { id: def.id, lvl: 1 }]);
  };
  return (
    <div className="col" style={{ gap: 6 }}>
      <div className="row" style={{ justifyContent: "space-between" }}>
        <span style={{ fontWeight: 600, fontSize: 13 }}>Enchantments</span>
        <button className="btn ghost" style={{ padding: "5px 10px" }} onClick={add}>
          <Icon.plus size={14} /> Add
        </button>
      </div>
      {enchants.length === 0 && <span className="muted">None.</span>}
      {enchants.map((e, i) => {
        const def = ENCHANTMENTS.find((d) => d.id === e.id);
        return (
          <div className="row" key={i} style={{ gap: 8 }}>
            <Select
              value={e.id}
              onChange={(v) => onChange(enchants.map((x, j) => (j === i ? { ...x, id: v } : x)))}
              minWidth={190}
              options={(def ? ENCHANTMENTS : [...ENCHANTMENTS, { id: e.id, name: e.id, max: 255 }]).map((d) => ({
                value: d.id,
                label: d.name,
              }))}
            />
            <input
              className="input"
              type="number"
              min={1}
              value={e.lvl}
              onChange={(ev) => onChange(enchants.map((x, j) => (j === i ? { ...x, lvl: Number(ev.target.value) } : x)))}
              style={{ width: 70 }}
            />
            {def && <span className="muted" style={{ fontSize: 11 }}>max {def.max}</span>}
            <div style={{ flex: 1 }} />
            <button className="btn danger ghost" style={{ padding: "5px 9px" }} onClick={() => onChange(enchants.filter((_, j) => j !== i))}>
              <Icon.trash size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}

function Slot({
  slot,
  item,
  selected,
  onClick,
}: {
  slot: number;
  item?: ItemSlot;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`slot ${item ? "" : "empty"} ${selected ? "sel" : ""}`} onClick={onClick} title={item?.id ?? `slot ${slot}`}>
      {item ? <ItemIcon id={item.id} size={34} /> : <span style={{ opacity: 0.3, fontSize: 11 }}>{slot}</span>}
      {item && item.count > 1 && <span className="cnt">{item.count}</span>}
      {item && item.enchantments.length > 0 && <span className="ench-dot" />}
    </button>
  );
}

/** Creative-style inventory editor — a real slot grid with an icon picker. */
export function InventoryEditor({ target, onClose }: { target: ContentTarget; onClose: () => void }) {
  const { showToast } = useLauncher();
  const [worlds, setWorlds] = useState<string[]>([]);
  const [world, setWorld] = useState("");
  const [players, setPlayers] = useState<PlayerRef[]>([]);
  const [source, setSource] = useState("");
  const [items, setItems] = useState<ItemSlot[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const [picking, setPicking] = useState(false);

  useEffect(() => {
    api
      .listWorlds(target.kind, target.id)
      .then((w) => {
        setWorlds(w);
        setWorld(w[0] ?? "");
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [target]);

  useEffect(() => {
    if (!world) return;
    api
      .listPlayers(target.kind, target.id, world)
      .then((p) => {
        setPlayers(p);
        setSource(p[0]?.source ?? "");
      })
      .catch(() => {});
  }, [target, world]);

  const loadInv = useCallback(async () => {
    if (!world || !source) return;
    try {
      setItems(await api.getInventory(target.kind, target.id, world, source));
    } catch (e) {
      showToast(`${e}`);
      setItems([]);
    }
    setSelected(null);
  }, [target, world, source, showToast]);
  useEffect(() => {
    loadInv();
  }, [loadInv]);

  const bySlot = new Map(items.map((it) => [it.slot, it]));
  const selItem = selected != null ? bySlot.get(selected) : undefined;

  const setSlot = (slot: number, patch: Partial<ItemSlot>) =>
    setItems((arr) => arr.map((it) => (it.slot === slot ? { ...it, ...patch } : it)));
  const removeSlot = (slot: number) => {
    setItems((arr) => arr.filter((it) => it.slot !== slot));
    setSelected(null);
  };
  const assign = (slot: number, id: string) =>
    setItems((arr) => {
      if (arr.some((it) => it.slot === slot)) return arr.map((it) => (it.slot === slot ? { ...it, id } : it));
      return [...arr, { slot, id, count: 1, enchantments: [] }];
    });

  const clickSlot = (slot: number) => {
    setSelected(slot);
    if (!bySlot.get(slot)) setPicking(true); // empty → pick an item to place
  };

  const save = async () => {
    setSaving(true);
    try {
      await api.saveInventory(target.kind, target.id, world, source, items);
      showToast("Inventory saved");
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setSaving(false);
    }
  };

  const row = (slots: number[]) => (
    <div className="inv-row">
      {slots.map((s) => (
        <Slot key={s} slot={s} item={bySlot.get(s)} selected={selected === s} onClick={() => clickSlot(s)} />
      ))}
    </div>
  );

  return (
    <div className="dash-overlay" onClick={onClose}>
      <div className="dash" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div className="eyebrow">Inventory · {target.kind === "server" ? "Server" : "Instance"}</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 22 }}>{target.name}</div>
          </div>
          <button className="btn ghost" onClick={onClose}>
            <Icon.close size={16} /> Close
          </button>
        </div>

        <div className="row wrap" style={{ alignItems: "flex-end" }}>
          <Field label="World">
            <Select
              value={world}
              onChange={setWorld}
              minWidth={160}
              options={worlds.length ? worlds.map((w) => ({ value: w, label: w })) : [{ value: "", label: "no worlds yet" }]}
            />
          </Field>
          <Field label="Player">
            <Select
              value={source}
              onChange={setSource}
              minWidth={210}
              options={players.length ? players.map((p) => ({ value: p.source, label: p.label })) : [{ value: "", label: "no players" }]}
            />
          </Field>
          <div style={{ flex: 1 }} />
          <button className="btn-play" style={{ padding: "11px 22px", fontSize: 14 }} disabled={!source || saving} onClick={save}>
            <Icon.check size={16} /> {saving ? "Saving…" : "Save inventory"}
          </button>
        </div>

        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", paddingRight: 6 }}>
          {loading && <p className="muted">Loading…</p>}
          {!loading && worlds.length === 0 && (
            <p className="muted">No worlds yet — launch this {target.kind} once to generate one.</p>
          )}
          {!loading && worlds.length > 0 && (
            <div className="row wrap" style={{ alignItems: "flex-start", gap: 28 }}>
              <div className="inv-grid">
                <div className="slot-label">Armor &amp; offhand</div>
                <div className="inv-row">
                  {ARMOR_SLOTS.map((s, i) => (
                    <div key={s} style={{ textAlign: "center" }}>
                      <div style={{ fontSize: 9, color: "var(--text-mute)" }}>{ARMOR_LABELS[i]}</div>
                      <Slot slot={s} item={bySlot.get(s)} selected={selected === s} onClick={() => clickSlot(s)} />
                    </div>
                  ))}
                  <div style={{ textAlign: "center" }}>
                    <div style={{ fontSize: 9, color: "var(--text-mute)" }}>Off</div>
                    <Slot slot={OFFHAND} item={bySlot.get(OFFHAND)} selected={selected === OFFHAND} onClick={() => clickSlot(OFFHAND)} />
                  </div>
                </div>
                <div className="slot-label" style={{ marginTop: 8 }}>Inventory</div>
                {row(MAIN.slice(0, 9))}
                {row(MAIN.slice(9, 18))}
                {row(MAIN.slice(18, 27))}
                <div className="slot-label" style={{ marginTop: 8 }}>Hotbar</div>
                {row(HOTBAR)}
              </div>

              {/* Selected item editor */}
              {selItem && (
                <div className="surface" style={{ padding: 16, borderRadius: 16, minWidth: 280, flex: 1 }}>
                  <div className="row" style={{ gap: 10, marginBottom: 10 }}>
                    <ItemIcon id={selItem.id} size={40} />
                    <div className="grow">
                      <div style={{ fontWeight: 600 }}>{selItem.id.replace("minecraft:", "")}</div>
                      <div className="sub" style={{ color: "var(--text-mute)" }}>Slot {selItem.slot}</div>
                    </div>
                    <button className="btn ghost" onClick={() => setPicking(true)}>
                      Change
                    </button>
                  </div>
                  <div className="row" style={{ marginBottom: 12 }}>
                    <Field label="Count">
                      <input
                        className="input"
                        type="number"
                        value={selItem.count}
                        onChange={(e) => setSlot(selItem.slot, { count: Number(e.target.value) })}
                        style={{ width: 90 }}
                      />
                    </Field>
                    <div style={{ flex: 1 }} />
                    <button className="btn danger ghost" onClick={() => removeSlot(selItem.slot)}>
                      <Icon.trash size={15} /> Remove
                    </button>
                  </div>
                  <EnchantPanel
                    enchants={selItem.enchantments}
                    onChange={(en) => setSlot(selItem.slot, { enchantments: en })}
                  />
                </div>
              )}
              {!selItem && (
                <div className="muted" style={{ flex: 1, minWidth: 220 }}>
                  Click a slot to place an item, or click a filled slot to edit its count and
                  enchantments. Existing item NBT is preserved.
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {picking && selected != null && (
        <ItemPickerGrid
          onClose={() => setPicking(false)}
          onPick={(id) => {
            assign(selected, id);
            setPicking(false);
          }}
        />
      )}
    </div>
  );
}
