pub mod application;
pub mod deserializers;
pub mod device;
pub mod expmap;
pub mod expmap_operator;
pub mod expmap_simkey;
pub mod key;
pub mod key_combo;
pub mod keymap;
pub mod keymap_action;
pub mod keymap_action_without_args;
pub mod modmap;
pub mod modmap_operator;
pub mod nested_remap;
#[cfg(test)]
mod tests;
pub mod validation;

use crate::config::expmap::Expmap;
use crate::config::key::parse_key;
use crate::config::keymap::{build_keymap_table, Keymap, KeymapEntry};
use crate::config::validation::validate_config_file;
use crate::event_handler::DISGUISED_EVENT_OFFSETTER;
use crate::event_handler::MODIFIER_KEYS;
use evdev::KeyCode as Key;
use modmap::Modmap;
use serde::{de::IgnoredAny, Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{error, fs};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    // Config interface
    // Event stages, applied in order. Each stage is a list of entries.
    #[serde(default = "Vec::new")]
    pub stages: Vec<Vec<Expmap>>,
    // Legacy sections, moved into `stages` by `resolve_stages`.
    #[serde(default = "Vec::new")]
    pub experimental_map: Vec<Expmap>,
    #[serde(default = "Vec::new")]
    pub modmap: Vec<Modmap>,
    #[serde(default = "Vec::new")]
    pub keymap: Vec<Keymap>,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(deserialize_with = "deserialize_virtual_modifier", default = "Vec::new")]
    pub virtual_modifiers: Vec<Key>,
    #[serde(default)]
    pub keypress_delay_ms: u64,
    #[serde(default)]
    pub throttle_ms: u64,
    #[serde(default)]
    pub config_watch_debounce_ms: u64,
    #[serde(default)]
    pub notifications: bool,

    // Data is not used by any part of the application.
    // but can be used with Anchors and Aliases
    #[allow(dead_code)]
    #[serde(default)]
    pub shared: IgnoredAny,

    // Internals
    #[serde(skip)]
    pub keymap_table: HashMap<Key, Vec<KeymapEntry>>,
    #[serde(default = "const_true")]
    pub enable_wheel: bool,
}

enum ConfigFiletype {
    Yaml,
    Toml,
}

fn get_file_ext(filename: &Path) -> ConfigFiletype {
    match filename.extension() {
        Some(f) => {
            if f.to_str().unwrap_or("").to_lowercase() == "toml" {
                ConfigFiletype::Toml
            } else {
                ConfigFiletype::Yaml
            }
        }
        _ => ConfigFiletype::Yaml,
    }
}

pub fn load_configs(filenames: &[PathBuf]) -> Result<Config, Box<dyn error::Error>> {
    assert!(!filenames.is_empty(), "config is set, if not completions");

    // Assumes filenames is non-empty
    let config_contents = fs::read_to_string(&filenames[0])?;

    let mut config: Config = match get_file_ext(&filenames[0]) {
        ConfigFiletype::Yaml => serde_yaml::from_str(&config_contents)?,
        ConfigFiletype::Toml => toml::from_str(&config_contents)?,
    };

    for filename in &filenames[1..] {
        let config_contents = fs::read_to_string(filename)?;
        let c: Config = match get_file_ext(filename) {
            ConfigFiletype::Yaml => serde_yaml::from_str(&config_contents)?,
            ConfigFiletype::Toml => toml::from_str(&config_contents)?,
        };

        config.stages.extend(c.stages);
        config.experimental_map.extend(c.experimental_map);
        config.modmap.extend(c.modmap);
        config.keymap.extend(c.keymap);
        config.virtual_modifiers.extend(c.virtual_modifiers);
    }

    // Convert keymap for efficient keymap lookup
    config.keymap_table = build_keymap_table(&config.keymap);

    validate_config_file(&config)?;
    resolve_stages(&mut config)?;

    Ok(config)
}

// Moves the legacy sections into `stages`, so the rest of xremap only has to know about stages.
// They become two stages, `experimental_map` first, as they were applied before.
pub fn resolve_stages(config: &mut Config) -> anyhow::Result<()> {
    if config.experimental_map.is_empty() && config.modmap.is_empty() {
        return Ok(());
    }
    if !config.stages.is_empty() {
        anyhow::bail!("`stages` can't be combined with `experimental_map` or `modmap`");
    }
    if !config.experimental_map.is_empty() {
        config.stages.push(std::mem::take(&mut config.experimental_map));
    }
    if !config.modmap.is_empty() {
        let modmap = std::mem::take(&mut config.modmap);
        config.stages.push(modmap.into_iter().map(Modmap::into_stage_entry).collect());
    }
    Ok(())
}

fn default_mode() -> String {
    "default".to_string()
}

fn deserialize_keys<'de, D>(deserializer: D) -> Result<Vec<Key>, D::Error>
where
    D: Deserializer<'de>,
{
    let key_strs = Vec::<String>::deserialize(deserializer)?;
    let mut keys: Vec<Key> = vec![];
    for key_str in key_strs {
        keys.push(parse_key(&key_str).map_err(serde::de::Error::custom)?);
    }
    Ok(keys)
}

fn deserialize_virtual_modifier<'de, D>(deserializer: D) -> Result<Vec<Key>, D::Error>
where
    D: Deserializer<'de>,
{
    let key_strs = Vec::<String>::deserialize(deserializer)?;
    let mut keys: Vec<Key> = vec![];
    for key_str in key_strs {
        let key = parse_key(&key_str).map_err(serde::de::Error::custom)?;
        if MODIFIER_KEYS.contains(&key) {
            return Err(serde::de::Error::custom(format!("Can't use '{key_str}' as virtual modifier")));
        }
        if key.code() >= DISGUISED_EVENT_OFFSETTER {
            return Err(serde::de::Error::custom(format!(
                "Can't use a relative-event ({key_str}) as virtual modifier"
            )));
        }
        keys.push(key);
    }
    Ok(keys)
}

fn const_true() -> bool {
    true
}

pub fn deserialize_single_field<'de, D, T>(deserializer: D, name: &str) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let mut map = HashMap::<String, T>::deserialize(deserializer)?;

    if let Some(value) = map.remove(name) {
        if map.is_empty() {
            return Ok(value);
        }
    }

    Err(serde::de::Error::custom(format!("This error is never shown in an untagged enum")))
}
