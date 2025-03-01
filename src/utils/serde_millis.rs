use std::time::Duration;

use serde::{Deserialize, Deserializer};

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    let millisecs: u64 = Deserialize::deserialize(deserializer)?;
    Ok(Duration::from_millis(millisecs))
}
