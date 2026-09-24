use crate::config::deserialize_single_field;
use crate::config::deserializers::{deserialize_duration, deserialize_key, DurationWrapper, VectorOrSingleOrNull};
use crate::config::modmap::KeyWrapper;
use crate::config::modmap_operator::Keys;
use evdev::KeyCode as Key;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ExpmapOperator {
    DoubleTap(DoubleTap),
    #[serde(deserialize_with = "deserialize_throttle")]
    Throttle(Duration),
    #[serde(deserialize_with = "deserialize_oneshot")]
    OneShot(Key),
    #[serde(deserialize_with = "deserialize_select")]
    Select(Vec<ExpmapOperator>),
    #[serde(deserialize_with = "deserialize_tap_hold_next_release")]
    TapHoldNextRelease(TapHoldNextRelease),
}

pub fn deserialize_throttle<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    Ok(deserialize_single_field::<D, DurationWrapper>(deserializer, "throttle_ms")?.0)
}

pub fn deserialize_oneshot<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Key, D::Error> {
    Ok(deserialize_single_field::<D, KeyWrapper>(deserializer, "oneshot")?.0)
}

pub fn deserialize_select<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ExpmapOperator>, D::Error> {
    Ok(deserialize_single_field::<D, Vec<ExpmapOperator>>(deserializer, "select")?)
}

pub fn deserialize_tap_hold_next_release<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<TapHoldNextRelease, D::Error> {
    let mut map = HashMap::<String, TapHoldNextRelease>::deserialize(deserializer)?;
    if let Some(value) = map.remove("tap_hold_next_release").or_else(|| map.remove("tap-hold-next-release")) {
        if map.is_empty() {
            return Ok(value);
        }
    }
    Err(serde::de::Error::custom("expected tap_hold_next_release or tap-hold-next-release"))
}

pub fn deserialize_key_or_keys<'de, D>(deserializer: D) -> Result<Vec<Key>, D::Error>
where
    D: Deserializer<'de>,
{
    let keys = Keys::deserialize(deserializer)?;
    Ok(keys.into_vec())
}

pub fn deserialize_optional_key_or_keys<'de, D>(deserializer: D) -> Result<Option<Vec<Key>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<Keys>::deserialize(deserializer)?;
    Ok(opt.map(|k| k.into_vec()))
}

#[derive(Clone, Debug, Deserialize)]
pub struct TapHoldNextRelease {
    #[serde(deserialize_with = "deserialize_key_or_keys")]
    pub tap: Vec<Key>,
    #[serde(deserialize_with = "deserialize_key_or_keys")]
    pub hold: Vec<Key>,
    #[serde(
        default = "default_tap_hold_timeout",
        alias = "timeout_ms",
        deserialize_with = "deserialize_duration"
    )]
    pub timeout: Duration,
    #[serde(
        default,
        alias = "timeout_key",
        alias = "timeout-button",
        deserialize_with = "deserialize_optional_key_or_keys"
    )]
    pub timeout_button: Option<Vec<Key>>,
}

fn default_tap_hold_timeout() -> Duration {
    Duration::from_millis(200)
}

#[derive(Clone, Debug, Deserialize)]
pub struct DoubleTap {
    #[serde(rename = "double", deserialize_with = "deserialize_expmap_actions")]
    pub actions: Vec<ExpmapAction>,
    #[serde(default = "default_dbltap_timeout", deserialize_with = "deserialize_duration")]
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ExpmapAction {
    #[serde(deserialize_with = "deserialize_key")]
    Key(Key),
}

pub fn deserialize_expmap_actions<'de, D>(deserializer: D) -> Result<Vec<ExpmapAction>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(VectorOrSingleOrNull::deserialize(deserializer)?.into_vec())
}

fn default_dbltap_timeout() -> Duration {
    Duration::from_millis(200)
}
