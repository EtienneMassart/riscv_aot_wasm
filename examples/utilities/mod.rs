extern crate alloc;
use embedded_alloc::LlffHeap as Heap;
use riscv_aot_wasm::parse_and_translate::{WasmRuntime, LinearMemory,GlobalSpace};
use core::arch::asm;
use core::ptr::addr_of_mut;
// use aot_wasm::handle_misaligned_load_store;
use riscv_rt as _;
use riscv_defmt as _; // global defmt logger (also provides panic_handler)
use riscv_semihosting::debug::{self, EXIT_FAILURE};



#[global_allocator]
static HEAP: Heap = Heap::empty();

#[allow(unused_imports)]
use alloc::vec;



// Thanks to the usage of a custom page size within the wasm module,
// the memory in the benchmark modules has a size of 32KiB. 
// This allows allocating a smaller space for the linear memory.
const MAX_MEMORY_SIZE: usize = (1 << 17) + 7;

#[unsafe(link_section = ".data")]
static mut INSTRUCTIONS: [u16; 8192] = [0; 8192];
#[unsafe(link_section = ".data")]
static mut LINEAR_MEMORY: LinearMemory<MAX_MEMORY_SIZE> = LinearMemory::new();
#[unsafe(link_section = ".data")]
static mut GLOBAL_SPACE: GlobalSpace<256> = GlobalSpace::new();
#[unsafe(link_section = ".data")]
static mut TABLE: [u32; 256] = [0; 256];

#[unsafe(export_name = "ExceptionHandler")]
unsafe extern "C" fn exception_handler(_trap_frame: &riscv_rt::TrapFrame) -> ! {
    let cause = riscv::register::mcause::read().bits();
    defmt::println!("Exception cause {=usize}", cause);
    debug::exit(EXIT_FAILURE);
    loop {}
}

pub(crate) fn init() -> WasmRuntime<'static> {
    unsafe {
        use core::mem::MaybeUninit;

        use defmt as _;

        const HEAP_SIZE: usize = 20 * 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

        #[allow(static_mut_refs)]
        HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE);
        defmt::println!("End init");

        let runtime = WasmRuntime::new(
            &mut *addr_of_mut!(INSTRUCTIONS),
            &mut *addr_of_mut!( LINEAR_MEMORY),
            &mut *addr_of_mut!( GLOBAL_SPACE),
            &mut *addr_of_mut!(TABLE),
        );
        runtime
    }
}

pub(crate) struct Timer {
    last_value: u64,
}


impl Timer {
    pub(crate) fn new()->Timer {
        Timer{
            last_value:0
        }
    }

    fn read_cycle64() -> u64 {
        #[cfg(target_pointer_width = "32")]
        unsafe {
            // Read a stable 64-bit cycle counter on RV32 via hi/lo/hi sampling.
            let mut hi0: u32;
            let mut lo: u32;
            let mut hi1: u32;
            loop {
                asm!("rdcycleh {0}", out(reg) hi0);
                asm!("rdcycle {0}", out(reg) lo);
                asm!("rdcycleh {0}", out(reg) hi1);
                if hi0 == hi1 {
                    break;
                }
            }
            ((hi1 as u64) << 32) | (lo as u64)
        }

        #[cfg(target_pointer_width = "64")]
        unsafe {
            let mut val: u64;
            asm!("rdcycle {0}", out(reg) val);
            val
        }
    }

    pub(crate) fn read_timer(&mut self) {
        self.last_value = Self::read_cycle64();
    }

    pub(crate) fn get_delta(&mut self) -> u64 {
        let previous_value = self.last_value;
        self.read_timer();
        self.last_value - previous_value
    }
}
