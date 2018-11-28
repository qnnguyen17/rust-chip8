extern crate piston_window;

use piston_window::*;
use std::sync::Arc;
use std::sync::RwLock;

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const PIXEL_SCALE_FACTOR: f64 = 10.0;

pub struct WindowHandler {
    frame_buffer: Arc<RwLock<[u8; 8 * 32]>>,
}

impl WindowHandler {
    pub fn new(frame_buffer: Arc<RwLock<[u8; 8 * 32]>>) -> WindowHandler {
        WindowHandler { frame_buffer }
    }

    pub fn run(&mut self) {
        let mut window: PistonWindow = WindowSettings::new("Chip8", (640, 320))
            .exit_on_esc(false)
            .build()
            .unwrap_or_else(|e| panic!("Failed to build PistonWindow: {}", e));
        while let Some(e) = window.next() {
            self.draw_frame_buffer(&mut window, &e);
        }
    }

    fn draw_frame_buffer(&mut self, window: &mut PistonWindow, e: &Event) {
        window.draw_2d(e, |c, g| {
            clear(BLACK, g);
            for (index, byte) in self.frame_buffer.read().unwrap().iter().enumerate() {
                let row = index / 8;
                let octet_index = index % 8;
                for bit_index in 0..8 {
                    if bit_is_set(*byte, bit_index) {
                        let top = PIXEL_SCALE_FACTOR * row as f64;
                        let left = PIXEL_SCALE_FACTOR
                            * (octet_index * 8 + (8 - bit_index - 1) as usize) as f64;
                        rectangle(
                            GREEN,
                            [left, top, PIXEL_SCALE_FACTOR, PIXEL_SCALE_FACTOR],
                            c.transform,
                            g,
                        );
                    }
                }
            }
        });
    }
}

// Return whether or not the bit at index |bit_index| (from least significant)
// is set.
fn bit_is_set(byte: u8, bit_index: u8) -> bool {
    byte & (1 << bit_index) > 0
}
