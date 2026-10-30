use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig, SupportedStreamConfig};
use hound::{WavSpec, WavWriter};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufWriter;
use std::sync::mpsc::{self, Receiver, Sender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHANNELS: u16 = 1;
pub const FRAME_SAMPLES: usize = 1280; // 80 ms at 16 kHz

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub index: usize,
    pub name: String,
    pub inputs: u16,
    pub outputs: u16,
    pub default_rate: f64,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

/// Return every audio device the host can see.
pub fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let host = cpal::default_host();
    let default_input = host.default_input_device();
    let default_output = host.default_output_device();

    let mut devices = Vec::new();

    // We enumerate input + output devices. cpal doesn't give a single global index,
    // so we build our own stable list.
    let mut index = 0;

    if let Ok(input_devices) = host.input_devices() {
        for dev in input_devices {
            let name = dev.name().unwrap_or_else(|_| "Unknown".into());
            let max_in = supported_channels(&dev, true);
            let max_out = supported_channels(&dev, false);
            let default_rate = default_sample_rate(&dev);
            let is_default_input = default_input
                .as_ref()
                .and_then(|d| d.name().ok())
                .map(|n| n == name)
                .unwrap_or(false);

            devices.push(DeviceInfo {
                index,
                name,
                inputs: max_in,
                outputs: max_out,
                default_rate,
                is_default_input,
                is_default_output: false,
            });
            index += 1;
        }
    }

    if let Ok(output_devices) = host.output_devices() {
        for dev in output_devices {
            let name = dev.name().unwrap_or_else(|_| "Unknown".into());
            // Avoid duplicating devices that already appeared as inputs
            if devices.iter().any(|d| d.name == name) {
                // just mark it as also having outputs if needed
                if let Some(existing) = devices.iter_mut().find(|d| d.name == name) {
                    existing.outputs = supported_channels(&dev, false);
                    existing.is_default_output = default_output
                        .as_ref()
                        .and_then(|d| d.name().ok())
                        .map(|n| n == name)
                        .unwrap_or(false);
                }
                continue;
            }

            let max_in = supported_channels(&dev, true);
            let max_out = supported_channels(&dev, false);
            let default_rate = default_sample_rate(&dev);
            let is_default_output = default_output
                .as_ref()
                .and_then(|d| d.name().ok())
                .map(|n| n == name)
                .unwrap_or(false);

            devices.push(DeviceInfo {
                index,
                name,
                inputs: max_in,
                outputs: max_out,
                default_rate,
                is_default_input: false,
                is_default_output,
            });
            index += 1;
        }
    }

    Ok(devices)
}

fn supported_channels(device: &cpal::Device, input: bool) -> u16 {
    if input {
        device.supported_input_configs()
            .ok()
            .and_then(|mut c| c.next())
            .map(|c| c.channels())
            .unwrap_or(0)
    } else {
        device.supported_output_configs()
            .ok()
            .and_then(|mut c| c.next())
            .map(|c| c.channels())
            .unwrap_or(0)
    }
}

fn default_sample_rate(device: &Device) -> f64 {
    device
        .default_input_config()
        .or_else(|_| device.default_output_config())
        .map(|c| c.sample_rate().0 as f64)
        .unwrap_or(SAMPLE_RATE as f64)
}

/// Play float32 mono audio and block until it finishes.
pub fn play(samples: &[f32], rate: u32) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no default output device")?;

    let config = StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let samples = samples.to_vec();
    let sample_count = samples.len();
    let pos = Arc::new(Mutex::new(0usize));
    let pos2 = pos.clone();

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                let mut p = pos2.lock().unwrap();
                for sample in data.iter_mut() {
                    *sample = if *p < sample_count {
                        let s = samples[*p];
                        *p += 1;
                        s
                    } else {
                        0.0
                    };
                }
            },
            |err| eprintln!("play stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // Wait until all samples have been consumed
    let duration = Duration::from_secs_f64(sample_count as f64 / rate as f64);
    thread::sleep(duration + Duration::from_millis(50));

    Ok(())
}

