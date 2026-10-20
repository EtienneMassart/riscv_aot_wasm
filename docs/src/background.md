# 2 Background

<!-- source-pdf-page: 12 -->

## 2.1 WebAssembly Core Specification

The WebAssembly core specification is maintained by the W3C WebAssembly Working
Group [3]. It defines the module structure, validation rules, runtime semantics, and two
official formats: a human-readable text format (.wat) used for debugging, and a compact
binary format (.wasm) used for transmission, parsing, and execution. This thesis focuses on
WebAssembly 1.0, the first stable release of the core standard.

### 2.1.1 Concepts and Data Representation

**Primitive Value Types and Linear Memory.** WebAssembly defines four primitive value
types: 32-bit and 64-bit integers (i32, i64), and single- and double-precision floating-point
numbers (f32, f64). At the type level, integer types do not distinguish between signed and
unsigned values. Instead, whether a value is treated as signed or unsigned is determined
later, by specific instructions that interpret integer bit patterns as signed in two’s complement
or unsigned. There are no explicit primitive types for booleans or memory addresses, both of
which are represented as 32-bit integers. Conversion between any of the four basic types is
supported through explicit reinterpretation, promotion, demotion, or truncation instructions,
which will be detailed in the instruction overview.

Floating-point operations strictly adhere to the IEEE 754-2019 standard using round-to-nearest, ties-to-even rounding mode. To minimize platform-dependent non-determinism and
guarantee predictable behavior across hardware architectures, the specification defines strict
rules for handling Not-a-Number (NaN) values. IEEE 754 differentiates between quiet NaNs,
which propagate through computations without signaling exceptions, and signaling NaNs,
which raise error flags. WebAssembly makes no distinction between signaling and quiet
NaNs, defining instead two operational categories of NaNs:

- Canonical NaN: A NaN value with a specific, canonical payload defined by the specification, where the most significant bit of the mantissa is set while all remaining mantissa
  bits are unset. If all NaN operands in an operation are canonical NaNs, the result is
  guaranteed to be a canonical NaN.
- Arithmetic NaN: A broader range of NaN representations that includes the canonical
  NaN alongside any valid NaN bit pattern. When an operation consumes one or
  more non-canonical NaN operands, the specification allows the result to be any valid
  arithmetic NaN.

<!-- source-pdf-page: 13 -->

This controlled non-determinism around NaN bit patterns avoids the high runtime cost
of normalizing NaN payloads after every operation on platforms where hardware FPUs
generate different default NaNs. At the same time, it provides sufficient guarantees for source
language compilers targeting WebAssembly to reliably employ techniques like NaN-boxing.

Complex data structures such as arrays, structures, and strings are not natively supported
and high-level languages must compile these abstractions down to linear memory. WebAssembly employs a linear memory model, structured as a contiguous byte array starting at
address 0. Linear memory is kept strictly separate from local variables, the operand stack,
and compiled instruction code, adhering to a Harvard architecture. This isolation ensures
that code memory is non-writable and data memory is non-executable, providing a strong
security boundary that inherently prevents arbitrary code injection attacks. Linear memory
is allocated and grown dynamically in 64 KiB pages, bounded by a 32-bit address space
limit of 65,536 pages (4 GiB), where invalid memory expansions or out-of-bounds accesses
immediately trigger traps [3].

**The Operand Stack and Type Validation.** Computations in WebAssembly rely on an implicit,
unbounded operand stack. Instructions consume arguments from the top of the stack and
push their results back, with WebAssembly 1.0 instructions producing at most one result
value. Function parameters are passed on the stack, and function return values are left on top
of the stack upon completion. The operand stack is an abstract compile-time construct rather
than a physical memory structure exposed to the program, code cannot reference stack slots
directly via memory addresses or register names.

Stack manipulation is governed by a strong static type system. Every instruction has a
fixed function type signature defining its required input types and produced output types.
For instance, the binary addition instruction i32.add consumes two 32-bit integers from the
stack and pushes one 32-bit integer result, represented by the type signature [i32 i32] ->
[i32]. Because instruction type signatures are fixed, the complete layout and height of the
operand stack can be determined statically at every point in a function body. Type validation
rules enforce stack invariance across control flow join points, ensuring that regardless of
which execution path reaches a given instruction, the stack height and type sequence remain
identical.

This static determinism is at the heart of WebAssembly’s security and performance model.
Validation verifies that the stack is well-formed, that all variable indices exist, and that
instructions never execute on invalid operand types. Because the operand stack can be fully
analyzed in a single pass before execution, baseline compilers can virtualize the stack and
generate target machine code without incurring the cost of pushing and popping intermediate
values to physical hardware memory. However, static type validation cannot prevent all
runtime errors. Operations whose safety depends on dynamic values, such as linear memory
address bounds, memory allocation limits, or division by zero, must be checked at runtime.
When such an invalid condition occurs, execution generates a trap, which immediately
interrupts control flow and safely yields control back to the host environment [3].

