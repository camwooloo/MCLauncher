//! Player inventory editor — reads/writes the NBT player data for instance
//! singleplayer worlds and hosted servers, so the launcher can edit inventories
//! without a third-party tool.
//!
//! Player data lives in gzipped NBT: the singleplayer host in
//! `<world>/level.dat` under `Data.Player.Inventory`, and each other player in
//! `<world>/playerdata/<uuid>.dat` under `Inventory`. Items carry `Slot`, `id`,
//! and a count field that's `Count` (byte) pre-1.20.5 and `count` (int) after —
//! we read whichever is present and preserve any extra keys (enchantments,
//! components) when editing.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;

use fastnbt::Value;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRef {
    pub label: String,
    /// "host" (level.dat) or a player UUID (playerdata/<uuid>.dat).
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Enchant {
    pub id: String,
    pub lvl: i32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemSlot {
    pub slot: i32,
    pub id: String,
    pub count: i32,
    #[serde(default)]
    pub enchantments: Vec<Enchant>,
}

fn base_dir(state: &AppState, kind: &str, id: &str) -> PathBuf {
    if kind == "server" {
        launcher_core::server::server_dir(&state.paths, id)
    } else {
        crate::instances::instance_dir(state, id)
    }
}

fn worlds_root(base: &std::path::Path, kind: &str) -> PathBuf {
    if kind == "server" {
        base.to_path_buf()
    } else {
        base.join("saves")
    }
}

fn world_dir(state: &AppState, kind: &str, id: &str, world: &str) -> PathBuf {
    worlds_root(&base_dir(state, kind, id), kind).join(world)
}

fn read_nbt(path: &std::path::Path) -> Result<Value, String> {
    let raw = std::fs::read(path).map_err(err)?;
    let mut gz = GzDecoder::new(&raw[..]);
    let mut buf = Vec::new();
    gz.read_to_end(&mut buf).map_err(err)?;
    fastnbt::from_bytes(&buf).map_err(err)
}

fn write_nbt(path: &std::path::Path, val: &Value) -> Result<(), String> {
    let buf = fastnbt::to_bytes(val).map_err(err)?;
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&buf).map_err(err)?;
    let out = enc.finish().map_err(err)?;
    std::fs::write(path, out).map_err(err)
}

/// Locate the player's `.dat` file + whether it's the level.dat host slot.
fn player_file(state: &AppState, kind: &str, id: &str, world: &str, source: &str) -> PathBuf {
    let wdir = world_dir(state, kind, id, world);
    if source == "host" {
        wdir.join("level.dat")
    } else {
        wdir.join("playerdata").join(format!("{source}.dat"))
    }
}

fn inventory_list_mut<'a>(root: &'a mut Value, source: &str) -> Option<&'a mut Vec<Value>> {
    let Value::Compound(top) = root else { return None };
    let inv = if source == "host" {
        let Value::Compound(data) = top.get_mut("Data")? else { return None };
        let Value::Compound(player) = data.get_mut("Player")? else { return None };
        player.get_mut("Inventory")?
    } else {
        top.get_mut("Inventory")?
    };
    match inv {
        Value::List(l) => Some(l),
        _ => None,
    }
}

fn slot_of(m: &HashMap<String, Value>) -> Option<i32> {
    match m.get("Slot") {
        Some(Value::Byte(b)) => Some(*b as i32),
        Some(Value::Int(i)) => Some(*i),
        _ => None,
    }
}

