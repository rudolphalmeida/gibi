mod options;
mod ui;

fn main() {
    env_logger::init();
    let ui = ui::Ui::new();

    ui.run();
}
