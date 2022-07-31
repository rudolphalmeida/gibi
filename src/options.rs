use clap::Parser;

/// Command-line arguments and settings for the emulator
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Options {
    #[clap(value_parser)]
    pub rom_file: String,

    #[clap(short, long, value_parser, default_value_t = 1)]
    pub scale_factor: u32,
}
