//! A single version's JSON: the recipe for installing and launching it.
//!
//! This module models both the modern (1.13+) schema with structured
//! `arguments` and the legacy schema with a flat `minecraftArguments` string,
//! and it knows how to:
//!
//! * pick the libraries that apply to the current platform,
//! * select the correct *native* artifact for the current OS, and
//! * resolve the (rule-filtered) JVM and game argument token lists.
//!
//! Modloaders (Fabric/Forge/Quilt) ship a partial version JSON with
//! `inheritsFrom`; [`VersionJson::merge_onto_parent`] composes the two.

use std::collections::HashMap;

use serde::Deserialize;

use crate::platform::{rules_allow, Environment, Os, Rule};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,

    /// Present on modloader JSONs: the vanilla version to merge onto.
    #[serde(default)]
    pub inherits_from: Option<String>,

    /// Fully-qualified entry point class, e.g. `net.minecraft.client.main.Main`
    /// (vanilla) or `net.fabricmc.loader.impl.launch.knot.KnotClient` (Fabric).
    #[serde(default)]
    pub main_class: Option<String>,

    /// Asset index name (e.g. "1.21", "legacy", "pre-1.6").
    #[serde(default)]
    pub assets: Option<String>,

    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,

    #[serde(default)]
    pub downloads: Option<Downloads>,

    #[serde(default)]
    pub libraries: Vec<Library>,

    /// Modern (1.13+) structured arguments.
    #[serde(default)]
    pub arguments: Option<Arguments>,

    /// Legacy (pre-1.13) flat game-argument string.
    #[serde(default)]
    pub minecraft_arguments: Option<String>,

    #[serde(default)]
    pub java_version: Option<JavaVersion>,

    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub client: Option<DownloadInfo>,
    pub server: Option<DownloadInfo>,
    #[serde(rename = "client_mappings")]
    pub client_mappings: Option<DownloadInfo>,
    #[serde(rename = "server_mappings")]
    pub server_mappings: Option<DownloadInfo>,
}

/// A downloadable file with integrity metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersion {
    /// e.g. "jre-legacy", "java-runtime-gamma".
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

// --- Libraries -----------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    /// Maven coordinate: `group:artifact:version[:classifier]`.
    pub name: String,

    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,

    /// Maven repository base URL (modloader libraries that omit `downloads`).
    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub rules: Vec<Rule>,

    /// Maps an OS name to a classifier key in `downloads.classifiers`
    /// (old-style natives). Values may contain `${arch}`.
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,

    /// Extraction rules for native jars (which paths to exclude).
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    /// Old-style natives: classifier key -> artifact.
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    /// Maven-style relative path under `libraries/`. May be absent for some
    /// modloader libraries, in which case it's derived from `name`.
    #[serde(default)]
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Library {
    /// Whether this library applies to the given environment.
    pub fn applies(&self, env: &Environment) -> bool {
        rules_allow(&self.rules, env)
    }

    /// The main (classpath) artifact for this library, if it has one that
    /// applies. Returns a [`ResolvedArtifact`] with an absolute-ready relative
    /// path and a download URL.
    pub fn classpath_artifact(&self) -> Option<ResolvedArtifact> {
        // Preferred: explicit downloads.artifact.
        if let Some(dl) = &self.downloads {
            if let Some(artifact) = &dl.artifact {
                let path = artifact
                    .path
                    .clone()
                    .unwrap_or_else(|| maven_to_path(&self.name, None));
                return Some(ResolvedArtifact {
                    path,
                    url: artifact.url.clone(),
                    sha1: Some(artifact.sha1.clone()),
                    size: Some(artifact.size),
                });
            }
        }
        // Fallback: a Maven `name` + repo `url` (modloader style).
        if let Some(base) = &self.url {
            let rel = maven_to_path(&self.name, None);
            let url = format!("{}/{}", base.trim_end_matches('/'), rel);
            return Some(ResolvedArtifact {
                path: rel,
                url,
                sha1: None,
                size: None,
            });
        }
        None
    }

    /// The native artifact for this OS, if the library ships one.
    pub fn native_artifact(&self, env: &Environment) -> Option<ResolvedArtifact> {
        // New-style natives live in their own library entry with a
        // classifier-suffixed `name` and a normal artifact; those are handled
        // by `classpath_artifact`. Here we resolve the *old-style* `natives`
        // map → `downloads.classifiers`.
        let natives = self.natives.as_ref()?;
        let key_template = natives.get(env.os.mojang_name())?;
        let key = key_template.replace("${arch}", arch_bits(env.os));
        let classifiers = self.downloads.as_ref()?.classifiers.as_ref()?;
        let artifact = classifiers.get(&key)?;
        let path = artifact
            .path
            .clone()
            .unwrap_or_else(|| maven_to_path(&self.name, Some(&key)));
        Some(ResolvedArtifact {
            path,
            url: artifact.url.clone(),
            sha1: Some(artifact.sha1.clone()),
            size: Some(artifact.size),
        })
    }
}

