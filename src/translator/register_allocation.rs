use crate::isa_model::{
	Const12, FPRegister, GPRegister, MapperLocation, Register, RegisterPair, RiscvInstr,
	ValueSize, ValueType, MAX_FP_REGISTERS, MAX_GP_REGISTERS, MIN_ALLOCATABLE_GP_REGISTER,
	MAX_USABLE_GP_REGISTERS, STACK_POINTER,
};
use alloc::vec;
use alloc::vec::Vec;

use super::Translator;

impl Translator<'_, '_> {
	// Spills any scratch location to the runtime stack to preserve LIFO ordering.
	// Updates gp_alloc when a GP register or pair is freed; fp_alloc when an FP register is freed.
	fn spill_any_location(&mut self, location: &mut MapperLocation, gp_alloc: &mut [bool], fp_alloc: &mut [bool]) -> bool {
		match location {
			MapperLocation::GPRegister(GPRegister(idx)) => {
				// Claim 4 bytes above the current stack top, compute the SP-relative offset
				// (which also rebases SP downward if the gap would exceed 12-bit range), then store.
				let idx = *idx;
				let new_height = self.modeled_stack_height.add(4);
				let offset = self.stack_logical_offset_to_imm(new_height);
				self.push_instruction(RiscvInstr::SW { rs2: GPRegister(idx), rs1: STACK_POINTER, imm: Const12::new(offset) });
				self.modeled_stack_height = new_height;
				gp_alloc[idx as usize] = false;
				*location = MapperLocation::Stack { offset: new_height.to_i32(), val_type: ValueType::I32 };
				true
			}
			MapperLocation::RegisterPair(RegisterPair { low: GPRegister(l), high: GPRegister(h) }) => {
				// Same as above but 8 bytes; low word at base_offset, high word at base_offset+4.
				let (low_idx, high_idx) = (*l, *h);
				let new_height = self.modeled_stack_height.add(8);
				let base_offset = self.stack_logical_offset_to_imm(new_height);
				self.push_instruction(RiscvInstr::SW { rs2: GPRegister(low_idx), rs1: STACK_POINTER, imm: Const12::new(base_offset) });
				self.push_instruction(RiscvInstr::SW { rs2: GPRegister(high_idx), rs1: STACK_POINTER, imm: Const12::new(base_offset + 4) });
				self.modeled_stack_height = new_height;
				gp_alloc[low_idx as usize] = false;
				gp_alloc[high_idx as usize] = false;
				*location = MapperLocation::Stack { offset: new_height.to_i32(), val_type: ValueType::I64 };
				true
			}
			MapperLocation::FPRegister { reg, val_size } => {
				// Same claim-and-store pattern; FSW for f32, FSD for f64.
				let (idx, val_size) = (reg.0, *val_size);
				let new_height = self.modeled_stack_height.add(val_size.as_bytes());
				let offset = self.stack_logical_offset_to_imm(new_height);
				match val_size {
					ValueSize::Word => self.push_instruction(RiscvInstr::FSW { rs2: FPRegister(idx), rs1: STACK_POINTER, imm: Const12::new(offset) }),
					ValueSize::DoubleWord => self.push_instruction(RiscvInstr::FSD { rs2: FPRegister(idx), rs1: STACK_POINTER, imm: Const12::new(offset) }),
				}
				let val_type = match val_size { ValueSize::Word => ValueType::F32, ValueSize::DoubleWord => ValueType::F64 };
				self.modeled_stack_height = new_height;
				fp_alloc[idx as usize] = false;
				*location = MapperLocation::Stack { offset: new_height.to_i32(), val_type };
				true
			}
			// Location is already on the stack or in non-register storage; nothing to spill.
			_ => false,
		}
	}

