# 1 Introduction

<!-- source-pdf-page: 8 -->

## 1.1 WebAssembly
WebAssembly (abbreviated Wasm) is a safe and compact binary instruction format designed
for a stack-based virtual machine. Originally introduced in 2017 as a low-level, portable code
format made to run with near-native performance on the web, it has since evolved into a
general-purpose sandboxed compilation target for languages such as C/C++, Rust, and Go.
Its safety model relies on running modules in an isolated environment separated from the
host system, where memory accesses are strictly bounds checked and control flow is validated
at load time. Nondeterministic operations are intentionally limited to guarantee predictable
behavior across different execution engines.

At runtime, WebAssembly executes instructions on a typed operand stack. Instructions
push and pop 32-bit and 64-bit integer or floating-point values from this stack rather than
explicitly naming registers. Functions maintain locals and globals in separate index spaces,
while program data resides in linear memory, a contiguous, growable byte array where
every access is dynamically bounds checked. Control flow is structured using labeled blocks,
loops, and conditional constructs. Branches target block labels rather than arbitrary memory
addresses, which simplifies binary validation and code compilation.

These characteristics make WebAssembly particularly well-suited for resource-constrained
embedded devices, where portability, safety, and a minimal memory footprint are crucial. In
modern automotive systems, software must frequently perform tasks such as vehicle data
collection, telemetry analysis, and dynamic feature configuration. Traditionally, updating or
tweaking these tasks required reflashing full firmware images across dozens of heterogeneous
electronic control units (ECUs). Dynamic scripts provide a much more flexible alternative, allowing developers to deploy lightweight routines on demand. However, interpreted scripting
environments like MicroPython and Lua come with significant trade-offs on embedded platforms, including high runtime memory overhead, slow execution loops, and weak type safety.
WebAssembly offers the same dynamic deployment benefits as these scripting languages, but
delivers near-native performance, predictable memory consumption, and hardware-isolated
execution guarantees.

Interpreting WebAssembly bytecode directly on microcontrollers still introduces significant overhead due to decoding and instruction dispatch. Ahead-of-time (AOT) translation
addresses this by shifting the translation cost entirely to compile time, providing reliable,
deterministic execution times alongside a minimal runtime memory footprint. Full-featured
optimizing compilers are far too large and complex to run directly on-target within microcontrollers, which is where the lightweight valent-block translation approach [1] comes
in.

<!-- source-pdf-page: 9 -->

## 1.2 Existing TriCore Compiler
Our work builds on a pre-existing WebAssembly-to-machine-code compiler for the TriCore
architecture, developed internally at Infineon by a team that the author was also part of prior
to this thesis. TriCore is a proprietary ISA used in AURIX microcontrollers for the automotive
industry. It is a single-core 32-bit microcontroller/DSP architecture optimized for real-time
systems. The compiler is written in Rust, a systems programming language with inbuilt
memory safety features that is well-suited for embedded systems.

The stack-based nature of WebAssembly can’t be naively translated to register-based
architectures, as a straightforward translation would push and pop values to and from the
runtime stack and heavily degrade the performance. The valent-block approach addresses this
by building a valent-block intermediate representation in a single pass over the Wasm code. A
valent-block is a grouping of instructions in a tree-like structure. Machine code is then emitted
lazily only when encountering instructions with non-stack side effects. Numeric instructions
or conversions only affect the WebAssembly runtime stack and are considered side-effect-free.
On the other hand, control-flow instructions (e.g. `br`, `call`), variable instructions that modify
local and global variables (e.g. `local.tee`, `global.set`), and memory instructions that modify
the linear memory (e.g. `i32.store`), all have effects beyond the runtime stack. When such
an instruction is encountered, the pending valent-blocks are resolved: each one is computed
using registers, and its result is written to wherever it is ultimately needed, preserving the
order WebAssembly’s own operand stack requires.
## 1.3 RISC-V
RISC-V is an open standard ISA with a reduced instruction set. Since it was first introduced
in 2010, RISC-V has seen rapid adoption across microcontrollers and embedded systems
due to its lack of licensing fees, large and modular extension ecosystem, and vendor-neutral
openness. This shared toolchain ecosystem significantly simplifies hardware verification,
which further reduces costs and speeds development. Consequently, the new generation of
Infineon AURIX devices will integrate RISC-V alongside established proprietary cores.
## 1.4 Goals and Contributions
The primary objective of this thesis is to port the existing Rust-based single-pass Valent-Block
compiler from the proprietary TriCore architecture to the open-standard RISC-V architecture,
and to conduct a rigorous comparative performance analysis against traditional embedded
scripting environments.

To achieve this, several key architectural and comparative challenges must be addressed:

<!-- source-pdf-page: 10 -->

### 1.4.1 New instruction set architecture
The first challenge is adapting the existing translation approach to a structurally different
instruction set architecture. TriCore separates its general-purpose registers into a 16-entry
address register file and a 16-entry data register file, used for memory and arithmetic
operations respectively, whereas RISC-V exposes a single unified file of 32 general-purpose
registers with no such separation. This changes how values reserved for the stack pointer,
frame pointer, and linear memory base must be protected from being overwritten by ordinary
computation. RISC-V’s F and D extensions natively provide 32-bit and 64-bit floating-point
registers, unlike TriCore’s data registers, which must be paired into extended registers to
hold 64-bit values. Conversely, RISC-V has no such register-pairing mechanism for 64-bit
integers, so those still have to be handled manually. RISC-V further departs from TriCore by
mixing 16-bit compressed instructions with 32-bit ones, and by replacing TriCore’s dedicated
stack push/pop instructions with a load/store model in which the stack pointer must be
tracked and updated explicitly. Finally, individual TriCore and RISC-V instructions must
be compared directly to determine how each WebAssembly instruction can be efficiently
translated. These differences, and how the valent-block approach is adapted to address them,
are detailed in the Approach chapter.
## 1.5 Evaluation
Evaluation will be based on two main criteria: binary size and execution performance.
First, the runtime itself needs to maintain a minimal memory footprint to remain viable for
constrained embedded systems. This can easily be measured using the `cargo-binutils` crate
in Rust. For reference, the existing TriCore implementation has a binary size of around
360 KiB, though direct comparisons across two distinct architectures naturally present some
limitations.

Execution performance will be measured using selected workloads from the `PolyBench/C
4.2.1` benchmark suite, which consists of various numerical computation routines. For our
evaluation, we use `floyd-warshall` and `nussinov` to test integer operations, alongside `correlation`
and `jacobi-1d` for single-precision floating-point performance.

To thoroughly assess the trade-offs of our approach, we benchmark our compiled output
against both native code and traditional interpreted scripting environments like MicroPython
and Lua. While these scripting languages are popular in embedded development due to their
dynamic deployment flexibility, their reliance on interpreter loops and dynamic memory
allocation creates significant performance overhead. This comparison follows the same
structure used by Moron and Wallentowitz [2] in their own evaluation of WebAssembly
runtimes against established embedded scripting engines, MicroPython and Lua chief among
them. We compile the PolyBench C source files directly to native RISC-V machine code
using the `-O3` optimization level to establish an upper bound on performance. We also
implement equivalent versions of these benchmarks for MicroPython and Lua to quantify the
baseline overhead of interpreted scripts. Finally, we translate WebAssembly modules of the
same benchmarks, compiled at the `-O3` flag, to RISC-V machine code using our Valent-Block

<!-- source-pdf-page: 11 -->

compiler, and measure the execution time. This allows us to evaluate whether on-target
WebAssembly compilation can deliver the dynamic execution benefits of scripting engines
while achieving near-native execution speed and predictable resource consumption.
