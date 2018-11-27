mod cpu;
mod digits;
mod window;

use std::env;
use std::sync::mpsc::channel;
use std::thread;

fn main() {
    let (graphics_bus_out, graphics_bus_in) = channel();

    let args: Vec<String> = env::args().collect();
    let filename = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from("BRIX")
    };

    // TODO: gracefully handle failure/abort
    let window_thread = thread::Builder::new()
        .name("window".to_string())
        .spawn(move || {
            let mut window = window::WindowHandler::new(graphics_bus_in);
            window.run();
        })
        .expect("failed to spawn window thread");

    let processor_thread = thread::Builder::new()
        .name("processor".to_string())
        .spawn(move || {
            let mut processor = cpu::CPU::new(graphics_bus_out);
            processor.load_game_data(&filename).unwrap();
            processor.run();
        })
        .expect("failed to spawn processor thread");

    window_thread.join().unwrap();
    processor_thread.join().unwrap();
}
