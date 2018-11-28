mod cpu;
mod digits;
mod window;

use std::env;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;

fn main() {
    let args: Vec<String> = env::args().collect();
    let filename = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from("BRIX")
    };

    let frame_buffer_1 = Arc::new(RwLock::new([0; 8 * 32]));
    let frame_buffer_2 = frame_buffer_1.clone();

    // TODO: gracefully handle failure/abort
    let window_thread = thread::Builder::new()
        .name("window".to_string())
        .spawn(move || {
            let mut window = window::WindowHandler::new(frame_buffer_1);
            window.run();
        })
        .expect("failed to spawn window thread");

    let processor_thread = thread::Builder::new()
        .name("processor".to_string())
        .spawn(move || {
            let mut processor = cpu::CPU::new(frame_buffer_2);
            processor.load_game_data(&filename).unwrap();
            processor.run();
        })
        .expect("failed to spawn processor thread");

    window_thread.join().unwrap();
    processor_thread.join().unwrap();
}
