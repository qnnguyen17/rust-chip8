use std::fs::File;
use std::io::prelude::*;
use std::io::Result;
use std::sync::mpsc::Sender;

use self::OpCode::*;
use super::digits::DIGITS;

#[derive(Clone, Copy, Debug, PartialEq)]
enum OpCode {
    AddIReg {
        reg: u8,
    },
    AddRegByte {
        reg: u8,
        val: u8,
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
    LdMemIRegs {
        last_reg: u8,
    },
    LdRegByte {
        reg: u8,
        val: u8,
    },
    LdRegsMemI {
        last_reg: u8,
    },
    Jump {
        addr: u16,
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
    key: [bool; 16],

    // TODO: make this a RefCell so we can share access with WindowHandler for drawing
    frame_buffer: [u8; 8 * 32],

    // Output for sending graphics instructions to the window handler
    graphics_bus_out: Sender<[u8; 8 * 32]>,
}

impl CPU {
    pub fn new(graphics_bus_out: Sender<[u8; 8 * 32]>) -> CPU {
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
            key: [false; 16],
            frame_buffer: [0; 8 * 32],
            graphics_bus_out,
        }
    }

    pub fn load_game_data(&mut self, file_name: &str) -> Result<()> {
        let mut game_file = File::open(file_name)?;
        assert!(game_file.read(&mut self.memory[0x200..]).unwrap() > 0);
        Ok(())
    }

    pub fn run(&mut self) {
        loop {
            let pc = self.pc as usize;
            let code = &self.memory[pc..pc + 2];
            let instr = decode_instruction(&code);
            self.execute(instr);
        }
    }

    fn execute(&mut self, op: OpCode) {
        let mut new_pc = self.pc + 2;
        match op {
            AddIReg { reg } => {
                let reg_val = self.get_reg_val(reg);
                println!(
                    "Adding {:x} from reg V{} to I's current value {:x}",
                    reg_val, reg, self.i
                );
                self.i += u16::from(reg_val);
            }
            AddRegByte { reg, val } => {
                println!(
                    "Adding val {:x} to register V{} to get value: {}",
                    val,
                    reg,
                    self.get_reg_val(reg) + val
                );
                self.v[reg as usize] += val;
            }
            AndRegs { reg_x, reg_y } => {
                println!(
                    "AND-ing register V{} and V{}, storing value in V{}",
                    reg_x, reg_y, reg_x
                );
                self.v[reg_x as usize] &= self.get_reg_val(reg_y);
            }
            Call { addr } => {
                println!(
                    "Storing current PC {:x} on the stack and jumping to {:x}",
                    self.pc, addr
                );
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                new_pc = addr;
            }
            Clear => {
                println!("Clearing screen");
                self.frame_buffer = [0; 8 * 32];
                self.graphics_bus_out
                    .send(self.frame_buffer)
                    .expect("failed to send frame after clearing");
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
                println!(
                    "Drawing {} bytes of sprite from address {:x} at location {},{} on the screen",
                    sprite_bytes, i, x, y
                );
                self.draw_sprite(i, sprite_bytes, x, y);
                self.graphics_bus_out
                    .send(self.frame_buffer)
                    .expect("failed to send frame after drawing sprite");
            }
            LdIAddr { addr } => {
                println!("Loading reg I with address {:x}", addr);
                self.i = addr;
            }
            LdIDigitReg { reg } => {
                let sprite_digit = self.get_reg_val(reg);
                let addr = 5 * u16::from(sprite_digit);
                println!(
                    "Loading I with address {:x} from V{}, where sprite digit {:x} is stored",
                    addr, reg, sprite_digit
                );
                self.i = addr;
            }
            LdMemIRegs { last_reg } => {
                println!(
                    "Copying regs 0 through {} into memory address {:x}",
                    last_reg, self.pc
                );
                for i in 0..=last_reg {
                    self.memory[(self.i + u16::from(i)) as usize] = self.get_reg_val(i);
                }
            }
            LdRegByte { reg, val } => {
                println!("Loading reg V{} with value {:x}", reg, val);
                self.v[reg as usize] = val;
            }
            LdRegsMemI { last_reg } => {
                println!(
                    "Loading regs 0 through {} with data in memory starting at address {:x}",
                    last_reg, self.i
                );
                for i in 0..=last_reg as usize {
                    self.v[i] = self.memory[self.i as usize + i]
                }
            }
            Jump { addr } => {
                println!("Jumping to address {:x} instead of {:x}", addr, new_pc);
                new_pc = addr;
            }
            Ret => {
                self.sp -= 1;
                new_pc = self.stack[self.sp as usize];
            }
            SkipEqRegBytes { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val == val {
                    println!(
                        "Skipping next instr because {}(V{}) == {}",
                        reg_val, reg, val
                    );
                    new_pc += 2;
                }
            }
            SkipNEqRegBytes { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val != val {
                    println!(
                        "Skipping next instr because {}(V{}) != {}",
                        reg_val, reg, val
                    );
                    new_pc += 2;
                }
            }
            Sys => println!("SYS instruction found, ignoring"),
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

        for (i, byte) in sprite.iter().enumerate() {
            let bit_offset = x % 8;
            let old_first_byte: u8 = self.frame_buffer[first_byte + (i * 8)];
            let old_second_byte: u8 = self.frame_buffer[second_byte + (i * 8)];

            self.frame_buffer[first_byte + (i * 8)] ^= byte >> bit_offset;

            let new_first_byte = self.frame_buffer[first_byte + (i * 8)];

            if let Some(lower_bits) = byte.checked_shl(u32::from(8 - bit_offset)) {
                self.frame_buffer[second_byte + (i * 8)] ^= lower_bits
            }

            let new_second_byte = self.frame_buffer[second_byte + (i * 8)];

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
        [msb @ 0x80...0x8F, lsb @ 0x02...0xF2] => AndRegs {
            reg_x: msb & 0xF,
            reg_y: (lsb & 0xF0) >> 4,
        },
        [msb @ 0xA0...0xAF, lsb] => LdIAddr {
            addr: extract_addr(*msb, *lsb),
        },
        [msb @ 0xD0...0xDF, lsb] => Draw {
            reg_x: msb & 0xF,
            reg_y: (lsb & 0xF0) >> 4,
            sprite_bytes: lsb & 0xF,
        },
        [msb @ 0xF0...0xFF, 0x1E] => AddIReg { reg: msb & 0xF },
        [msb @ 0xF0...0xFF, 0x29] => LdIDigitReg { reg: msb & 0xF },
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

#[cfg(test)]
mod tests {
    use super::decode_instruction;
    use super::OpCode::*;
    use super::CPU;
    use std::sync::mpsc::*;

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
                sprite_bytes: 5
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
    fn decode_ld_regs_mem_i() {
        assert_eq!(
            LdRegsMemI { last_reg: 4 },
            decode_instruction(&[0xF4, 0x65])
        );
    }

    #[test]
    fn decode_jump() {
        assert_eq!(Jump { addr: 0x500 }, decode_instruction(&[0x15, 0x00]));
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

    //
    // EXECUTE tests
    //
    #[test]
    fn execute_add_i_reg() {
        let (mut cpu, _) = create_cpu();
        cpu.v[0] = 1;
        cpu.i = 5;
        cpu.execute(AddIReg { reg: 0 });
        assert_eq!(6, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_add_reg_byte() {
        let (mut cpu, _) = create_cpu();
        cpu.v[0] = 0;
        cpu.execute(AddRegByte { reg: 0, val: 16 });
        assert_eq!(16, cpu.v[0]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_and_regs() {
        let (mut cpu, _) = create_cpu();
        cpu.v[0] = 0b111;
        cpu.v[1] = 0b101;
        cpu.execute(AndRegs { reg_x: 0, reg_y: 1 });
        assert_eq!(0b101, cpu.v[0]);
        assert_eq!(0b101, cpu.v[1]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_call() {
        let (mut cpu, _) = create_cpu();
        cpu.sp = 1;
        cpu.pc = 0x112;
        cpu.execute(Call { addr: 0x114 });
        assert_eq!(2, cpu.sp);
        assert_eq!(0x112, cpu.stack[1]);
        assert_eq!(0x114, cpu.pc);
    }

    #[test]
    fn execute_draw() {
        // No collision
        let (mut cpu, _receiver) = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 4;
        cpu.v[1] = 0;
        cpu.memory[0] = 0b01110111;
        cpu.frame_buffer[0] = 0b1000;
        cpu.frame_buffer[1] = 0b10000000;

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 1,
        });

        assert_eq!(0xF, cpu.frame_buffer[0]);
        assert_eq!(0xF0, cpu.frame_buffer[1]);
        assert_eq!(0, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);

        // Collision
        let (mut cpu, _receiver) = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 4;
        cpu.v[1] = 1;
        cpu.memory[0] = 0b01110111;
        cpu.frame_buffer[8] = 0xF;
        cpu.frame_buffer[9] = 0xF0;

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 1,
        });

        assert_eq!(0x8, cpu.frame_buffer[8]);
        assert_eq!(0x80, cpu.frame_buffer[9]);
        assert_eq!(1, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);

        // Wrap around
        let (mut cpu, _reciever) = create_cpu();
        cpu.i = 0;
        cpu.v[0] = 60;
        cpu.v[1] = 0;
        cpu.memory[0] = 0xFF;
        cpu.frame_buffer[0] = 0x80;
        cpu.frame_buffer[7] = 0x1;

        cpu.execute(Draw {
            reg_x: 0,
            reg_y: 1,
            sprite_bytes: 1,
        });
        assert_eq!(0b1110, cpu.frame_buffer[7]);
        assert_eq!(0b01110000, cpu.frame_buffer[0]);
        assert_eq!(1, cpu.v[0xF]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_i_addr() {
        let (mut cpu, _) = create_cpu();
        cpu.execute(LdIAddr { addr: 0x123 });
        assert_eq!(0x123, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_i_digit_reg() {
        let (mut cpu, _) = create_cpu();
        cpu.v[1] = 2;
        cpu.execute(LdIDigitReg { reg: 1 });
        assert_eq!(10, cpu.i);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_mem_i_regs() {
        let (mut cpu, _) = create_cpu();
        cpu.i = 0x100;
        cpu.v[0] = 5;
        cpu.v[1] = 6;
        cpu.execute(LdMemIRegs { last_reg: 1 });
        assert_eq!(5, cpu.memory[0x100]);
        assert_eq!(6, cpu.memory[0x101]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_reg_byte() {
        let (mut cpu, _) = create_cpu();
        cpu.execute(LdRegByte { reg: 1, val: 0xFF });
        assert_eq!(0xFF, cpu.v[1]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_ld_regs_mem_i() {
        let (mut cpu, _) = create_cpu();
        cpu.i = 0x100;
        cpu.memory[0x100] = 1;
        cpu.memory[0x101] = 2;
        cpu.memory[0x102] = 3;
        cpu.execute(LdRegsMemI { last_reg: 2 });
        assert_eq!(1, cpu.v[0]);
        assert_eq!(2, cpu.v[1]);
        assert_eq!(3, cpu.v[2]);
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_jump() {
        let (mut cpu, _) = create_cpu();
        cpu.execute(Jump { addr: 0x2e8 });
        assert_eq!(0x2e8, cpu.pc);
    }

    #[test]
    fn execute_ret() {
        let (mut cpu, _) = create_cpu();
        cpu.sp = 1;
        cpu.stack[0] = 0x200;
        cpu.execute(Ret);
        assert_eq!(0x200, cpu.pc);
        assert_eq!(0, cpu.sp);
    }

    #[test]
    fn execute_skip_eq_reg_bytes() {
        let (mut cpu, _) = create_cpu();
        cpu.v[4] = 16;
        cpu.execute(SkipEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x204, cpu.pc);

        let (mut cpu, _) = create_cpu();
        cpu.v[4] = 14;
        cpu.execute(SkipEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x202, cpu.pc);
    }

    #[test]
    fn execute_skip_neq_reg_bytes() {
        let (mut cpu, _) = create_cpu();
        cpu.v[4] = 16;
        cpu.execute(SkipNEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x202, cpu.pc);

        let (mut cpu, _) = create_cpu();
        cpu.v[4] = 14;
        cpu.execute(SkipNEqRegBytes { reg: 4, val: 16 });
        assert_eq!(0x204, cpu.pc);
    }

    fn create_cpu() -> (CPU, Receiver<[u8; 8 * 32]>) {
        let (sender, receiver) = channel();
        (CPU::new(sender), receiver)
    }
}
