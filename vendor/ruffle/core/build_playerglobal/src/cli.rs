use clap::{Parser, Subcommand};

#[derive(Parser)]
pub(crate) struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Compile {
        out_dir: String,
    },
    /// Rebuilds core/prebuilt/playerglobal (needs Java) and records its inputs' hash.
    Prebuild,
}
