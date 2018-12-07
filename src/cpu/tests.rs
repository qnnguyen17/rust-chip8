use crate::cpu::OpCode::*;
use crate::cpu::*;
use crate::FRAME_BUFFER_BYTES;
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
fn decode_shift_right_reg() {
    assert_eq!(ShiftRightReg { reg: 0 }, decode_instruction(&[0x80, 0x66]));
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
fn decode_skip_reg_key_pressed() {
    assert_eq!(
        SkipRegKeyPressed { reg: 9 },
        decode_instruction(&[0xE9, 0x9E])
    );
}

#[test]
fn decode_skip_reg_key_npressed() {
    assert_eq!(
        SkipRegKeyNPressed { reg: 0xC },
        decode_instruction(&[0xEC, 0xA1])
    );
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
fn execute_shift_right_reg_set_vf() {
    let mut cpu = create_cpu();
    cpu.v[0] = 0b1111;
    cpu.execute(ShiftRightReg { reg: 0 });
    assert_eq!(0b111, cpu.v[0]);
    assert_eq!(1, cpu.v[0xF]);
    assert_eq!(0x202, cpu.pc);
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
fn execute_skip_reg_key_pressed() {
    let mut cpu = create_cpu();
    cpu.v[4] = 7;
    cpu.key_state[7] = true;
    cpu.execute(SkipRegKeyPressed { reg: 4 });
    assert_eq!(0x204, cpu.pc);

    let mut cpu = create_cpu();
    cpu.v[5] = 6;
    cpu.key_state[6] = false;
    cpu.execute(SkipRegKeyPressed { reg: 5 });
    assert_eq!(0x202, cpu.pc);
}

#[test]
fn execute_skip_reg_key_npressed() {
    let mut cpu = create_cpu();
    cpu.v[4] = 7;
    cpu.key_state[7] = false;
    cpu.execute(SkipRegKeyNPressed { reg: 4 });
    assert_eq!(0x204, cpu.pc);

    let mut cpu = create_cpu();
    cpu.v[5] = 6;
    cpu.key_state[6] = true;
    cpu.execute(SkipRegKeyNPressed { reg: 5 });
    assert_eq!(0x202, cpu.pc);
}

#[cfg(test)]
fn create_cpu() -> CPU {
    let frame_buffer = Arc::new(RwLock::new([0; FRAME_BUFFER_BYTES]));
    CPU::new(frame_buffer, channel().1, channel().1)
}
