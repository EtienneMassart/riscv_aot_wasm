# riscv_aot_wasm

A Rust crate that implements a RISC-V AOT (Ahead-of-Time) WebAssembly runtime for the RISC-V architecture. 

This crate has been developed as part of a Master's thesis in Informatics at the Technical University of Munich (TUM). The thesis is titled "Compiling WebAssembly on a RISC-V Automotive Microcontroller" and was submitted by Etienne Massart. 
An mdbook version of the thesis is available in the [`docs`](docs) folder of this repository.

The objective of the master's thesis was the porting of a [pre-existing AOT WebAssembly compiler](https://github.com/Infineon/aurix_webassembly_aot) to a RISC-V architecture, specifically the Infineon AURIX TC3xx family of microcontrollers. The thesis also includes an evaluation of the performance of the ported compiler on the target architecture.

The target architecture is `riscv32imafd-ifx-none-elf`, which is a custom target build because the base RISC-V toolchain does not provide it.  A version of rust compiler that support this target has been built following the instruction in [add new target](https://rustc-dev-guide.rust-lang.org/building/new-target.html)

Target description exported using `rustc --print target-spec-json --target riscv32imafd-ifx-none-elf -Z unstable-options` is:

```json
{
  "arch": "riscv32",
  "cpu": "generic-rv32",
  "crt-objects-fallback": "false",
  "data-layout": "e-m:e-p:32:32-i64:64-n32-S128",
  "eh-frame-header": false,
  "emit-debug-gdb-scripts": false,
  "features": "+m,+a,+f,+d,+zba,+zbb,+zbb,+zbs,+zcf,+zifencei,+zicsr,+zihintpause,+zicond,+zicntr,+zfa,+zfh,+zca,+zcb,+zcmp,+zcmt,+zbc",
  "linker": "rust-lld",
  "linker-flavor": "gnu-lld",
  "llvm-abiname": "ilp32d",
  "llvm-target": "riscv32",
  "max-atomic-width": 32,
  "metadata": {
    "description": "RiscV 32-bit target with MAFD extensions",
    "host_tools": false,
    "std": false,
    "tier": 2
  },
  "panic-strategy": "abort",
  "relocation-model": "static",
  "target-pointer-width": "32"
}
```


## Required tools

- Primary compiler used by this repository: Rust compiler `rustc` (toolchain `1.85`)
	- Main runtime, tests, and Rust benchmark paths are built with `rustc`
	- Target required by this project: `riscv32imafd-ifx-none-elf`
	  - See previous section for target description and details
	- Required components: `rustfmt`, `clippy`, `llvm-tools`
	- Verify installed compiler: `rustc --version --verbose`

- [cargo-make](https://crates.io/crates/cargo-make)
	- Used to run simulator tests and benchmark tasks defined in `Makefile.toml`
- `qemu-system-riscv32` (min version `8.2.2`)
	- it can be istalled via:
		```bash
		sudo apt uninstall qemu-system-misc qemu-utils
		```
- [defmt-print](https://crates.io/crates/defmt-print)
	- Used by `simulator.sh` to decode target logs
	- Installed automatically by `cargo make test-qemu` / `cargo make run-benchmark-qemu`
- [rustfilt](https://crates.io/crates/rustfilt)
	- Used during test output post-processing
	- Installed automatically by `cargo make test-qemu` / `cargo make run-benchmark-qemu`
- [WABT `wast2json` v1.0.36](https://github.com/WebAssembly/wabt/releases/tag/1.0.36)
	- Required only when regenerating tests from files in `wast_tests`
	- Checked by `cargo make` task `check-wast2json`

## Optional tooling used in this repository

- [wasm-tools](https://crates.io/crates/wasm-tools)
	- Optional helper tool; install task is available in `Makefile.toml`

## Environment setup (Linux)

## Common commands

All commands must be run from the `` subdirectory:

```bash
cd riscv_aot_wasm
```

-- Run all generated/runtime tests on QEMU:

```bash
cargo make test-qemu --tests
```

- Run a specific generated test:

```bash
cargo make test-qemu --test <test_name e.g address_0>
```


## Test generation notes

Most integration tests in `generated_tests` are produced from W3C core `.wast` files.

- Source `.wast` files: `wast_tests`
- Intermediate `.json`/`.wasm`: `wasm_json_from_wast`
- Generated Rust integration tests: `generated_tests`

Note: `device.x` (the linker script) lives at the repo root and is referenced as `device.x` from `build.rs`.

When `.wast` inputs change, `cargo make test-qemu` automatically triggers regeneration via `wast2json` and the local `tests_generator` crate.