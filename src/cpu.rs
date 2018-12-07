#[cfg(test)]
mod tests;

use log::*;
use piston_window::*;
use rand::prelude::*;
use rand::rngs::mock::StepRng;
use std::fs::File;
use std::io::prelude::*;
use std::io::Result;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::sync::RwLock;

use self::OpCode::*;
use super::digits::DIGITS;
use super::FRAME_BUFFER_BYTES;

#[derive(Clone, Copy, Debug, PartialEq)]
enum OpCode {
    AddIReg {
        reg: usize,
    },
    AddRegByte {
        reg: usize,
        val: u8,
    },
    AddRegs {
        reg_x: usize,
        reg_y: usize,
    },
    AndRegs {
        reg_x: usize,
        reg_y: usize,
    },
    Call {
        addr: usize,
    },
    Clear,
    Draw {
        reg_x: usize,
        reg_y: usize,
        sprite_bytes: u8,
    },
    LdIAddr {
        addr: usize,
    },
    LdIDigitReg {
        reg: usize,
    },
    LdMemIBcdReg {
        reg: usize,
    },
    LdMemIRegs {
        last_reg: usize,
    },
    LdRegByte {
        reg: usize,
        val: u8,
    },
    LdRegKey {
        reg: usize,
    },
    LdRegsMemI {
        last_reg: usize,
    },
    LdRegReg {
        reg_x: usize,
        reg_y: usize,
    },
    Jump {
        addr: usize,
    },
    RandRegByte {
        reg: usize,
        val: u8,
    },
    Ret,
    ShiftRightReg {
        reg: usize,
    },
    SkipEqRegBytes {
        reg: usize,
        val: u8,
    },
    SkipNEqRegBytes {
        reg: usize,
        val: u8,
    },
    SkipNEqRegs {
        reg_x: usize,
        reg_y: usize,
    },
    SkipRegKeyPressed {
        reg: usize,
    },
    SkipRegKeyNPressed {
        reg: usize,
    },
    Sys,
}

pub struct CPU {
    // General-purpose registers
    v: [u8; 16],
    // Memory address register
    i: usize,
    sound_timer: u8,
    delay_timer: u8,
    // Program counter
    pc: usize,
    stack: [u16; 16],
    // Stack pointer
    sp: usize,
    // Address space
    memory: [u8; 4096],
    // State of the 16 input keys
    key_state: [bool; 16],

    frame_buffer: Arc<RwLock<[u8; FRAME_BUFFER_BYTES]>>,

    // Random number generator used for Rand operations
    rng: WrappedRng,

    // Allows the CPU to be notified when the emulator window is closed, so it can complete as
    // well.
    window_closed_receiver: Receiver<bool>,
    key_event_receiver: Receiver<Event>,
}

#[allow(dead_code)]
enum WrappedRng {
    // Used for normal operation
    Standard(ThreadRng),
    // Used for testing
    Mock(StepRng),
}

impl WrappedRng {
    fn gen_byte(&mut self) -> u8 {
        match self {
            WrappedRng::Standard(rng) => rng.gen(),
            WrappedRng::Mock(rng) => rng.gen(),
        }
    }
}

impl CPU {
    pub fn new(
        frame_buffer: Arc<RwLock<[u8; FRAME_BUFFER_BYTES]>>,
        window_closed_receiver: Receiver<bool>,
        key_event_receiver: Receiver<Event>,
    ) -> CPU {
        let mut memory = [0 as u8; 4096];
        memory[..DIGITS.len()].clone_from_slice(&DIGITS);
        CPU {
            v: [0; 16],
            i: 0,
            sound_timer: 0,
            delay_timer: 0,
            // Most chip8 programs start at 0x200
            pc: 0x200,
            stack: [0; 16],
            sp: 0,
            memory,
            key_state: [false; 16],
            frame_buffer,
            rng: WrappedRng::Standard(thread_rng()),
            window_closed_receiver,
            key_event_receiver,
        }
    }

    pub fn load_game_data(&mut self, file_name: &str) -> Result<()> {
        let mut game_file = File::open(file_name)?;
        assert!(game_file.read(&mut self.memory[0x200..]).unwrap() > 0);
        Ok(())
    }

    pub fn run(&mut self) {
        loop {
            if let Ok(true) = self.window_closed_receiver.try_recv() {
                break;
            }

            self.update_key_state();

            let pc = self.pc as usize;
            let code = &self.memory[pc..pc + 2];
            let instr = decode_instruction(&code);
            self.execute(instr);
        }
    }

