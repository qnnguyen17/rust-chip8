mod decode;
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
use std::sync::Mutex;
use std::sync::RwLock;

use self::decode::*;
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
    LdDtReg {
        reg: usize,
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
    LdRegDt {
        reg: usize,
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
    OrRegs {
        reg_x: usize,
        reg_y: usize,
    },
    RandRegByte {
        reg: usize,
        val: u8,
    },
    Ret,
    ShiftLeftReg {
        reg: usize,
    },
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
    SubRegs {
        reg_x: usize,
        reg_y: usize,
    },
    Sys,
    XorRegs {
        reg_x: usize,
        reg_y: usize,
    },
}

pub struct CPU {
    // General-purpose registers
    v: [u8; 16],
    // Memory address register
    i: usize,
    sound_timer: u8,
    delay_timer: Arc<Mutex<u8>>,

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
        delay_timer: Arc<Mutex<u8>>,
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
            delay_timer,
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
                if let Some(keycode) = decode_key(button) {
                    self.key_state[keycode] = true;
                }
            });
            event.release(|button| {
                if let Some(keycode) = decode_key(button) {
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
            LdDtReg { reg } => {
                info!("Loading delay timer with {}(V{})", self.v[reg], reg);
                let mut delay_timer = self.delay_timer.lock().unwrap();
                *delay_timer = self.v[reg];
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
            LdRegDt { reg } => {
                info!("Loading reg V{} with value {} from DT", reg, self.v[reg]);
                self.v[reg] = *self.delay_timer.lock().unwrap();
            }
            LdRegKey { reg } => {
                while let Ok(event) = self.key_event_receiver.recv() {
                    if let Some(true) = event.press(|button| {
                        if let Some(keycode) = decode_key(button) {
                            self.key_state[keycode] = true;
                            self.v[reg] = keycode as u8;
                            return true;
                        }
                        false
                    }) {
                        break;
                    }
                    event.release(|button| {
                        if let Some(keycode) = decode_key(button) {
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
            OrRegs { reg_x, reg_y } => {
                info!(
                    "ORing {}(V{}) with {}(V{}) and storing in V{}",
                    self.v[reg_x], reg_x, self.v[reg_y], reg_y, reg_x
                );
                self.v[reg_x] |= self.v[reg_y];
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
            ShiftLeftReg { reg } => {
                info!("Shifting-left V{} value: {:x}", reg, self.v[reg]);
                self.v[0xF] = (self.v[reg] >> 7) & 1;
                self.v[reg] <<= 1;
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
            SubRegs { reg_x, reg_y } => {
                info!(
                    "Subtracting {}(V{}) from {}(V{}) and storing in V{}",
                    self.v[reg_y], reg_y, self.v[reg_x], reg_x, reg_x
                );
                self.v[0xF] = if self.v[reg_x] > self.v[reg_y] { 1 } else { 0 };
                self.v[reg_x] = self.v[reg_x].wrapping_sub(self.v[reg_y]);
            }
            Sys => info!("SYS instruction found, ignoring"),
            XorRegs { reg_x, reg_y } => {
                info!(
                    "XORing {}(V{}) with {}(V{}) and storing in V{}",
                    self.v[reg_x], reg_x, self.v[reg_y], reg_y, reg_x
                );
                self.v[reg_x] ^= self.v[reg_y];
            }
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
