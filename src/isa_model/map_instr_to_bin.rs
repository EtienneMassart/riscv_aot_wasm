use super::{
    RiscvInstr, GPRegister, FPRegister,
    machine_instructions::{
        Const12, Const20, CSpWordOffset,
        CWordOffset, CConst6,
    }};

pub(crate) enum InstructionFormat {
    Standard(u32),
    Compressed(u16),
}

fn format_r_to_binary(rd: &GPRegister, rs1: &GPRegister, rs2: &GPRegister, opcode: u8, funct3: u8, funct7: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | (opcode as u32))
}

fn format_i_to_binary(rd: &GPRegister, rs1: &GPRegister, imm: &Const12, opcode: u8, funct3: u8) -> InstructionFormat {
    InstructionFormat::Standard((imm.as_u32()) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | (opcode as u32))
}

fn format_s_to_binary(rs1: &GPRegister, rs2: &GPRegister, imm: &Const12, opcode: u8, funct3: u8) -> InstructionFormat {
    let imm11_5 = (imm.as_u32()) >> 5;
    let imm4_0 = (imm.as_u32()) & 0x1F;
    InstructionFormat::Standard(imm11_5 << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | imm4_0 << 7 | (opcode as u32))
}

fn format_b_to_binary(rs1: &GPRegister, rs2: &GPRegister, imm: &Const12, opcode: u8, funct3: u8) -> InstructionFormat {
    let imm12 = (imm.as_u32()) >> 12;
    let imm10_5 = ((imm.as_u32()) >> 5) & 0x3F;
    let imm4_1 = ((imm.as_u32()) >> 1) & 0xF;
    let imm11 = (imm.as_u32()) >> 11 & 0x1;
    InstructionFormat::Standard(imm12 << 31 | imm11 << 7 | imm10_5 << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | imm4_1 << 8 | (opcode as u32))
}

fn format_u_to_binary(rd: &GPRegister, imm: &Const20, opcode: u8) -> InstructionFormat {
    InstructionFormat::Standard(imm.as_u32() << 12 | (rd.0 as u32) << 7 | (opcode as u32))
}

fn format_j_to_binary(rd: &GPRegister, imm: &Const20, opcode: u8) -> InstructionFormat {
    let imm20 = imm.as_u32() >> 20;
    let imm10_1 = (imm.as_u32() >> 1) & 0x3FF;
    let imm11 = (imm.as_u32() >> 11) & 0x1;
    let imm19_12 = (imm.as_u32() >> 12) & 0xFF;
    InstructionFormat::Standard(imm20 << 31 | imm19_12 << 12 | imm11 << 20 | imm10_1 << 21 | (rd.0 as u32) << 7 | (opcode as u32))
}

/// rd can also be rs1 for some compressed instructions, we call it rd for simplicity but it's not always a destination
fn format_cr_to_binary(rd: &GPRegister, rs2: &GPRegister, funct4: u8, opcode: u8) -> InstructionFormat {
    InstructionFormat::Compressed((funct4 as u16) << 12 | (rd.0 as u16) << 7 | (rs2.0 as u16) << 2 | (opcode as u16))
}

fn format_ci_spw_to_binary(rd: &GPRegister, imm: &CSpWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    // C.LWSP immediate mapping with u = offset>>2 (u[5:0] == imm[7:2]):
    // inst[12]=imm[5], inst[6:4]=imm[4:2], inst[3:2]=imm[7:6].
    let u = imm.as_u16() & 0x3F;
    let inst_12 = (u >> 3) & 0x1;
    let inst_6_4 = u & 0x7;
    let inst_3_2 = (u >> 4) & 0x3;
    InstructionFormat::Compressed(
        (funct3 as u16) << 13
            | inst_12 << 12
            | (rd.0 as u16) << 7
            | inst_6_4 << 4
            | inst_3_2 << 2
            | (opcode as u16),
    )
}

fn format_ci_fpw_to_binary(rd: &FPRegister, imm: &CSpWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    // C.FLWSP immediate mapping matches C.LWSP.
    let u = imm.as_u16() & 0x3F;
    let inst_12 = (u >> 3) & 0x1;
    let inst_6_4 = u & 0x7;
    let inst_3_2 = (u >> 4) & 0x3;
    InstructionFormat::Compressed(
        (funct3 as u16) << 13
            | inst_12 << 12
            | (rd.0 as u16) << 7
            | inst_6_4 << 4
            | inst_3_2 << 2
            | (opcode as u16),
    )
}