/// Record a fixed duration. Returns float32 mono, range [-1, 1].
pub fn record(seconds: f32, device_index: Option<usize>) -> Result<Vec<f32>, String> {
    let host = cpal::default_host();
    let device = select_input_device(&host, device_index)?;

    let config = StreamConfig {
        channels: CHANNELS,
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Default,
    };

    let total_samples = (seconds * SAMPLE_RATE as f32) as usize;
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let collected = Arc::new(Mutex::new(Vec::with_capacity(total_samples)));
    let collected2 = collected.clone();

    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                let mut buf = collected2.lock().unwrap();
                if buf.len() < total_samples {
                    let remaining = total_samples - buf.len();
                    let take = data.len().min(remaining);
                    buf.extend_from_slice(&data[..take]);
                }
                // signal when done
                if buf.len() >= total_samples {
                    let _ = tx.send(Vec::new()); // just a wakeup
                }
            },
            |err| eprintln!("record stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // Block until we have enough samples (or timeout)
    let timeout = Duration::from_secs_f32(seconds + 1.0);
    let _ = rx.recv_timeout(timeout);

    let samples = collected.lock().unwrap().clone();
    Ok(samples)
}

/// Yield 80 ms mono float32 frames from the mic until the iterator is dropped.
///
/// Usage:
/// ```ignore
/// for frame in frames(None)? {
///     // frame: Vec<f32> of length FRAME_SAMPLES
/// }
/// ```
pub fn frames(device_index: Option<usize>) -> Result<FrameIterator, String> {
    let host = cpal::default_host();
    let device = select_input_device(&host, device_index)?;

    let config = StreamConfig {
        channels: CHANNELS,
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(FRAME_SAMPLES as u32),
    };

    // Bounded channel – drop frames if the consumer is too slow
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(100);

    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                // cpal may give us interleaved data; we requested mono so data is already mono
                let frame = data.to_vec();
                match tx.try_send(frame) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        // drop rather than block the audio callback
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        // consumer gone
                    }
                }
            },
            |err| eprintln!("audio status: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    Ok(FrameIterator {
        rx,
        _stream: stream, // keep the stream alive
    })
}

pub struct FrameIterator {
    rx: Receiver<Vec<f32>>,
    _stream: cpal::Stream,
}

impl Iterator for FrameIterator {
    type Item = Vec<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        self.rx.recv().ok()
    }
}

/// A short, soft beep as float32 mono at SAMPLE_RATE.
pub fn chime(freq: f32, seconds: f32) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * seconds) as usize;
    let mut samples = Vec::with_capacity(n);

    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let tone = (2.0 * std::f32::consts::PI * freq * t).sin();
        // 10 ms fade in/out
        let fade = (t.min(seconds - t) / 0.01).min(1.0);
        samples.push(0.3 * tone * fade);
    }
    samples
}

/// Save float32 mono audio as a 16-bit WAV file.
pub fn save_wav(path: &str, audio: &[f32], rate: u32) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec).map_err(|e| e.to_string())?;

    for &sample in audio {
        let clipped = sample.clamp(-1.0, 1.0);
        let pcm = (clipped * 32767.0) as i16;
        writer.write_sample(pcm).map_err(|e| e.to_string())?;
    }

    writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}


fn select_input_device(host: &cpal::Host, index: Option<usize>) -> Result<Device, String> {
    if let Some(idx) = index {
        let devices: Vec<_> = host
            .input_devices()
            .map_err(|e| e.to_string())?
            .collect();
        devices
            .into_iter()
            .nth(idx)
            .ok_or_else(|| format!("input device index {idx} not found"))
    } else {
        host.default_input_device()
            .ok_or_else(|| "no default input device".into())
    }
}
