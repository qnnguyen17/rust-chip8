mod cpu;

fn main() {
    let mut processor = cpu::CPU::new();
    processor.load_game_data("15PUZZLE").unwrap();
    processor.run();
}
