mod audio;
mod chat;
mod cli;
mod ice;
mod peer;
mod relay;
mod signal;

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};

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
        Commands::Text { code } => cmd_text(&code),
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

    let offer_url = signal::offer_to_url(&offer).context("encode offer URL")?;

    println!("\nShare this URL with your peer:");
    println!("  {}", offer_url);
    println!("\nThey can open it in a browser or run:");
    println!("  tin-can text \"{}\"", offer_url);
    println!("\nPaste their answer URL (or base64) here and press Enter:");

    let answer_input = read_line()?;
    let answer = signal::answer_from_input(&answer_input).context("decode answer")?;

    rtc.sdp_api()
        .accept_answer(pending, answer)
        .context("accept answer")?;

    println!("Connecting...");
    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

fn cmd_text(input: &str) -> Result<()> {
    if input.starts_with("http://") || input.starts_with("https://") {
        text_from_url(input)
    } else {
        text_from_code(input)
    }
}

/// Connect using a daniellafeir.com/can/#o=... URL (no relay needed).
fn text_from_url(url: &str) -> Result<()> {
    let offer = signal::offer_from_url(url).context("decode offer from URL")?;

    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;

    let answer_url = signal::answer_to_url(&answer).context("encode answer URL")?;

    println!("\nSend this URL back to your peer:");
    println!("  {}", answer_url);
    println!("\n(They paste it into their waiting prompt to complete the connection.)");
    println!("\nConnecting — waiting for peer to accept...");

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

/// Connect using a relay room code (requires relay backend to be deployed).
fn text_from_code(code: &str) -> Result<()> {
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
        Some(c) => talk_join(c, audio),
    }
}

fn talk_create(audio: audio::AudioPipeline) -> Result<()> {
    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (mut rtc, offer, pending, _cid) =
        peer::build_offerer(candidates, "tin-can:voice").context("build offerer")?;

    let offer_url = signal::offer_to_url(&offer).context("encode offer URL")?;

    println!("\nShare this URL with your peer:");
    println!("  {}", offer_url);
    println!("\nThey can open it in a browser or run:");
    println!("  tin-can talk \"{}\"", offer_url);
    println!("\nPaste their answer URL (or base64) here and press Enter:");

    let answer_input = read_line()?;
    let answer = signal::answer_from_input(&answer_input).context("decode answer")?;

    rtc.sdp_api()
        .accept_answer(pending, answer)
        .context("accept answer")?;

    println!("Connecting...");
    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, Some(audio))
}

fn talk_join(input: &str, audio: audio::AudioPipeline) -> Result<()> {
    let offer = if input.starts_with("http://") || input.starts_with("https://") {
        signal::offer_from_url(input).context("decode offer from URL")?
    } else {
        // Relay-based: fetch offer by code
        let relay = relay::RelayClient::new();
        let b64 = relay.get_offer(input).context("fetch offer")?;
        signal::decode_offer(&b64).context("decode offer")?
    };

    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
    let answer_url = signal::answer_to_url(&answer).context("encode answer URL")?;

    println!("\nSend this URL back to your peer:");
    println!("  {}", answer_url);
    println!("\nConnecting — waiting for peer to accept...");

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, Some(audio))
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn read_line() -> Result<String> {
    print!("> ");
    io::stdout().flush().ok();
    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("read from stdin")?;
    Ok(line.trim().to_string())
}

