use crate::config::application::ApplicationMatch;
use crate::config::deserializers::deserialize_string_or_vec;
use crate::config::device::DeviceMatcher;
use crate::config::modmap::KeyWrapper;
use crate::config::{expmap_simkey::Simkey, stage_operator::StageOperator};
use evdev::KeyCode as Key;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageEntry {
    #[allow(dead_code)]
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub chords: Vec<Simkey>,
    #[serde(default, deserialize_with = "deserialize_experimental_remap")]
    pub remap: IndexMap<Key, StageOperator>,
    pub application: Option<ApplicationMatch>,
    pub window: Option<ApplicationMatch>,
    pub device: Option<DeviceMatcher>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub mode: Option<Vec<String>>,
}

fn deserialize_experimental_remap<'de, D>(deserializer: D) -> Result<IndexMap<Key, StageOperator>, D::Error>
where
    D: Deserializer<'de>,
{
    let remap = IndexMap::<KeyWrapper, StageOperator>::deserialize(deserializer)?;
    Ok(remap
        .into_iter()
        .map(|(KeyWrapper(key), actions)| (key, actions))
        .collect())
}
