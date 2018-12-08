mod cpu;
mod digits;
mod timers;
mod window;

use std::env;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::thread;

const FRAME_BUFFER_BYTES: usize = 8 * 32;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    let filename = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from("BRIX")
    };

    let frame_buffer_1 = Arc::new(RwLock::new([0; FRAME_BUFFER_BYTES]));
    let frame_buffer_2 = frame_buffer_1.clone();

    let (window_closed_sender, window_closed_receiver) = channel();
    let (key_event_sender, key_event_receiver) = channel();

    let window_thread = thread::Builder::new()
        .name("window".to_string())
        .spawn(move || {
            let mut window =
                window::WindowHandler::new(frame_buffer_1, window_closed_sender, key_event_sender);
            window.run();
        })
        .expect("failed to spawn window thread");

    let delay_timer = Arc::new(Mutex::new(0));

    let mut timers = timers::Timers::new(delay_timer.clone());
    timers.start();

    let processor_thread = thread::Builder::new()
        .name("processor".to_string())
        .spawn(move || {
            let mut processor = cpu::CPU::new(
                delay_timer,
                frame_buffer_2,
                window_closed_receiver,
                key_event_receiver,
            );
            processor.load_game_data(&filename).unwrap();
            processor.run();
        })
        .expect("failed to spawn processor thread");

    window_thread.join().unwrap();
    processor_thread.join().unwrap();
    timers.stop();
}
