use super::device::DeviceMatcher;
use crate::config::application::ApplicationMatch;
use crate::config::deserializers::{deserialize_key, deserialize_string_or_vec};
use crate::config::expmap::Expmap;
use crate::config::expmap_operator::ExpmapOperator;
use crate::config::modmap_operator::ModmapOperator;
use evdev::KeyCode as Key;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modmap {
    #[allow(dead_code)]
    #[serde(default = "String::new")]
    pub name: String,
    #[serde(deserialize_with = "deserialize_remap")]
    pub remap: HashMap<Key, ModmapOperator>,
    pub application: Option<ApplicationMatch>,
    pub window: Option<ApplicationMatch>,
    pub device: Option<DeviceMatcher>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub mode: Option<Vec<String>>,
}

impl Modmap {
    // A modmap entry is a stage entry without chords.
    pub fn into_stage_entry(self) -> Expmap {
        Expmap {
            name: self.name,
            chords: vec![],
            remap: self
                .remap
                .into_iter()
                .map(|(key, operator)| {
                    let operator = match operator {
                        ModmapOperator::Keys(keys) => ExpmapOperator::Keys(keys),
                        ModmapOperator::MultiPurposeKey(config) => ExpmapOperator::MultiPurposeKey(config),
                        ModmapOperator::PressReleaseKey(config) => ExpmapOperator::PressReleaseKey(config),
                    };
                    (key, operator)
                })
                .collect(),
            application: self.application,
            window: self.window,
            device: self.device,
            mode: self.mode,
        }
    }
}

#[derive(Deserialize, Eq, Hash, PartialEq)]
pub struct KeyWrapper(#[serde(deserialize_with = "deserialize_key")] pub Key);

fn deserialize_remap<'de, D>(deserializer: D) -> Result<HashMap<Key, ModmapOperator>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = HashMap::<KeyWrapper, ModmapOperator>::deserialize(deserializer)?;
    Ok(v.into_iter().map(|(KeyWrapper(k), v)| (k, v)).collect())
}