/// A library artifact resolved to a concrete relative path + URL.
#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    /// Relative path under the `libraries/` directory.
    pub path: String,
    pub url: String,
    /// SHA-1 if known (absent for bare Maven libraries).
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

// --- Arguments -----------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

/// One entry in a modern `arguments.game` / `arguments.jvm` array: either a
/// bare string or a rule-gated value.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        rules: Vec<Rule>,
        value: StringOrList,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

impl StringOrList {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrList::One(s) => vec![s],
            StringOrList::Many(v) => v,
        }
    }
}

impl VersionJson {
    /// Resolve the JVM argument tokens (with `${...}` placeholders intact) for
    /// the given environment. For legacy versions without structured JVM args,
    /// returns the classic defaults.
    pub fn jvm_arguments(&self, env: &Environment) -> Vec<String> {
        match &self.arguments {
            Some(args) => resolve_args(&args.jvm, env),
            None => legacy_jvm_arguments(env.os),
        }
    }

    /// Resolve the game argument tokens (with `${...}` placeholders intact).
    pub fn game_arguments(&self, env: &Environment) -> Vec<String> {
        if let Some(args) = &self.arguments {
            return resolve_args(&args.game, env);
        }
        if let Some(legacy) = &self.minecraft_arguments {
            return legacy.split_whitespace().map(str::to_owned).collect();
        }
        Vec::new()
    }

    /// Libraries that apply to this environment, paired with their classpath
    /// artifacts.
    pub fn classpath_libraries(
        &self,
        env: &Environment,
    ) -> Vec<(&Library, ResolvedArtifact)> {
        self.libraries
            .iter()
            .filter(|lib| lib.applies(env))
            .filter_map(|lib| lib.classpath_artifact().map(|a| (lib, a)))
            .collect()
    }

    /// Native artifacts that must be extracted for this environment.
    pub fn native_libraries(
        &self,
        env: &Environment,
    ) -> Vec<(&Library, ResolvedArtifact)> {
        self.libraries
            .iter()
            .filter(|lib| lib.applies(env))
            .filter_map(|lib| lib.native_artifact(env).map(|a| (lib, a)))
            .collect()
    }

    /// Compose a child (modloader) JSON onto its resolved parent (vanilla).
    ///
    /// The child wins for scalar fields it specifies; libraries are
    /// concatenated with the child's first (so its overrides take precedence
    /// on the classpath); arguments are appended.
    ///
    /// Crucially, libraries are **deduplicated by Maven coordinate**
    /// (`group:artifact[:classifier]`, ignoring version). A modloader profile
    /// often pins a newer version of a library the vanilla profile also ships
    /// (e.g. Fabric needs `org.ow2.asm:asm:9.10.1` while 1.21.5 vanilla carries
    /// `9.6`). Putting *both* on the classpath makes Fabric's loader abort with
    /// "duplicate ASM classes found on classpath". Keeping the first occurrence
    /// means the child's (loader's) version wins, exactly as the official
    /// launcher resolves it.
    pub fn merge_onto_parent(self, parent: VersionJson) -> VersionJson {
        let mut combined = self.libraries;
        combined.extend(parent.libraries);

        let mut seen = std::collections::HashSet::new();
        let libraries: Vec<Library> = combined
            .into_iter()
            .filter(|lib| seen.insert(library_dedupe_key(&lib.name)))
            .collect();

        let arguments = match (self.arguments, parent.arguments) {
            (Some(mut child), Some(p)) => {
                // Parent args first, then child's additions.
                let mut jvm = p.jvm;
                jvm.append(&mut child.jvm);
                let mut game = p.game;
                game.append(&mut child.game);
                Some(Arguments { game, jvm })
            }
            (Some(c), None) => Some(c),
            (None, p) => p,
        };

        VersionJson {
            id: self.id,
            inherits_from: None,
            main_class: self.main_class.or(parent.main_class),
            assets: self.assets.or(parent.assets),
            asset_index: self.asset_index.or(parent.asset_index),
            downloads: self.downloads.or(parent.downloads),
            libraries,
            arguments,
            minecraft_arguments: self.minecraft_arguments.or(parent.minecraft_arguments),
            java_version: self.java_version.or(parent.java_version),
            kind: self.kind.or(parent.kind),
        }
    }
}