	pub(crate) fn next_available_gp_register(
		&mut self,
		scratch_variable_map: &mut Vec<MapperLocation>,
		used_registers: &[MapperLocation],
	) -> GPRegister {
		let mut gp_alloc = vec![false; MAX_GP_REGISTERS];
		let mut fp_alloc = vec![false; MAX_FP_REGISTERS];
		// Mark the pinned register if any.
		match self.locked_register {
			Some(Register::GP(r)) => gp_alloc[r.0 as usize] = true,
			Some(Register::Pair(p)) => { gp_alloc[p.low.0 as usize] = true; gp_alloc[p.high.0 as usize] = true; }
			Some(Register::FP(r)) => fp_alloc[r.0 as usize] = true,
			_ => ()
		}
		// Mark every register currently holding a local variable, a scratch value, or a live operand.
		for loc in self.locals_map.iter().chain(scratch_variable_map.iter()).chain(used_registers.iter()) {
			match loc {
				MapperLocation::GPRegister(GPRegister(i)) => gp_alloc[*i as usize] = true,
				MapperLocation::RegisterPair(RegisterPair { low, high }) => { gp_alloc[low.0 as usize] = true; gp_alloc[high.0 as usize] = true; }
				MapperLocation::FPRegister { reg, .. } => fp_alloc[reg.0 as usize] = true,
				_ => ()
			}
		}
		loop {
			// Scan the allocatable range for a free slot; return immediately if one is found.
			for i in MIN_ALLOCATABLE_GP_REGISTER..MAX_USABLE_GP_REGISTERS as u8 {
				if !gp_alloc[i as usize] { return GPRegister(i); }
			}
			// No free register: spill the oldest scratch entry that is still in a register,
			// which frees its register in gp_alloc/fp_alloc and retries the scan.
			// We spill every scratch location in order to preserve LIFO ordering of the stack, no matter the type.
			for location in scratch_variable_map.iter_mut() {
				if self.spill_any_location(location, &mut gp_alloc, &mut fp_alloc) { break; }
			}
		}
	}

	pub(crate) fn next_available_register_pair(
		&mut self,
		scratch_variable_map: &mut Vec<MapperLocation>,
		used_registers: &[MapperLocation],
	) -> RegisterPair {
		let mut gp_alloc = vec![false; MAX_GP_REGISTERS];
		let mut fp_alloc = vec![false; MAX_FP_REGISTERS];
		match self.locked_register {
			Some(Register::GP(r)) => gp_alloc[r.0 as usize] = true,
			Some(Register::Pair(p)) => { gp_alloc[p.low.0 as usize] = true; gp_alloc[p.high.0 as usize] = true; }
			Some(Register::FP(r)) => fp_alloc[r.0 as usize] = true,
			_ => ()
		}
		for loc in self.locals_map.iter().chain(scratch_variable_map.iter()).chain(used_registers.iter()) {
			match loc {
				MapperLocation::GPRegister(GPRegister(i)) => gp_alloc[*i as usize] = true,
				MapperLocation::RegisterPair(RegisterPair { low, high }) => { gp_alloc[low.0 as usize] = true; gp_alloc[high.0 as usize] = true; }
				MapperLocation::FPRegister { reg, .. } => fp_alloc[reg.0 as usize] = true,
				_ => ()
			}
		}
		loop {
			for low in MIN_ALLOCATABLE_GP_REGISTER..(MAX_USABLE_GP_REGISTERS as u8 - 1) {
				if !gp_alloc[low as usize] && !gp_alloc[low as usize + 1] {
					return RegisterPair { low: GPRegister(low), high: GPRegister(low + 1) };
				}
			}
			for location in scratch_variable_map.iter_mut() {
				if self.spill_any_location(location, &mut gp_alloc, &mut fp_alloc) { break; }
			}
		}
	}

