use std::{path::PathBuf, sync::mpsc};

use eframe::egui::{self};

use gibi::framebuffer::access;
use gibi::{gameboy::Gameboy, joypad::JoypadKeys, EmulatorEvent, GameFrame};
use ui::GameboyApp;

mod ui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1240.0, 760.0)),
        // TODO: This makes the emulator run at the frame rate of the monitor on
        //       which the window is. Change this to `false` and make the emulation
        //       sync to audio or use a timer
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "GiBi: Gameboy Color Emulator",
        options,
        Box::new(|cc| Box::<GameboyApp>::new(GameboyApp::new(cc))),
    )
}

#[derive(Debug)]
enum EmulatorCommand {
    LoadRom(PathBuf),
    SendDebugData,

    // Emulation control
    Start,
    RunFrame,
    Pause,
    Stop,

    // Joypad events
    KeyPressed(JoypadKeys),
    KeyReleased(JoypadKeys),

    // Exit
    Exit,
}

struct EmulationThread {
    loaded_rom_file: Option<PathBuf>,
    gameboy: Option<Gameboy>,
    running: bool,

    frame_writer: access::AccessW<GameFrame>,

    command_rc: mpsc::Receiver<EmulatorCommand>,
    event_tx: mpsc::Sender<EmulatorEvent>,
}

impl EmulationThread {
    fn new(
        frame_writer: access::AccessW<GameFrame>,
        command_rc: mpsc::Receiver<EmulatorCommand>,
        event_tx: mpsc::Sender<EmulatorEvent>,
    ) -> Self {
        Self {
            loaded_rom_file: None,
            gameboy: None,
            running: false,
            command_rc,
            event_tx,
            frame_writer,
        }
    }

    fn run(&mut self) {
        loop {
            match self.command_rc.recv() {
                Ok(m) => match m {
                    EmulatorCommand::LoadRom(path) => {
                        // TODO: Save if a game is already running
                        let rom = std::fs::read(&path).unwrap();
                        self.loaded_rom_file = Some(path);
                        self.gameboy = Some(Gameboy::new(rom, None, self.event_tx.clone()));
                    }
                    EmulatorCommand::Start if self.gameboy.is_some() => self.running = true,
                    EmulatorCommand::Start => {}
                    EmulatorCommand::RunFrame if self.running => {
                        let gb_ctx = self.gameboy.as_mut().unwrap();
                        gb_ctx.run_one_frame();
                        gb_ctx.write_frame(&mut self.frame_writer);
                        self.event_tx.send(EmulatorEvent::CompletedFrame).unwrap();
                    }
                    EmulatorCommand::RunFrame => {}
                    EmulatorCommand::Pause => self.running = false,
                    EmulatorCommand::Stop => {
                        // TODO: Save if a game is already running
                        self.running = false;
                        self.gameboy = None;
                        self.loaded_rom_file = None;
                    }
                    EmulatorCommand::KeyPressed(key) if self.running => {
                        self.gameboy.as_mut().unwrap().keydown(key)
                    }
                    EmulatorCommand::KeyPressed(_) => {}
                    EmulatorCommand::KeyReleased(key) if self.running => {
                        self.gameboy.as_mut().unwrap().keyup(key)
                    }
                    EmulatorCommand::KeyReleased(_) => {}
                    EmulatorCommand::Exit => {
                        // TODO: Save if a game is already running
                        log::info!("Received request to quit. Terminate emulation thread");
                        break;
                    }
                    EmulatorCommand::SendDebugData if self.gameboy.is_some() => {
                        self.gameboy.as_ref().unwrap().send_debug_data()
                    }
                    EmulatorCommand::SendDebugData => {}
                },
                Err(e) => log::error!("{}", e),
            }
        }
    }
}
