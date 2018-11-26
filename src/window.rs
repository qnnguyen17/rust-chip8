extern crate piston_window;

use self::GraphicsOp::*;
use piston_window::*;
use std::sync::mpsc::Receiver;

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const PIXEL_SCALE_FACTOR: f64 = 10.0;

pub struct WindowHandler {
    frame_buffer: [u8; 8 * 32],
    graphics_bus_in: Receiver<GraphicsOp>,
}

#[derive(Clone, Debug)]
pub enum GraphicsOp {
    ClearScreen,
    DrawSprite { x: u8, y: u8, sprite: Vec<u8> },
}

impl WindowHandler {
    pub fn new(graphics_bus_in: Receiver<GraphicsOp>) -> WindowHandler {
        WindowHandler {
            frame_buffer: [0; 8 * 32],
            graphics_bus_in,
        }
    }

    pub fn run(&mut self) {
        let mut window: PistonWindow = WindowSettings::new("Chip8", (640, 320))
            .exit_on_esc(false)
            .build()
            .unwrap_or_else(|e| panic!("Failed to build PistonWindow: {}", e));
        while let Some(e) = window.next() {
            self.update_frame_buffer();
            self.draw_frame_buffer(&mut window, &e);
        }
    }

    fn update_frame_buffer(&mut self) {
        while let Ok(op) = self.graphics_bus_in.try_recv() {
            self.handle_graphics_operation(op)
        }
    }

    fn draw_frame_buffer(&mut self, window: &mut PistonWindow, e: &Event) {
        window.draw_2d(e, |c, g| {
            clear(BLACK, g);
            for (index, byte) in self.frame_buffer.into_iter().enumerate() {
                let row = index / 8;
                let octet_index = index % 8;
                for bit_index in 0..8 {
                    if bit_is_set(*byte, bit_index) {
                        let top = PIXEL_SCALE_FACTOR * row as f64;
                        let left = PIXEL_SCALE_FACTOR
                            * (octet_index * 8 + (8 - bit_index) as usize) as f64;
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

    fn handle_graphics_operation(&mut self, op: GraphicsOp) {
        match op {
            ClearScreen => {
                self.frame_buffer = [0; 8 * 32];
            }
            DrawSprite { x, y, sprite } => {
                let first_byte = (y * 8 + (x / 8)) as usize;
                let mut second_byte = first_byte + 1;

                // Wrap around rather than going to the next row
                if second_byte % 8 == 0 {
                    second_byte -= 8;
                }

                for (i, byte) in sprite.into_iter().enumerate() {
                    let bit_offset = x % 8;
                    self.frame_buffer[first_byte + (i * 8)] ^= byte >> bit_offset;
                    self.frame_buffer[second_byte + (i * 8)] ^= byte << (8 - bit_offset);
                }
            }
        }
    }
}

// Return whether or not the bit at index |bit_index| (from least significant)
// is set.
fn bit_is_set(byte: u8, bit_index: u8) -> bool {
    return byte & (1 << bit_index) > 0;
}
