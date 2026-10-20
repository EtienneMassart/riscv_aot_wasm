use crate::isa_model::{FPRegister, GPRegister};
use defmt::Format;

pub(crate) enum RiscvInstr {

    // ********************* RV32I BASE INSTRUCTIONS *********************

    // Immediate instructions
    ADDI { rd: GPRegister, rs1: GPRegister, imm: Const12 },
    XORI { rd: GPRegister, rs1: GPRegister, imm: Const12 },
    SLTIU { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Set Less Than Immediate Unsigned
    SLLI { rd: GPRegister, rs1: GPRegister, imm: ShAmt5 }, // Shift Left Logical Immediate
    SRAI { rd: GPRegister, rs1: GPRegister, imm: ShAmt5 }, // Shift Right Arithmetic Immediate
    LUI { rd: GPRegister, imm: Const20 }, // Load Upper Immediate, sets upper 20 bits of rd to imm and lower 12 bits to 0
    AUIPC { rd: GPRegister, imm: Const20 }, // Add Upper Immediate to PC, sets rd to current PC + (imm << 12). Used with JALR for lower 12 bits of jump address

    // Register-register instructions
    ADD { rd: GPRegister, rs1: GPRegister, rs2: GPRegister },
    SUB { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rs1 - rs2
    AND { rd: GPRegister, rs1: GPRegister, rs2: GPRegister },
    OR { rd: GPRegister, rs1: GPRegister, rs2: GPRegister },
    XOR { rd: GPRegister, rs1: GPRegister, rs2: GPRegister },
    SLT { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // Set Less Than, set rd to 1 if rs1 < rs2 (signed), else 0
    SLTU { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // Set Less Than Unsigned
    SLL { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // Shift Left Logical, shift rs1 left by the amount in the lower 5 bits of rs2 and store in rd
    SRL { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // Shift Right Logical, shift rs1 right logically by the amount in the lower 5 bits of rs2 and store in rd
    SRA { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // Shift Right Arithmetic, shift rs1 right arithmetically by the amount in the lower 5 bits of rs2 and store in rd

    // Control flow instructions

    // Unconditional jump instructions
    JAL { rd: GPRegister, imm: Const20 }, // Jump and Link, jump to PC + imm and store rd = PC + 4
    JALR { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Jump and Link Register, jump to (rs1 + imm) & ~1 and rd = PC + 4.
    RET, // encoded as JALR x0, x1, 0

    // Conditional branch instructions
    BEQ { rs1: GPRegister, rs2: GPRegister, imm: Const12 }, // Branch if Equal, if rs1 == rs2 then PC += imm
    BNE { rs1: GPRegister, rs2: GPRegister, imm: Const12 }, // Branch if Not Equal, if rs1 != rs2 then PC += imm


    // Load and store instructions
    // All load and store operations have to be aligned to their size (e.g., LW must be 4-byte aligned), otherwise they will cause an exception.
    LB { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Load Byte, rd = sign-extended byte at address rs1 + imm
    LH { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Load Halfword, rd = sign-extended halfword at address rs1 + imm
    LW { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Load Word, rd = word at address rs1 + imm
    LBU { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Load Byte Unsigned, rd = zero-extended byte at address rs1 + imm
    LHU { rd: GPRegister, rs1: GPRegister, imm: Const12 }, // Load Halfword Unsigned, rd = zero-extended halfword at address rs1 + imm

    SB { rs2: GPRegister, rs1: GPRegister, imm: Const12 }, // Store Byte, store least significant byte of rs2 at address rs1 + imm
    SH { rs2: GPRegister, rs1: GPRegister, imm: Const12 }, // Store Halfword, store least significant halfword of rs2 at address rs1 + imm
    SW { rs2: GPRegister, rs1: GPRegister, imm: Const12 }, // Store Word, store word in rs2 at address rs1 + imm

    // ********************* RV32M MULTIPLY/DIVIDE INSTRUCTIONS *********************
    MUL { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = (rs1 * rs2)[31:0], lower 32 bits of multiply
    MULHU { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = (rs1 * rs2)[63:32], upper 32 bits of unsigned*unsigned multiply
    DIV { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rs1 / rs2 (signed division)
    DIVU { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rs1 / rs2 (unsigned division)
    REM { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rs1 % rs2 (signed remainder)
    REMU { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rs1 % rs2 (unsigned remainder)

    // ********************* RV32 Zbb BIT MANIPULATION INSTRUCTIONS *********************
    CLZ { rd: GPRegister, rs1: GPRegister }, // rd = count leading zeros in rs1
    CTZ { rd: GPRegister, rs1: GPRegister }, // rd = count trailing zeros in rs1
    CPOP { rd: GPRegister, rs1: GPRegister }, // rd = population count (number of set bits) in rs1
    ROL { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rotate rs1 left by rs2[4:0]
    ROR { rd: GPRegister, rs1: GPRegister, rs2: GPRegister }, // rd = rotate rs1 right by rs2[4:0]

    // ********************* Floating point instructions *********************
    // Loads and stores
    FLW { rd: FPRegister, rs1: GPRegister, imm: Const12 }, // Floating load word, rd = f32 at address rs1 + imm
    FLD { rd: FPRegister, rs1: GPRegister, imm: Const12 }, // Floating load double, rd = f64 at address rs1 + imm
    FSW { rs2: FPRegister, rs1: GPRegister, imm: Const12 }, // Floating store word, store f32 rs2 at address rs1 + imm
    FSD { rs2: FPRegister, rs1: GPRegister, imm: Const12 }, // Floating store double, store f64 rs2 at address rs1 + imm

    // Single precision arithmetic and compare
    FADDS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 + rs2 (f32)
    FSUBS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 - rs2 (f32)
    FMULS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 * rs2 (f32)
    FDIVS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 / rs2 (f32)
    FSQRTS { rd: FPRegister, rs1: FPRegister, rm: RoundingMode }, // rd = sqrt(rs1) (f32)
    FSGNJS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister }, // rd magnitude from rs1, sign from rs2 (f32), used for f32 move with rs2 = rs1
    FSGNJNS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister }, // rd magnitude from rs1, inverted sign from rs2 (f32)
    FSGNJXS { rd: FPRegister, rs1: FPRegister, rs2: FPRegister }, // rd magnitude from rs1, sign = rs1.sign xor rs2.sign (f32)
    FEQS { rd: GPRegister, rs1: FPRegister, rs2: FPRegister }, // rd = 1 if rs1 == rs2 (f32), else 0
    FLTS { rd: GPRegister, rs1: FPRegister, rs2: FPRegister }, // rd = 1 if rs1 < rs2 (f32), else 0
    FLES { rd: GPRegister, rs1: FPRegister, rs2: FPRegister }, // rd = 1 if rs1 <= rs2 (f32), else 0
    FMVXW { rd: GPRegister, rs1: FPRegister }, // Move raw f32 bits from fp rs1 to integer rd
    FMVWX { rd: FPRegister, rs1: GPRegister }, // Move raw 32-bit integer bits from rs1 to fp rd

    // Double precision arithmetic and compare
    FADDD { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 + rs2 (f64)
    FSUBD { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 - rs2 (f64)
    FMULD { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 * rs2 (f64)
    FDIVD { rd: FPRegister, rs1: FPRegister, rs2: FPRegister, rm: RoundingMode }, // rd = rs1 / rs2 (f64)
    FSQRTD { rd: FPRegister, rs1: FPRegister, rm: RoundingMode }, // rd = sqrt(rs1) (f64)
    FSGNJD { rd: FPRegister, rs1: FPRegister, rs2: FPRegister }, // rd magnitude from rs1, sign from rs2 (f64), used for f64 move with rs2 = rs1
    FSGNJND { rd: FPRegister, rs1: FPRegister, rs2: FPRegister }, // rd magnitude from rs1, inverted sign from rs2 (f64)
    FSGNJXD { rd: FPRegister, rs1: FPRegister, rs2: FPRegister }, // rd magnitude from rs1, sign = rs1.sign xor rs2.sign (f64)
    FEQD { rd: GPRegister, rs1: FPRegister, rs2: FPRegister }, // rd = 1 if rs1 == rs2 (f64), else 0
    FLTD { rd: GPRegister, rs1: FPRegister, rs2: FPRegister }, // rd = 1 if rs1 < rs2 (f64), else 0
    FLED { rd: GPRegister, rs1: FPRegister, rs2: FPRegister }, // rd = 1 if rs1 <= rs2 (f64), else 0

    // Conversions between integer/single/double
    FCVTWS { rd: GPRegister, rs1: FPRegister, rm: RoundingMode }, // Convert f32 rs1 to signed i32 rd
    FCVTWUS { rd: GPRegister, rs1: FPRegister, rm: RoundingMode }, // Convert f32 rs1 to unsigned i32 rd
    FCVTSW { rd: FPRegister, rs1: GPRegister, rm: RoundingMode }, // Convert signed i32 rs1 to f32 rd
    FCVTSWU { rd: FPRegister, rs1: GPRegister, rm: RoundingMode }, // Convert unsigned i32 rs1 to f32 rd
    FCVTWD { rd: GPRegister, rs1: FPRegister, rm: RoundingMode }, // Convert f64 rs1 to signed i32 rd
    FCVTWUD { rd: GPRegister, rs1: FPRegister, rm: RoundingMode }, // Convert f64 rs1 to unsigned i32 rd
    FCVTDW { rd: FPRegister, rs1: GPRegister, rm: RoundingMode }, // Convert signed i32 rs1 to f64 rd
    FCVTDWU { rd: FPRegister, rs1: GPRegister, rm: RoundingMode }, // Convert unsigned i32 rs1 to f64 rd
    FCVTSD { rd: FPRegister, rs1: FPRegister, rm: RoundingMode }, // Convert f64 rs1 to f32 rd
    FCVTDS { rd: FPRegister, rs1: FPRegister, rm: RoundingMode }, // Convert f32 rs1 to f64 rd

    // ********************* Compressed instructions *********************

    // Quadrant 0
    CLW { rd: GPRegister, rs1: GPRegister, imm: CWordOffset }, // Compressed LW using compact register subset
    CFLW { rd: FPRegister, rs1: GPRegister, imm: CWordOffset }, // Compressed FLW using compact register subset
    CSW { rs2: GPRegister, rs1: GPRegister, imm: CWordOffset }, // Compressed SW using compact register subset
    CFSW { rs2: FPRegister, rs1: GPRegister, imm: CWordOffset }, // Compressed FSW using compact register subset

    // Quadrant 1
    CNOP, // Compressed no-op
    CADDI { rd: GPRegister, imm: CConst6 }, // rd = rd + imm (compressed)
    CLI { rd: GPRegister, imm: CConst6 }, // Load small immediate into rd (compressed)
    CADDI16SP { imm: CAddi16SpImm }, // Adjust SP by a scaled immediate
    CSRAI { rd: GPRegister, imm: CConst6 }, // Arithmetic right shift immediate (compressed)
    CSUB { rd: GPRegister, rs2: GPRegister }, // rd = rd - rs2 (compressed)
    CXOR { rd: GPRegister, rs2: GPRegister }, // rd = rd xor rs2 (compressed)
    COR { rd: GPRegister, rs2: GPRegister }, // rd = rd or rs2 (compressed)
    CAND { rd: GPRegister, rs2: GPRegister }, // rd = rd and rs2 (compressed)

    // Quadrant 2
    CSLLI { rd: GPRegister, imm: CConst6 }, // Logical left shift immediate (compressed)

    // Load and store instructions with stack pointer as base, used for local variables and parameters on the stack. Immediate is shifted
    CLWSP { rd: GPRegister, imm: CSpWordOffset }, // Compressed Load Word from Stack Pointer, rd = word at address SP (x2) + 4*imm. rd = x0 triggers an exception
    CFLWSP { rd: FPRegister, imm: CSpWordOffset }, // Compressed FP Load Word from Stack Pointer, rd = word at address SP (x2) + 4*imm

    CMV { rd: GPRegister, rs2: GPRegister }, // Move rs2 to rd (compressed)
    CEBREAK, // Compressed breakpoint exception
    CJALR { rs1: GPRegister }, // Jump and link register (compressed), writes return address
    CADD { rd: GPRegister, rs2: GPRegister }, // rd = rd + rs2 (compressed)

    // Zcmp compressed prologue/epilogue instructions
    // rlist: encoded register list selector (4..15 valid in RV32I)
    // spimm: encoded stack adjustment extension (2 bits)
    CMPUSH { rlist: u8, spimm: u8 },
    CMPOP { rlist: u8, spimm: u8 },

    CSWSP { rs2: GPRegister, imm: CSpWordOffset }, // Compressed Store Word to Stack Pointer, store word in rs2 at address SP (x2) + 4*imm
    CFSWSP { rs2: FPRegister, imm: CSpWordOffset }, // Compressed FP Store Word to Stack Pointer, store word in rs2 at address SP (x2) + 4*imm
}

impl RiscvInstr {
    /// Instructions are either 2 bytes (16 bits) for compressed instructions or 4 bytes (32 bits) for standard instructions.
    /// The instruction memory is written by blocks of 2 bytes, so even 4-byte instructions are written in two steps. The size method returns the number of 2-byte blocks needed to store the instruction.
    pub(crate) fn size(&self) -> usize {
        match self {
            RiscvInstr::ADDI { .. } | RiscvInstr::XORI { .. } | RiscvInstr::SLTIU { .. }
            | RiscvInstr::SLLI { .. } | RiscvInstr::SRAI { .. } | RiscvInstr::LUI { .. } | RiscvInstr::AUIPC { .. }
            | RiscvInstr::ADD { .. } | RiscvInstr::SUB { .. } | RiscvInstr::AND { .. } | RiscvInstr::OR { .. } | RiscvInstr::XOR { .. }
            | RiscvInstr::SLT { .. } | RiscvInstr::SLTU { .. } | RiscvInstr::SLL { .. } | RiscvInstr::SRL { .. } | RiscvInstr::SRA { .. }
            | RiscvInstr::MUL { .. } | RiscvInstr::MULHU { .. }
            | RiscvInstr::DIV { .. } | RiscvInstr::DIVU { .. } | RiscvInstr::REM { .. } | RiscvInstr::REMU { .. }
            | RiscvInstr::CLZ { .. } | RiscvInstr::CTZ { .. } | RiscvInstr::CPOP { .. }
            | RiscvInstr::ROL { .. } | RiscvInstr::ROR { .. }
            | RiscvInstr::JAL { .. } | RiscvInstr::JALR { .. } | RiscvInstr::RET
            | RiscvInstr::BEQ { .. } | RiscvInstr::BNE { .. }
            | RiscvInstr::LB { .. } | RiscvInstr::LH { .. } | RiscvInstr::LW { .. } | RiscvInstr::LBU { .. } | RiscvInstr::LHU { .. }
            | RiscvInstr::SB { .. } | RiscvInstr::SH { .. } | RiscvInstr::SW { .. }
            | RiscvInstr::FLW { .. } | RiscvInstr::FLD { .. } | RiscvInstr::FSW { .. } | RiscvInstr::FSD { .. }
            | RiscvInstr::FADDS { .. } | RiscvInstr::FSUBS { .. } | RiscvInstr::FMULS { .. } | RiscvInstr::FDIVS { .. }
            | RiscvInstr::FSQRTS { .. } | RiscvInstr::FSGNJS { .. } | RiscvInstr::FSGNJNS { .. } | RiscvInstr::FSGNJXS { .. }
            | RiscvInstr::FEQS { .. } | RiscvInstr::FLTS { .. } | RiscvInstr::FLES { .. }
            | RiscvInstr::FMVXW { .. } | RiscvInstr::FMVWX { .. }
            | RiscvInstr::FADDD { .. } | RiscvInstr::FSUBD { .. } | RiscvInstr::FMULD { .. } | RiscvInstr::FDIVD { .. }
            | RiscvInstr::FSQRTD { .. } | RiscvInstr::FSGNJD { .. } | RiscvInstr::FSGNJND { .. } | RiscvInstr::FSGNJXD { .. }
            | RiscvInstr::FEQD { .. } | RiscvInstr::FLTD { .. } | RiscvInstr::FLED { .. }
            | RiscvInstr::FCVTWS { .. } | RiscvInstr::FCVTWUS { .. } | RiscvInstr::FCVTSW { .. } | RiscvInstr::FCVTSWU { .. }
            | RiscvInstr::FCVTWD { .. } | RiscvInstr::FCVTWUD { .. } | RiscvInstr::FCVTDW { .. } | RiscvInstr::FCVTDWU { .. }
            | RiscvInstr::FCVTSD { .. } | RiscvInstr::FCVTDS { .. }
             => 2,
            RiscvInstr::CLW { .. } | RiscvInstr::CFLW { .. }
            | RiscvInstr::CSW { .. } | RiscvInstr::CFSW { .. }
            | RiscvInstr::CNOP | RiscvInstr::CADDI { .. } | RiscvInstr::CLI { .. }
            | RiscvInstr::CADDI16SP { .. } | RiscvInstr::CSRAI { .. }
            | RiscvInstr::CSUB { .. } | RiscvInstr::CXOR { .. } | RiscvInstr::COR { .. }
            | RiscvInstr::CAND { .. }
            | RiscvInstr::CSLLI { .. }
            | RiscvInstr::CLWSP { .. } | RiscvInstr::CFLWSP { .. }
            | RiscvInstr::CMV { .. } | RiscvInstr::CEBREAK | RiscvInstr::CJALR { .. } | RiscvInstr::CADD { .. }
            | RiscvInstr::CMPUSH { .. } | RiscvInstr::CMPOP { .. }
            | RiscvInstr::CSWSP { .. } | RiscvInstr::CFSWSP { .. }
            => 1,
        }
    }

    fn sign_extend_12(imm: Const12) -> i32 {
        ((imm.as_u32() << 20) as i32) >> 20
    }

    fn is_compact_gp(reg: GPRegister) -> bool {
        (8..=15).contains(&reg.0)
    }

    fn is_compact_fp(reg: FPRegister) -> bool {
        (8..=15).contains(&reg.0)
    }

    fn fits_cconst6_signed(imm: i32) -> bool {
        (-32..=31).contains(&imm)
    }

    fn as_cconst6_signed(imm: i32) -> CConst6 {
        CConst6::new((imm as i8) as u8)
    }

    fn can_use_c_sp_word_offset(imm: i32) -> bool {
        (0..=252).contains(&imm) && (imm & 0x3) == 0
    }

    fn can_use_c_word_offset(imm: i32) -> bool {
        (0..=124).contains(&imm) && (imm & 0x3) == 0
    }

    /// Rewrites a subset of 32-bit instructions into compressed equivalents
    /// when encoding constraints are met and no control-flow fixup semantics are involved.
    pub(crate) fn auto_compress_non_control(self) -> Self {
        match self {
            RiscvInstr::ADDI { rd, rs1, imm } => {
                let signed_imm = Self::sign_extend_12(imm);
                if rd.0 == 0 && rs1.0 == 0 && signed_imm == 0 {
                    return RiscvInstr::CNOP;
                }

                if rd.0 == 2
                    && rs1.0 == 2
                    && signed_imm != 0
                    && (-512..=496).contains(&signed_imm)
                    && (signed_imm & 0xF) == 0
                {
                    return RiscvInstr::CADDI16SP {
                        imm: CAddi16SpImm::new(signed_imm as i16),
                    };
                }

                if rs1.0 == 0 && rd.0 != 0 && Self::fits_cconst6_signed(signed_imm) {
                    return RiscvInstr::CLI {
                        rd,
                        imm: Self::as_cconst6_signed(signed_imm),
                    };
                }

                if rd == rs1 && rd.0 != 0 && signed_imm != 0 && Self::fits_cconst6_signed(signed_imm) {
                    return RiscvInstr::CADDI {
                        rd,
                        imm: Self::as_cconst6_signed(signed_imm),
                    };
                }

                // C.MV: rd = rs1, when rd != rs1, both non-zero, and imm == 0
                if rd.0 != 0 && rs1.0 != 0 && rd != rs1 && signed_imm == 0 {
                    return RiscvInstr::CMV { rd, rs2: rs1 };
                }

                RiscvInstr::ADDI { rd, rs1, imm }
            }
            RiscvInstr::SLLI { rd, rs1, imm } => {
                let shamt = imm.as_i16() as i32;
                if rd == rs1 && rd.0 != 0 && (1..=31).contains(&shamt) {
                    return RiscvInstr::CSLLI {
                        rd,
                        imm: CConst6::new(shamt as u8),
                    };
                }
                RiscvInstr::SLLI { rd, rs1, imm }
            }
            RiscvInstr::SRAI { rd, rs1, imm } => {
                let shamt = imm.as_i16() as i32;
                if rd == rs1 && Self::is_compact_gp(rd) && (1..=31).contains(&shamt) {
                    return RiscvInstr::CSRAI {
                        rd,
                        imm: CConst6::new(shamt as u8),
                    };
                }
                RiscvInstr::SRAI { rd, rs1, imm }
            }
            RiscvInstr::ADD { rd, rs1, rs2 } => {
                if rd == rs1 && rd.0 != 0 && rs2.0 != 0 {
                    return RiscvInstr::CADD { rd, rs2 };
                }
                RiscvInstr::ADD { rd, rs1, rs2 }
            }
            RiscvInstr::SUB { rd, rs1, rs2 } => {
                if rd == rs1 && Self::is_compact_gp(rd) && Self::is_compact_gp(rs2) {
                    return RiscvInstr::CSUB { rd, rs2 };
                }
                RiscvInstr::SUB { rd, rs1, rs2 }
            }
            RiscvInstr::XOR { rd, rs1, rs2 } => {
                if rd == rs1 && Self::is_compact_gp(rd) && Self::is_compact_gp(rs2) {
                    return RiscvInstr::CXOR { rd, rs2 };
                }
                RiscvInstr::XOR { rd, rs1, rs2 }
            }
            RiscvInstr::OR { rd, rs1, rs2 } => {
                if rd == rs1 && Self::is_compact_gp(rd) && Self::is_compact_gp(rs2) {
                    return RiscvInstr::COR { rd, rs2 };
                }
                RiscvInstr::OR { rd, rs1, rs2 }
            }
            RiscvInstr::AND { rd, rs1, rs2 } => {
                if rd == rs1 && Self::is_compact_gp(rd) && Self::is_compact_gp(rs2) {
                    return RiscvInstr::CAND { rd, rs2 };
                }
                RiscvInstr::AND { rd, rs1, rs2 }
            }
            RiscvInstr::LW { rd, rs1, imm } => {
                let signed_imm = Self::sign_extend_12(imm);
                if rs1.0 == 2 && rd.0 != 0 && Self::can_use_c_sp_word_offset(signed_imm) {
                    return RiscvInstr::CLWSP {
                        rd,
                        imm: CSpWordOffset::new(signed_imm as u8),
                    };
                }
                if Self::is_compact_gp(rd) && Self::is_compact_gp(rs1) && Self::can_use_c_word_offset(signed_imm) {
                    return RiscvInstr::CLW {
                        rd,
                        rs1,
                        imm: CWordOffset::new(signed_imm as u8),
                    };
                }
                RiscvInstr::LW { rd, rs1, imm }
            }
            RiscvInstr::SW { rs2, rs1, imm } => {
                let signed_imm = Self::sign_extend_12(imm);
                if rs1.0 == 2 && Self::can_use_c_sp_word_offset(signed_imm) {
                    return RiscvInstr::CSWSP {
                        rs2,
                        imm: CSpWordOffset::new(signed_imm as u8),
                    };
                }
                if Self::is_compact_gp(rs2) && Self::is_compact_gp(rs1) && Self::can_use_c_word_offset(signed_imm) {
                    return RiscvInstr::CSW {
                        rs2,
                        rs1,
                        imm: CWordOffset::new(signed_imm as u8),
                    };
                }
                RiscvInstr::SW { rs2, rs1, imm }
            }
            RiscvInstr::FLW { rd, rs1, imm } => {
                let signed_imm = Self::sign_extend_12(imm);
                if rs1.0 == 2 && Self::can_use_c_sp_word_offset(signed_imm) {
                    return RiscvInstr::CFLWSP {
                        rd,
                        imm: CSpWordOffset::new(signed_imm as u8),
                    };
                }
                if Self::is_compact_fp(rd) && Self::is_compact_gp(rs1) && Self::can_use_c_word_offset(signed_imm) {
                    return RiscvInstr::CFLW {
                        rd,
                        rs1,
                        imm: CWordOffset::new(signed_imm as u8),
                    };
                }
                RiscvInstr::FLW { rd, rs1, imm }
            }
            RiscvInstr::FSW { rs2, rs1, imm } => {
                let signed_imm = Self::sign_extend_12(imm);
                if rs1.0 == 2 && Self::can_use_c_sp_word_offset(signed_imm) {
                    return RiscvInstr::CFSWSP {
                        rs2,
                        imm: CSpWordOffset::new(signed_imm as u8),
                    };
                }
                if Self::is_compact_fp(rs2) && Self::is_compact_gp(rs1) && Self::can_use_c_word_offset(signed_imm) {
                    return RiscvInstr::CFSW {
                        rs2,
                        rs1,
                        imm: CWordOffset::new(signed_imm as u8),
                    };
                }
                RiscvInstr::FSW { rs2, rs1, imm }
            }
            _ => self,
        }
    }
}






// ************************** Instruction Format Immediates for Standard Instructions **************************

#[derive(Debug, Copy, Clone, Format)]
pub(crate) struct Const12(u32); // 12-bit immediate, usually sign-extended

impl Const12 {
    pub(crate) fn new(value: i16) -> Self {
        Const12(value as u32 & 0xFFF) // Mask to 12 bits
    }

    pub(crate) fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, Format)]
pub(crate) struct Const20(u32); // 20-bit immediate, usually shifted left by 12

impl Const20 {
    pub(crate) fn new(value: i32) -> Self {
        Const20(value as u32 & 0xFFFFF) // Mask to 20 bits
    }

    pub(crate) fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, Format)]
/// 5-bit shift amount
pub(crate) struct ShAmt5(u8);
impl ShAmt5 {
    pub(crate) fn new(value: u8) -> Self {
        ShAmt5(value & 0x1F) // Mask to 5 bits
    }

    pub(crate) fn to_const12(&self) -> Const12 {
        Const12(self.0 as u32)
    }

    pub(crate) fn as_i16(self) -> i16 {
        self.0 as i16
    }
}

// ************************** Rounding Mode for Floating-Point Instructions **************************


#[derive(Debug, Copy, Clone, Format)]
pub(crate) enum RoundingMode { // There are other rounding modes but they are never constructed so we deleted them for simplicity
    RNE,
    RTZ,
}

impl RoundingMode {
    pub(crate) fn bits(self) -> u8 {
        match self {
            RoundingMode::RNE => 0b000,
            RoundingMode::RTZ => 0b001,
        }
    }
}

// ************************** Instruction Format Immediates for Compressed Instructions **************************

#[derive(Debug, Copy, Clone, Format)]
/// 6-bit immediate, used for compressed instructions C.ADDI, C.ANDI etc
pub(crate) struct CConst6(u8);

impl CConst6 {
    pub(crate) fn new(value: u8) -> Self {
        CConst6(value & 0x3F) // Mask to 6 bits
    }

    pub(crate) fn as_u16(self) -> u16 {
        self.0 as u16
    }
}

#[derive(Debug, Copy, Clone, Format)]
/// 6-bit signed immediate for C.ADDI16SP. Stored as imm[9:4].
pub(crate) struct CAddi16SpImm(u8);

impl CAddi16SpImm {
    pub(crate) fn new(value: i16) -> Self {
        CAddi16SpImm(((value >> 4) as u8) & 0x3F)
    }

    pub(crate) fn as_u8(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, Format)]
/// Shifted word offset for CL/CS formats. Stored as uimm[6:2].
pub(crate) struct CWordOffset(u8);

impl CWordOffset {
    pub(crate) fn new(value: u8) -> Self {
        CWordOffset((value >> 2) & 0x1F)
    }

    pub(crate) fn as_u16(self) -> u16 {
        self.0 as u16
    }
}

#[derive(Debug, Copy, Clone, Format)]
/// 8-bit offset relative , used for single word compressed instructions like C.LWSP, C.SWSP etc.
/// Can only represent multiples of 4, if the value is not a multiple of 4 it will be rounded down
pub(crate) struct CSpWordOffset(u8);

impl CSpWordOffset {
    pub(crate) fn new(value: u8) -> Self {
        CSpWordOffset(value >> 2) // Only keep the upper 6 bits, ensuring it's a multiple of 4
    }

    pub(crate) fn as_u16(self) -> u16 {
        self.0 as u16
    }

}