<!-- source-pdf-page: 14 -->

### 2.1.2 Instruction Set Architecture and Semantics

**Numeric and Parametric Instructions.** Numeric instructions perform arithmetic, bitwise,
comparison, testing, and type conversion operations. They are strictly non-polymorphic,
requiring distinct instructions for each data type (e.g., i32.add, i64.sub, f32.mul, or f64.div).
Numeric instructions fall into six main categories based on their function signatures and
operand behavior:

- Constants ([] -> [t]): Instructions such as i32.const, i64.const, f32.const, and
  f64.const take a static immediate value and push it directly onto the operand stack.
- Unary Operations ([t] -> [t]): Single-operand calculations that yield a result of the
  same type, such as bit counting operations on integers or square roots and absolute
  values on floating-point numbers.
- Binary Operations ([t t] -> [t]): Two-operand operations consuming values of
  matching type t to compute an arithmetic, bitwise, or logical result of the same type.
- Test Operations ([t] -> [i32]): Instructions that test a single operand and return an
  i32 boolean result, such as i32.eqz, which evaluates whether an integer is equal to
  zero.
- Comparison Operations ([t t] -> [i32]): Instructions evaluating equality or ordering
  between two operands of type t, returning an i32 boolean result. Integer comparisons
  supply explicit variants to evaluate operands as signed (\_s) in two’s complement or
  unsigned (\_u), such as i32.lt_s versus i32.lt_u.
- Conversion Operations ([t1] -> [t2]): Operations that cast a value from type t1 to
  type t2. The specification distinguishes between numerical conversions that preserve
  numeric value as closely as possible (e.g., f32.convert_i32_s) and bitwise reinterpretations that leave underlying bit patterns unchanged (e.g., i32.reinterpret_f32).

Parametric instructions manipulate values on top of the operand stack without inspecting
or restricting their concrete primitive types. WebAssembly 1.0 defines two value-polymorphic
parametric instructions. The drop instruction ([t] -> []) discards the top value from the
operand stack regardless of whether it is an integer or floating-point number. The select
instruction ([t t i32] -> [t]) consumes an i32 boolean condition alongside two values
of matching type t, returning the first value if the condition is non-zero and the second
value otherwise. Unlike numeric instructions, parametric instructions use a single unified
instruction byte for all value types, leaving operand type inference to compile-time static
analysis [3].

**Variable and Memory Instructions.** State outside the operand stack is managed through
local and global variables. Local variables are declared within function signatures and bodies
and act as virtual registers. Function parameters and declared locals share a unified, zero-indexed local index space. Parameters are initialized with the incoming caller arguments,

<!-- source-pdf-page: 15 -->

while declared locals are automatically zero-initialized upon function entry. Global variables
are declared at the module level, in a separate index space, as either mutable or immutable.
Variable instructions access locals and globals using static index immediates:

- local.get idx ([] -> [t]): Pushes the value of a local onto the stack.
- local.set idx ([t] -> []): Pops the top stack value and writes it to a local.
- local.tee idx ([t] -> [t]): Writes a local while leaving the value on top of the stack
  for later use.
- global.get idx ([] -> [t]): Pushes the value of a global variable onto the stack.
- global.set idx ([t] -> []): Pops the top stack value and writes it to a global variable.
  Static validation enforces that this may only target a mutable global.

Memory instructions perform explicit load and store operations on linear memory. Load
instructions consume a dynamic base address from the stack and push the fetched value
([i32] -> [t]), while store instructions consume both a base address and a target value,
producing no result ([i32 t] -> []). Every memory instruction carries two static immediate
parameters: a byte offset and an alignment hint. The effective target address is computed
at runtime as address + offset, where address overflow or an out-of-bounds access triggers
an immediate trap. Load and store instructions exist for full 32-bit and 64-bit widths as
well as sub-widths (8-bit and 16-bit). Sub-width loads specify whether the loaded bytes
are sign-extended (\_s) or zero-extended (\_u) into the full stack value (e.g., i32.load8_s,
i32.load16_u), while sub-width stores truncate the stack value to the target byte size (e.g.,
i32.store8). All memory accesses use little-endian byte ordering. The alignment hint
indicates that the effective address is expected to be a multiple of $2^{\text{align}}$ bytes. Misaligned
addresses do not trap on hardware supporting misaligned access, but the hint still allows the
compilation backend to generate optimized access patterns. Memory capacity is inspected
and adjusted at runtime via memory.size ([] -> [i32]), returning the current footprint in 64
KiB pages, and memory.grow ([i32] -> [i32]), which attempts to allocate additional pages
and returns the previous page count, or −1 on failure [3].

