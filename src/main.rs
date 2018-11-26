mod cpu;
mod digits;
mod window;

use std::sync::mpsc::channel;
use std::thread;

fn main() {
    let (graphics_bus_out, graphics_bus_in) = channel();

    // TODO: gracefully handle failure/abort
    let window_thread = thread::spawn(move || {
        let mut window = window::WindowHandler::new(graphics_bus_in);
        window.run();
    });

    let processor_thread = thread::spawn(move || {
        let mut processor = cpu::CPU::new(graphics_bus_out);
        processor.load_game_data("15PUZZLE").unwrap();
        processor.run();
    });

    window_thread.join().unwrap();
    processor_thread.join().unwrap();
}
