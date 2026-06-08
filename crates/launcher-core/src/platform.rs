//! Platform detection and the Mojang "rule" evaluator.
//!
//! Version JSONs are cross-platform: libraries, JVM arguments, and game
//! arguments are gated by *rules* that test the current OS, OS version, CPU
//! architecture, and launcher *features* (demo mode, custom resolution, …).
//! Getting this evaluator right is what lets a single parser drive Windows,
//! macOS, and Linux.

use std::collections::HashMap;

use serde::Deserialize;

/// Operating systems Minecraft distinguishes between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    /// The OS this binary was compiled for.
    pub const CURRENT: Os = {
        #[cfg(target_os = "windows")]
        {
            Os::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Os::MacOs
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Os::Linux
        }
    };

    /// The name Mojang uses for this OS in version JSON `os.name` fields.
    pub fn mojang_name(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::MacOs => "osx",
            Os::Linux => "linux",
        }
    }

    /// The classifier suffix used for native libraries (`natives-<x>`).
    pub fn natives_classifier(self) -> &'static str {
        match self {
            Os::Windows => "natives-windows",
            Os::MacOs => "natives-macos",
            Os::Linux => "natives-linux",
        }
    }
}

/// CPU architectures Minecraft distinguishes between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86,
    X64,
    Arm64,
    Arm32,
}

impl Arch {
    pub const CURRENT: Arch = {
        #[cfg(target_arch = "x86")]
        {
            Arch::X86
        }
        #[cfg(target_arch = "x86_64")]
        {
            Arch::X64
        }
        #[cfg(target_arch = "aarch64")]
        {
            Arch::Arm64
        }
        #[cfg(target_arch = "arm")]
        {
            Arch::Arm32
        }
    };

    /// The value Mojang uses in `os.arch` rules.
    pub fn mojang_name(self) -> &'static str {
        match self {
            Arch::X86 => "x86",
            Arch::X64 => "x86_64",
            Arch::Arm64 => "arm64",
            Arch::Arm32 => "arm32",
        }
    }
}

/// Launcher feature flags referenced by rules' `features` blocks.
///
/// Unknown features are treated as `false` (rule does not match), matching the
/// behaviour of the official launcher.
#[derive(Debug, Clone, Default)]
pub struct Features {
    pub is_demo_user: bool,
    pub has_custom_resolution: bool,
    pub has_quick_plays_support: bool,
    pub is_quick_play_singleplayer: bool,
    pub is_quick_play_multiplayer: bool,
    pub is_quick_play_realms: bool,
}

impl Features {
    fn get(&self, name: &str) -> bool {
        match name {
            "is_demo_user" => self.is_demo_user,
            "has_custom_resolution" => self.has_custom_resolution,
            "has_quick_plays_support" => self.has_quick_plays_support,
            "is_quick_play_singleplayer" => self.is_quick_play_singleplayer,
            "is_quick_play_multiplayer" => self.is_quick_play_multiplayer,
            "is_quick_play_realms" => self.is_quick_play_realms,
            _ => false,
        }
    }
}

/// The full environment a rule is evaluated against.
#[derive(Debug, Clone)]
pub struct Environment {
    pub os: Os,
    pub arch: Arch,
    /// OS version string (e.g. "10.0" on Windows). Compared against the
    /// `os.version` regex in rules. We store it as a plain string and do a
    /// substring/prefix check rather than pulling in a regex engine for the
    /// rare rules that use it.
    pub os_version: String,
    pub features: Features,
}

impl Environment {
    /// Detect the current environment with default (all-false) features.
    pub fn detect() -> Self {
        Self {
            os: Os::CURRENT,
            arch: Arch::CURRENT,
            os_version: os_version_string(),
            features: Features::default(),
        }
    }

    pub fn with_features(mut self, features: Features) -> Self {
        self.features = features;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

/// The `os` constraint inside a rule.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OsConstraint {
    pub name: Option<String>,
    /// A regex in the JSON; we treat it as a literal/prefix match to avoid a
    /// regex dependency, which covers every value Mojang actually ships.
    pub version: Option<String>,
    pub arch: Option<String>,
}

/// A single rule entry from a `rules` array.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsConstraint>,
    /// Feature predicates: `{"is_demo_user": true}`.
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

impl Rule {
    /// Does this rule's *condition* match the environment? (Independent of
    /// whether the action is allow or disallow.)
    fn matches(&self, env: &Environment) -> bool {
        if let Some(os) = &self.os {
            if let Some(name) = &os.name {
                if name != env.os.mojang_name() {
                    return false;
                }
            }
            if let Some(arch) = &os.arch {
                if arch != env.arch.mojang_name() {
                    return false;
                }
            }
            if let Some(version) = &os.version {
                // Mojang uses anchored regexes like "^10\\." — strip common
                // anchors/escapes and do a prefix check, which is sufficient
                // for the handful of real-world cases.
                let needle = version.trim_start_matches('^').replace("\\.", ".");
                if !env.os_version.starts_with(needle.trim_end_matches('$')) {
                    return false;
                }
            }
        }
        if let Some(features) = &self.features {
            for (key, expected) in features {
                if env.features.get(key) != *expected {
                    return false;
                }
            }
        }
        true
    }
}

/// Evaluate a `rules` array against an environment.
///
/// Algorithm (matching the official launcher): if there are no rules the item
/// is allowed. Otherwise the *last matching rule wins*; an item with rules but
/// no matching `allow` rule is excluded.
pub fn rules_allow(rules: &[Rule], env: &Environment) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.matches(env) {
            allowed = matches!(rule.action, RuleAction::Allow);
        }
    }
    allowed
}

/// Best-effort OS version string. Only used by the rare `os.version` rule, so
/// a coarse value is acceptable.
fn os_version_string() -> String {
    #[cfg(target_os = "windows")]
    {
        // Windows reports e.g. "10.0" for both Win10 and Win11.
        "10.0".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> Environment {
        Environment {
            os: Os::Windows,
            arch: Arch::X64,
            os_version: "10.0".into(),
            features: Features::default(),
        }
    }

    #[test]
    fn no_rules_allows() {
        assert!(rules_allow(&[], &win()));
    }

    #[test]
    fn allow_only_on_other_os_excludes_us() {
        // "allow only on osx" => disallow elsewhere.
        let rules = vec![Rule {
            action: RuleAction::Allow,
            os: Some(OsConstraint {
                name: Some("osx".into()),
                version: None,
                arch: None,
            }),
            features: None,
        }];
        assert!(!rules_allow(&rules, &win()));
    }

    #[test]
    fn allow_then_disallow_our_os() {
        let rules = vec![
            Rule {
                action: RuleAction::Allow,
                os: None,
                features: None,
            },
            Rule {
                action: RuleAction::Disallow,
                os: Some(OsConstraint {
                    name: Some("windows".into()),
                    version: None,
                    arch: None,
                }),
                features: None,
            },
        ];
        assert!(!rules_allow(&rules, &win()));
    }

    #[test]
    fn feature_gated_rule() {
        let mut features = HashMap::new();
        features.insert("is_demo_user".to_string(), true);
        let rules = vec![Rule {
            action: RuleAction::Allow,
            os: None,
            features: Some(features),
        }];
        // Default features => not a demo user => excluded.
        assert!(!rules_allow(&rules, &win()));

        let demo = Environment {
            features: Features {
                is_demo_user: true,
                ..Default::default()
            },
            ..win()
        };
        assert!(rules_allow(&rules, &demo));
    }
}