	pub(crate) fn next_available_fp_register(
		&mut self,
		scratch_variable_map: &mut Vec<MapperLocation>,
		used_registers: &[MapperLocation],
	) -> FPRegister {
		let mut gp_alloc = vec![false; MAX_GP_REGISTERS];
		let mut fp_alloc = vec![false; MAX_FP_REGISTERS];
		match self.locked_register {
			Some(Register::GP(r)) => gp_alloc[r.0 as usize] = true,
			Some(Register::Pair(p)) => { gp_alloc[p.low.0 as usize] = true; gp_alloc[p.high.0 as usize] = true; }
			Some(Register::FP(r)) => fp_alloc[r.0 as usize] = true,
			_ => ()
		}
		for loc in self.locals_map.iter().chain(scratch_variable_map.iter()).chain(used_registers.iter()) {
			match loc {
				MapperLocation::GPRegister(GPRegister(i)) => gp_alloc[*i as usize] = true,
				MapperLocation::RegisterPair(RegisterPair { low, high }) => { gp_alloc[low.0 as usize] = true; gp_alloc[high.0 as usize] = true; }
				MapperLocation::FPRegister { reg, .. } => fp_alloc[reg.0 as usize] = true,
				_ => ()
			}
		}
		loop {
			for i in 0..MAX_FP_REGISTERS as u8 {
				if !fp_alloc[i as usize] { return FPRegister(i); }
			}
			for location in scratch_variable_map.iter_mut() {
				if self.spill_any_location(location, &mut gp_alloc, &mut fp_alloc) { break; }
			}
		}
	}

	pub(crate) fn get_dest_gp_register(
		&mut self, 
		potential_target: Option<&MapperLocation>, 
		scratch_variable_map: &mut Vec<MapperLocation>, 
		used_registers: &[MapperLocation]
	) -> GPRegister {
		let mut occupied_used_registers = vec![false; MAX_GP_REGISTERS ];
        for used_register in used_registers {
            match used_register {
                MapperLocation::GPRegister(GPRegister(index)) => occupied_used_registers[*index as usize] = true,
                MapperLocation::RegisterPair(RegisterPair { low, high }) => {
                    occupied_used_registers[low.0 as usize] = true;
                    occupied_used_registers[high.0 as usize] = true;
                },
                _ => ()
            }
        }
        match potential_target {
            // we don't need to check the scratch_variable_map here, because if the target is specified we're at the top of the valent block tree and therefore the scratch variable map is empty
            Some(MapperLocation::GPRegister(GPRegister(index)) ) if !occupied_used_registers[*index as usize] => GPRegister(*index),
            _ => self.next_available_gp_register(scratch_variable_map, used_registers)
        }
	}

	pub(crate) fn get_dest_register_pair(
		&mut self, 
		potential_target: Option<&MapperLocation>, 
		scratch_variable_map: &mut Vec<MapperLocation>, 
		used_registers: &[MapperLocation]
	) -> RegisterPair {
		let mut occupied_used_registers = vec![false; MAX_GP_REGISTERS ];
		for used_register in used_registers {
			match used_register {
				MapperLocation::GPRegister(GPRegister(index)) => occupied_used_registers[*index as usize] = true,
				MapperLocation::RegisterPair(RegisterPair { low, high }) => {
					occupied_used_registers[low.0 as usize] = true;
					occupied_used_registers[high.0 as usize] = true;
				},
				_ => ()
			}
		}
		match potential_target {
			// we don't need to check the scratch_variable_map here, because if the target is specified we're at the top of the valent block tree and therefore the scratch variable map is empty
			Some(MapperLocation::RegisterPair(RegisterPair { low: GPRegister(low_index), high: GPRegister(high_index) })) if !occupied_used_registers[*low_index as usize] && !occupied_used_registers[*high_index as usize] => RegisterPair { low: GPRegister(*low_index), high: GPRegister(*high_index) },
			_ => self.next_available_register_pair(scratch_variable_map, used_registers)
		}
	}

	pub(crate) fn get_dest_fp_register(
		&mut self,
		potential_target: Option<&MapperLocation>,
		scratch_variable_map: &mut Vec<MapperLocation>,
		used_registers: &[MapperLocation],
	) -> FPRegister {
		let mut occupied_used_registers = vec![false; MAX_FP_REGISTERS ];
		for used_register in used_registers {
			match used_register {
				MapperLocation::FPRegister { reg, .. } => occupied_used_registers[reg.0 as usize] = true,
				_ => (),
			}
		}
		match potential_target {
			Some(MapperLocation::FPRegister { reg, .. }) if !occupied_used_registers[reg.0 as usize] => *reg,
			_ => self.next_available_fp_register(scratch_variable_map, used_registers),
		}
	}
}

