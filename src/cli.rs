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
    /// Create a text chat room and get a code to share with your peer
    AttachString,

    /// Join a peer's room using their code (text chat)
    Join {
        /// The 6-character room code from your peer
        code: String,
    },

    /// Start a voice call — omit code to create a room, provide one to join
    Talk {
        /// Room code from your peer (omit to create a new room)
        code: Option<String>,
    },
}
