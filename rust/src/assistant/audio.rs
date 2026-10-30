use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, SampleRate, StreamConfig};
use rubato::{FftFixedIn, Resampler};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHANNELS: u16 = 1;
pub const FRAME_SAMPLES: usize = 1280; // 80 ms @ 16 kHz

pub type AudioResult<T> = Result<T, String>;

// Device listing

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub inputs: u16,
    pub outputs: u16,
    pub default_rate: Option<u32>,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

/// Return every audio device the host can see.
pub fn list_devices() -> AudioResult<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let default_input = host
        .default_input_device()
        .and_then(|d| d.name().ok());
    let default_output = host
        .default_output_device()
        .and_then(|d| d.name().ok());

    let mut devices = Vec::new();
    let device_iter = host
        .devices()
        .map_err(|e| format!("failed to enumerate devices: {e}"))?;

    for (index, device) in device_iter.enumerate() {
        let name = device.name().unwrap_or_else(|_| "<unknown>".into());
        let inputs = device
            .default_input_config()
            .map(|c| c.channels())
            .unwrap_or(0);
        let outputs = device
            .default_output_config()
            .map(|c| c.channels())
            .unwrap_or(0);

        let default_rate = device
            .default_input_config()
            .or_else(|_| device.default_output_config())
            .ok()
            .map(|c| c.sample_rate().0);

        devices.push(AudioDeviceInfo {
            index,
            is_default_input: default_input.as_ref() == Some(&name),
            is_default_output: default_output.as_ref() == Some(&name),
            name,
            inputs,
            outputs,
            default_rate,
        });
    }
    Ok(devices)
}

// Playback

/// Play float32 mono audio and block until it finishes.
pub fn play(samples: &[f32], rate: u32) -> AudioResult<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;

    let config = device
        .default_output_config()
        .map_err(|e| format!("default output config: {e}"))?;
    let sample_format = config.sample_format();
    let mut stream_config: StreamConfig = config.into();
    stream_config.channels = 1; // we always feed mono

    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(2);
    let finished = Arc::new(Mutex::new(false));
    let finished_flag = finished.clone();

    let samples = samples.to_vec();
    let device_rate = stream_config.sample_rate.0;

    let stream = match sample_format {
        SampleFormat::F32 => build_output_stream_f32(&device, stream_config, rx, finished_flag)?,
        SampleFormat::I16 => build_output_stream_i16(&device, stream_config, rx, finished_flag)?,
        other => return Err(format!("unsupported output sample format: {other:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("failed to start output stream: {e}"))?;

    // Resample if necessary, then send the whole buffer
    let to_play = if device_rate == rate {
        samples
    } else {
        resample_f32(&samples, rate, device_rate)?
    };
    let _ = tx.send(to_play);

    // Wait until the callback has drained the buffer
    while !*finished.lock().unwrap() {
        thread::sleep(Duration::from_millis(5));
    }
    // Give the last buffers a moment to play out
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

fn build_output_stream_f32(
    device: &cpal::Device,
    config: StreamConfig,
    rx: Receiver<Vec<f32>>,
    finished: Arc<Mutex<bool>>,
) -> AudioResult<cpal::Stream> {
    let mut buffer: VecDeque<f32> = VecDeque::new();
    let mut received = false;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                if !received {
                    if let Ok(chunk) = rx.try_recv() {
                        buffer.extend(chunk);
                        received = true;
                    }
                }
                for sample in data.iter_mut() {
                    *sample = buffer.pop_front().unwrap_or(0.0);
                }
                if received && buffer.is_empty() {
                    *finished.lock().unwrap() = true;
                }
            },
            |err| eprintln!("output stream error: {err}"),
            None,
        )
        .map_err(|e| format!("build output stream (f32): {e}"))?;
    Ok(stream)
}

fn build_output_stream_i16(
    device: &cpal::Device,
    config: StreamConfig,
    rx: Receiver<Vec<f32>>,
    finished: Arc<Mutex<bool>>,
) -> AudioResult<cpal::Stream> {
    let mut buffer: VecDeque<f32> = VecDeque::new();
    let mut received = false;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [i16], _| {
                if !received {
                    if let Ok(chunk) = rx.try_recv() {
                        buffer.extend(chunk);
                        received = true;
                    }
                }
                for sample in data.iter_mut() {
                    let s = buffer.pop_front().unwrap_or(0.0).clamp(-1.0, 1.0);
                    *sample = (s * i16::MAX as f32) as i16;
                }
                if received && buffer.is_empty() {
                    *finished.lock().unwrap() = true;
                }
            },
            |err| eprintln!("output stream error: {err}"),
            None,
        )
        .map_err(|e| format!("build output stream (i16): {e}"))?;
    Ok(stream)
}

