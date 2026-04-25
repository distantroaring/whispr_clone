use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Sample, SampleFormat, Stream,
};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use uuid::Uuid;

pub struct Recorder {
    active: Option<ActiveRecording>,
}

struct ActiveRecording {
    stream: Stream,
    writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    path: PathBuf,
}

impl Recorder {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.active.is_some() {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no input microphone found"))?;
        let supported_config = device.default_input_config()?;
        let config: cpal::StreamConfig = supported_config.clone().into();
        let path = std::env::temp_dir().join(format!("whispr-{}.wav", Uuid::new_v4()));
        let writer = Arc::new(Mutex::new(Some(WavWriter::create(
            &path,
            WavSpec {
                channels: config.channels,
                sample_rate: config.sample_rate.0,
                bits_per_sample: 16,
                sample_format: WavSampleFormat::Int,
            },
        )?)));

        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, writer.clone())?,
            SampleFormat::I16 => build_stream::<i16>(&device, &config, writer.clone())?,
            SampleFormat::U16 => build_stream::<u16>(&device, &config, writer.clone())?,
            _ => anyhow::bail!("unsupported microphone sample format"),
        };

        stream.play()?;
        self.active = Some(ActiveRecording {
            stream,
            writer,
            path,
        });
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let Some(active) = self.active.take() else {
            return Ok(None);
        };

        drop(active.stream);
        std::thread::sleep(Duration::from_millis(80));
        if let Some(writer) = active
            .writer
            .lock()
            .expect("recorder writer poisoned")
            .take()
        {
            writer.finalize()?;
        }

        Ok(Some(active.path))
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,
) -> anyhow::Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample,
    i16: cpal::FromSample<T>,
{
    let err_fn = |error| eprintln!("audio input stream error: {error}");
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            if let Ok(mut guard) = writer.lock() {
                if let Some(writer) = guard.as_mut() {
                    for sample in data {
                        let converted: i16 = i16::from_sample(*sample);
                        let _ = writer.write_sample(converted);
                    }
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
