use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

const SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;
const FRAME_SAMPLES: u32 = 1280;

#[derive(Debug, Serialize)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub device_id: String,
    pub name: String,
    pub inputs: u16,
    pub outputs: u16,
    pub default_rate: Option<u32>,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

pub fn list_devices() -> Result<Vec<AudioDeviceInfo>, Box<dyn std::error::Error>> {
    let host = cpal::default_host();

    let default_input = host.default_input_device();
    let default_output = host.default_output_device();

    let default_input_name = default_input.as_ref().and_then(|d| d.description().ok()).map(|d| d.to_string());
    let default_output_name = default_output.as_ref().and_then(|d| d.description().ok()).map(|d| d.to_string());

    let mut devices = Vec::new();

    for (index, device) in host.devices()?.enumerate() {
        let name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "Unknown".into());
        let device_id = device
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| format!("unknown-{}",index));
        let inputs = device
            .supported_input_configs()
            .map(|configs| configs.map(|c| c.channels()).max().unwrap_or(0))
            .unwrap_or(0);
        let outputs = device
            .supported_output_configs()
            .map(|configs| configs.map(|c| c.channels()).max().unwrap_or(0))
            .unwrap_or(0);
        let default_rate = device
            .default_input_config()
            .or_else(|_| device.default_output_config())
            .ok()
            .map(|c| c.sample_rate());
        let is_default_input = default_input_name.as_ref() == Some(&name);
        let is_default_output = default_output_name.as_ref() == Some(&name);

        devices.push(AudioDeviceInfo {
            index,
            device_id,
            name,
            inputs,
            outputs,
            default_rate,
            is_default_input,
            is_default_output,
        });
    }

    Ok(devices)
}