fn format_css_spw_to_binary(rs2: &GPRegister, imm: &CSpWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    // C.SWSP immediate mapping with u = offset>>2 (u[5:0] == imm[7:2]):
    // inst[12:9]=imm[5:2], inst[8:7]=imm[7:6], inst[6:2]=rs2.
    let u = imm.as_u16() & 0x3F;
    let inst_12_9 = u & 0xF;
    let inst_8_7 = (u >> 4) & 0x3;
    InstructionFormat::Compressed(
        (funct3 as u16) << 13
            | inst_12_9 << 9
            | inst_8_7 << 7
            | (rs2.0 as u16) << 2
            | (opcode as u16),
    )
}

fn format_css_fpw_to_binary(rs2: &FPRegister, imm: &CSpWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    // C.FSWSP immediate mapping matches C.SWSP.
    let u = imm.as_u16() & 0x3F;
    let inst_12_9 = u & 0xF;
    let inst_8_7 = (u >> 4) & 0x3;
    InstructionFormat::Compressed(
        (funct3 as u16) << 13
            | inst_12_9 << 9
            | inst_8_7 << 7
            | (rs2.0 as u16) << 2
            | (opcode as u16),
    )
}

fn format_fp_load_to_binary(rd: &FPRegister, rs1: &GPRegister, imm: &Const12, funct3: u8) -> InstructionFormat {
    InstructionFormat::Standard((imm.as_u32()) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | 0b0000111)
}

fn format_fp_store_to_binary(rs1: &GPRegister, rs2: &FPRegister, imm: &Const12, funct3: u8) -> InstructionFormat {
    let imm11_5 = (imm.as_u32()) >> 5;
    let imm4_0 = (imm.as_u32()) & 0x1F;
    InstructionFormat::Standard(imm11_5 << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | imm4_0 << 7 | 0b0100111)
}