**Structured Control Instructions.** Unlike traditional instruction sets, which allow jumping to
an arbitrary address, WebAssembly only lets control flow move between structured constructs:
block, loop, and if. Each of these can optionally produce a result, left on top of the stack
once it exits. Branch instructions (br, br_if, br_table) don’t target an address either. They
target one of the enclosing constructs instead, chosen by counting outward: label 0 is the
innermost one, label 1 the one enclosing that, and so on. What a branch actually does depends
on which kind of construct it targets. Branching to a block or an if jumps forward, to that
construct’s own end, leaving it early. Branching to a loop jumps backward instead, to the
loop’s own start, restarting it.

Taking a branch also unwinds the stack: any values pushed since the target construct was
entered are discarded, keeping only whatever the branch target’s own declared type expects.

<!-- source-pdf-page: 16 -->

A backward branch to a loop is a special case here, since WebAssembly 1.0 always gives it an
empty type. It restores the stack to exactly the height it had when the loop began, regardless
of whatever result the loop itself declares. That declared result only shows up when the loop
ends naturally, falling off its own end, never when a branch instruction restarts it instead. br,
br_table, return, and unreachable are all unconditional. Since whatever code follows one
of them in the same block can never actually run, the validator allows any stack shape go
through the type checking. [3].

The following example demonstrates nested control constructs, relative label indexing, and
stack unwinding:

Listing 2.1: WAT code computing ⌊log2 ( x )⌋ via repeated halving, demonstrating if, block,
loop, br_if, and br instructions

```wat
(module
  (func $example (param $x i32) (result i32)
    (local $n i32)
    local.get $x
    i32.const 1
    i32.lt_s
    if (result i32)
      i32.const 0 ;; log2 is undefined for $x <= 0; return 0 as a sentinel
    else
      block (result i32)
        loop ;; Label 0: loop
          local.get $n ;; Keep as the candidate result if we break
          local.get $x
          i32.const 1
          i32.le_s ;; $x <= 1 ?
          br_if 1 ;; If so, break out of block (label 1) with $n as its result
          local.get $x
          i32.const 1
          i32.shr_u ;; Halve $x
          local.set $x
          i32.const 1
          i32.add ;; Reuse the leftover $n from the stack to compute $n + 1
          local.set $n
          br 0 ;; Continue loop (label 0)
        end
        unreachable ;; Loop always exits via br_if or br 0, never falls through
      end ;; Label 1: block
    end
  )
  (export "example" (func $example))
)
```

<!-- source-pdf-page: 17 -->

**Call Instructions and Dynamic Dispatch.** Functions invoke one another using direct or
indirect call mechanics. Direct calls (call) take a static function index and consume the
target function’s arguments while pushing its result, a single value in WebAssebly 1.0
([t1*] -> [t2]). Indirect calls (call_indirect) implement dynamic dispatch through module tables, which in WebAssembly 1.0 exclusively store function references (funcref). The
instruction consumes a dynamic table index alongside the function arguments, pushing the
result ([t1* i32] -> [t2]). Because the target function is resolved dynamically at runtime,
call_indirect performs three checks before transferring control: the table index must be
within bounds, the referenced table entry must not be uninitialized, and the entry’s function
type must match the instruction’s declared type index. Any violation of one of these checks
causes a trap [3].

### 2.1.3 Module Architecture and Binary Layout

A WebAssembly binary is organized as a single module, which serves as the fundamental
unit of deployment, validation, and compilation. Aside from an optional custom section,
which may appear multiple times at arbitrary positions in the binary and carries auxiliary
data such as debug symbols, a module packages types, functions, tables, memories, and
globals into the following sections, each of which may appear at most once and in this fixed
relative order [3]:

- Type Section: Declares function type signatures (parameter and return types).
- Import Section: Specifies external functions, memories, tables, or globals supplied by
  the host environment.
- Function Section: Maps each function index to its corresponding signature in the type
  section.
- Table Section: Declares the module’s tables and their element bounds. WebAssembly 1.0 permits at most one table per module.
- Memory Section: Defines initial and maximum page limits for linear memory. WebAssembly 1.0 permits at most one memory per module.
- Global Section: Declares module-level global variables, their mutability, and initial
  values.
- Export Section: Lists functions, tables, memories, or globals exposed to the host system.
- Start Section: Identifies an optional initialization function executed automatically when
  the module loads.
