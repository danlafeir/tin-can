mod audio;
mod chat;
mod cli;
mod ice;
mod morse;
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
        Commands::Tap { static_link, value } => {
            if static_link {
                let url = value.context(
                    "provide the offer URL from your peer, e.g.: tin-can tap --static-link \"https://...\"",
                )?;
                tap_static(&url)
            } else {
                let s = value.context(
                    "provide a shared secret, e.g.: tin-can tap \"my secret\"\n\
                     Or use --static-link with an offer URL",
                )?;
                tap_relay(&s)
            }
        }
        Commands::Upgrade => cmd_upgrade(),
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

    println!("  room code: {}", code);

    match relay.try_get_offer(&code).context("check for session")? {
        Some(offer_b64) => {
            let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

            println!("Found session — joining as answerer. Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;
            println!("  local addr: {}  candidates: {}", local_addr, candidates.len());

            let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
            let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
            relay.put_knot_tie(&code, &answer_b64).context("upload answer")?;
            println!("  knot-tie sent to relay");

            println!("Connecting...");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, None, true)
        }
        None => {
            println!("No session found — starting as offerer. Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;
            println!("  local addr: {}  candidates: {}", local_addr, candidates.len());

            let (mut rtc, offer, pending, _cid) =
                peer::build_offerer(candidates, "tin-can:text").context("build offerer")?;

            let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;
            relay.upload_offer(&code, &offer_b64).context("upload offer")?;
            println!("  offer uploaded to relay");

            println!("\nWaiting for peer... Tell them to run:");
            println!("  tin-can attach-string {:?}", secret);

            let answer_b64 = poll_for_answer(&relay, &code)?;
            let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
            rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

            println!("Connected!");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, None, false)
        }
    }
}

fn tap_relay(secret: &str) -> Result<()> {
    let code = signal::derive_room_code(secret);
    let relay = relay::RelayClient::new();

    println!("  room code: {}", code);

    match relay.try_get_offer(&code).context("check for session")? {
        Some(offer_b64) => {
            let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

            println!("Found session — joining as answerer. Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;
            println!("  local addr: {}  candidates: {}", local_addr, candidates.len());

            let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
            let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
            relay.put_knot_tie(&code, &answer_b64).context("upload answer")?;
            println!("  knot-tie sent to relay");

            println!("Connecting...");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, None, true)
        }
        None => {
            println!("No session found — starting as offerer. Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;
            println!("  local addr: {}  candidates: {}", local_addr, candidates.len());

            let (mut rtc, offer, pending, _cid) =
                peer::build_offerer(candidates, "tin-can:text").context("build offerer")?;

            let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;
            relay.upload_offer(&code, &offer_b64).context("upload offer")?;
            println!("  offer uploaded to relay");

            println!("\nWaiting for peer... Tell them to run:");
            println!("  tin-can tap {:?}", secret);

            let answer_b64 = poll_for_answer(&relay, &code)?;
            let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
            rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

            println!("Connected!");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, None, false)
        }
    }
}

fn talk_relay(secret: &str) -> Result<()> {
    let code = signal::derive_room_code(secret);
    let relay = relay::RelayClient::new();
    let audio = audio::AudioPipeline::new().context("start audio")?;

    println!("  room code: {}", code);

    match relay.try_get_offer(&code).context("check for session")? {
        Some(offer_b64) => {
            let offer = signal::decode_offer(&offer_b64).context("decode offer")?;

            println!("Found session — joining as answerer. Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;
            println!("  local addr: {}  candidates: {}", local_addr, candidates.len());

            let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
            let answer_b64 = signal::encode_answer(&answer).context("encode answer")?;
            relay.put_knot_tie(&code, &answer_b64).context("upload answer")?;
            println!("  knot-tie sent to relay");

            println!("Connecting...");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, Some(audio), true)
        }
        None => {
            println!("No session found — starting as offerer. Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;
            println!("  local addr: {}  candidates: {}", local_addr, candidates.len());

            let (mut rtc, offer, pending, _cid) =
                peer::build_offerer(candidates, "tin-can:voice").context("build offerer")?;

            let offer_b64 = signal::encode_offer(&offer).context("encode offer")?;
            relay.upload_offer(&code, &offer_b64).context("upload offer")?;
            println!("  offer uploaded to relay");

            println!("\nWaiting for peer... Tell them to run:");
            println!("  tin-can talk {:?}", secret);

            let answer_b64 = poll_for_answer(&relay, &code)?;
            let answer = signal::decode_answer(&answer_b64).context("decode answer")?;
            rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

            println!("Connected!");
            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, Some(audio), false)
        }
    }
}

// ── Static-link mode (copy/paste URL → daniellafeir.com) ──────────────────────

