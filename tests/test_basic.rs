#![no_std]
#![no_main]
use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicU32, Ordering},
};
use defmt_test::tests;
use riscv as _;
use riscv_defmt as _; // global logger
#[cfg(test)]
#[defmt::panic_handler]
fn panic() -> ! {
    core::panic!("panic via `defmt::panic!`")
}

static COUNT: AtomicU32 = AtomicU32::new(0);
defmt::timestamp!("{=u32:us}", COUNT.fetch_add(1, Ordering::Relaxed));

#[cfg(test)]
#[tests]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
