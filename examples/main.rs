//!
//! My Demo crate
#![no_std]
#![no_main]
use core::fmt::Write;
use core::num::Wrapping;
use core::panic::PanicInfo;
use defmt::println;
use riscv as _;
use riscv_defmt as _; // global logger
use riscv_rt::entry;
use riscv_semihosting::debug::{self, EXIT_FAILURE, EXIT_SUCCESS}; // global logger

/// Main
#[entry]
fn main() -> ! {
    let mut a = Wrapping(1);
    a += 1;
    println!("Hello, world! a = {=i32}", a.0);
    debug::exit(EXIT_SUCCESS);
    loop {}
}
