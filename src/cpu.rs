use std::fs::File;
use std::io::prelude::*;
use std::io::Result;

use self::OpCode::*;

#[derive(Clone, Copy, Debug)]
enum OpCode {
    AddIReg {
        reg: u8,
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

    // 64 (i.e. 8 bytes) by 32 bit graphics buffer
    graphics: [u8; 8 * 32],
}

impl CPU {
    pub fn new() -> CPU {
        // TODO: store the digit sprites in 0x000 -> 0x1FF of memory
        CPU {
            v: [0; 16],
            i: 0,
            sound_timer: 0,
            delay_timer: 0,
            // Most chip8 programs start at 0x200
            pc: 0x200,
            stack: [0; 16],
            sp: 0,
            memory: [0; 4096],
            key: [false; 16],
            graphics: [0; 8 * 32],
        }
    }

    pub fn load_game_data(&mut self, file_name: &str) -> Result<()> {
        let mut game_file = File::open(file_name)?;
        game_file.read(&mut self.memory[0x200..])?;
        Ok(())
    }

    pub fn run(&mut self) {
        loop {
            let instr = self.decode();
            self.execute(instr);
        }
    }

    fn decode(&self) -> OpCode {
        let pc = self.pc as usize;
        match &self.memory[pc..pc + 2] {
            [0x00, 0xe0] => Clear,
            // Must come after all the other 0NNN instructions
            [0x00...0x0F, _] => Sys,
            [msb @ 0x10...0x1F, lsb] => Jump {
                addr: extract_addr(msb, lsb),
            },
            [msb @ 0x20...0x2F, lsb] => Call {
                addr: extract_addr(msb, lsb),
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
            [msb @ 0xA0...0xAF, lsb] => LdIAddr {
                addr: extract_addr(msb, lsb),
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

    fn execute(&mut self, op: OpCode) {
        let mut new_pc = self.pc + 2;
        match op {
            AddIReg { reg } => {
                let reg_val = self.get_reg_val(reg);
                println!("Adding {:x} to I's current value {:x}", reg_val, self.i);
                self.i += reg_val as u16;
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
                self.graphics = [0; 8 * 32];
            }
            Draw {
                reg_x,
                reg_y,
                sprite_bytes,
            } => {
                let i = self.i;
                let x = self.get_reg_val(reg_x);
                let y = self.get_reg_val(reg_y);
                println!(
                    "Drawing {} bytes of sprite from address {:x} at location {},{} on the screen",
                    sprite_bytes, i, x, y
                );
                // TODO: implement graphical changes
            }
            LdIAddr { addr } => {
                println!("Loading reg I with address {:x}", addr);
                self.i = addr;
            }
            LdIDigitReg { reg } => {
                let sprite_digit = self.get_reg_val(reg);
                let addr = 5 * sprite_digit as u16;
                println!(
                    "Loading I with address {:x}, where sprite digit {:x} is stored",
                    addr, sprite_digit
                );
                self.i = addr;
            }
            LdMemIRegs { last_reg } => {
                println!(
                    "Copying regs 0 through {} into memory address {:x}",
                    last_reg, self.pc
                );
                for i in 0..=last_reg {
                    self.memory[(self.i + i as u16) as usize] = self.get_reg_val(i);
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
            SkipEqRegBytes { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val == val {
                    println!("Skipping next instr because {} == {}", reg_val, val);
                    new_pc += 2;
                }
            }
            SkipNEqRegBytes { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val != val {
                    println!("Skipping next instr because {} != {}", reg_val, val);
                    new_pc += 2;
                }
            }
            Sys => println!("SYS instruction found, ignoring"),
        }

        self.pc = new_pc;
    }

    fn get_reg_val(&mut self, reg: u8) -> u8 {
        return self.v[reg as usize];
    }
}

// Extracts the least significant 12 bits out of the two bytes representing a
// memory address (possibly part of an instruction).
fn extract_addr(msb: &u8, lsb: &u8) -> u16 {
    *lsb as u16 | ((*msb as u16 & 0x0F) << 8)
}
