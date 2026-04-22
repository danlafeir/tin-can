mod audio;
mod chat;
mod cli;
mod ice;
mod peer;
mod relay;
mod signal;

use std::io::{self, BufRead, Write};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
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
        Commands::AttachString { static_link, secret } => {
            if static_link {
                attach_string_static()
            } else {
                let s = secret.context(
                    "provide a shared secret, e.g.: tin-can attach-string \"my secret\"\n\
                     Or use --static-link for copy/paste URL signaling",
                )?;
                attach_string_relay(&s)
            }
        }
        Commands::Text { static_link, value } => {
            if static_link {
                let url = value.context(
                    "provide the offer URL from your peer, e.g.: tin-can text --static-link \"https://...\"",
                )?;
                text_static(&url)
            } else {
                let s = value.context(
                    "provide a shared secret, e.g.: tin-can text \"my secret\"\n\
                     Or use --static-link with an offer URL",
                )?;
                text_relay(&s)
            }
        }
        Commands::Talk { static_link, value } => {
            if static_link {
                talk_static(value.as_deref())
            } else {
                let s = value.context(
                    "provide a shared secret, e.g.: tin-can talk \"my secret\"\n\
                     Or use --static-link (optionally with an offer URL to join)",
                )?;
                talk_relay(&s)
            }
        }
    }
}

// ── Relay mode (shared secret → lafeir.com) ───────────────────────────────────

fn attach_string_relay(secret: &str) -> Result<()> {
    let code = signal::derive_room_code(secret);
    let relay = relay::RelayClient::new();

    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (mut rtc, offer, pending, _cid) =
        peer::build_offerer(candidates, "tin-can:text").context("build offerer")?;

    let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;
    relay.upload_offer(&code, &offer_b64).context("upload offer")?;

    println!("\nWaiting for peer... Tell them to run:");
    println!("  tin-can text {:?}", secret);

    let answer_b64 = poll_for_answer(&relay, &code)?;
    let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
    rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

    println!("Connected!");
    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

fn text_relay(secret: &str) -> Result<()> {
    let code = signal::derive_room_code(secret);
    let relay = relay::RelayClient::new();

    println!("Looking up session...");
    let offer_b64 = relay.get_offer(&code).context("fetch offer")?;
    let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
    let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
    relay.put_answer(&code, &answer_b64).context("upload answer")?;

    println!("Connecting...");
    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

fn talk_relay(secret: &str) -> Result<()> {
    let code = signal::derive_room_code(secret);
    let relay = relay::RelayClient::new();
    let audio = audio::AudioPipeline::new().context("start audio")?;

    match relay.try_get_offer(&code).context("check for session")? {
        Some(offer_b64) => {
            let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

            println!("Found session. Gathering network candidates...");
            let (socket, candidates) = ice::gather().context("ICE gather")?;
            let local_addr = socket.local_addr()?;

            let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
            let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
            relay.put_answer(&code, &answer_b64).context("upload answer")?;

            println!("Connecting...");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, Some(audio))
        }
        None => {
            println!("Gathering network candidates...");
            let (socket, candidates) = ice::gather().context("ICE gather")?;
            let local_addr = socket.local_addr()?;

            let (mut rtc, offer, pending, _cid) =
                peer::build_offerer(candidates, "tin-can:voice").context("build offerer")?;

            let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;
            relay.upload_offer(&code, &offer_b64).context("upload offer")?;

            println!("\nWaiting for peer... Tell them to run:");
            println!("  tin-can talk {:?}", secret);

            let answer_b64 = poll_for_answer(&relay, &code)?;
            let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
            rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

            println!("Connected!");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, Some(audio))
        }
    }
}

// ── Static-link mode (copy/paste URL → daniellafeir.com) ──────────────────────

fn attach_string_static() -> Result<()> {
    println!("Gathering network candidates...");
    let (socket, candidates) = ice::gather().context("ICE gather")?;
    let local_addr = socket.local_addr()?;

    let (mut rtc, offer, pending, _cid) =
        peer::build_offerer(candidates, "tin-can:text").context("build offerer")?;

    let offer_url = signal::offer_to_url(&offer).context("encode offer URL")?;

    println!("\nShare this URL with your peer:");
    println!("  {}", offer_url);
    println!("\nThey can open it in a browser or run:");
    println!("  tin-can text --static-link \"{}\"", offer_url);
    println!("\nPaste their answer URL (or base64) here and press Enter:");

    let answer_input = read_line()?;
    let answer = signal::answer_from_input(&answer_input).context("decode answer")?;
    rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

    println!("Connecting...");
    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None)
}

fn text_static(url: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("expected a URL, got: {url}\nDid you mean: tin-can text {:?}", url);
    }

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

fn talk_static(url: Option<&str>) -> Result<()> {
    let audio = audio::AudioPipeline::new().context("start audio")?;

    match url {
        None => {
            println!("Gathering network candidates...");
            let (socket, candidates) = ice::gather().context("ICE gather")?;
            let local_addr = socket.local_addr()?;

            let (mut rtc, offer, pending, _cid) =
                peer::build_offerer(candidates, "tin-can:voice").context("build offerer")?;

            let offer_url = signal::offer_to_url(&offer).context("encode offer URL")?;

            println!("\nShare this URL with your peer:");
            println!("  {}", offer_url);
            println!("\nThey can open it in a browser or run:");
            println!("  tin-can talk --static-link \"{}\"", offer_url);
            println!("\nPaste their answer URL (or base64) here and press Enter:");

            let answer_input = read_line()?;
            let answer = signal::answer_from_input(&answer_input).context("decode answer")?;
            rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

            println!("Connecting...");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, Some(audio))
        }
        Some(url) => {
            let offer = signal::offer_from_url(url).context("decode offer from URL")?;

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
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn poll_for_answer(relay: &relay::RelayClient, code: &str) -> Result<String> {
    print!("Waiting");
    io::stdout().flush().ok();
    loop {
        thread::sleep(Duration::from_secs(2));
        match relay.poll_answer(code).context("poll for answer")? {
            Some(b64) => {
                println!();
                return Ok(b64);
            }
            None => {
                print!(".");
                io::stdout().flush().ok();
            }
        }
    }
}

fn read_line() -> Result<String> {
    print!("> ");
    io::stdout().flush().ok();
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).context("read from stdin")?;
    Ok(line.trim().to_string())
}
