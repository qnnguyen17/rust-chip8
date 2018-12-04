extern crate piston_window;
extern crate rand;

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
        reg: u8,
    },
    AddRegByte {
        reg: u8,
        val: u8,
    },
    AddRegs {
        reg_x: u8,
        reg_y: u8,
    },
    AndRegs {
        reg_x: u8,
        reg_y: u8,
    },
    Call {
        addr: u16,
    },
    Clear,
    Draw {
        reg_x: u8,
        reg_y: u8,
        sprite_bytes: u8,
    },
    LdIAddr {
        addr: u16,
    },
    LdIDigitReg {
        reg: u8,
    },
    LdMemIBcdReg {
        reg: u8,
    },
    LdMemIRegs {
        last_reg: u8,
    },
    LdRegByte {
        reg: u8,
        val: u8,
    },
    LdRegKey {
        reg: u8,
    },
    LdRegsMemI {
        last_reg: u8,
    },
    LdRegReg {
        reg_x: u8,
        reg_y: u8,
    },
    Jump {
        addr: u16,
    },
    RandRegByte {
        reg: u8,
        val: u8,
    },
    Ret,
    SkipEqRegBytes {
        reg: u8,
        val: u8,
    },
    SkipNEqRegBytes {
        reg: u8,
        val: u8,
    },
    ShiftRightReg {
        reg: u8,
    },
    Sys,
}

