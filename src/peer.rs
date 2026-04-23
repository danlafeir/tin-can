use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};
use tracing::{debug, info, warn};

use crate::audio::AudioPipeline;
use crate::morse;
use crate::ui::ChatUi;

const AUDIO_TICK: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChatMode {
    Control, // attach-string: pipe only, mode chosen in-session
    Text,    // tap: morse text
    Voice,   // talk: voice call
}

pub fn run(
    mut rtc: Rtc,
    socket: UdpSocket,
    local_addr: SocketAddr,
    rx: mpsc::Receiver<Option<String>>,
    mut mode: ChatMode,
    mut audio: Option<AudioPipeline>,
    ui: ChatUi,
) -> Result<()> {
    let mut buf = vec![0u8; 2000];
    let mut channel: Option<ChannelId> = None;
    let mut connected = false;
    let mut next_audio_tick = Instant::now() + AUDIO_TICK;
    let mut mode_announced = false;

    loop {
        // ── Drain all pending outputs ────────────────────────────────────────
        let deadline = loop {
            match rtc.poll_output().context("poll_output")? {
                Output::Transmit(t) => {
                    socket
                        .send_to(&t.contents, t.destination)
                        .context("UDP send")?;
                }
                Output::Event(event) => {
                    handle_event(event, &mut channel, &mut connected, &mut mode, &mut audio, &ui)?;
                }
                Output::Timeout(deadline) => break deadline,
            }
        };

        if !rtc.is_alive() {
            info!("connection closed");
            break;
        }

        // ── Announce mode once after channel opens ───────────────────────────
        // Text and Voice peers send a control prefix so the attach-string peer
        // knows which mode to enter.
        if !mode_announced {
            if let Some(cid) = channel {
                let ctrl: Option<&[u8]> = match mode {
                    ChatMode::Text => Some(b"::tap"),
                    ChatMode::Voice => Some(b"::talk"),
                    ChatMode::Control => None,
                };
                if let Some(msg) = ctrl {
                    if let Some(mut ch) = rtc.channel(cid) {
                        ch.write(false, msg).context("send mode announcement")?;
                    }
                }
                mode_announced = true;
            }
        }

        // ── Outgoing messages / commands from the input thread ───────────────
        if let Some(cid) = channel {
            loop {
                match rx.try_recv() {
                    Ok(Some(msg)) => match mode {
                        ChatMode::Control => match msg.trim() {
                            "tap" => {
                                if let Some(mut ch) = rtc.channel(cid) {
                                    ch.write(false, b"::tap").context("send tap")?;
                                }
                                mode = ChatMode::Text;
                                ui.print_message(&[
                                    "Switching to tap mode. Type a message.".to_string(),
                                ]);
                            }
                            "talk" => {
                                if let Some(mut ch) = rtc.channel(cid) {
                                    ch.write(false, b"::talk").context("send talk")?;
                                }
                                match AudioPipeline::new() {
                                    Ok(a) => {
                                        audio = Some(a);
                                        mode = ChatMode::Voice;
                                        ui.print_message(&[
                                            "Switching to talk mode. Speak freely.".to_string(),
                                        ]);
                                    }
                                    Err(e) => {
                                        ui.print_message(&[format!("Audio failed: {}", e)]);
                                    }
                                }
                            }
                            _ => {
                                ui.print_message(&[
                                    "Type 'tap' for text chat or 'talk' for voice.".to_string(),
                                ]);
                            }
                        },
                        ChatMode::Text | ChatMode::Voice => {
                            let morse = morse::encode(&msg);
                            if let Some(mut ch) = rtc.channel(cid) {
                                ch.write(false, morse.as_bytes()).context("send text")?;
                            }
                            ui.print_message(&[
                                format!("\x1b[1m[you]>\x1b[0m {}", msg),
                                format!("  \x1b[1m→\x1b[0m m[{}]", morse),
                            ]);
                        }
                    },
                    Ok(None) => return Ok(()),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                }
            }
        }

        // ── Encode and send audio frame if it's time ─────────────────────────
        let now = Instant::now();
        if audio.is_some() && now >= next_audio_tick {
            if let (Some(cid), Some(ref mut a)) = (channel, audio.as_mut()) {
                if let Some(packet) = a.encode_frame().context("encode audio")? {
                    if let Some(mut ch) = rtc.channel(cid) {
                        ch.write(true, &packet).context("send audio")?;
                    }
                }
            }
            next_audio_tick = Instant::now() + AUDIO_TICK;
        }

        // ── Block on socket until network data or timeout ─────────────────────
        let now = Instant::now();
        let deadline_duration = deadline.saturating_duration_since(now);
        let wait = if audio.is_some() {
            deadline_duration.min(next_audio_tick.saturating_duration_since(now))
        } else {
            deadline_duration
        };

        if wait.is_zero() {
            rtc.handle_input(Input::Timeout(Instant::now()))
                .context("handle timeout")?;
            continue;
        }

        socket.set_read_timeout(Some(wait)).context("set timeout")?;
        buf.resize(2000, 0);

        let input = match socket.recv_from(&mut buf) {
            Ok((n, source)) => {
                buf.truncate(n);
                match Receive::new(Protocol::Udp, source, local_addr, &buf) {
                    Ok(recv) => Input::Receive(Instant::now(), recv),
                    Err(e) => {
                        debug!("drop unrecognised UDP packet: {}", e);
                        continue;
                    }
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Input::Timeout(Instant::now())
            }
            Err(e) => return Err(e).context("UDP recv"),
        };

        rtc.handle_input(input).context("handle_input")?;
    }

    Ok(())
}

fn handle_event(
    event: Event,
    channel: &mut Option<ChannelId>,
    connected: &mut bool,
    mode: &mut ChatMode,
    audio: &mut Option<AudioPipeline>,
    ui: &ChatUi,
) -> Result<()> {
    match event {
        Event::Connected => {
            *connected = true;
            info!("DTLS connected");
        }
        Event::IceConnectionStateChange(state) => {
            info!("ICE: {:?}", state);
            if state == IceConnectionState::Disconnected {
                anyhow::bail!("ICE disconnected");
            }
        }
        Event::ChannelOpen(cid, label) => {
            info!("channel open: '{}' ({:?})", label, cid);
            *channel = Some(cid);
            let msg = match mode {
                ChatMode::Control => {
                    "Connected. Type 'tap' for text chat or 'talk' for voice.".to_string()
                }
                ChatMode::Text => "Connected! Type a message. Ctrl-D to exit.".to_string(),
                ChatMode::Voice => {
                    "Connected! Speak freely. Type to send text. Ctrl-D to exit.".to_string()
                }
            };
            ui.print_message(&[msg]);
        }
        Event::ChannelData(data) => {
            if data.binary {
                if let Some(ref mut a) = audio {
                    a.decode_and_queue(&data.data).context("decode audio")?;
                }
            } else if let Ok(text) = std::str::from_utf8(&data.data) {
                if let Some(ctrl) = text.strip_prefix("::") {
                    match ctrl {
                        "tap" => {
                            *mode = ChatMode::Text;
                            ui.print_message(&[
                                "Peer wants to tap. Switching to text chat.".to_string(),
                                "Type a message or Ctrl-D to exit.".to_string(),
                            ]);
                        }
                        "talk" => {
                            match AudioPipeline::new() {
                                Ok(a) => {
                                    *audio = Some(a);
                                    *mode = ChatMode::Voice;
                                    ui.print_message(&[
                                        "Peer wants to talk. Switching to voice.".to_string(),
                                        "Speak freely or Ctrl-D to exit.".to_string(),
                                    ]);
                                }
                                Err(e) => {
                                    ui.print_message(&[format!("Audio failed: {}", e)]);
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    let decoded = morse::decode(text);
                    ui.print_message(&[
                        format!("  \x1b[1m←\x1b[0m m[{}]", text),
                        format!("\x1b[1m[friend]>\x1b[0m {}", decoded),
                    ]);
                }
            }
        }
        Event::ChannelClose(cid) => {
            warn!("channel {:?} closed by peer", cid);
            anyhow::bail!("peer closed the channel");
        }
        other => {
            debug!("unhandled event: {:?}", other);
        }
    }
    Ok(())
}

pub fn build_offerer(
    candidates: Vec<Candidate>,
    label: &str,
) -> Result<(Rtc, str0m::change::SdpOffer, str0m::change::SdpPendingOffer, ChannelId)> {
    let mut rtc = Rtc::new(Instant::now());
    for c in candidates {
        rtc.add_local_candidate(c);
    }
    let mut change = rtc.sdp_api();
    let cid = change.add_channel(label.to_string());
    let (offer, pending) = change
        .apply()
        .context("failed to generate SDP offer — no changes applied")?;
    Ok((rtc, offer, pending, cid))
}

pub fn build_answerer(
    candidates: Vec<Candidate>,
    offer: str0m::change::SdpOffer,
) -> Result<(Rtc, str0m::change::SdpAnswer)> {
    let mut rtc = Rtc::new(Instant::now());
    for c in candidates {
        rtc.add_local_candidate(c);
    }
    let answer = rtc
        .sdp_api()
        .accept_offer(offer)
        .context("accept SDP offer")?;
    Ok((rtc, answer))
}
