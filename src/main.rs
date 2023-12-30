use eframe::{self, egui};

use gibi::GameFrame;
use ui::GameboyApp;

mod ui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1240.0, 760.0]),
        // TODO: Separate framerate from monitor
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "GiBi",
        options,
        Box::new(|cc| Box::<GameboyApp>::new(GameboyApp::new(cc))),
    )
}
