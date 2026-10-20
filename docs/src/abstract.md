# Abstract

Automotive microcontrollers increasingly need to run small, updatable routines, sensor
polling, telemetry parsing, feature configuration, without reflashing firmware for every change.
MicroPython and Lua already provide that flexibility, but both carry real overhead in speed
and memory on resource-constrained hardware. WebAssembly offers the same portability
and sandboxing, but interpreting it is just as slow. This thesis ports an existing ahead-of-time, on-target WebAssembly compiler to RISC-V, an open instruction set architecture that
Infineon’s next generation of automotive microcontrollers will adopt alongside TriCore, the
architecture the compiler originally targeted. The compiler implements the valent-block
technique [1], resolving WebAssembly’s stack-based instructions lazily into machine code
rather than materializing the operand stack in memory, and porting it required adapting
register allocation, the calling convention, and instruction selection to RISC-V’s unified
register file and compressed instruction encoding.

We evaluate the resulting compiler on four PolyBench/C kernels against MicroPython,
Lua, and native C on a simulated RV32IMAFD target. The compiled output runs an order of
magnitude faster than MicroPython, several times faster than Lua, and within a single-digit
factor of native C, the appropriate ceiling for this comparison rather than its target, at the
cost of a roughly 50% larger flashed image than either interpreter. We identify concrete,
unimplemented opportunities to narrow that gap further. Trap handling is explicitly out of
scope: this thesis demonstrates that the valent-block approach transfers cleanly to RISC-V and
delivers strong performance, not that it yet matches WebAssembly’s full safety guarantees.

<!-- source-pdf-page: 6 -->
