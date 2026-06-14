import { useEffect, useMemo, useRef, useState } from "react";

import { EditorView, basicSetup } from "codemirror";
import { EditorState, type Extension } from "@codemirror/state";
import { StreamLanguage } from "@codemirror/language";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { oneDark } from "@codemirror/theme-one-dark";
import { properties } from "@codemirror/legacy-modes/mode/properties";
import { toml } from "@codemirror/legacy-modes/mode/toml";

import { useLauncher } from "../store";
import * as api from "../lib/api";
import type { ContentTarget } from "../lib/types";
import { Icon } from "./ui";

/** Syntax-highlighting extension for a file, by extension. Anything unknown is
 *  still fully editable as plain text. */
function langFor(path: string): Extension[] {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  if (["json", "json5", "mcmeta"].includes(ext)) return [json()];
  if (["yaml", "yml"].includes(ext)) return [yaml()];
  if (["js", "mjs", "cjs", "ts", "jsx", "tsx"].includes(ext))
    return [javascript({ jsx: ext.endsWith("x"), typescript: ext.startsWith("ts") })];
  if (ext === "py") return [python()];
  if (["properties", "cfg", "conf", "ini", "env"].includes(ext)) return [StreamLanguage.define(properties)];
  if (ext === "toml") return [StreamLanguage.define(toml)];
  return [];
}

export function ConfigEditor({ target, onClose }: { target: ContentTarget; onClose: () => void }) {
  const { showToast } = useLauncher();
  const [files, setFiles] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);

  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);

  useEffect(() => {
    api.listConfigFiles(target.kind, target.id).then(setFiles).catch(() => {});
  }, [target.kind, target.id]);

  const open = async (path: string) => {
    if (dirty && !confirm("Discard unsaved changes?")) return;
    setSelected(path);
    setLoaded(null);
    setDirty(false);
    try {
      setLoaded(await api.readConfigFile(target.kind, target.id, path));
    } catch (e) {
      showToast(`${e}`);
      setLoaded("");
    }
  };

  // (Re)create the editor whenever a fresh file is loaded.
  useEffect(() => {
    if (loaded === null || !host.current || selected === null) return;
    view.current?.destroy();
    view.current = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: loaded,
        extensions: [
          basicSetup,
          oneDark,
          ...langFor(selected),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) setDirty(true);
          }),
        ],
      }),
    });
    return () => {
      view.current?.destroy();
      view.current = null;
    };
  }, [loaded, selected]);

  const save = async () => {
    if (!selected || !view.current) return;
    setBusy(true);
    try {
      await api.writeConfigFile(target.kind, target.id, selected, view.current.state.doc.toString());
      setDirty(false);
      showToast("Saved");
    } catch (e) {
      showToast(`${e}`);
    } finally {
      setBusy(false);
    }
  };

  const shown = useMemo(
    () => files.filter((f) => f.toLowerCase().includes(filter.toLowerCase())),
    [files, filter]
  );

  return (
    <div className="dash-overlay" onClick={onClose}>
      <div className="dash editor-dash" onClick={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div className="eyebrow">Config editor</div>
            <div style={{ fontFamily: "var(--font-display)", fontWeight: 700, fontSize: 20 }}>
              {target.name}
            </div>
          </div>
          <div className="row" style={{ gap: 8 }}>
            <button className="btn-play" disabled={!selected || !dirty || busy} onClick={save}>
              <Icon.check size={15} /> {busy ? "Saving…" : dirty ? "Save" : "Saved"}
            </button>
            <button className="btn ghost" onClick={onClose}>
              <Icon.close size={16} /> Close
            </button>
          </div>
        </div>

        <div className="editor-body">
          <div className="editor-files">
            <input
              className="input"
              style={{ marginBottom: 8 }}
              placeholder="Filter files…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            />
            <div className="editor-file-list">
              {shown.length === 0 && <p className="muted" style={{ padding: "4px 6px" }}>No editable files found.</p>}
              {shown.map((f) => (
                <button
                  key={f}
                  className={`editor-file ${selected === f ? "sel" : ""}`}
                  onClick={() => open(f)}
                  title={f}
                >
                  {f}
                </button>
              ))}
            </div>
          </div>
          <div className="editor-pane">
            {selected === null ? (
              <div className="editor-empty">Pick a file to edit. JSON, YAML, JS, Python and more are highlighted; everything else opens as plain text.</div>
            ) : (
              <div className="editor-host" ref={host} />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