    fn update_key_state(&mut self) {
        while let Ok(event) = self.key_event_receiver.try_recv() {
            event.press(|button| {
                if let Some(keycode) = get_keycode(button) {
                    self.key_state[keycode] = true;
                }
            });
            event.release(|button| {
                if let Some(keycode) = get_keycode(button) {
                    self.key_state[keycode] = false;
                }
            });
        }
    }

    fn execute(&mut self, op: OpCode) {
        let mut new_pc = self.pc + 2;
        match op {
            AddIReg { reg } => {
                let reg_val = self.v[reg];
                info!(
                    "Adding {:x} from reg V{} to I's current value {:x}",
                    reg_val, reg, self.i
                );
                self.i += reg_val as usize;
            }
            AddRegByte { reg, val } => {
                info!(
                    "Adding val {:x} to register V{} to get value: {}",
                    val,
                    reg,
                    self.v[reg].wrapping_add(val),
                );
                self.v[reg] = self.v[reg].wrapping_add(val);
            }
            AddRegs { reg_x, reg_y } => {
                info!(
                    "Adding val {}(V{}) and {}(V{}), storing in V{}",
                    self.v[reg_x], reg_x, self.v[reg_y], reg_y, reg_x
                );
                let (sum, did_overflow) = self.v[reg_x].overflowing_add(self.v[reg_y]);
                self.v[reg_x] = sum;
                self.v[0xF] = if did_overflow { 1 } else { 0 };
            }
            AndRegs { reg_x, reg_y } => {
                info!(
                    "AND-ing register V{} and V{}, storing value in V{}",
                    reg_x, reg_y, reg_x
                );
                self.v[reg_x] &= self.v[reg_y];
            }
            Call { addr } => {
                info!(
                    "Storing current PC {:x} on the stack and jumping to {:x}",
                    self.pc, addr
                );
                self.stack[self.sp] = self.pc as u16;
                self.sp += 1;
                new_pc = addr;
            }
            Clear => {
                info!("Clearing screen");
                {
                    let mut fb = self.frame_buffer.write().unwrap();
                    *fb = [0; FRAME_BUFFER_BYTES];
                }
            }
            Draw {
                reg_x,
                reg_y,
                sprite_bytes,
            } => {
                let sprite_bytes = sprite_bytes as usize;
                let x = self.v[reg_x];
                let y = self.v[reg_y];
                info!(
                    "Drawing {} bytes of sprite from address {:x} at location {},{} on the screen",
                    sprite_bytes, self.i, x, y
                );
                self.draw_sprite(self.i, sprite_bytes, x, y);
            }
            LdIAddr { addr } => {
                info!("Loading reg I with address {:x}", addr);
                self.i = addr as usize;
            }
            LdIDigitReg { reg } => {
                let sprite_digit = self.v[reg];
                let addr = 5 * u16::from(sprite_digit);
                info!(
                    "Loading I with address {:x} from V{}, where sprite digit {:x} is stored",
                    addr, reg, sprite_digit
                );
                self.i = addr as usize;
            }
            LdMemIBcdReg { reg } => {
                let reg_val = self.v[reg];
                let hundreds = reg_val / 100;
                let tens = reg_val / 10 % 10;
                let ones = reg_val % 10;
                self.memory[self.i] = hundreds;
                self.memory[self.i + 1] = tens;
                self.memory[self.i + 2] = ones;
            }
            LdMemIRegs { last_reg } => {
                info!(
                    "Copying regs 0 through {} into memory address {:x} and incrementing I by {}",
                    last_reg,
                    self.i,
                    last_reg + 1
                );
                for i in 0..=last_reg {
                    self.memory[self.i + i] = self.v[i];
                }
                self.i += last_reg + 1;
            }
            LdRegByte { reg, val } => {
                info!("Loading reg V{} with value {:x}", reg, val);
                self.v[reg] = val;
            }
            LdRegKey { reg } => {
                while let Ok(event) = self.key_event_receiver.recv() {
                    if let Some(true) = event.press(|button| {
                        if let Some(keycode) = get_keycode(button) {
                            self.key_state[keycode] = true;
                            self.v[reg] = keycode as u8;
                            return true;
                        }
                        false
                    }) {
                        break;
                    }
                    event.release(|button| {
                        if let Some(keycode) = get_keycode(button) {
                            self.key_state[keycode] = false;
                        }
                    });
                }
            }
            LdRegsMemI { last_reg } => {
                info!(
                    "Loading regs 0 through {} with data in memory starting at address {:x} and incrementing I by {}",
                    last_reg, self.i, last_reg + 1
                );
                for i in 0..=last_reg {
                    self.v[i] = self.memory[self.i + i]
                }
                self.i += last_reg + 1;
            }
            LdRegReg { reg_x, reg_y } => {
                info!(
                    "Setting the value of V{} to {}(V{})",
                    reg_x, self.v[reg_y], reg_y
                );
                self.v[reg_x] = self.v[reg_y];
            }
            Jump { addr } => {
                info!("Jumping to address {:x} instead of {:x}", addr, new_pc);
                new_pc = addr;
            }
            RandRegByte { reg, val } => {
                let rand_val = self.rng.gen_byte();
                info!(
                    "Generating a random byte, {:x}, AND-ing with {:x}, and storing in V{}",
                    rand_val, val, reg
                );
                self.v[reg] = val & rand_val;
            }
            Ret => {
                self.sp -= 1;
                info!("returning to address {:x}", self.stack[self.sp]);
                new_pc = self.stack[self.sp] as usize + 2;
            }
            ShiftRightReg { reg } => {
                info!("Shifting-right V{} value: {:x}", reg, self.v[reg]);
                self.v[0xF] = self.v[reg] & 1;
                self.v[reg] >>= 1;
            }
            SkipEqRegBytes { reg, val } => {
                let reg_val = self.v[reg];
                if reg_val == val {
                    info!(
                        "Skiping next instr because {}(V{}) == {}",
                        reg_val, reg, val
                    );
                    new_pc += 2;
                }
            }
            SkipNEqRegBytes { reg, val } => {
                let reg_val = self.v[reg];
                if reg_val != val {
                    info!(
                        "Skiping next instr because {}(V{}) != {}",
                        reg_val, reg, val
                    );
                    new_pc += 2;
                }
            }
            SkipNEqRegs { reg_x, reg_y } => {
                info!(
                    "Skipping next instruction if {}(V{}) != {}(V{})",
                    self.v[reg_x], reg_x, self.v[reg_y], reg_y
                );
                if self.v[reg_x] != self.v[reg_y] {
                    new_pc += 2;
                }
            }
            SkipRegKeyPressed { reg } => {
                info!(
                    "Skipping next instr if key {:x}, indicated by register V{} is pressed",
                    self.v[reg], reg
                );
                if self.key_state[self.v[reg] as usize] {
                    new_pc += 2;
                }
            }
            SkipRegKeyNPressed { reg } => {
                info!(
                    "Skipping next instr if key {:x}, indicated by register V{} is _not_ pressed",
                    self.v[reg], reg
                );
                if !self.key_state[self.v[reg] as usize] {
                    new_pc += 2;
                }
            }
            Sys => info!("SYS instruction found, ignoring"),
        }

        self.pc = new_pc;
    }