/// Key a Maven library `name` by everything *except* its version, so two
/// versions of the same artifact collapse to one entry while the main jar and
/// its natives sibling (which differ by classifier) stay distinct.
///
/// `group:artifact:version`              → `group:artifact`
/// `group:artifact:version:classifier`   → `group:artifact:classifier`
fn library_dedupe_key(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    match parts.as_slice() {
        [group, artifact, _version, classifier, ..] => {
            format!("{group}:{artifact}:{classifier}")
        }
        [group, artifact, ..] => format!("{group}:{artifact}"),
        _ => name.to_string(),
    }
}

fn resolve_args(args: &[Argument], env: &Environment) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(s.clone()),
            Argument::Conditional { rules, value } => {
                if rules_allow(rules, env) {
                    out.extend(value.clone().into_vec());
                }
            }
        }
    }
    out
}

/// Classic JVM arguments for versions predating structured `arguments`.
fn legacy_jvm_arguments(os: Os) -> Vec<String> {
    let mut args = vec![
        "-Djava.library.path=${natives_directory}".to_string(),
        "-cp".to_string(),
        "${classpath}".to_string(),
    ];
    if os == Os::MacOs {
        args.insert(0, "-XstartOnFirstThread".to_string());
    }
    args
}

/// 32/64-bit token used to fill `${arch}` in old natives keys.
fn arch_bits(_os: Os) -> &'static str {
    // Modern installs are 64-bit; the only `${arch}` natives are legacy LWJGL.
    if cfg!(target_pointer_width = "32") {
        "32"
    } else {
        "64"
    }
}

/// Convert a Maven coordinate to a relative jar path.
///
/// `group:artifact:version` → `group/parts/artifact/version/artifact-version.jar`
/// An optional classifier (or one embedded as the 4th `:`-segment) is appended
/// before the extension. A non-default extension may be given as `...@ext`.
pub fn maven_to_path(coord: &str, extra_classifier: Option<&str>) -> String {
    // Split off a trailing `@extension` if present.
    let (coord, ext) = match coord.split_once('@') {
        Some((c, e)) => (c, e),
        None => (coord, "jar"),
    };

    let parts: Vec<&str> = coord.split(':').collect();
    let group = parts.first().copied().unwrap_or("");
    let artifact = parts.get(1).copied().unwrap_or("");
    let version = parts.get(2).copied().unwrap_or("");
    let embedded_classifier = parts.get(3).copied();

    let classifier = extra_classifier.or(embedded_classifier);

    let group_path = group.replace('.', "/");
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    format!("{group_path}/{artifact}/{version}/{file}")
}

