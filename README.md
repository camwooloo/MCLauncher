<div align="center">

![Aurora Launcher](docs/banner.svg)

# Aurora Launcher

**A beautiful, liquid-glass multi-game launcher** — host & play modded Minecraft, launch Skyrim Together, and run Elden Ring Seamless Co-op, all from one place.

[![Built with Rust](https://img.shields.io/badge/core-Rust-000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri 2](https://img.shields.io/badge/shell-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![React + TypeScript](https://img.shields.io/badge/ui-React%20%2B%20TypeScript-3178C6?logo=react&logoColor=white)](https://react.dev/)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D6?logo=windows&logoColor=white)](#)
[![License: MIT](https://img.shields.io/badge/license-MIT-2ee6b0)](LICENSE)

</div>

---

## ✨ Why Aurora?

Most launchers are powerful but cluttered, or pretty but limited. Aurora aims for both: a **clean, animated liquid-glass UI** on top of a fast **Rust** core that handles the heavy lifting — Java provisioning, mod-loader installs, server hosting, and authentication — with zero manual setup.

> One app to **play**, **host**, and **mod** — across Minecraft, Skyrim, and Elden Ring.

---

## 🎮 Features

### 🟩 Minecraft
| | |
|---|---|
| **Instances** | Unlimited isolated profiles — each with its own version, loader, mods, worlds & RAM |
| **Loaders** | Vanilla · **Fabric** · **Quilt** · **Forge** · **NeoForge** (auto-installed) |
| **Modpacks** | One-click install from **Modrinth**, **CurseForge**, **FTB** & **Technic** — right inside *New Instance* |
| **Server hosting** | Spin up **Vanilla / Paper / Fabric / Forge** servers with a live in-app dashboard (players, RAM, console) |
| **Content browser** | Search & install mods, shaders, resource packs and plugins, version-scoped per instance/server |
| **One-click upgrades** | Bump an instance/server to a newer Minecraft version *and* auto-update its mods |
| **Creative inventory editor** | A real slot-grid NBT editor — drop in any item (incl. modpack items) and add enchantments, no third-party tools |
| **Skins** | Upload and switch skins in-app |

### ⚔️ Skyrim
- Auto-detects your install, **SKSE**, and **Skyrim Together Reborn**
- Launch into co-op in a couple of clicks

### 🔆 Elden Ring
- Detects **Seamless Co-op** and launches with the right setup

### 🎨 The whole thing is *gorgeous*
- **Liquid-glass UI** with a drifting aurora backdrop
- **Light / Dark** themes + an **Aurora / Apple-style Liquid Glass** look that tints to each game's color
- Animated tab indicator, spring transitions, and custom dropdowns/menus throughout

---

## 🛠️ Tech stack

- **`launcher-core`** — Rust library: Mojang manifest, parallel download engine, Adoptium Java auto-download, Microsoft (device-code) auth, Fabric/Quilt/Forge/NeoForge installers, Modrinth/CurseForge/FTB/Technic modpack installs, server hosting, NBT inventory editing.
- **`launcher-desktop`** — [Tauri 2](https://tauri.app/) shell + a React 18 / TypeScript / Vite frontend, bridged by typed commands.
- Packaged as an **NSIS** Windows installer.

```
crates/
├─ launcher-core/      # all the Rust logic (no UI)
└─ launcher-desktop/   # Tauri app
   ├─ src/             # React + TypeScript UI
   └─ src-tauri/       # Tauri commands bridging the core
```

---

## 🚀 Build from source

**Prerequisites:** [Rust](https://rustup.rs/) (stable), [Node.js](https://nodejs.org/) 18+, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) (on Windows: WebView2 + MSVC build tools).

```bash
# 1. Frontend deps
cd crates/launcher-desktop
npm install

# 2. Dev (hot-reload)
npm run tauri dev

# 3. Production build → installer in target/release/bundle/nsis/
npm run tauri build
```

### CurseForge modpacks (optional)
CurseForge modpack installs need a personal API key from [console.curseforge.com](https://console.curseforge.com/). It's **not** committed to this repo — provide it at build time:

```powershell
$env:AURORA_CF_KEY = "<your CurseForge key>"; npm run tauri build
```

Modrinth, FTB, sign-in and server hosting all work without any key.

---

## 🔐 Privacy & accounts

Aurora uses the official **Microsoft device-code** flow for Minecraft sign-in — no passwords pass through the app, and tokens are stored locally on your machine. Offline accounts are supported for testing.

---

## 📜 Disclaimer

Aurora Launcher is an independent, fan-made project. It is **not** affiliated with, endorsed by, or associated with Mojang Studios, Microsoft, Bethesda, or FromSoftware. *Minecraft*, *Skyrim*, and *Elden Ring* are trademarks of their respective owners. You must own a legitimate copy of each game to play it.

## 📄 License

[MIT](LICENSE) © camwooloo
