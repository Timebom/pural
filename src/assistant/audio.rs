use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};
use crossbeam_channel::{bounded, Receiver};
use std::error::Error;
use std::thread;
use std::time::Duration;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHANNELS: u16 = 1;
pub const FRAME_SAMPLES: usize = 1280;

type AudioResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Information about an audio device.
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub inputs: u16,
    pub outputs: u16,
    pub default_rate: f64,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

fn device_at(index: Option<usize>) -> AudioResult<Device> {
    let host = cpal::default_host();

    match index {
        Some(target) => host
            .devices()?
            .nth(target)
            .ok_or_else(|| format!("Audio device index {target} not found").into()),
        None => Err("Use the default input/output device explicitly".into()),
    }
}

fn input_device(index: Option<usize>) -> AudioResult<Device> {
    match index {
        Some(i) => device_at(Some(i)),
        None => cpal::default_host()
            .default_input_device()
            .ok_or_else(|| "No default input device available".into()),
    }
}

fn output_device(index: Option<usize>) -> AudioResult<Device> {
    match index {
        Some(i) => device_at(Some(i)),
        None => cpal::default_host()
            .default_output_device()
            .ok_or_else(|| "No default output device available".into()),
    }
}

/// List all audio devices visible to CPAL.
pub fn list_devices() -> AudioResult<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();

    let default_input = host
        .default_input_device()
        .and_then(|d| d.description().ok())
        .map(|d| d.to_string());

    let default_output = host
        .default_output_device()
        .and_then(|d| d.description().ok())
        .map(|d| d.to_string());

    let mut devices = Vec::new();

    for (index, device) in host.devices()?.enumerate() {
        let name = device
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "Unknown".into());

        let (inputs, default_rate) = device
            .default_input_config()
            .map(|c| (c.channels(), c.sample_rate() as f64))
            .unwrap_or((0, 0.0));

        let outputs = device
            .default_output_config()
            .map(|c| c.channels())
            .unwrap_or(0);

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

/// Play mono f32 audio and block until it finishes.
///
/// Uses the device's default output sample rate and resamples
/// the supplied audio if necessary.
pub fn play(samples: &[f32], rate: u32) -> AudioResult<()> {
    play_on_device(samples, rate, None)
}

