use clap::Parser;
use gibi::{gameboy::Gameboy, options::Options};

fn main() {
    let options = Options::parse();
    println!("ROM filename: {:?}", options.rom_file);

    let mut gameboy = Gameboy::new(&options);
    gameboy.run();
}