fn format_fp_r_rm_to_binary(rd: &FPRegister, rs1: &FPRegister, rs2: &FPRegister, funct7: u8, rm: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (rm as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_fp_r_to_binary(rd: &FPRegister, rs1: &FPRegister, rs2: &FPRegister, funct7: u8, funct3: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_fp_sqrt_to_binary(rd: &FPRegister, rs1: &FPRegister, funct7: u8, rm: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs1.0 as u32) << 15 | (rm as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_fp_cmp_to_binary(rd: &GPRegister, rs1: &FPRegister, rs2: &FPRegister, funct7: u8, funct3: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2.0 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_fp_to_int_to_binary(rd: &GPRegister, rs1: &FPRegister, funct7: u8, rs2: u8, rm: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2 as u32) << 20 | (rs1.0 as u32) << 15 | (rm as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_int_to_fp_to_binary(rd: &FPRegister, rs1: &GPRegister, funct7: u8, rs2: u8, rm: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2 as u32) << 20 | (rs1.0 as u32) << 15 | (rm as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_fp_class_or_move_to_int(rd: &GPRegister, rs1: &FPRegister, funct7: u8, rs2: u8, funct3: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn format_move_int_to_fp(rd: &FPRegister, rs1: &GPRegister, funct7: u8, rs2: u8, funct3: u8) -> InstructionFormat {
    InstructionFormat::Standard((funct7 as u32) << 25 | (rs2 as u32) << 20 | (rs1.0 as u32) << 15 | (funct3 as u32) << 12 | (rd.0 as u32) << 7 | 0b1010011)
}

fn compact_reg(reg: u8) -> u16 {
    (reg as u16) & 0x7
}

fn format_caddi16sp_to_binary(imm: u8, opcode: u8) -> InstructionFormat {
    // C.ADDI16SP with u = imm[9:4] in bits [5:0]:
    // inst[12]=u[5], inst[6]=u[0], inst[5]=u[2], inst[4:3]=u[4:3], inst[2]=u[1].
    let u = (imm as u16) & 0x3F;
    let inst_12 = (u >> 5) & 0x1;
    let inst_6 = u & 0x1;
    let inst_5 = (u >> 2) & 0x1;
    let inst_4_3 = (u >> 3) & 0x3;
    let inst_2 = (u >> 1) & 0x1;
    InstructionFormat::Compressed(
        0b011 << 13
            | inst_12 << 12
            | (2u16 << 7)
            | inst_6 << 6
            | inst_5 << 5
            | inst_4_3 << 3
            | inst_2 << 2
            | (opcode as u16),
    )
}

fn format_ci_to_binary(rd: &GPRegister, imm: &CConst6, funct3: u8, opcode: u8) -> InstructionFormat {
    let imm5 = (imm.as_u16() >> 5) & 0x1;
    let imm4_0 = imm.as_u16() & 0x1F;
    InstructionFormat::Compressed((funct3 as u16) << 13 | imm5 << 12 | (rd.0 as u16) << 7 | imm4_0 << 2 | (opcode as u16))
}

fn format_cl_to_binary(rd: &GPRegister, rs1: &GPRegister, imm: &CWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    let u = imm.as_u16();
    // C.LW CL-format immediate mapping:
    // inst[12:10] <- uimm[5:3], inst[6] <- uimm[2], inst[5] <- uimm[6]
    // Here `u` stores uimm[6:2] in bits [4:0].
    let inst_12_10 = (u >> 1) & 0x7;
    let inst_6 = u & 0x1;
    let inst_5 = (u >> 4) & 0x1;
    InstructionFormat::Compressed((funct3 as u16) << 13 | inst_12_10 << 10 | compact_reg(rs1.0) << 7 | inst_6 << 6 | inst_5 << 5 | compact_reg(rd.0) << 2 | (opcode as u16))
}

fn format_cl_fp_to_binary(rd: &FPRegister, rs1: &GPRegister, imm: &CWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    let u = imm.as_u16();
    let inst_12_10 = (u >> 1) & 0x7;
    let inst_6 = u & 0x1;
    let inst_5 = (u >> 4) & 0x1;
    InstructionFormat::Compressed((funct3 as u16) << 13 | inst_12_10 << 10 | compact_reg(rs1.0) << 7 | inst_6 << 6 | inst_5 << 5 | compact_reg(rd.0) << 2 | (opcode as u16))
}

fn format_cs_to_binary(rs2: &GPRegister, rs1: &GPRegister, imm: &CWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    let u = imm.as_u16();
    let inst_12_10 = (u >> 1) & 0x7;
    let inst_6 = u & 0x1;
    let inst_5 = (u >> 4) & 0x1;
    InstructionFormat::Compressed((funct3 as u16) << 13 | inst_12_10 << 10 | compact_reg(rs1.0) << 7 | inst_6 << 6 | inst_5 << 5 | compact_reg(rs2.0) << 2 | (opcode as u16))
}

fn format_cs_fp_to_binary(rs2: &FPRegister, rs1: &GPRegister, imm: &CWordOffset, funct3: u8, opcode: u8) -> InstructionFormat {
    let u = imm.as_u16();
    let inst_12_10 = (u >> 1) & 0x7;
    let inst_6 = u & 0x1;
    let inst_5 = (u >> 4) & 0x1;
    InstructionFormat::Compressed((funct3 as u16) << 13 | inst_12_10 << 10 | compact_reg(rs1.0) << 7 | inst_6 << 6 | inst_5 << 5 | compact_reg(rs2.0) << 2 | (opcode as u16))
}

fn format_cb_shift_imm_to_binary(rd: &GPRegister, imm: &CConst6, subop: u8, opcode: u8) -> InstructionFormat {
    let imm5 = (imm.as_u16() >> 5) & 0x1;
    let imm4_0 = imm.as_u16() & 0x1F;
    InstructionFormat::Compressed(0b100 << 13 | imm5 << 12 | (subop as u16) << 10 | compact_reg(rd.0) << 7 | imm4_0 << 2 | (opcode as u16))
}

fn format_ca_to_binary(rd: &GPRegister, rs2: &GPRegister, subop: u8, opcode: u8) -> InstructionFormat {
    InstructionFormat::Compressed(0b100 << 13 | 0b11 << 10 | compact_reg(rd.0) << 7 | (subop as u16) << 5 | compact_reg(rs2.0) << 2 | (opcode as u16))
}

fn format_cm_pushpop_to_binary(rlist: u8, spimm: u8, funct5: u8) -> InstructionFormat {
    // Zcmp layout (RV32/RV64):
    // [15:13]=0b101, [12:8]=funct5, [7:4]=rlist, [3:2]=spimm, [1:0]=0b10
    InstructionFormat::Compressed(
        (0b101u16 << 13)
            | (((funct5 as u16) & 0x1F) << 8)
            | (((rlist as u16) & 0x0F) << 4)
            | (((spimm as u16) & 0x03) << 2)
            | 0b10,
    )
}

impl RiscvInstr {
    pub(crate) fn map_to_binary(&self) -> InstructionFormat {
        match self {
            RiscvInstr::ADDI { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0010011, 0x0),
            RiscvInstr::XORI { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0010011, 0x4),
            RiscvInstr::SLTIU { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0010011, 0x3),
            RiscvInstr::SLLI { rd, rs1, imm } => format_i_to_binary(rd, rs1, &imm.to_const12(), 0b0010011, 0x1),
            RiscvInstr::SRAI { rd, rs1, imm } => {
                let imm_with_funct7 = Const12::new(imm.as_i16() | 0b010000000000);
                format_i_to_binary(rd, rs1, &imm_with_funct7, 0b0010011, 0x5)
            }
            RiscvInstr::LUI { rd, imm } => format_u_to_binary(rd, imm, 0b0110111),
            RiscvInstr::AUIPC { rd, imm } => format_u_to_binary(rd, imm, 0b0010111),
            RiscvInstr::ADD { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x0, 0x0),
            RiscvInstr::SUB { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x0, 0x20),
            RiscvInstr::AND { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x7, 0x0),
            RiscvInstr::OR { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x6, 0x0),
            RiscvInstr::XOR { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x4, 0x0),
            RiscvInstr::SLT { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x2, 0x0),
            RiscvInstr::SLTU { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x3, 0x0),
            RiscvInstr::SLL { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x1, 0x0),
            RiscvInstr::SRL { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x5, 0x0),
            RiscvInstr::SRA { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x5, 0x20),

            // RV32M multiply/divide
            RiscvInstr::MUL { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x0, 0x01),
            RiscvInstr::MULHU { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x3, 0x01),
            RiscvInstr::DIV { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x4, 0x01),
            RiscvInstr::DIVU { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x5, 0x01),
            RiscvInstr::REM { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x6, 0x01),
            RiscvInstr::REMU { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x7, 0x01),

            // RV32 Zbb bit manipulation
            RiscvInstr::CLZ { rd, rs1 } => format_i_to_binary(rd, rs1, &Const12::new(0b011000000000u16 as i16), 0b0010011, 0x1),
            RiscvInstr::CTZ { rd, rs1 } => format_i_to_binary(rd, rs1, &Const12::new(0b011000000001u16 as i16), 0b0010011, 0x1),
            RiscvInstr::CPOP { rd, rs1 } => format_i_to_binary(rd, rs1, &Const12::new(0b011000000010u16 as i16), 0b0010011, 0x1),
            RiscvInstr::ROL { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x1, 0x30),
            RiscvInstr::ROR { rd, rs1, rs2 } => format_r_to_binary(rd, rs1, rs2, 0b0110011, 0x5, 0x30),

            RiscvInstr::JAL { rd, imm } => format_j_to_binary(rd, imm, 0b1101111),
            RiscvInstr::JALR { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b1100111, 0x0),
            RiscvInstr::RET => format_i_to_binary(&GPRegister(0), &GPRegister(1), &Const12::new(0), 0b1100111, 0x0),
            RiscvInstr::BEQ { rs1, rs2, imm } => format_b_to_binary(rs1, rs2, imm, 0b1100011, 0x0),
            RiscvInstr::BNE { rs1, rs2, imm } => format_b_to_binary(rs1, rs2, imm, 0b1100011, 0x1),
            RiscvInstr::LB { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0000011, 0x0),
            RiscvInstr::LH { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0000011, 0x1),
            RiscvInstr::LW { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0000011, 0x2),
            RiscvInstr::LBU { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0000011, 0x4),
            RiscvInstr::LHU { rd, rs1, imm } => format_i_to_binary(rd, rs1, imm, 0b0000011, 0x5),
            RiscvInstr::SB { rs2, rs1, imm } => format_s_to_binary(rs1, rs2, imm, 0b0100011, 0x0),
            RiscvInstr::SH { rs2, rs1, imm } => format_s_to_binary(rs1, rs2, imm, 0b0100011, 0x1),
            RiscvInstr::SW { rs2, rs1, imm } => format_s_to_binary(rs1, rs2, imm, 0b0100011, 0x2),

            RiscvInstr::FLW { rd, rs1, imm } => format_fp_load_to_binary(rd, rs1, imm, 0b010),
            RiscvInstr::FLD { rd, rs1, imm } => format_fp_load_to_binary(rd, rs1, imm, 0b011),
            RiscvInstr::FSW { rs2, rs1, imm } => format_fp_store_to_binary(rs1, rs2, imm, 0b010),
            RiscvInstr::FSD { rs2, rs1, imm } => format_fp_store_to_binary(rs1, rs2, imm, 0b011),

            RiscvInstr::FADDS { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0000000, rm.bits()),
            RiscvInstr::FSUBS { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0000100, rm.bits()),
            RiscvInstr::FMULS { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0001000, rm.bits()),
            RiscvInstr::FDIVS { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0001100, rm.bits()),
            RiscvInstr::FSQRTS { rd, rs1, rm } => format_fp_sqrt_to_binary(rd, rs1, 0b0101100, rm.bits()),
            RiscvInstr::FSGNJS { rd, rs1, rs2 } => format_fp_r_to_binary(rd, rs1, rs2, 0b0010000, 0b000),
            RiscvInstr::FSGNJNS { rd, rs1, rs2 } => format_fp_r_to_binary(rd, rs1, rs2, 0b0010000, 0b001),
            RiscvInstr::FSGNJXS { rd, rs1, rs2 } => format_fp_r_to_binary(rd, rs1, rs2, 0b0010000, 0b010),
            RiscvInstr::FEQS { rd, rs1, rs2 } => format_fp_cmp_to_binary(rd, rs1, rs2, 0b1010000, 0b010),
            RiscvInstr::FLTS { rd, rs1, rs2 } => format_fp_cmp_to_binary(rd, rs1, rs2, 0b1010000, 0b001),
            RiscvInstr::FLES { rd, rs1, rs2 } => format_fp_cmp_to_binary(rd, rs1, rs2, 0b1010000, 0b000),
            RiscvInstr::FMVXW { rd, rs1 } => format_fp_class_or_move_to_int(rd, rs1, 0b1110000, 0b00000, 0b000),
            RiscvInstr::FMVWX { rd, rs1 } => format_move_int_to_fp(rd, rs1, 0b1111000, 0b00000, 0b000),

            RiscvInstr::FADDD { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0000001, rm.bits()),
            RiscvInstr::FSUBD { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0000101, rm.bits()),
            RiscvInstr::FMULD { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0001001, rm.bits()),
            RiscvInstr::FDIVD { rd, rs1, rs2, rm } => format_fp_r_rm_to_binary(rd, rs1, rs2, 0b0001101, rm.bits()),
            RiscvInstr::FSQRTD { rd, rs1, rm } => format_fp_sqrt_to_binary(rd, rs1, 0b0101101, rm.bits()),
            RiscvInstr::FSGNJD { rd, rs1, rs2 } => format_fp_r_to_binary(rd, rs1, rs2, 0b0010001, 0b000),
            RiscvInstr::FSGNJND { rd, rs1, rs2 } => format_fp_r_to_binary(rd, rs1, rs2, 0b0010001, 0b001),
            RiscvInstr::FSGNJXD { rd, rs1, rs2 } => format_fp_r_to_binary(rd, rs1, rs2, 0b0010001, 0b010),
            RiscvInstr::FEQD { rd, rs1, rs2 } => format_fp_cmp_to_binary(rd, rs1, rs2, 0b1010001, 0b010),
            RiscvInstr::FLTD { rd, rs1, rs2 } => format_fp_cmp_to_binary(rd, rs1, rs2, 0b1010001, 0b001),
            RiscvInstr::FLED { rd, rs1, rs2 } => format_fp_cmp_to_binary(rd, rs1, rs2, 0b1010001, 0b000),

            RiscvInstr::FCVTWS { rd, rs1, rm } => format_fp_to_int_to_binary(rd, rs1, 0b1100000, 0b00000, rm.bits()),
            RiscvInstr::FCVTWUS { rd, rs1, rm } => format_fp_to_int_to_binary(rd, rs1, 0b1100000, 0b00001, rm.bits()),
            RiscvInstr::FCVTSW { rd, rs1, rm } => format_int_to_fp_to_binary(rd, rs1, 0b1101000, 0b00000, rm.bits()),
            RiscvInstr::FCVTSWU { rd, rs1, rm } => format_int_to_fp_to_binary(rd, rs1, 0b1101000, 0b00001, rm.bits()),
            RiscvInstr::FCVTWD { rd, rs1, rm } => format_fp_to_int_to_binary(rd, rs1, 0b1100001, 0b00000, rm.bits()),
            RiscvInstr::FCVTWUD { rd, rs1, rm } => format_fp_to_int_to_binary(rd, rs1, 0b1100001, 0b00001, rm.bits()),
            RiscvInstr::FCVTDW { rd, rs1, rm } => format_int_to_fp_to_binary(rd, rs1, 0b1101001, 0b00000, rm.bits()),
            RiscvInstr::FCVTDWU { rd, rs1, rm } => format_int_to_fp_to_binary(rd, rs1, 0b1101001, 0b00001, rm.bits()),
            RiscvInstr::FCVTSD { rd, rs1, rm } => format_fp_r_rm_to_binary(rd, rs1, &FPRegister(1), 0b0100000, rm.bits()),
            RiscvInstr::FCVTDS { rd, rs1, rm } => format_fp_r_rm_to_binary(rd, rs1, &FPRegister(0), 0b0100001, rm.bits()),

            RiscvInstr::CLW { rd, rs1, imm } => format_cl_to_binary(rd, rs1, imm, 0b010, 0b00),
            RiscvInstr::CFLW { rd, rs1, imm } => format_cl_fp_to_binary(rd, rs1, imm, 0b011, 0b00),
            RiscvInstr::CSW { rs2, rs1, imm } => format_cs_to_binary(rs2, rs1, imm, 0b110, 0b00),
            RiscvInstr::CFSW { rs2, rs1, imm } => format_cs_fp_to_binary(rs2, rs1, imm, 0b111, 0b00),

            RiscvInstr::CNOP => format_ci_to_binary(&GPRegister(0), &CConst6::new(0), 0b000, 0b01),
            RiscvInstr::CADDI { rd, imm } => format_ci_to_binary(rd, imm, 0b000, 0b01),
            RiscvInstr::CLI { rd, imm } => format_ci_to_binary(rd, imm, 0b010, 0b01),
            RiscvInstr::CADDI16SP { imm } => format_caddi16sp_to_binary(imm.as_u8(), 0b01),
            RiscvInstr::CSRAI { rd, imm } => format_cb_shift_imm_to_binary(rd, imm, 0b01, 0b01),
            RiscvInstr::CSUB { rd, rs2 } => format_ca_to_binary(rd, rs2, 0b00, 0b01),
            RiscvInstr::CXOR { rd, rs2 } => format_ca_to_binary(rd, rs2, 0b01, 0b01),
            RiscvInstr::COR { rd, rs2 } => format_ca_to_binary(rd, rs2, 0b10, 0b01),
            RiscvInstr::CAND { rd, rs2 } => format_ca_to_binary(rd, rs2, 0b11, 0b01),

            RiscvInstr::CSLLI { rd, imm } => format_ci_to_binary(rd, imm, 0b000, 0b10),
            
            RiscvInstr::CLWSP { rd, imm } => format_ci_spw_to_binary(rd, imm, 0b010, 0b10),
            RiscvInstr::CFLWSP { rd, imm } => format_ci_fpw_to_binary(rd, imm, 0b011, 0b10),
            RiscvInstr::CMV { rd, rs2 } => format_cr_to_binary(rd, rs2, 0b1000, 0b10),
            RiscvInstr::CEBREAK => format_cr_to_binary(&GPRegister(0), &GPRegister(0), 0b1001, 0b10),
            RiscvInstr::CJALR { rs1 } => format_cr_to_binary(rs1, &GPRegister(0), 0b1001, 0b10),
            RiscvInstr::CADD { rd, rs2 } => format_cr_to_binary(rd, rs2, 0b1001, 0b10),
            RiscvInstr::CMPUSH { rlist, spimm } => format_cm_pushpop_to_binary(*rlist, *spimm, 0x18),
            RiscvInstr::CMPOP { rlist, spimm } => format_cm_pushpop_to_binary(*rlist, *spimm, 0x1A),
            RiscvInstr::CSWSP { rs2, imm } => format_css_spw_to_binary(rs2, imm, 0b110, 0b10),
            RiscvInstr::CFSWSP { rs2, imm } => format_css_fpw_to_binary(rs2, imm, 0b111, 0b10),
        }
    }
}