pub struct CPU {
    // General-purpose registers
    v: [u8; 16],
    // Memory address register
    i: u16,
    sound_timer: u8,
    delay_timer: u8,
    // Program counter
    pc: u16,
    stack: [u16; 16],
    // Stack pointer
    sp: u8,
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
                    self.key_state[keycode as usize] = true;
                }
            });
            event.release(|button| {
                if let Some(keycode) = get_keycode(button) {
                    self.key_state[keycode as usize] = false;
                }
            });
        }
    }

    fn execute(&mut self, op: OpCode) {
        let mut new_pc = self.pc + 2;
        match op {
            AddIReg { reg } => {
                let reg_val = self.get_reg_val(reg);
                info!(
                    "Adding {:x} from reg V{} to I's current value {:x}",
                    reg_val, reg, self.i
                );
                self.i += u16::from(reg_val);
            }
            AddRegByte { reg, val } => {
                info!(
                    "Adding val {:x} to register V{} to get value: {}",
                    val,
                    reg,
                    self.get_reg_val(reg).wrapping_add(val),
                );
                self.v[reg as usize] = self.get_reg_val(reg).wrapping_add(val);
            }
            AddRegs { reg_x, reg_y } => {
                info!(
                    "Adding val {}(V{}) and {}(V{}), storing in V{}",
                    self.get_reg_val(reg_x),
                    reg_x,
                    self.get_reg_val(reg_y),
                    reg_y,
                    reg_x
                );
                let (sum, did_overflow) = self
                    .get_reg_val(reg_x)
                    .overflowing_add(self.get_reg_val(reg_y));
                self.v[reg_x as usize] = sum;
                self.v[0xF] = if did_overflow { 1 } else { 0 };
            }
            AndRegs { reg_x, reg_y } => {
                info!(
                    "AND-ing register V{} and V{}, storing value in V{}",
                    reg_x, reg_y, reg_x
                );
                self.v[reg_x as usize] &= self.get_reg_val(reg_y);
            }
            Call { addr } => {
                info!(
                    "Storing current PC {:x} on the stack and jumping to {:x}",
                    self.pc, addr
                );
                self.stack[self.sp as usize] = self.pc;
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
                let i = self.i as usize;
                let sprite_bytes = sprite_bytes as usize;
                let x = self.get_reg_val(reg_x);
                let y = self.get_reg_val(reg_y);
                info!(
                    "Drawing {} bytes of sprite from address {:x} at location {},{} on the screen",
                    sprite_bytes, i, x, y
                );
                self.draw_sprite(i, sprite_bytes, x, y);
            }
            LdIAddr { addr } => {
                info!("Loading reg I with address {:x}", addr);
                self.i = addr;
            }
            LdIDigitReg { reg } => {
                let sprite_digit = self.get_reg_val(reg);
                let addr = 5 * u16::from(sprite_digit);
                info!(
                    "Loading I with address {:x} from V{}, where sprite digit {:x} is stored",
                    addr, reg, sprite_digit
                );
                self.i = addr;
            }
            LdMemIBcdReg { reg } => {
                let reg_val = self.get_reg_val(reg);
                let hundreds = reg_val / 100;
                let tens = reg_val / 10 % 10;
                let ones = reg_val % 10;
                let i = self.i as usize;
                self.memory[i] = hundreds;
                self.memory[i + 1] = tens;
                self.memory[i + 2] = ones;
            }
            LdMemIRegs { last_reg } => {
                info!(
                    "Copying regs 0 through {} into memory address {:x} and incrementing I by {}",
                    last_reg,
                    self.i,
                    last_reg + 1
                );
                for i in 0..=last_reg {
                    self.memory[(self.i + u16::from(i)) as usize] = self.get_reg_val(i);
                }
                self.i += u16::from(last_reg) + 1;
            }
            LdRegByte { reg, val } => {
                info!("Loading reg V{} with value {:x}", reg, val);
                self.v[reg as usize] = val;
            }
            LdRegKey { reg } => {
                while let Ok(event) = self.key_event_receiver.recv() {
                    if let Some(true) = event.press(|button| {
                        if let Some(keycode) = get_keycode(button) {
                            self.key_state[keycode as usize] = true;
                            self.v[reg as usize] = keycode;
                            return true;
                        }
                        return false;
                    }) {
                        break;
                    }
                    event.release(|button| {
                        if let Some(keycode) = get_keycode(button) {
                            self.key_state[keycode as usize] = false;
                        }
                    });
                }
            }
            LdRegsMemI { last_reg } => {
                info!(
                    "Loading regs 0 through {} with data in memory starting at address {:x} and incrementing I by {}",
                    last_reg, self.i, last_reg + 1
                );
                for i in 0..=last_reg as usize {
                    self.v[i] = self.memory[self.i as usize + i]
                }
                self.i += u16::from(last_reg) + 1;
            }
            LdRegReg { reg_x, reg_y } => {
                info!(
                    "Setting the value of V{} to {}(V{})",
                    reg_x,
                    self.get_reg_val(reg_y),
                    reg_y
                );
                self.v[reg_x as usize] = self.get_reg_val(reg_y);
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
                self.v[reg as usize] = val & rand_val;
            }
            Ret => {
                self.sp -= 1;
                info!("returning to address {:x}", self.stack[self.sp as usize]);
                new_pc = self.stack[self.sp as usize] + 2;
            }
            SkipEqRegBytes { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val == val {
                    info!(
                        "Skipping next instr because {}(V{}) == {}",
                        reg_val, reg, val
                    );
                    new_pc += 2;
                }
            }
            SkipNEqRegBytes { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val != val {
                    info!(
                        "Skipping next instr because {}(V{}) != {}",
                        reg_val, reg, val
                    );
                    new_pc += 2;
                }
            }
            ShiftRightReg { reg } => {
                info!("Shifting-right V{} value: {:x}", reg, self.get_reg_val(reg));
                self.v[0xF] = self.get_reg_val(reg) & 1;
                self.v[reg as usize] >>= 1;
            }
            Sys => info!("SYS instruction found, ignoring"),
        }

        self.pc = new_pc;
    }

    fn get_reg_val(&mut self, reg: u8) -> u8 {
        self.v[reg as usize]
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
            reg: msb & 0xF,
            val: *lsb,
        },
        [msb @ 0x40...0x4F, lsb] => SkipNEqRegBytes {
            reg: msb & 0xF,
            val: *lsb,
        },
        [msb @ 0x60...0x6F, lsb] => LdRegByte {
            reg: msb & 0xF,
            val: *lsb,
        },
        [msb @ 0x70...0x7F, lsb] => AddRegByte {
            reg: msb & 0xF,
            val: *lsb,
        },
        [msb @ 0x80...0x8F, lsb] => match lsb & 0xF {
            0x0 => LdRegReg {
                reg_x: msb & 0xF,
                reg_y: extract_upper_nibble(*lsb),
            },
            0x2 => AndRegs {
                reg_x: msb & 0xF,
                reg_y: extract_upper_nibble(*lsb),
            },
            0x4 => AddRegs {
                reg_x: msb & 0xF,
                reg_y: extract_upper_nibble(*lsb),
            },
            0x6 => ShiftRightReg { reg: msb & 0xF },
            _ => panic!(
                "Unknown op code {:x}",
                *lsb as usize | ((*msb as usize) << 8)
            ),
        },
        [msb @ 0xA0...0xAF, lsb] => LdIAddr {
            addr: extract_addr(*msb, *lsb),
        },
        [msb @ 0xC0...0xCF, lsb] => RandRegByte {
            reg: msb & 0xF,
            val: *lsb,
        },
        [msb @ 0xD0...0xDF, lsb] => Draw {
            reg_x: msb & 0xF,
            reg_y: extract_upper_nibble(*lsb),
            sprite_bytes: lsb & 0xF,
        },
        [msb @ 0xF0...0xFF, 0x0A] => LdRegKey { reg: msb & 0xF },
        [msb @ 0xF0...0xFF, 0x1E] => AddIReg { reg: msb & 0xF },
        [msb @ 0xF0...0xFF, 0x29] => LdIDigitReg { reg: msb & 0xF },
        [msb @ 0xF0...0xFF, 0x33] => LdMemIBcdReg { reg: msb & 0xF },
        [msb @ 0xF0...0xFF, 0x55] => LdMemIRegs {
            last_reg: msb & 0x0F,
        },
        [msb @ 0xF0...0xFF, 0x65] => LdRegsMemI {
            last_reg: msb & 0x0F,
        },
        [msb, lsb] => panic!(
            "Unknown op code {:x}",
            *lsb as usize | ((*msb as usize) << 8)
        ),
        _ => panic!("Invalid program counter"),
    }
}

