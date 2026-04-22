# tin-can

Peer-to-peer terminal communication. Two cans, one string — direct encrypted text chat and voice calls, no accounts or servers.

Both peers agree on a shared secret. One starts the session, the other joins with the same secret. That's it.

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

Both peers just need to agree on a secret phrase — say it over the phone, text it, anything.

### Text chat

```sh
# Peer A — create the session
tin-can attach-string "our secret phrase"

# Peer B — join it
tin-can tap "our secret phrase"
```

Peer A prints a message telling them to wait. Once Peer B runs their command, both are connected.

### Voice call

Requires `--features voice` build and `brew install opus`.

```sh
# Either peer runs this first — whoever goes first creates the session
tin-can talk "our secret phrase"

# The other peer runs the same command to join
tin-can talk "our secret phrase"
```

You can still type text messages during a voice call.

---

## How it works

```
Alice                                     Bob
  │                                        │
  │── tin-can attach-string "secret"       │
  │   hashes secret → room code            │
  │   generates SDP offer                  │
  │   uploads offer to lafeir.com ──────►  │
  │   polls for answer...           tin-can tap "secret"
  │                                 hashes secret → same room code
  │                                 fetches offer from lafeir.com
  │                                 generates SDP answer
  │   ◄────────── answer uploaded   uploads answer to lafeir.com
  │   accepts answer                       │
  │                                        │
  │◄══════════ WebRTC (direct UDP) ═══════►│
```

- **Signaling**: SDP offer/answer relayed through [lafeir.com](https://lafeir.com) — blobs expire after 10 minutes. The room code is a hash of the secret; the secret itself never leaves your machine.
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
