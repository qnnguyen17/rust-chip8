use std::fs::File;
use std::io::prelude::*;
use std::io::Result;

use self::OpCode::*;

#[derive(Clone, Copy, Debug)]
enum OpCode {
    CLS,
    LD_I_ADDR { val: u16 },
    LD_MEMI_REGS { last_reg: u8 },
    LD_VX_BYTE { reg: u8, val: u8 },
    SNE { reg: u8, val: u8 },
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

    // 64*32 graphics buffer
    graphics: [bool; 64 * 32],
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
            graphics: [false; 64 * 32],
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
            [msb @ 0x40...0x4F, lsb] => SNE {
                reg: *msb & 0x0f,
                val: *lsb,
            },
            [msb @ 0x60...0x6F, lsb] => LD_VX_BYTE {
                reg: *msb & 0x0f,
                val: *lsb,
            },
            [msb @ 0xA0...0xAF, lsb] => LD_I_ADDR {
                val: *lsb as u16 | ((*msb as u16 & 0x0F) << 8),
            },
            [msb @ 0xF0...0xFF, 0x55] => LD_MEMI_REGS {
                last_reg: *msb & 0x0F,
            },
            [msb, lsb] => panic!("Unknown op code {}", *lsb as usize | ((*msb as usize) << 8)),
            _ => panic!("Invalid program counter"),
        }
    }

    fn execute(&mut self, op: OpCode) {
        let mut new_pc = self.pc + 2;
        match op {
            CLS => {
                println!("Clearing screen");
                self.graphics = [false; 64 * 32];
            }
            LD_I_ADDR { val } => {
                let address = val & 0xFFF;
                println!("Loading reg I with address {:x}", address);
                self.i = address;
            }
            LD_MEMI_REGS { last_reg } => {
                println!(
                    "Copying regs 0 through {} into memory address {:x}",
                    last_reg, self.pc
                );
                for i in 0..=(last_reg as usize) {
                    self.memory[self.pc as usize] = self.v[i];
                }
            }
            LD_VX_BYTE { reg, val } => {
                println!("Loading reg V{} with value {:x}", reg, val);
                self.v[reg as usize] = val;
            }
            SNE { reg, val } => {
                if self.v[reg as usize] != val {
                    println!(
                        "Skipping next instr because {} != {}",
                        self.v[reg as usize], val
                    );
                    new_pc += 2;
                }
            }
            SYS => println!("SYS instruction found, ignoring"),
        }

        self.pc = new_pc;
    }
}
