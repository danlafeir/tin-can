# tin-can

Peer-to-peer terminal communication. Two cans, one string — direct encrypted text chat and voice calls, no accounts or servers.

Connections are established by exchanging a URL. One peer generates a link, sends it to the other, and they're connected directly over WebRTC.

---

## Install

**Prerequisites (macOS):**

```sh
brew install opus   # only required for voice (tin-can talk)
```

**Build from source:**

If `cargo` is not on your PATH after installing Rust, run:

```sh
source ~/.cargo/env
```

Then build:

```sh
# Text chat only
cargo build --release

# Text + voice
cargo build --release --features voice
```

The binary is at `target/release/tin-can`. Copy it anywhere on your `$PATH`:

```sh
cp target/release/tin-can /usr/local/bin/
```

The release binary is self-contained — libopus is statically linked. No Homebrew required on the machine running the binary.

---

## Usage

### Text chat

**Start a chat (you go first):**

```sh
tin-can attach-string
```

This prints a URL like:

```
Share this URL with your peer:
  https://daniellafeir.com/can/#o=eyJ0eXBl...

They can open it in a browser or run:
  tin-can text "https://daniellafeir.com/can/#o=eyJ0eXBl..."

Paste their answer URL (or base64) here and press Enter:
>
```

Send the URL to your peer however you like — text, email, anything. The SDP data lives only in the URL fragment and never touches any server.

**Join a chat (your peer goes first):**

```sh
tin-can text "https://daniellafeir.com/can/#o=eyJ0eXBl..."
```

This prints an answer URL. Send it back. Once your peer pastes it, you're connected.

If your peer opens the URL in a browser, [daniellafeir.com/can/](https://daniellafeir.com/can/) shows the exact command to run.

---

### Voice call

Requires `--features voice` build and `brew install opus`.

**Start a call:**

```sh
tin-can talk
```

Works the same way as `attach-string` — prints a URL, waits for the peer's answer URL.

**Join a call:**

```sh
tin-can talk "https://daniellafeir.com/can/#o=eyJ0eXBl..."
```

You can still type text messages during a voice call.

---

## How it works

```
Alice                              Bob
  │                                 │
  │── tin-can attach-string         │
  │   generates SDP offer           │
  │   encodes in URL ───────────►   │
  │                          opens URL / runs tin-can text
  │                          generates SDP answer
  │   ◄──────────────── answer URL  │
  │   accepts answer                │
  │                                 │
  │◄══════ WebRTC (direct UDP) ════►│
```

- **Signaling**: SDP offer/answer exchanged via URL fragment at [daniellafeir.com/can/](https://daniellafeir.com/can/). The fragment is never sent to the server.
- **Transport**: Direct UDP between peers (WebRTC via [str0m](https://github.com/algesten/str0m))
- **Encryption**: DTLS-SRTP (built into WebRTC)
- **NAT traversal**: STUN via `stun.l.google.com`
- **Audio codec**: Opus at 48 kHz mono, 20 ms frames

---

## Building on Linux

```sh
# Install libopus (for voice)
sudo apt install libopus-dev   # Debian/Ubuntu
sudo dnf install opus-devel    # Fedora

cargo build --release --features voice
```

On Linux, libopus links dynamically by default. To produce a portable static binary use musl:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --features voice --target x86_64-unknown-linux-musl
```

---

## License

GPL-3.0