fn attach_string_static() -> Result<()> {
    println!("Gathering network candidates...");
    let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;

    let (mut rtc, offer, pending, _cid) =
        peer::build_offerer(candidates, "tin-can:text").context("build offerer")?;

    let offer_url = signal::offer_to_url(&offer).context("encode offer URL")?;

    println!("\nShare this URL with your peer:");
    println!("  {}", offer_url);
    println!("\nThey can open it in a browser or run:");
    println!("  tin-can tap --static-link \"{}\"", offer_url);
    println!("\nPaste their answer URL (or base64) here and press Enter:");

    let answer_input = read_line()?;
    let answer = signal::answer_from_input(&answer_input).context("decode answer")?;
    rtc.sdp_api().accept_answer(pending, answer).context("accept answer")?;

    println!("Connecting...");
    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None, false)
}

fn tap_static(url: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("expected a URL, got: {url}\nDid you mean: tin-can tap {:?}", url);
    }

    let offer = signal::offer_from_url(url).context("decode offer from URL")?;

    println!("Gathering network candidates...");
    let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;

    let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
    let answer_url = signal::answer_to_url(&answer).context("encode answer URL")?;

    println!("\nSend this URL back to your peer:");
    println!("  {}", answer_url);
    println!("\n(They paste it into their waiting prompt to complete the connection.)");
    println!("\nConnecting — waiting for peer to accept...");

    let rx = chat::spawn_input_thread();
    peer::run(rtc, socket, local_addr, rx, None, true)
}

fn talk_static(url: Option<&str>) -> Result<()> {
    let audio = audio::AudioPipeline::new().context("start audio")?;

    match url {
        None => {
            println!("Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;

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
            peer::run(rtc, socket, local_addr, rx, Some(audio), false)
        }
        Some(url) => {
            let offer = signal::offer_from_url(url).context("decode offer from URL")?;

            println!("Gathering network candidates...");
            let (socket, local_addr, candidates) = ice::gather().context("ICE gather")?;

            let (rtc, answer) = peer::build_answerer(candidates, offer).context("build answerer")?;
            let answer_url = signal::answer_to_url(&answer).context("encode answer URL")?;

            println!("\nSend this URL back to your peer:");
            println!("  {}", answer_url);
            println!("\nConnecting — waiting for peer to accept...");

            let rx = chat::spawn_input_thread();
            peer::run(rtc, socket, local_addr, rx, Some(audio), true)
        }
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn poll_for_answer(relay: &relay::RelayClient, code: &str) -> Result<String> {
    const POLL_INTERVAL: Duration = Duration::from_secs(2);
    const STATUS_EVERY: u32 = 15; // elapsed timestamp every ~30s

    print!("Waiting");
    io::stdout().flush().ok();

    let started = std::time::Instant::now();
    let mut ticks: u32 = 0;
    loop {
        thread::sleep(POLL_INTERVAL);
        match relay.poll_knot_tie(code).context("poll for knot-tie")? {
            Some(b64) => {
                println!();
                return Ok(b64);
            }
            None => {
                ticks += 1;
                if ticks % STATUS_EVERY == 0 {
                    let secs = started.elapsed().as_secs();
                    print!(" ({}:{:02})", secs / 60, secs % 60);
                } else {
                    print!(".");
                }
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

// ── Upgrade command ───────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct GithubEntry {
    name: String,
}

fn cmd_upgrade() -> Result<()> {
    let current_hash = env!("GIT_HASH");
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let prefix = format!("tin-can-{}-{}-", os, arch);

    println!("Checking for updates (current: {})...", current_hash);

    let client = reqwest::blocking::Client::builder()
        .user_agent("tin-can-upgrade")
        .timeout(Duration::from_secs(30))
        .build()
        .context("build HTTP client")?;

    let entries: Vec<GithubEntry> = client
        .get("https://api.github.com/repos/danlafeir/tin-can/contents/bin")
        .send()
        .context("query GitHub API")?
        .error_for_status()
        .context("GitHub API error")?
        .json()
        .context("parse GitHub API response")?;

    let mut matches: Vec<&GithubEntry> = entries
        .iter()
        .filter(|e| e.name.starts_with(&prefix))
        .collect();
    matches.sort_by(|a, b| a.name.cmp(&b.name));

    let latest = matches
        .last()
        .ok_or_else(|| anyhow::anyhow!("no binary found for {}-{}", os, arch))?;

    let latest_hash = latest.name.rsplit('-').next().unwrap_or("");
    if latest_hash == current_hash {
        println!("Already up to date ({}).", current_hash);
        return Ok(());
    }

    println!("Downloading {}...", latest.name);
    let url = format!(
        "https://raw.githubusercontent.com/danlafeir/tin-can/main/bin/{}",
        latest.name
    );
    let bytes = client
        .get(&url)
        .send()
        .context("download binary")?
        .error_for_status()
        .context("download failed")?
        .bytes()
        .context("read response")?;

    let current_exe = std::env::current_exe().context("locate current binary")?;
    let tmp = current_exe.with_extension("tmp");
    std::fs::write(&tmp, &bytes).context("write new binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .context("set executable bit")?;
    }

    std::fs::rename(&tmp, &current_exe).context("replace binary")?;
    println!("Updated: {} → {}.", current_hash, latest_hash);
    Ok(())
}