- Element Section: Initializes table entries with function references at specified constant
  offsets.

<!-- source-pdf-page: 18 -->

- Code Section: Stores local variable declarations and WebAssembly instruction sequences
  for function bodies.
- Data Section: Defines static byte arrays used to initialize linear memory regions at
  specified offsets.

This fixed, non-repeating ordering allows a translator to process a module in a single linear
scan of the binary, section by section, without needing to look ahead or reparse earlier data.
This is a structural property the single-pass valent-block approach relies on.

## 2.2 RISC-V Architecture

RISC-V is an open, royalty-free instruction set architecture whose specification is maintained
by RISC-V International [4]. Its openness and lack of licensing fees have driven rapid
adoption across academic research, open hardware projects, and commercial silicon, including
embedded and automotive microcontrollers. The architecture is deliberately minimal at its
core: a small, mandatory base integer instruction set (RV32I, for 32-bit implementations)
defines the general-purpose registers, memory model, and fundamental arithmetic and
control-flow instructions, while additional capabilities such as multiplication, floating-point
arithmetic, or compressed instruction encoding are added through a modular system of
standard extensions that an implementation can select independently of one another. Beyond
the base integer ISA, we make use of the M (integer multiplication and division), F and D
(single- and double-precision floating-point), and Zbb (basic bit manipulation) extensions,
together with the compressed-instruction extensions Zca and Zcf, and Zcmp, which adds
dedicated compressed instructions for saving and restoring multiple registers at once.

**Registers.** The base integer ISA (RV32I) provides 32 general-purpose registers (x0 to x31),
each 32 bits wide, matching the architecture’s word size. Register x0 is hardwired to
the constant zero: writes to it are discarded, and reads always return zero, which allows
several common operations (such as an unconditional jump or a register-to-register move)
to be synthesized from ordinary instructions without dedicated opcodes. RISC-V draws no
architectural distinction between registers used for addresses and those used for data: any of
the 31 non-zero registers may hold either, and the same arithmetic and logical instructions
operate on both uniformly. Constants too large for a single instruction’s immediate field are
constructed in two instructions: lui loads a 20-bit value into the upper bits of a register, and
a following instruction such as addi adds in the remaining 12 bits.

RV32I provides no hardware support for integer values wider than 32 bits: a 64-bit integer
must be represented across a pair of general-purpose registers and manipulated through
explicit multi-instruction sequences, with no dedicated pairing convention linking the two
registers together the way some other architectures do. This extends to carry propagation:
RISC-V has no condition-code or flags register, so detecting whether a 32-bit addition
overflowed, in order to propagate a carry into the upper half of a 64-bit value, must itself
be synthesized from ordinary instructions, typically by comparing the sum against one of

<!-- source-pdf-page: 19 -->

the operands with an unsigned less-than instruction. Floating-point values, by contrast, are
held entirely separate from the integer register file. As detailed below, the F and D extensions
provide native hardware support for 32-bit and 64-bit floating-point values respectively, each
in its own dedicated register.

RISC-V likewise has no dedicated register for subroutine return addresses: the jal and
jalr instructions write the address of the following instruction to whichever general-purpose
register is specified as their destination operand, rather than to a fixed hardware register.
Similarly, conditional branches compare two general-purpose registers directly against each
other, rather than testing a flags register, which RISC-V does not have. More generally, jal
and jalr perform no register saving of any kind beyond writing the return address: there
is no hardware-assisted context switching triggered automatically by a call or return, so
which registers, if any, are preserved across a call is entirely a matter of software convention.
The Zcmp extension’s push/pop instructions narrow this gap somewhat, saving or restoring
several registers in one compressed instruction, but they still have to be explicitly inserted at
function entry and exit rather than being an implicit effect of the call and return instructions
themselves.

**Memory and Addressing.** RV32I has a 32-bit (4 GiB) address space. Implementations are
typically little-endian, matching WebAssembly’s linear memory. Loads and stores compute
their effective address as a base register plus a 12-bit signed immediate offset, the only
addressing mode the base ISA provides: there is no auto-increment or auto-decrement
mode that updates the base register as a side effect of the access, so a register used as a
stack or buffer pointer must be adjusted with a separate instruction before or after each
access. The specification does not mandate uniform handling of misaligned accesses: whether
a misaligned load or store executes transparently, traps, or faults is left to the execution
environment. The target used in this thesis resolves this by supporting misaligned accesses
transparently in hardware, rather than trapping on them.