    fn draw_sprite(&mut self, sprite_location: usize, sprite_bytes: usize, x: u8, y: u8) {
        let sprite: &[u8] = &self.memory[sprite_location..(sprite_location + sprite_bytes)];

        let first_byte = (y * 8 + (x / 8)) as usize;
        let mut second_byte = first_byte + 1;

        // Wrap around rather than going to the next row
        if second_byte % 8 == 0 {
            second_byte -= 8;
        }

        let mut collision = false;

        let mut frame_buffer = self.frame_buffer.write().unwrap();

        for (i, byte) in sprite.iter().enumerate() {
            let bit_offset = x % 8;
            let sprite_location_first_byte = (first_byte + (i * 8)) % FRAME_BUFFER_BYTES;
            let sprite_location_second_byte = (second_byte + (i * 8)) % FRAME_BUFFER_BYTES;

            let old_first_byte: u8 = frame_buffer[sprite_location_first_byte];
            let old_second_byte: u8 = frame_buffer[sprite_location_second_byte];

            frame_buffer[sprite_location_first_byte] ^= byte >> bit_offset;

            let new_first_byte = frame_buffer[sprite_location_first_byte];

            if let Some(lower_bits) = byte.checked_shl(u32::from(8 - bit_offset)) {
                frame_buffer[sprite_location_second_byte] ^= lower_bits
            }

            let new_second_byte = frame_buffer[sprite_location_second_byte];

            // Check if any pixel went from 1 -> 0
            for i in 0..8 {
                if ((new_first_byte >> i) & 0x1) < ((old_first_byte >> i) & 0x1)
                    || ((new_second_byte >> i) & 0x1) < ((old_second_byte >> i) & 0x1)
                {
                    collision = true;
                }
            }
        }

        if collision {
            self.v[0xF] = 1;
        } else {
            self.v[0xF] = 0;
        }
    }
}

