use std::fs::File;
use std::io::prelude::*;
use std::io::Result;

use self::OpCode::*;

#[derive(Clone, Copy, Debug)]
enum OpCode {
    ADD_I_REG {
        reg: u8,
    },
    CALL {
        addr: u16,
    },
    CLS,
    DRW {
        reg_x: u8,
        reg_y: u8,
        sprite_bytes: u8,
    },
    LD_I_ADDR {
        addr: u16,
    },
    LD_I_DIGIT_REG {
        reg: u8,
    },
    LD_MEMI_REGS {
        last_reg: u8,
    },
    LD_REG_BYTE {
        reg: u8,
        val: u8,
    },
    LD_REGS_MEMI {
        last_reg: u8,
    },
    JP {
        addr: u16,
    },
    SE_REG_BYTE {
        reg: u8,
        val: u8,
    },
    SNE_REG_BYTE {
        reg: u8,
        val: u8,
    },
    SYS,
}

pub struct Chip8 {
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

impl Chip8 {
    pub fn new() -> Chip8 {
        // TODO: store the digit sprites in 0x000 -> 0x1FF of memory
        Chip8 {
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
            [0x00, 0xe0] => CLS,
            // Must come after all the other 0NNN instructions
            [0x00...0x0F, _] => SYS,
            [msb @ 0x10...0x1F, lsb] => JP {
                addr: extract_addr(msb, lsb),
            },
            [msb @ 0x20...0x2F, lsb] => CALL {
                addr: extract_addr(msb, lsb),
            },
            [msb @ 0x30...0x3F, lsb] => SE_REG_BYTE {
                reg: msb & 0xF,
                val: *lsb,
            },
            [msb @ 0x40...0x4F, lsb] => SNE_REG_BYTE {
                reg: msb & 0xF,
                val: *lsb,
            },
            [msb @ 0x60...0x6F, lsb] => LD_REG_BYTE {
                reg: msb & 0xF,
                val: *lsb,
            },
            [msb @ 0xA0...0xAF, lsb] => LD_I_ADDR {
                addr: extract_addr(msb, lsb),
            },
            [msb @ 0xD0...0xDF, lsb] => DRW {
                reg_x: msb & 0xF,
                reg_y: (lsb & 0xF0) >> 4,
                sprite_bytes: lsb & 0xF,
            },
            [msb @ 0xF0...0xFF, 0x1E] => ADD_I_REG { reg: msb & 0xF },
            [msb @ 0xF0...0xFF, 0x29] => LD_I_DIGIT_REG { reg: msb & 0xF },
            [msb @ 0xF0...0xFF, 0x55] => LD_MEMI_REGS {
                last_reg: msb & 0x0F,
            },
            [msb @ 0xF0...0xFF, 0x65] => LD_REGS_MEMI {
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
            ADD_I_REG { reg } => {
                let reg_val = self.get_reg_val(reg);
                println!("Adding {:x} to I's current value {:x}", reg_val, self.i);
                self.i += reg_val as u16;
            }
            CALL { addr } => {
                println!(
                    "Storing current PC {:x} on the stack and jumping to {:x}",
                    self.pc, addr
                );
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                new_pc = addr;
            }
            CLS => {
                println!("Clearing screen");
                self.graphics = [0; 8 * 32];
            }
            DRW {
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
            LD_I_ADDR { addr } => {
                println!("Loading reg I with address {:x}", addr);
                self.i = addr;
            }
            LD_I_DIGIT_REG { reg } => {
                let sprite_digit = self.get_reg_val(reg);
                let addr = 5 * sprite_digit as u16;
                println!(
                    "Loading I with address {:x}, where sprite digit {:x} is stored",
                    addr, sprite_digit
                );
                self.i = addr;
            }
            LD_MEMI_REGS { last_reg } => {
                println!(
                    "Copying regs 0 through {} into memory address {:x}",
                    last_reg, self.pc
                );
                for i in 0..=last_reg {
                    self.memory[(self.i + i as u16) as usize] = self.get_reg_val(i);
                }
            }
            LD_REG_BYTE { reg, val } => {
                println!("Loading reg V{} with value {:x}", reg, val);
                self.v[reg as usize] = val;
            }
            LD_REGS_MEMI { last_reg } => {
                println!(
                    "Loading regs 0 through {} with data in memory starting at address {:x}",
                    last_reg, self.i
                );
                for i in 0..=last_reg as usize {
                    self.v[i] = self.memory[self.i as usize + i]
                }
            }
            JP { addr } => {
                println!("Jumping to address {:x} instead of {:x}", addr, new_pc);
                new_pc = addr;
            }
            SE_REG_BYTE { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val == val {
                    println!("Skipping next instr because {} == {}", reg_val, val);
                    new_pc += 2;
                }
            }
            SNE_REG_BYTE { reg, val } => {
                let reg_val = self.get_reg_val(reg);
                if reg_val != val {
                    println!("Skipping next instr because {} != {}", reg_val, val);
                    new_pc += 2;
                }
            }
            SYS => {}
            // SYS => println!("SYS instruction found, ignoring"),
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