// Fixed-length recording

/// Record a fixed duration. Returns float32 mono, range [-1, 1].
pub fn record(seconds: f32, device_index: Option<usize>) -> AudioResult<Vec<f32>> {
    let host = cpal::default_host();
    let device = select_input_device(&host, device_index)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("default input config: {e}"))?;
    let sample_format = config.sample_format();
    let mut stream_config: StreamConfig = config.into();
    let device_channels = stream_config.channels;

    let target_samples = (seconds * SAMPLE_RATE as f32).round() as usize;
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = tx.clone();
            device
                .build_input_stream(
                    stream_config,
                    move |data: &[f32], _| {
                        let mono = downmix_to_mono(data, device_channels as usize);
                        let _ = tx.send(mono);
                    },
                    |err| eprintln!("input stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("build input stream (f32): {e}"))?
        }
        SampleFormat::I16 => {
            let tx = tx.clone();
            device
                .build_input_stream(
                    stream_config,
                    move |data: &[i16], _| {
                        let f32s: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        let mono = downmix_to_mono(&f32s, device_channels as usize);
                        let _ = tx.send(mono);
                    },
                    |err| eprintln!("input stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("build input stream (i16): {e}"))?
        }
        other => return Err(format!("unsupported input sample format: {other:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("failed to start input stream: {e}"))?;

    let device_rate = stream_config.sample_rate.0;
    let needed = (target_samples as f64 * device_rate as f64 / SAMPLE_RATE as f64) as usize;
    let mut collected = Vec::with_capacity(needed);

    while collected.len() < needed {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(chunk) => collected.extend(chunk),
            Err(_) => break,
        }
    }
    drop(stream); // stop capture

    // Resample to 16 kHz if needed
    if device_rate != SAMPLE_RATE {
        let mut out = resample_f32(&collected, device_rate, SAMPLE_RATE)?;
        out.truncate(target_samples);
        Ok(out)
    } else {
        collected.truncate(target_samples);
        Ok(collected)
    }
}

// Continuous 80 ms frames

/// Yield 80 ms mono float32 frames from the mic until the iterator is dropped.
///
/// The returned iterator owns the stream; dropping it releases the microphone.
pub fn frames(device_index: Option<usize>) -> AudioResult<FramesIter> {
    let host = cpal::default_host();
    let device = select_input_device(&host, device_index)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("default input config: {e}"))?;
    let sample_format = config.sample_format();
    let mut stream_config: StreamConfig = config.into();
    let device_channels = stream_config.channels;
    let device_rate = stream_config.sample_rate.0;

    // Ask for roughly 80 ms blocks when possible
    stream_config.buffer_size = BufferSize::Fixed(FRAME_SAMPLES as u32);

    let (tx, rx): (SyncSender<Vec<f32>>, Receiver<Vec<f32>>) = mpsc::sync_channel(32);

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = tx.clone();
            device
                .build_input_stream(
                    stream_config,
                    move |data: &[f32], _| {
                        let mono = downmix_to_mono(data, device_channels as usize);
                        let _ = tx.try_send(mono); // drop on overflow rather than block
                    },
                    |err| eprintln!("input stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("build input stream (f32): {e}"))?
        }
        SampleFormat::I16 => {
            let tx = tx.clone();
            device
                .build_input_stream(
                    stream_config,
                    move |data: &[i16], _| {
                        let f32s: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        let mono = downmix_to_mono(&f32s, device_channels as usize);
                        let _ = tx.try_send(mono);
                    },
                    |err| eprintln!("input stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("build input stream (i16): {e}"))?
        }
        other => return Err(format!("unsupported input sample format: {other:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("failed to start input stream: {e}"))?;

    // Build a resampler if the device rate ≠ 16 kHz
    let resampler = if device_rate != SAMPLE_RATE {
        Some(build_resampler(device_rate, SAMPLE_RATE)?)
    } else {
        None
    };

    Ok(FramesIter {
        _stream: stream, // keep the stream alive
        rx,
        resampler,
        leftover: Vec::new(),
        device_rate,
    })
}

pub struct FramesIter {
    _stream: cpal::Stream,
    rx: Receiver<Vec<f32>>,
    resampler: Option<FftFixedIn<f32>>,
    leftover: Vec<f32>,
    device_rate: u32,
}

impl Iterator for FramesIter {
    type Item = Vec<f32>; // always exactly FRAME_SAMPLES of 16 kHz mono f32

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Pull more data from the mic if we don't have a full frame yet
            while self.leftover.len() < FRAME_SAMPLES {
                let chunk = self.rx.recv().ok()?; // stream ended / error
                let resampled = if let Some(r) = self.resampler.as_mut() {
                    match r.process(&[&chunk], None) {
                        Ok(out) => out.into_iter().next().unwrap_or_default(),
                        Err(_) => chunk, // fallback: use as-is
                    }
                } else {
                    chunk
                };
                self.leftover.extend(resampled);
            }

            // Emit one exact frame
            let frame: Vec<f32> = self.leftover.drain(..FRAME_SAMPLES).collect();
            return Some(frame);
        }
    }
}

// Chime

/// A short, soft beep as float32 mono at SAMPLE_RATE.
pub fn chime(freq: f32, seconds: f32) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * seconds).round() as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let tone = (2.0 * std::f32::consts::PI * freq * t).sin();
        // 10 ms linear fade in/out
        let fade = (t.min(seconds - t) / 0.01).min(1.0);
        out.push(0.3 * tone * fade);
    }
    out
}

// Helpers

fn select_input_device(host: &cpal::Host, index: Option<usize>) -> AudioResult<cpal::Device> {
    if let Some(idx) = index {
        host.input_devices()
            .map_err(|e| format!("input devices: {e}"))?
            .nth(idx)
            .ok_or_else(|| format!("input device index {idx} not found"))
    } else {
        host.default_input_device()
            .ok_or_else(|| "no default input device".to_string())
    }
}

fn downmix_to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn build_resampler(from_rate: u32, to_rate: u32) -> AudioResult<FftFixedIn<f32>> {
    FftFixedIn::new(
        from_rate as usize,
        to_rate as usize,
        1024, // chunk size used internally
        1,    // sub-chunks
        1,    // channels
    )
    .map_err(|e| format!("resampler init failed: {e}"))
}

fn resample_f32(input: &[f32], from_rate: u32, to_rate: u32) -> AudioResult<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(input.to_vec());
    }
    let mut resampler = build_resampler(from_rate, to_rate)?;
    let mut output = Vec::new();
    let block = 1024;
    for chunk in input.chunks(block) {
        let mut padded = chunk.to_vec();
        if padded.len() < block {
            padded.resize(block, 0.0);
        }
        let out = resampler
            .process(&[&padded], None)
            .map_err(|e| format!("resample failed: {e}"))?;
        if let Some(ch) = out.into_iter().next() {
            output.extend(ch);
        }
    }
    Ok(output)
}
