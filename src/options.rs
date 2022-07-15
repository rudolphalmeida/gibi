// TODO: This needs to be in the binary crate and the library crate should provide
// the fields decoupled from `clap`

use clap::Parser;

/// Command-line arguments and settings for the emulator
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Options {
    #[clap(value_parser)]
    pub rom_file: String,
}
