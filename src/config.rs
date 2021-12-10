use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub sound: SoundConfig,
}

impl Config {
    pub fn new() -> Config {
        Config::default()
    }

    pub fn from_json(json_content: &str) -> Result<Config, Error> {
        let mut config: Config = serde_json::from_str(json_content)?;
        config.sound.volume = config.sound.volume.clamp(0.0, 1.0);
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sound: SoundConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SoundConfig {
    pub enabled: bool,
    pub volume: f32,
}

impl SoundConfig {
    pub fn new() -> SoundConfig {
        SoundConfig::default()
    }
}

impl Default for SoundConfig {
    fn default() -> Self {
        SoundConfig {
            enabled: true,
            volume: 1.0,
        }
    }
}