pub fn play_on_device(
    samples: &[f32],
    rate: u32,
    device_index: Option<usize>,
) -> AudioResult<()> {
    if rate == 0 {
        return Err("Sample rate must be greater than zero".into());
    }

    let device = output_device(device_index)?;
    let supported = device.default_output_config()?;
    let output_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();

    // Linear interpolation if the device uses a different sample rate.
    let output_samples = resample(samples, rate, output_rate);

    let config: StreamConfig = supported.into();
    let mut position = 0usize;

    let err_fn = |err| eprintln!("Audio output error: {err}");

    macro_rules! play_typed {
        ($ty:ty, $convert:expr) => {{
            let stream = device.build_output_stream(
                config,
                move |data: &mut [$ty], _| {
                    for frame in data.chunks_mut(channels) {
                        let sample = output_samples
                            .get(position)
                            .copied()
                            .unwrap_or(0.0);

                        if position < output_samples.len() {
                            position += 1;
                        }

                        let converted: $ty = $convert(sample);

                        for channel in frame {
                            *channel = converted;
                        }
                    }
                },
                err_fn,
                None,
            )?;

            stream.play()?;

            let duration = Duration::from_secs_f64(
                samples.len() as f64 / rate as f64
            );

            thread::sleep(
                duration + Duration::from_millis(100)
            );

            drop(stream);
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }};
    }

    match sample_format {
        SampleFormat::F32 => play_typed!(f32, |s: f32| s),
        SampleFormat::I16 => play_typed!(
            i16,
            |s: f32| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
        ),
        SampleFormat::U16 => play_typed!(
            u16,
            |s: f32| ((s.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16
        ),
        format => Err(format!("Unsupported output sample format: {format:?}").into()),
    }
}

/// Simple linear resampling.
fn resample(samples: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    if input_rate == output_rate {
        return samples.to_vec();
    }

    let output_len =
        (samples.len() as u64 * output_rate as u64 / input_rate as u64) as usize;

    (0..output_len)
        .map(|i| {
            let position = i as f64 * input_rate as f64 / output_rate as f64;
            let left = (position.floor() as usize).min(samples.len() - 1);
            let right = (left + 1).min(samples.len() - 1);
            let fraction = (position - left as f64) as f32;

            samples[left] * (1.0 - fraction)
                + samples[right] * fraction
        })
        .collect()
}

/// Record a fixed duration of mono f32 audio.
pub fn record(seconds: f64, device_index: Option<usize>) -> AudioResult<Vec<f32>> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("Duration must be a finite, non-negative number".into());
    }

    let device = input_device(device_index)?;
    let supported = device.default_input_config()?;
    let input_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();

    let frames_needed = (seconds * SAMPLE_RATE as f64) as usize;
    let input_frames_needed =
        (seconds * input_rate as f64).ceil() as usize;

    let (tx, rx) = bounded::<Vec<f32>>(32);
    let err_fn = |err| eprintln!("Audio input error: {err}");

    macro_rules! record_typed {
        ($ty:ty, $convert:expr) => {{
            let stream = device.build_input_stream(
                StreamConfig {
                    channels: channels as u16,
                    sample_rate: input_rate,
                    buffer_size: cpal::BufferSize::Default,
                },
                move |data: &[$ty], _| {
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|frame| $convert(frame[0]))
                        .collect();

                    let _ = tx.try_send(mono);
                },
                err_fn,
                None,
            )?;

            stream.play()?;

            let mut input = Vec::with_capacity(input_frames_needed);

            while input.len() < input_frames_needed {
                match rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(chunk) => input.extend(chunk),
                    Err(e) => {
                        return Err(
                            format!("Recording failed or timed out: {e}").into()
                        );
                    }
                }
            }

            drop(stream);
            input.truncate(input_frames_needed);

            let mut output = resample(&input, input_rate, SAMPLE_RATE);
            output.truncate(frames_needed);

            if output.len() < frames_needed {
                output.resize(frames_needed, 0.0);
            }

            Ok::<Vec<f32>, Box<dyn Error + Send + Sync>>(output)
        }};
    }

    match sample_format {
        SampleFormat::F32 => record_typed!(f32, |s: f32| s),
        SampleFormat::I16 => record_typed!(
            i16,
            |s: i16| s as f32 / 32768.0
        ),
        SampleFormat::U16 => record_typed!(
            u16,
            |s: u16| (s as f32 - 32768.0) / 32768.0
        ),
        format => Err(format!("Unsupported input sample format: {format:?}").into()),
    }
}

/// Start continuous microphone capture.
///
/// Returns a receiver of 1280-sample mono frames. Each frame is 80 ms
/// at 16 kHz. If the bounded queue is full, incoming audio is dropped.
pub fn frames(device_index: Option<usize>) -> AudioResult<Receiver<Vec<f32>>> {
    let device = input_device(device_index)?;
    let supported = device.default_input_config()?;
    let input_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();

    let (tx, rx) = bounded::<Vec<f32>>(100);

    macro_rules! start_stream {
        ($ty:ty, $convert:expr) => {{
            let tx = tx.clone();
            let mut pending: Vec<f32> = Vec::new();

            device.build_input_stream(
                &StreamConfig {
                    channels: channels as u16,
                    sample_rate: input_rate,
                    buffer_size: cpal::BufferSize::Fixed(
                        FRAME_SAMPLES as u32
                    ),
                },
                move |data: &[$ty], _| {
                    for frame in data.chunks(channels) {
                        pending.push($convert(frame[0]));
                    }

                    while pending.len() >= FRAME_SAMPLES {
                        let chunk: Vec<f32> =
                            pending.drain(..FRAME_SAMPLES).collect();

                        // Drop the chunk instead of blocking the callback.
                        let _ = tx.try_send(chunk);
                    }
                },
                |err| eprintln!("Audio input error: {err}"),
                None,
            )?
        }};
    }

    // Keep the stream alive for as long as the receiver is used.
    // The stream itself must outlive this function.
    // Use AudioFrames below to manage this lifetime.
    let _ = sample_format;

    Err("Use AudioFrames::start() for continuous capture".into())
}

