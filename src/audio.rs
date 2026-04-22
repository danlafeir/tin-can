/// Audio pipeline: mic → Opus encode → data channel → Opus decode → speakers.
/// Compiled only when the `voice` feature is enabled; the stub variant compiles
/// without cpal/opus so peer::run() can always take Option<AudioPipeline>.

#[cfg(feature = "voice")]
pub use voice::AudioPipeline;

#[cfg(not(feature = "voice"))]
pub use stub::AudioPipeline;

// ── Voice implementation ─────────────────────────────────────────────────────

#[cfg(feature = "voice")]
mod voice {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use anyhow::{Context, Result};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{BufferSize, Stream, StreamConfig};

    pub const SAMPLE_RATE: u32 = 48_000;
    pub const FRAME_SAMPLES: usize = 960; // 20 ms at 48 kHz mono
    const MAX_CAPTURE_SAMPLES: usize = SAMPLE_RATE as usize * 2; // 2 s back-log cap

    pub struct AudioPipeline {
        capture_buf: Arc<Mutex<Vec<f32>>>,
        playback_buf: Arc<Mutex<VecDeque<f32>>>,
        encoder: opus::Encoder,
        decoder: opus::Decoder,
        _input_stream: Stream,
        _output_stream: Stream,
    }

    impl AudioPipeline {
        pub fn new() -> Result<Self> {
            let host = cpal::default_host();
            let in_dev = host.default_input_device().context("no microphone found")?;
            let out_dev = host.default_output_device().context("no speakers found")?;

            let in_def = in_dev
                .default_input_config()
                .context("query input config")?;
            let out_def = out_dev
                .default_output_config()
                .context("query output config")?;

            let in_ch = in_def.channels() as usize;
            let out_ch = out_def.channels() as usize;

            // Request 48 kHz from cpal; CoreAudio / PipeWire handle the resampling.
            let in_config = StreamConfig {
                channels: in_def.channels(),
                sample_rate: SAMPLE_RATE,
                buffer_size: BufferSize::Default,
            };
            let out_config = StreamConfig {
                channels: out_def.channels(),
                sample_rate: SAMPLE_RATE,
                buffer_size: BufferSize::Default,
            };

            let capture_buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
            let playback_buf: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));

            // Capture: mix multichannel down to mono before buffering.
            let cap = capture_buf.clone();
            let input_stream = in_dev
                .build_input_stream(
                    &in_config,
                    move |data: &[f32], _| {
                        let mono: Vec<f32> = if in_ch == 1 {
                            data.to_vec()
                        } else {
                            data.chunks(in_ch)
                                .map(|ch| ch.iter().sum::<f32>() / in_ch as f32)
                                .collect()
                        };
                        let mut buf = cap.lock().unwrap();
                        if buf.len() < MAX_CAPTURE_SAMPLES {
                            buf.extend(mono);
                        }
                    },
                    |e| eprintln!("mic error: {e}"),
                    None,
                )
                .context("build input stream")?;

            // Playback: duplicate mono sample to all output channels.
            let play = playback_buf.clone();
            let output_stream = out_dev
                .build_output_stream(
                    &out_config,
                    move |data: &mut [f32], _| {
                        let mut buf = play.lock().unwrap();
                        for frame in data.chunks_mut(out_ch) {
                            let s = buf.pop_front().unwrap_or(0.0);
                            for ch in frame.iter_mut() {
                                *ch = s;
                            }
                        }
                    },
                    |e| eprintln!("speaker error: {e}"),
                    None,
                )
                .context("build output stream")?;

            input_stream.play().context("start microphone")?;
            output_stream.play().context("start speakers")?;

            let encoder =
                opus::Encoder::new(SAMPLE_RATE, opus::Channels::Mono, opus::Application::Voip)
                    .context("create opus encoder")?;
            let decoder =
                opus::Decoder::new(SAMPLE_RATE, opus::Channels::Mono)
                    .context("create opus decoder")?;

            Ok(Self {
                capture_buf,
                playback_buf,
                encoder,
                decoder,
                _input_stream: input_stream,
                _output_stream: output_stream,
            })
        }

        /// Encode one 20 ms frame from the capture buffer. Returns None if not
        /// enough samples have accumulated yet.
        pub fn encode_frame(&mut self) -> Result<Option<Vec<u8>>> {
            let mut buf = self.capture_buf.lock().unwrap();
            if buf.len() < FRAME_SAMPLES {
                return Ok(None);
            }
            let frame: Vec<f32> = buf.drain(..FRAME_SAMPLES).collect();
            drop(buf);
            let mut encoded = vec![0u8; 4000];
            let n = self
                .encoder
                .encode_float(&frame, &mut encoded)
                .context("opus encode")?;
            encoded.truncate(n);
            Ok(Some(encoded))
        }

        /// Decode an incoming Opus packet into the playback buffer.
        pub fn decode_and_queue(&mut self, packet: &[u8]) -> Result<()> {
            let mut decoded = vec![0f32; FRAME_SAMPLES];
            let n = self
                .decoder
                .decode_float(packet, &mut decoded, false)
                .context("opus decode")?;
            self.playback_buf.lock().unwrap().extend(&decoded[..n]);
            Ok(())
        }
    }
}

// ── Stub (no voice feature) ──────────────────────────────────────────────────

#[cfg(not(feature = "voice"))]
mod stub {
    pub struct AudioPipeline;

    impl AudioPipeline {
        pub fn new() -> anyhow::Result<Self> {
            anyhow::bail!(
                "voice support is not compiled in — rebuild with: cargo build --features voice"
            )
        }

        pub fn encode_frame(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
            unreachable!()
        }

        pub fn decode_and_queue(&mut self, _: &[u8]) -> anyhow::Result<()> {
            unreachable!()
        }
    }
}
