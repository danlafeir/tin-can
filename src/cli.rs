use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tin-can",
    about = "P2P terminal communication — two cans, one string",
    version
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

    /// Join a text chat session
    Text {
        /// Use copy/paste URL signaling via daniellafeir.com instead of the relay
        #[arg(long)]
        static_link: bool,

        /// Shared secret (relay mode) or offer URL (--static-link mode)
        value: Option<String>,
    },

    /// Start or join a voice call
    Talk {
        /// Use copy/paste URL signaling via daniellafeir.com instead of the relay
        #[arg(long)]
        static_link: bool,

        /// Shared secret (relay mode) or offer URL to join (--static-link mode)
        value: Option<String>,
    },
}
