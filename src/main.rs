use clap::Parser;
use gibi::{gameboy::Gameboy, options::Options};

fn main() {
    env_logger::init();

    let options = Options::parse();
    log::info!("ROM filename: {:?}", options.rom_file);

    let mut gameboy = Gameboy::new(&options);
    gameboy.run();
}
