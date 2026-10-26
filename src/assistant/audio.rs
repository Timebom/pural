use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, StreamConfig};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHANNELS: u16 = 1;
pub const FRAME_SAMPLES: u32 = 1280; // 80 ms at 16 kHz

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub inputs: u16,
    pub outputs: u16,
    pub default_rate: Option<u32>,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

/// Return every audio device the system can see.
pub fn list_devices() -> Result<Vec<AudioDeviceInfo>, Box<dyn std::error::Error>> {
    let host = cpal::default_host();

    let default_input = host.default_input_device();
    let default_output = host.default_output_device();

    let default_input_name = default_input
        .as_ref()
        .and_then(|d| d.description().ok())
        .map(|d| d.to_string());

    let default_output_name = default_output
        .as_ref()
        .and_then(|d| d.description().ok())
        .map(|d| d.to_string());

    let mut devices = Vec::new();

    for (index, device) in host.devices()?.enumerate() {
        let name = device
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "Unknown".into());

        let id = device
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| format!("unknown-{}", index));

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
            id,
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

/// Play float32 mono audio and block until it finishes.
pub fn play(samples: &[f32], rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no default output device")?;

    let config = StreamConfig {
        channels: CHANNELS,
        sample_rate: rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let samples = samples.to_vec();
    let total = samples.len();
    let pos = Arc::new(Mutex::new(0usize));
    let pos2 = Arc::clone(&pos);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [f32], _| {
            let mut pos = pos2.lock().unwrap();
            for sample in data.iter_mut() {
                if *pos < total {
                    *sample = samples[*pos];
                    *pos += 1;
                } else {
                    *sample = 0.0;
                }
            }
        },
        |err| eprintln!("output stream error: {}", err),
        None,
    )?;

    stream.play()?;

    // Wait until all samples have been written
    let duration_ms = (total as f64 / rate as f64 * 1000.0) as u64 + 50;
    thread::sleep(Duration::from_millis(duration_ms));

    Ok(())
}

/// Unified get_device function to get right device for a device_index
pub fn get_device(device_index: Option<usize>) -> Result<Device, Box<dyn std::error::Error>> {
    let host = cpal::default_host();

    match device_index {
        Some(idx) => host
            .devices()?
            .nth(idx)
            .ok_or_else(|| "device index out of range".into()),
        None => host
            .default_input_device()
            .ok_or_else(|| "no default input device".into()),
    }
}

/// Record a fixed time. Returns float32 mono in range [-1, 1].
pub fn record(seconds: f32, device_index: Option<usize>) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let device = get_device(device_index)?;

    let config = StreamConfig {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };

    let target_samples = (seconds * SAMPLE_RATE as f32) as usize;
    let buffer = Arc::new(Mutex::new(Vec::with_capacity(target_samples)));
    let buffer2 = Arc::clone(&buffer);
    let done = Arc::new(Mutex::new(false));
    let done2 = Arc::clone(&done);

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _| {
            let mut buf = buffer2.lock().unwrap();
            if buf.len() >= target_samples {
                *done2.lock().unwrap() = true;
                return;
            }
            let remaining = target_samples - buf.len();
            let take = data.len().min(remaining);
            buf.extend_from_slice(&data[..take]);
        },
        |err| eprintln!("input stream error: {}", err),
        None,
    )?;

    stream.play()?;

    // Wait until we have enough samples
    while !*done.lock().unwrap() {
        thread::sleep(Duration::from_millis(10));
        let len = buffer.lock().unwrap().len();
        if len >= target_samples {
            break;
        }
    }

    let result = buffer.lock().unwrap().clone();
    Ok(result)
}

/// Yields 80 ms mono float32 frames from the mic.
/// Drop the returned `FrameStream` (or call `.stop()`) to end streaming.
/// Custom iterator struct that owns the active CPAL stream and the MPSC receiver channel.
pub struct FrameStream {
    _stream: cpal::Stream,
    receiver: Receiver<Vec<f32>>,
}

impl Iterator for FrameStream {
    type Item = Vec<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}
/// Frames is currently broken wait for fix does not work on (macos)
/// Yields mono float32 frames from the microphone (equivalent to Python's generator).
pub fn frames(device_index: Option<usize>) -> Result<FrameStream, Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = match device_index {
        Some(idx) => host.input_devices()?.nth(idx).ok_or("Device not found")?,
        None => host.default_input_device().ok_or("No default input device found")?,
    };

    let config = cpal::StreamConfig {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Fixed(FRAME_SAMPLES as u32),
    };

    // Bounded channel with maxsize=100, dropping frames if full (just like queue.Full pass)
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(100);

    let err_fn = |err| eprintln!("Audio status warning/error: {}", err);

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let _ = tx.try_send(data.to_vec());
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    Ok(FrameStream {
        _stream: stream,
        receiver: rx,
    })
}
