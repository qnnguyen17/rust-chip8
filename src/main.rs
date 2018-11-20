extern crate chip8;

use chip8::Chip8;

fn main() {
    let mut chip8 = Chip8::new();
    chip8.load_game_data("15PUZZLE").unwrap();
    chip8.run();
}