fn read_enchants(m: &HashMap<String, Value>) -> Vec<Enchant> {
    // Legacy (<1.20.5): tag.Enchantments = [{ id, lvl }]
    if let Some(Value::Compound(tag)) = m.get("tag") {
        if let Some(Value::List(l)) = tag.get("Enchantments") {
            return l
                .iter()
                .filter_map(|e| {
                    let Value::Compound(em) = e else { return None };
                    let id = match em.get("id") {
                        Some(Value::String(s)) => s.clone(),
                        _ => return None,
                    };
                    let lvl = match em.get("lvl") {
                        Some(Value::Short(n)) => *n as i32,
                        Some(Value::Int(n)) => *n,
                        Some(Value::Byte(n)) => *n as i32,
                        _ => 1,
                    };
                    Some(Enchant { id, lvl })
                })
                .collect();
        }
    }
    // Modern (1.20.5+): components."minecraft:enchantments".levels = { id: lvl }
    if let Some(Value::Compound(comp)) = m.get("components") {
        if let Some(Value::Compound(ench)) = comp.get("minecraft:enchantments") {
            if let Some(Value::Compound(levels)) = ench.get("levels") {
                return levels
                    .iter()
                    .map(|(id, v)| Enchant {
                        id: id.clone(),
                        lvl: match v {
                            Value::Int(n) => *n,
                            Value::Short(n) => *n as i32,
                            Value::Byte(n) => *n as i32,
                            _ => 1,
                        },
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

fn write_enchants(m: &mut HashMap<String, Value>, enchants: &[Enchant], modern: bool) {
    if modern {
        let comp = m.entry("components".into()).or_insert_with(|| Value::Compound(HashMap::new()));
        if let Value::Compound(cm) = comp {
            if enchants.is_empty() {
                cm.remove("minecraft:enchantments");
            } else {
                let levels: HashMap<String, Value> =
                    enchants.iter().map(|e| (e.id.clone(), Value::Int(e.lvl))).collect();
                let mut ench = HashMap::new();
                ench.insert("levels".into(), Value::Compound(levels));
                cm.insert("minecraft:enchantments".into(), Value::Compound(ench));
            }
        }
    } else {
        let tag = m.entry("tag".into()).or_insert_with(|| Value::Compound(HashMap::new()));
        if let Value::Compound(tm) = tag {
            if enchants.is_empty() {
                tm.remove("Enchantments");
            } else {
                let list: Vec<Value> = enchants
                    .iter()
                    .map(|e| {
                        let mut em = HashMap::new();
                        em.insert("id".into(), Value::String(e.id.clone()));
                        em.insert("lvl".into(), Value::Short(e.lvl as i16));
                        Value::Compound(em)
                    })
                    .collect();
                tm.insert("Enchantments".into(), Value::List(list));
            }
        }
    }
}

fn read_items(list: &[Value]) -> Vec<ItemSlot> {
    list.iter()
        .filter_map(|v| {
            let Value::Compound(m) = v else { return None };
            let slot = slot_of(m)?;
            let id = match m.get("id") {
                Some(Value::String(s)) => s.clone(),
                _ => return None,
            };
            let count = match m.get("count").or_else(|| m.get("Count")) {
                Some(Value::Int(i)) => *i,
                Some(Value::Byte(b)) => *b as i32,
                _ => 1,
            };
            Some(ItemSlot { slot, id, count, enchantments: read_enchants(m) })
        })
        .collect()
}

fn data_version(root: &Value, source: &str) -> i32 {
    let Value::Compound(top) = root else { return 0 };
    let dv = if source == "host" {
        top.get("Data").and_then(|d| match d {
            Value::Compound(m) => m.get("DataVersion"),
            _ => None,
        })
    } else {
        top.get("DataVersion")
    };
    match dv {
        Some(Value::Int(i)) => *i,
        _ => 0,
    }
}

/// Reconcile the inventory list to `items`, preserving extra keys on kept items.
fn apply_items(list: &mut Vec<Value>, items: &[ItemSlot], modern: bool) {
    let desired: HashSet<i32> = items.iter().map(|i| i.slot).collect();
    list.retain(|v| matches!(v, Value::Compound(m) if slot_of(m).map(|s| desired.contains(&s)).unwrap_or(false)));

    for it in items {
        let existing = list
            .iter_mut()
            .find(|v| matches!(v, Value::Compound(m) if slot_of(m) == Some(it.slot)));
        if let Some(Value::Compound(m)) = existing {
            m.insert("id".into(), Value::String(it.id.clone()));
            if m.contains_key("count") {
                m.insert("count".into(), Value::Int(it.count));
            } else if m.contains_key("Count") {
                m.insert("Count".into(), Value::Byte(it.count as i8));
            } else if modern {
                m.insert("count".into(), Value::Int(it.count));
            } else {
                m.insert("Count".into(), Value::Byte(it.count as i8));
            }
            write_enchants(m, &it.enchantments, modern);
        } else {
            let mut m = HashMap::new();
            m.insert("Slot".into(), Value::Byte(it.slot as i8));
            m.insert("id".into(), Value::String(it.id.clone()));
            if modern {
                m.insert("count".into(), Value::Int(it.count));
            } else {
                m.insert("Count".into(), Value::Byte(it.count as i8));
            }
            write_enchants(&mut m, &it.enchantments, modern);
            list.push(Value::Compound(m));
        }
    }
}

// --- Commands ------------------------------------------------------------

#[tauri::command]
pub fn list_worlds(state: State<'_, AppState>, target_kind: String, target_id: String) -> Vec<String> {
    let root = worlds_root(&base_dir(&state, &target_kind, &target_id), &target_kind);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("level.dat").exists() {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out
}

/// Build a uuid→name map from the server's `usercache.json` (Minecraft keeps
/// every player who has ever joined here). Keys are lowercased, dashes stripped.
fn usercache_names(base: &std::path::Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for file in ["usercache.json", "usernamecache.json"] {
        if let Ok(bytes) = std::fs::read(base.join(file)) {
            if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) {
                for v in arr {
                    if let (Some(uuid), Some(name)) = (
                        v.get("uuid").and_then(|x| x.as_str()),
                        v.get("name").and_then(|x| x.as_str()),
                    ) {
                        map.insert(uuid.replace('-', "").to_lowercase(), name.to_string());
                    }
                }
            }
        }
    }
    map
}

/// uuid → username via Mojang's session server (used when usercache misses).
async fn fetch_name(uuid_undashed: &str) -> Option<String> {
    let resp = launcher_core::http::client()
        .get(format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{uuid_undashed}"
        ))
        .send()
        .await
        .ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    v.get("name").and_then(|x| x.as_str()).map(|s| s.to_string())
}

#[tauri::command]
pub async fn list_players(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    world: String,
) -> Result<Vec<PlayerRef>, String> {
    let base = base_dir(&state, &target_kind, &target_id);
    let wdir = world_dir(&state, &target_kind, &target_id, &world);
    let cache = usercache_names(&base);
    let mut out = Vec::new();

    // Singleplayer host (level.dat Data.Player.Inventory).
    if let Ok(mut root) = read_nbt(&wdir.join("level.dat")) {
        if inventory_list_mut(&mut root, "host").is_some() {
            out.push(PlayerRef {
                label: "Singleplayer".into(),
                source: "host".into(),
            });
        }
    }
    // Per-player data — show a friendly username instead of a raw UUID.
    if let Ok(rd) = std::fs::read_dir(wdir.join("playerdata")) {
        for entry in rd.flatten() {
            let fname = entry.file_name().to_string_lossy().into_owned();
            let Some(uuid) = fname.strip_suffix(".dat") else { continue };
            // Skip the *_old / *.dat_old backups Minecraft writes.
            if uuid.ends_with("_old") {
                continue;
            }
            let key = uuid.replace('-', "").to_lowercase();
            let label = match cache.get(&key) {
                Some(name) => name.clone(),
                None => match fetch_name(&key).await {
                    Some(name) => name,
                    // No name available — show a short, recognisable id.
                    None => format!("Player ··{}", &uuid[uuid.len().saturating_sub(5)..]),
                },
            };
            out.push(PlayerRef { label, source: uuid.to_string() });
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn get_inventory(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    world: String,
    source: String,
) -> Result<Vec<ItemSlot>, String> {
    let path = player_file(&state, &target_kind, &target_id, &world, &source);
    let mut root = read_nbt(&path)?;
    let list = inventory_list_mut(&mut root, &source).ok_or_else(|| "No inventory found".to_string())?;
    Ok(read_items(list))
}

#[tauri::command]
pub fn save_inventory(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    world: String,
    source: String,
    items: Vec<ItemSlot>,
) -> Result<(), String> {
    let path = player_file(&state, &target_kind, &target_id, &world, &source);
    let mut root = read_nbt(&path)?;
    let modern = data_version(&root, &source) >= 3837; // 1.20.5+
    {
        let list = inventory_list_mut(&mut root, &source).ok_or_else(|| "No inventory found".to_string())?;
        apply_items(list, &items, modern);
    }
    write_nbt(&path, &root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_edit_roundtrip_via_nbt() {
        // Build a playerdata-shaped NBT, write+read it, edit, and verify.
        let mut item = HashMap::new();
        item.insert("Slot".to_string(), Value::Byte(0));
        item.insert("id".to_string(), Value::String("minecraft:stone".into()));
        item.insert("Count".to_string(), Value::Byte(5));
        let mut root_map = HashMap::new();
        root_map.insert("DataVersion".to_string(), Value::Int(3700));
        root_map.insert("Inventory".to_string(), Value::List(vec![Value::Compound(item)]));
        let root = Value::Compound(root_map);

        // NBT round-trip.
        let bytes = fastnbt::to_bytes(&root).unwrap();
        let mut back: Value = fastnbt::from_bytes(&bytes).unwrap();

        let items = read_items(inventory_list_mut(&mut back, "uuid").unwrap());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "minecraft:stone");
        assert_eq!(items[0].count, 5);

        // Edit: change to a sharpness-5 diamond sword + add a new slot.
        let edited = vec![
            ItemSlot {
                slot: 0,
                id: "minecraft:diamond_sword".into(),
                count: 1,
                enchantments: vec![Enchant { id: "minecraft:sharpness".into(), lvl: 5 }],
            },
            ItemSlot { slot: 1, id: "minecraft:netherite_ingot".into(), count: 2, enchantments: vec![] },
        ];
        apply_items(inventory_list_mut(&mut back, "uuid").unwrap(), &edited, false);
        let after = read_items(inventory_list_mut(&mut back, "uuid").unwrap());
        assert_eq!(after.len(), 2);
        let s0 = after.iter().find(|i| i.slot == 0).unwrap();
        assert_eq!(s0.id, "minecraft:diamond_sword");
        assert_eq!(s0.enchantments.len(), 1);
        assert_eq!(s0.enchantments[0].id, "minecraft:sharpness");
        assert_eq!(s0.enchantments[0].lvl, 5);
    }
}
