use clap::Parser;
use gibi::options::Options;

fn main() {
    let options = Options::parse();
    println!("ROM filename: {:?}", options.rom_file);
}
