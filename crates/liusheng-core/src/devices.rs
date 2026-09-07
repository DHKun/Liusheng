//! Read-only output discovery. PCM devices are opened only for playback.
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct OutputDevice {
    pub device: String,
    pub mixer: String,
    pub name: String,
}
#[cfg(target_os = "linux")]
pub fn output_devices() -> Vec<OutputDevice> {
    let Ok(pcms) = std::fs::read_to_string("/proc/asound/pcm") else {
        return Vec::new();
    };
    pcms.lines()
        .filter_map(|line| {
            if !line.contains("playback") {
                return None;
            }
            let (address, description) = line.split_once(':')?;
            let (card, device) = address.split_once('-')?;
            let card: usize = card.parse().ok()?;
            let device: usize = device.parse().ok()?;
            let id = std::fs::read_to_string(format!("/proc/asound/card{card}/id"))
                .ok()
                .map(|s| s.trim().to_owned())
                .unwrap_or(card.to_string());
            Some(OutputDevice {
                device: format!("hw:{id},{device}"),
                mixer: format!("hw:{id}"),
                name: description.trim().to_owned(),
            })
        })
        .collect()
}
#[cfg(not(target_os = "linux"))]
pub fn output_devices() -> Vec<OutputDevice> {
    Vec::new()
}
