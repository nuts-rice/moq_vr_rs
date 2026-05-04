use serde::{Deserialize};

#[derive(Deserialize)]
struct Config {
    pub relay: RelayConfig,
    pub video: VideoConfig,
}


#[derive(Deserialize)]
struct RelayConfig {
    pub url: String,
}

#[derive(Deserialize)]
struct VideoConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bittrate: u32
}


#[derive(Deserialize)]
struct PoseConfig {
    pub hz: u32,
    pub viewer_id: String
}


impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::args().nth(1).unwrap_or_else(|| "config.toml".to_string());
        let text = std::fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", path, e))?;
        toml::from_str(&text).map_err(Into::into)
    }
}