/// Continuous capture object that keeps the input stream alive.

pub struct AudioFrames {
    receiver: Receiver<Vec<f32>>,
    // Keep the stream alive until AudioFrames is dropped.
    _stream: cpal::Stream,
}

impl Iterator for AudioFrames {
    type Item = Vec<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

pub fn frames(device_index: Option<usize>) -> AudioResult<AudioFrames> {
    const FRAME_SAMPLES: usize = 1280;
    const SAMPLE_RATE: u32 = 16_000;
    const QUEUE_SIZE: usize = 100;

    let host = cpal::default_host();

    let device = match device_index {
        Some(index) => host
            .devices()?
            .nth(index)
            .ok_or_else(|| format!("Input device {index} not found"))?,
        None => host
            .default_input_device()
            .ok_or("No default microphone available")?,
    };

    let supported = device.default_input_config()?;
    let sample_format = supported.sample_format();
    let input_rate = supported.sample_rate();
    let channels = supported.channels() as usize;

    if input_rate != SAMPLE_RATE {
        return Err(format!(
            "Microphone sample rate is {input_rate} Hz; expected {SAMPLE_RATE} Hz"
        ).into());
    }

    let config = StreamConfig {
        channels: channels as u16,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Fixed(FRAME_SAMPLES as u32),
    };

    let (tx, rx) = bounded::<Vec<f32>>(QUEUE_SIZE);

    macro_rules! build_stream {
        ($sample_type:ty, $convert:expr) => {{
            let tx = tx.clone();
            let mut pending: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES * 2);

            device.build_input_stream(
                config,
                move |data: &[$sample_type], _| {
                    for sample_frame in data.chunks(channels) {
                        if let Some(&sample) = sample_frame.first() {
                            pending.push($convert(sample));
                        }
                    }

                    while pending.len() >= FRAME_SAMPLES {
                        let frame: Vec<f32> =
                            pending.drain(..FRAME_SAMPLES).collect();

                        // Never block the audio callback.
                        // Drop frames if the consumer falls behind.
                        let _ = tx.try_send(frame);
                    }
                },
                |err| eprintln!("Microphone stream error: {err}"),
                None,
            )?
        }};
    }

    let stream = match sample_format {
        SampleFormat::F32 => build_stream!(f32, |s: f32| s),
        SampleFormat::I16 => {
            build_stream!(i16, |s: i16| s as f32 / 32768.0)
        }
        SampleFormat::U16 => {
            build_stream!(u16, |s: u16| {
                (s as f32 - 32768.0) / 32768.0
            })
        }
        format => {
            return Err(
                format!("Unsupported microphone sample format: {format:?}").into()
            );
        }
    };

    stream.play()?;

    Ok(AudioFrames {
        receiver: rx,
        _stream: stream,
    })
}

/// Generate a short sine-wave beep with 10 ms fade-in/out.
pub fn chime(freq: f64, seconds: f64) -> Vec<f32> {
    let n = (SAMPLE_RATE as f64 * seconds.max(0.0)) as usize;

    (0..n)
        .map(|i| {
            let t = i as f64 / SAMPLE_RATE as f64;

            let tone = (2.0 * std::f64::consts::PI * freq * t).sin();

            let fade = (t.min(seconds - t) / 0.01)
                .clamp(0.0, 1.0);

            (0.3 * tone * fade) as f32
        })
        .collect()
}

/// Generate a chime with the Python function's default arguments.
pub fn default_chime() -> Vec<f32> {
    chime(880.0, 0.12)
}
