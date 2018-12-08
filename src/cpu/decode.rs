use super::OpCode;
use super::OpCode::*;
use piston_window::*;

pub(in crate::cpu) fn decode_instruction(code: &[u8]) -> OpCode {
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
            0x1 => OrRegs {
                reg_x: extract_lower_nibble(*msb),
                reg_y: extract_upper_nibble(*lsb),
            },
            0x2 => AndRegs {
                reg_x: extract_lower_nibble(*msb),
                reg_y: extract_upper_nibble(*lsb),
            },
            0x3 => XorRegs {
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
            0xE => ShiftLeftReg {
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
        [msb @ 0xF0...0xFF, 0x07] => LdRegDt {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x0A] => LdRegKey {
            reg: extract_lower_nibble(*msb),
        },
        [msb @ 0xF0...0xFF, 0x15] => LdDtReg {
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

/// Returns keycode 0 -> F of the button if there is one
/// Since the actual keypad of the CHIP8 spec doesn't exist in a normal
/// keyboard, we map it as follows: 1234            123C
/// QWER     =>     456D
/// ASDF            789E
/// ZXCV            A0BF
pub(in crate::cpu) fn decode_key(button: Button) -> Option<usize> {
    match button {
        Button::Keyboard(Key::D1) => Some(1),
        Button::Keyboard(Key::D2) => Some(2),
        Button::Keyboard(Key::D3) => Some(3),
        Button::Keyboard(Key::D4) => Some(0xC),
        Button::Keyboard(Key::Q) => Some(4),
        Button::Keyboard(Key::W) => Some(5),
        Button::Keyboard(Key::E) => Some(6),
        Button::Keyboard(Key::R) => Some(0xD),
        Button::Keyboard(Key::A) => Some(7),
        Button::Keyboard(Key::S) => Some(8),
        Button::Keyboard(Key::D) => Some(9),
        Button::Keyboard(Key::F) => Some(0xE),
        Button::Keyboard(Key::Z) => Some(0xA),
        Button::Keyboard(Key::C) => Some(0xB),
        Button::Keyboard(Key::X) => Some(0),
        Button::Keyboard(Key::V) => Some(0xF),
        _ => None,
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
