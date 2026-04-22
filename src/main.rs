mod audio;
mod chat;
mod cli;
mod ice;
mod peer;
mod relay;
mod signal;

use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::AttachString => cmd_attach_string(),
        Commands::Join { code } => cmd_join(&code),
        Commands::Talk { code } => cmd_talk(code.as_deref()),
    }
}

// ── Text commands ─────────────────────────────────────────────────────────────

fn cmd_attach_string() -> Result<()> {
    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (mut rtc, offer, pending, _cid) =
        peer::build_offerer(candidates, "tin-can:text").context("build offerer")?;

    let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;

    println!("Registering room with relay...");
    let relay = relay::RelayClient::new();
    let code = relay.create_room(&offer_b64).context("create room")?;

    println!("\nRoom code: {}", code);
    println!("Tell your peer to run:  tin-can join {}", code);
    println!("\nWaiting for peer to join...");

    let answer_b64 = poll_for_answer(&relay, &code)?;
    let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
    rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

fn cmd_join(code: &str) -> Result<()> {
    println!("Fetching room {}...", code);
    let relay = relay::RelayClient::new();
    let offer_b64 = relay.get_offer(code).context("fetch offer")?;
    let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
    let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
    relay.put_answer(code, &answer_b64).context("upload answer")?;

    println!("Answer sent. Connecting...");

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

// ── Voice command ─────────────────────────────────────────────────────────────

fn cmd_talk(code: Option<&str>) -> Result<()> {
    let audio = audio::AudioPipeline::new().context("start audio")?;

    match code {
        None => talk_create(audio),
        Some(code) => talk_join(code, audio),
    }
}

fn talk_create(audio: audio::AudioPipeline) -> Result<()> {
    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (mut rtc, offer, pending, _cid) =
        peer::build_offerer(candidates, "tin-can:voice").context("build offerer")?;

    let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;

    println!("Registering room with relay...");
    let relay = relay::RelayClient::new();
    let code = relay.create_room(&offer_b64).context("create room")?;

    println!("\nRoom code: {}", code);
    println!("Tell your peer to run:  tin-can talk {}", code);
    println!("\nWaiting for peer to join...");

    let answer_b64 = poll_for_answer(&relay, &code)?;
    let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
    rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, Some(audio))
}

fn talk_join(code: &str, audio: audio::AudioPipeline) -> Result<()> {
    println!("Fetching room {}...", code);
    let relay = relay::RelayClient::new();
    let offer_b64 = relay.get_offer(code).context("fetch offer")?;
    let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
    let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
    relay.put_answer(code, &answer_b64).context("upload answer")?;

    println!("Answer sent. Connecting...");

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, Some(audio))
}

// ── Shared helper ─────────────────────────────────────────────────────────────

fn poll_for_answer(relay: &relay::RelayClient, code: &str) -> Result<String> {
    loop {
        thread::sleep(Duration::from_secs(2));
        match relay.poll_answer(code).context("poll for answer")? {
            Some(b64) => {
                println!();
                info!("received answer from peer");
                return Ok(b64);
            }
            None => {
                print!(".");
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
        }
    }
}
