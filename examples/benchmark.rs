#![no_std]
#![no_main]
extern crate alloc;

mod utilities;

use alloc::vec;
use riscv_aot_wasm::isa_model::{Immediate, ValueType };
use riscv as _;
use riscv_defmt as _; // global logger
use utilities::*;
use riscv_rt::entry;
use riscv_semihosting::debug::{self, EXIT_SUCCESS};



#[entry]
fn main() -> ! {
    let mut timer = Timer::new();
    let mut runtime = init();

    macro_rules! run_benchmark {
    ($wasm_file:expr, $expected_result:expr) => {{
        runtime
            .parse_and_translate(include_bytes!(concat!("../benchmark-wasm/", $wasm_file)))
            .unwrap();
        let _ = runtime.call_exported_function("init", vec![], None);
        timer.read_timer();
        let _ = runtime.call_exported_function("kernel", vec![], None);
        let delta = timer.get_delta();
        let result = runtime.call_exported_function("result", vec![], Some(ValueType::I64));
        assert_eq!(result, Some(Immediate::I64($expected_result)));
        defmt::println!("{=str} Cycles: {=u64} Code Size:{=i32}", stringify!($wasm_file), delta,runtime.get_function_size("kernel"));
    }};
}
    
    run_benchmark!("correlation.wasm",781);
    run_benchmark!("floyd-warshall.wasm", 6594);
    run_benchmark!("jacobi-1d.wasm", 2);
    run_benchmark!("nussinov.wasm", 16254);
    debug::exit(EXIT_SUCCESS);
    loop{}
}
