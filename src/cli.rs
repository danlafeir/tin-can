use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tin-can",
    about = "P2P terminal communication — two cans, one string",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")"),
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a text chat session
    AttachString {
        /// Use copy/paste URL signaling via daniellafeir.com instead of the relay
        #[arg(long)]
        static_link: bool,

        /// Shared secret (relay mode only — omit with --static-link)
        secret: Option<String>,
    },

    /// Send a text message as Morse code and receive decoded replies
    Tap {
        /// Use copy/paste URL signaling via daniellafeir.com instead of the relay
        #[arg(long)]
        static_link: bool,

        /// Shared secret (relay mode) or offer URL (--static-link mode)
        value: Option<String>,
    },

    /// Download and install the latest release binary
    Upgrade,

    /// Start or join a voice call
    Talk {
        /// Use copy/paste URL signaling via daniellafeir.com instead of the relay
        #[arg(long)]
        static_link: bool,

        /// Shared secret (relay mode) or offer URL to join (--static-link mode)
        value: Option<String>,
    },
}