fn decode_instruction(code: &[u8]) -> OpCode {
    match code {
        [0x00, 0xE0] => Clear,
        [0x00, 0xEE] => Ret,
        [0x00...0x0F, _] => Sys,
        [msb @ 0x10...0x1F, lsb] => Jump {
            addr: extract_addr(*msb, *lsb),
        },
        [msb @ 0x20...0x2F, lsb] => Call {
            addr: extract_addr(*msb, *lsb),
        },
        [msb @ 0x30...0x3F, lsb] => SkipEqRegBytes {
            reg: extract_lower_nibble(*msb),
            val: *lsb,
        },
        [msb @ 0x40...0x4F, lsb] => SkipNEqRegBytes {
            reg: extract_lower_nibble(*msb),
            val: *lsb,
        },
        [msb @ 0x60...0x6F, lsb] => LdRegByte {
            reg: extract_lower_nibble(*msb),
            val: *lsb,
        },
        [msb @ 0x70...0x7F, lsb] => AddRegByte {
            reg: extract_lower_nibble(*msb) & 0xF,
            val: *lsb,
        },
        [msb @ 0x80...0x8F, lsb] => match lsb & 0xF {
            0x0 => LdRegReg {
                reg_x: extract_lower_nibble(*msb),
                reg_y: extract_upper_nibble(*lsb),
            },
            0x2 => AndRegs {
                reg_x: extract_lower_nibble(*msb),
                reg_y: extract_upper_nibble(*lsb),
            },
            0x4 => AddRegs {
                reg_x: extract_lower_nibble(*msb),
                reg_y: extract_upper_nibble(*lsb),
            },
            0x6 => ShiftRightReg {
                reg: extract_lower_nibble(*msb),
            },
            _ => panic!(
                "Unknown op code {:x}",
                *lsb as usize | ((*msb as usize) << 8)
            ),
        },
        [msb @ 0x90...0x9F, lsb] => SkipNEqRegs {
            reg_x: extract_lower_nibble(*msb),
            reg_y: extract_upper_nibble(*lsb),
        },
        [msb @ 0xA0...0xAF, lsb] => LdIAddr {
            addr: extract_addr(*msb, *lsb),
        },
        [msb @ 0xC0...0xCF, lsb] => RandRegByte {
            reg: extract_lower_nibble(*msb),
            val: *lsb,
        },
        [msb @ 0xD0...0xDF, lsb] => Draw {
            reg_x: extract_lower_nibble(*msb),
            reg_y: extract_upper_nibble(*lsb),
            sprite_bytes: lsb & 0xF,
        },
        [msb @ 0xE0...0xEF, 0x9E] => SkipRegKeyPressed {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xE0...0xEF, 0xA1] => SkipRegKeyNPressed {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x0A] => LdRegKey {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x1E] => AddIReg {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x29] => LdIDigitReg {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x33] => LdMemIBcdReg {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x55] => LdMemIRegs {
            last_reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x65] => LdRegsMemI {
            last_reg: extract_lower_nibble(*msb),
        },
        [msb, lsb] => panic!(
            "Unknown op code {:x}",
            *lsb as usize | ((*msb as usize) << 8)
        ),
        _ => panic!("Invalid program counter"),
    }
}

// Extracts the least significant 12 bits out of the two bytes representing a
// memory address (possibly part of an instruction), and casts to usize.
fn extract_addr(msb: u8, lsb: u8) -> usize {
    lsb as usize | (extract_lower_nibble(msb) << 8)
}

// Extracts the most significant 4 bits and casts to usize
fn extract_upper_nibble(byte: u8) -> usize {
    (byte as usize & 0xF0) >> 4
}

// Extract the least significant 4 bits and casts to usize
fn extract_lower_nibble(byte: u8) -> usize {
    byte as usize & 0xF
}

// Returns keycode 0 -> F of the button if there is one
fn get_keycode(button: Button) -> Option<usize> {
    match button {
        Button::Keyboard(Key::D0) => Some(0),
        Button::Keyboard(Key::D1) => Some(1),
        Button::Keyboard(Key::D2) => Some(2),
        Button::Keyboard(Key::D3) => Some(3),
        Button::Keyboard(Key::D4) => Some(4),
        Button::Keyboard(Key::D5) => Some(5),
        Button::Keyboard(Key::D6) => Some(6),
        Button::Keyboard(Key::D7) => Some(7),
        Button::Keyboard(Key::D8) => Some(8),
        Button::Keyboard(Key::D9) => Some(9),
        Button::Keyboard(Key::A) => Some(0xA),
        Button::Keyboard(Key::B) => Some(0xB),
        Button::Keyboard(Key::C) => Some(0xC),
        Button::Keyboard(Key::D) => Some(0xD),
        Button::Keyboard(Key::E) => Some(0xE),
        Button::Keyboard(Key::F) => Some(0xF),
        _ => None,
    }
}
