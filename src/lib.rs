#![no_std]
#![no_main]
#![feature(naked_functions)]
//! Rust LLD for RC1 platform
//!
extern crate alloc;
mod constant_expression_eval;
pub mod isa_model;
pub mod parse_and_translate;
mod translator;
mod vb;

#[cfg(test)]
#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();

#[cfg(test)]
#[defmt_test::tests]
mod test {
    use riscv as _;
    use riscv_defmt as _; // global logger
}