// Extracts the least significant 12 bits out of the two bytes representing a
// memory address (possibly part of an instruction).
fn extract_addr(msb: u8, lsb: u8) -> u16 {
    u16::from(lsb) | ((u16::from(msb) & 0x0F) << 8)
}

// Extracts the most significant 4 bits
fn extract_upper_nibble(byte: u8) -> u8 {
    (byte & 0xF0) >> 4
}

// Returns keycode 0 -> F of the button if there is one
fn get_keycode(button: Button) -> Option<u8> {
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

#[cfg(test)]
mod tests {
    use super::super::FRAME_BUFFER_BYTES;
    use super::decode_instruction;
    use super::OpCode::*;
    use super::WrappedRng;
    use super::CPU;
    use rand::rngs::mock::StepRng;
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::sync::RwLock;

    //
    // DECODE tests
    //
    #[test]
    fn decode_add_i_reg() {
        assert_eq!(AddIReg { reg: 0 }, decode_instruction(&[0xF0, 0x1E]));
    }

    #[test]
    fn decode_add_reg_byte() {
        assert_eq!(
            AddRegByte { reg: 0, val: 8 },
            decode_instruction(&[0x70, 0x08])
        );
    }

    #[test]
    fn decode_add_regs() {
        assert_eq!(
            AddRegs { reg_x: 2, reg_y: 7 },
            decode_instruction(&[0x82, 0x74])
        );
    }

    #[test]
    fn decode_and_regs() {
        assert_eq!(
            AndRegs { reg_x: 0, reg_y: 1 },
            decode_instruction(&[0x80, 0x12])
        );
    }

    #[test]
    fn decode_call() {
        assert_eq!(Call { addr: 0x123 }, decode_instruction(&[0x21, 0x23]));
    }

    #[test]
    fn decode_draw() {
        assert_eq!(
            Draw {
                reg_x: 6,
                reg_y: 7,
                sprite_bytes: 5,
            },
            decode_instruction(&[0xD6, 0x75])
        );
    }

    #[test]
    fn decode_ld_i_addr() {
        assert_eq!(LdIAddr { addr: 0x123 }, decode_instruction(&[0xA1, 0x23]));
    }

    #[test]
    fn decode_ld_i_digit_reg() {
        assert_eq!(LdIDigitReg { reg: 0 }, decode_instruction(&[0xF0, 0x29]));
    }

    #[test]
    fn decode_ld_mem_i_bcd_reg() {
        assert_eq!(LdMemIBcdReg { reg: 5 }, decode_instruction(&[0xF5, 0x33]));
    }

    #[test]
    fn decode_ld_mem_i_regs() {
        assert_eq!(
            LdMemIRegs { last_reg: 1 },
            decode_instruction(&[0xF1, 0x55])
        );
    }

    #[test]
    fn decode_ld_reg_byte() {
        assert_eq!(
            LdRegByte { reg: 1, val: 0xFF },
            decode_instruction(&[0x61, 0xFF])
        );
    }

    #[test]
    fn decode_ld_reg_key() {
        assert_eq!(LdRegKey { reg: 5 }, decode_instruction(&[0xF5, 0x0A]));
    }

    #[test]
    fn decode_ld_regs_mem_i() {
        assert_eq!(
            LdRegsMemI { last_reg: 4 },
            decode_instruction(&[0xF4, 0x65])
        );
    }

    #[test]
    fn decode_ld_reg_reg() {
        assert_eq!(
            LdRegReg { reg_x: 0, reg_y: 1 },
            decode_instruction(&[0x80, 0x10])
        );
    }

    #[test]
    fn decode_jump() {
        assert_eq!(Jump { addr: 0x500 }, decode_instruction(&[0x15, 0x00]));
    }

    #[test]
    fn decode_rand_reg_byte() {
        assert_eq!(
            RandRegByte { reg: 5, val: 0x15 },
            decode_instruction(&[0xC5, 0x15])
        );
    }

    #[test]
    fn decode_ret() {
        assert_eq!(Ret, decode_instruction(&[0x00, 0xEE]));
    }

    #[test]
    fn decode_skip_eq_reg_bytes() {
        assert_eq!(
            SkipEqRegBytes { reg: 0, val: 0x16 },
            decode_instruction(&[0x30, 0x16])
        );
    }

    #[test]
    fn decode_skip_neq_reg_bytes() {
        assert_eq!(
            SkipNEqRegBytes { reg: 0, val: 0x16 },
            decode_instruction(&[0x40, 0x16])
        );
    }

    #[test]
    fn decode_shift_right_reg() {
        assert_eq!(ShiftRightReg { reg: 0 }, decode_instruction(&[0x80, 0x66]));
    }

    //
    // EXECUTE tests
    //
    #[test]
    fn execute_add_i_reg() {
        let mut cpu = create_cpu();
        cpu.v[0] = 1;
        cpu.i = 5;
        cpu.execute(AddIReg { reg: 0 });
        assert_eq!(6, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_add_reg_byte() {
        let mut cpu = create_cpu();
        cpu.v[0] = 0;
        cpu.execute(AddRegByte { reg: 0, val: 16 });
        assert_eq!(16, cpu.v[0]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_add_regs_no_overflow() {
        let mut cpu = create_cpu();
        cpu.v[0] = 1;
        cpu.v[5] = 5;
        cpu.execute(AddRegs { reg_x: 0, reg_y: 5 });
        assert_eq!(6, cpu.v[0]);
        assert_eq!(0, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_add_regs_overflow() {
        let mut cpu = create_cpu();
        cpu.v[3] = 1;
        cpu.v[7] = 0xFF;
        cpu.execute(AddRegs { reg_x: 3, reg_y: 7 });
        assert_eq!(0, cpu.v[3]);
        assert_eq!(1, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_and_regs() {
        let mut cpu = create_cpu();
        cpu.v[0] = 0b111;
        cpu.v[1] = 0b101;
        cpu.execute(AndRegs { reg_x: 0, reg_y: 1 });
        assert_eq!(0b101, cpu.v[0]);
        assert_eq!(0b101, cpu.v[1]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_call() {
        let mut cpu = create_cpu();
        cpu.sp = 1;
        cpu.pc = 0x112;
        cpu.execute(Call { addr: 0x114 });
        assert_eq!(2, cpu.sp);
        assert_eq!(0x112, cpu.stack[1]);
        assert_eq!(0x114, cpu.pc);
    }

    #[test]
    fn execute_draw_no_collision() {
        let mut cpu = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 4;
        cpu.v[1] = 0;
        cpu.v[0xF] = 1;
        cpu.memory[0] = 0b01110111;
        {
            let mut fb = cpu.frame_buffer.write().unwrap();
            fb[0] = 0b1000;
            fb[1] = 0b10000000;
        }

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 1,
        });

        {
            let fb = cpu.frame_buffer.read().unwrap();
            assert_eq!(0xF, fb[0]);
            assert_eq!(0xF0, fb[1]);
            assert_eq!(0, cpu.v[0xF]);
            assert_eq!(0x202, cpu.pc);
        }
    }

    #[test]
    fn execute_draw_collision() {
        let mut cpu = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 4;
        cpu.v[1] = 1;
        cpu.v[0xF] = 0;
        cpu.memory[0] = 0b01110111;

        {
            let mut fb = cpu.frame_buffer.write().unwrap();
            fb[8] = 0xF;
            fb[9] = 0xF0;
        }

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 1,
        });

        {
            let fb = cpu.frame_buffer.read().unwrap();
            assert_eq!(0x8, fb[8]);
            assert_eq!(0x80, fb[9]);
            assert_eq!(1, cpu.v[0xF]);
            assert_eq!(0x202, cpu.pc);
        }
    }

    #[test]
    fn execute_draw_wraparound_horizontal() {
        let mut cpu = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 60;
        cpu.v[1] = 0;
        cpu.v[0xF] = 0;
        cpu.memory[0] = 0xFF;

        {
            let mut fb = cpu.frame_buffer.write().unwrap();
            fb[0] = 0x80;
            fb[7] = 0x1;
        }

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 1,
        });

        {
            let fb = cpu.frame_buffer.read().unwrap();
            assert_eq!(0b1110, fb[7]);
            assert_eq!(0b01110000, fb[0]);
            assert_eq!(1, cpu.v[0xF]);
            assert_eq!(0x202, cpu.pc);
        }
    }

    #[test]
    fn execute_draw_wraparound_vertical() {
        let mut cpu = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 0;
        cpu.v[1] = 31;
        cpu.memory[0] = 0xFF;
        cpu.memory[1] = 0xFF;

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 2,
        });

        {
            let fb = cpu.frame_buffer.read().unwrap();
            assert_eq!(0xFF, fb[0]);
            assert_eq!(0xFF, fb[248]); // beginning of last row, 31*8
            assert_eq!(0, cpu.v[0xF]);
            assert_eq!(0x202, cpu.pc);
        }
    }

    #[test]
    fn execute_ld_i_addr() {
        let mut cpu = create_cpu();
        cpu.execute(LdIAddr { addr: 0x123 });
        assert_eq!(0x123, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_i_digit_reg() {
        let mut cpu = create_cpu();
        cpu.v[1] = 2;
        cpu.execute(LdIDigitReg { reg: 1 });
        assert_eq!(10, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_mem_i_bcd_reg() {
        let mut cpu = create_cpu();
        cpu.v[5] = 254;
        cpu.i = 200;
        cpu.execute(LdMemIBcdReg { reg: 5 });
        assert_eq!(2, cpu.memory[200]);
        assert_eq!(5, cpu.memory[201]);
        assert_eq!(4, cpu.memory[202]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_mem_i_regs() {
        let mut cpu = create_cpu();
        cpu.i = 0x100;
        cpu.v[0] = 5;
        cpu.v[1] = 6;
        cpu.execute(LdMemIRegs { last_reg: 1 });
        assert_eq!(5, cpu.memory[0x100]);
        assert_eq!(6, cpu.memory[0x101]);
        assert_eq!(0x102, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_reg_byte() {
        let mut cpu = create_cpu();
        cpu.execute(LdRegByte { reg: 1, val: 0xFF });
        assert_eq!(0xFF, cpu.v[1]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_regs_mem_i() {
        let mut cpu = create_cpu();
        cpu.i = 0x100;
        cpu.memory[0x100] = 1;
        cpu.memory[0x101] = 2;
        cpu.memory[0x102] = 3;
        cpu.execute(LdRegsMemI { last_reg: 2 });
        assert_eq!(1, cpu.v[0]);
        assert_eq!(2, cpu.v[1]);
        assert_eq!(3, cpu.v[2]);
        assert_eq!(0x103, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_reg_reg() {
        let mut cpu = create_cpu();
        cpu.v[0] = 5;
        cpu.v[1] = 6;
        cpu.execute(LdRegReg { reg_x: 0, reg_y: 1 });
        assert_eq!(6, cpu.v[0]);
        assert_eq!(6, cpu.v[1]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_jump() {
        let mut cpu = create_cpu();
        cpu.execute(Jump { addr: 0x2e8 });
        assert_eq!(0x2e8, cpu.pc);
    }

    #[test]
    fn execute_rand_reg_byte() {
        let mut cpu = create_cpu();
        // The CPU's RNG will always yield 0xF
        cpu.rng = WrappedRng::Mock(StepRng::new(0xF, 0));
        cpu.execute(RandRegByte { reg: 3, val: 0xAB });
        assert_eq!(0xB, cpu.v[3]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ret() {
        let mut cpu = create_cpu();
        cpu.sp = 1;
        cpu.stack[0] = 0x200;
        cpu.execute(Ret);
        assert_eq!(0x202, cpu.pc);
        assert_eq!(0, cpu.sp);
    }

    #[test]
    fn execute_skip_eq_reg_bytes() {
        let mut cpu = create_cpu();
        cpu.v[4] = 16;
        cpu.execute(SkipEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x204, cpu.pc);

        let mut cpu = create_cpu();
        cpu.v[4] = 14;
        cpu.execute(SkipEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_skip_neq_reg_bytes() {
        let mut cpu = create_cpu();
        cpu.v[4] = 16;
        cpu.execute(SkipNEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x202, cpu.pc);

        let mut cpu = create_cpu();
        cpu.v[4] = 14;
        cpu.execute(SkipNEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x204, cpu.pc);
    }

    #[test]
    fn execute_shift_right_reg_unset_vf() {
        let mut cpu = create_cpu();
        cpu.v[0] = 0b110;
        cpu.v[0xF] = 1;
        cpu.execute(ShiftRightReg { reg: 0 });
        assert_eq!(0b11, cpu.v[0]);
        assert_eq!(0, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_shift_right_reg_set_vf() {
        let mut cpu = create_cpu();
        cpu.v[0] = 0b1111;
        cpu.execute(ShiftRightReg { reg: 0 });
        assert_eq!(0b111, cpu.v[0]);
        assert_eq!(1, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);
    }

    fn create_cpu() -> CPU {
        let frame_buffer = Arc::new(RwLock::new([0; FRAME_BUFFER_BYTES]));
        CPU::new(frame_buffer, channel().1, channel().1)
    }
}