As a load/store architecture, RV32I performs all computation on registers: memory is
accessed only through explicit load and store instructions, and the base ISA has no dedicated
instructions for pushing or popping values to and from a stack. Loads are available at 8-, 16-,
and 32-bit granularities, with sub-word loads specifying whether the loaded value is sign- or
zero-extended to fill the destination register. Stores are available at the same granularities
and simply truncate the register’s value to the target width, since no extension is needed
when writing to memory. As RV32I has no 64-bit load or store instructions, a 64-bit value
in memory must be transferred using two separate 32-bit accesses, one for each half. The F
and D extensions instead provide their own dedicated load and store instructions, moving
values directly between memory and the floating-point register file without passing through
the integer registers at all.

**Floating-Point.** The F extension adds 32 floating-point registers (f0 to f31), each usable
for IEEE-754 single-precision arithmetic, conversion, and comparison instructions. The D
extension does not add a separate register file: it widens the same 32 registers to 64 bits and

<!-- source-pdf-page: 20 -->

adds equivalent double-precision instructions, so any given register can hold either a single- or a double-precision value depending on which instruction accesses it, and both f32 and
f64 values are supported natively in hardware.

Each floating-point instruction specifies a rounding mode, either statically as part of
the instruction encoding or dynamically by reading a rounding-mode control and status
register. The default mode, round-to-nearest ties-to-even, matches the mode required by
WebAssembly’s specification. RISC-V’s NaN handling is fully deterministic: except where
otherwise stated, any floating-point operation whose result is NaN produces the canonical
NaN, a single fixed bit pattern with a positive sign and only the most significant fraction bit
set, regardless of the input operands’ own bit patterns. Load and store instructions are the
exception, transferring the exact bits held in memory without reinterpreting or canonicalizing
them.

One instruction pair does not follow this propagation rule: fmin and fmax implement the
IEEE 754-2019 minimumNumber/maximumNumber operations, under which a NaN operand is
suppressed rather than propagated whenever the other operand is a number, with NaN
only returned if both operands are NaN. This differs from WebAssembly’s own min and
max instructions, which, like its other floating-point operations, return NaN whenever either
operand is a NaN, so the two cannot be mapped onto one another directly.

**Instruction Format.** RV32I instructions are a fixed 32 bits wide, with the immediate operand
encoded differently depending on the instruction type: separate layouts exist for arithmetic
(12-bit), store (12-bit), branch (12-bit encoded, giving a 13-bit signed offset since the lowest
bit is always zero, for a ±4 KiB range), upper-immediate (20-bit), and jump (20-bit encoded,
giving a 21-bit signed offset for a ±1 MiB range) instructions.

A fixed 32-bit encoding is simple to decode but wastes space on instructions that do not
need the full width, such as common register moves, small constants, and function prologues.
The C extension addresses this with a compressed 16-bit encoding for a common subset of
instructions, freely interleavable with 32-bit ones. Since a compressed instruction takes half
the space its 32-bit equivalent would, this directly reduces a compiled module’s binary size,
and improves how much useful code a fixed-width instruction fetch can retrieve at once by
fitting more instructions into the same span of memory. Zca provides compressed encodings
for common base integer instructions, and Zcf separately provides compressed encodings for
the F extension’s own single-precision floating-point load and store instructions. Many of
Zca’s formats only have room for a 3-bit register field, so they can only address registers x8
through x15 rather than the full x0 to x31 range. Other Zca instructions use a full 5-bit field
instead and can address any register.

Zcmp goes further for one specific pattern: saving and restoring multiple registers at function
entry and exit. Its cm.push and cm.pop instructions each execute as a whole sequence of
ordinary stores or loads plus a stack pointer adjustment, compressed into a single 16-bit
instruction. An rlist field selects which registers to save from twelve predefined groupings
of the standard callee-saved registers, ranging from just the return address register alone up
to the return address plus all twelve saved registers, and a separate spimm field adjusts the

<!-- source-pdf-page: 21 -->

stack pointer by an additional multiple of 16 bytes in the same instruction.

A decoder distinguishes compressed from full-width encodings using only the low bits
of the first halfword: the instruction is 16 bits wide if they are not 11, and 32 bits otherwise.
This also affects instruction alignment: without the C extension, instructions must start on
a 4-byte boundary, but since the C extension allows 16- and 32-bit instructions to be freely
interleaved, it relaxes this to a 2-byte boundary instead.

Taken together, the absences noted throughout this section (no auto-increment addressing,
no dedicated stack instructions, no hardware-assisted register saving on a call) are not
unrelated omissions. They reflect a broader design principle: RISC-V’s specification favors a
small set of general-purpose mechanisms over specialized instructions for particular access
or control-flow patterns, leaving support for such patterns to be built up in software where
needed.