impl VersionJson {
    /// Parse from a JSON string.
    pub fn parse(s: &str) -> crate::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Arch, Features};

    fn env(os: Os) -> Environment {
        Environment {
            os,
            arch: Arch::X64,
            os_version: "10.0".into(),
            features: Features::default(),
        }
    }

    impl VersionJson {
        fn empty() -> Self {
            VersionJson {
                id: String::new(),
                inherits_from: None,
                main_class: None,
                assets: None,
                asset_index: None,
                downloads: None,
                libraries: Vec::new(),
                arguments: None,
                minecraft_arguments: None,
                java_version: None,
                kind: None,
            }
        }
    }

    fn lib(name: &str) -> Library {
        Library {
            name: name.to_string(),
            downloads: None,
            url: None,
            rules: Vec::new(),
            natives: None,
            extract: None,
        }
    }

    #[test]
    fn dedupe_key_ignores_version_keeps_classifier() {
        // Two ASM versions collapse to one key…
        assert_eq!(
            library_dedupe_key("org.ow2.asm:asm:9.10.1"),
            library_dedupe_key("org.ow2.asm:asm:9.6")
        );
        // …but the main jar and its natives sibling stay distinct.
        assert_ne!(
            library_dedupe_key("org.lwjgl:lwjgl:3.3.3"),
            library_dedupe_key("org.lwjgl:lwjgl:3.3.3:natives-windows")
        );
    }

    #[test]
    fn merge_dedupes_duplicate_library_versions_child_wins() {
        // Child (loader) pins asm 9.10.1; parent (vanilla) ships 9.6.
        let child = VersionJson {
            id: "fabric-loader-1.21.5".into(),
            inherits_from: Some("1.21.5".into()),
            libraries: vec![lib("org.ow2.asm:asm:9.10.1")],
            ..VersionJson::empty()
        };
        let parent = VersionJson {
            id: "1.21.5".into(),
            inherits_from: None,
            libraries: vec![lib("org.ow2.asm:asm:9.6"), lib("org.lwjgl:lwjgl:3.3.3")],
            ..VersionJson::empty()
        };
        let merged = child.merge_onto_parent(parent);
        let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
        // Only ONE asm, and it's the child's newer version.
        assert_eq!(names.iter().filter(|n| n.contains(":asm:")).count(), 1);
        assert!(names.contains(&"org.ow2.asm:asm:9.10.1"));
        assert!(!names.contains(&"org.ow2.asm:asm:9.6"));
        // Unrelated parent libs survive.
        assert!(names.contains(&"org.lwjgl:lwjgl:3.3.3"));
    }

    #[test]
    fn maven_coordinate_to_path() {
        assert_eq!(
            maven_to_path("com.mojang:blocklist:1.0.10", None),
            "com/mojang/blocklist/1.0.10/blocklist-1.0.10.jar"
        );
        assert_eq!(
            maven_to_path("net.fabricmc:fabric-loader:0.15.11", None),
            "net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar"
        );
        assert_eq!(
            maven_to_path("org.lwjgl:lwjgl:3.3.3", Some("natives-windows")),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
    }

    #[test]
    fn conditional_args_filtered_by_os() {
        let json = r#"{
            "id": "x",
            "arguments": {
                "game": ["--username", "${auth_player_name}"],
                "jvm": [
                    {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"-XstartOnFirstThread"},
                    "-cp",
                    "${classpath}"
                ]
            }
        }"#;
        let v = VersionJson::parse(json).unwrap();
        let win = v.jvm_arguments(&env(Os::Windows));
        assert_eq!(win, vec!["-cp", "${classpath}"]);
        let mac = v.jvm_arguments(&env(Os::MacOs));
        assert_eq!(mac, vec!["-XstartOnFirstThread", "-cp", "${classpath}"]);
    }

    #[test]
    fn legacy_arguments_split() {
        let json = r#"{"id":"old","minecraftArguments":"--username ${auth_player_name} --version ${version_name}"}"#;
        let v = VersionJson::parse(json).unwrap();
        let game = v.game_arguments(&env(Os::Linux));
        assert_eq!(game.len(), 4);
        assert_eq!(game[0], "--username");
    }

    #[test]
    fn bare_maven_library_builds_url() {
        let json = r#"{
            "id":"x",
            "libraries":[
                {"name":"net.fabricmc:fabric-loader:0.15.11","url":"https://maven.fabricmc.net/"}
            ]
        }"#;
        let v = VersionJson::parse(json).unwrap();
        let cp = v.classpath_libraries(&env(Os::Windows));
        assert_eq!(cp.len(), 1);
        assert_eq!(
            cp[0].1.url,
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar"
        );
    }
}
