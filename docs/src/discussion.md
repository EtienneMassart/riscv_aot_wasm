# 6 Discussion

<!-- source-pdf-page: 49 -->

Chapter 5 shows the core premise of on-target WebAssembly compilation holds on RISC-V:
our output runs an order of magnitude faster than MicroPython, several times faster than
Lua, and within a single-digit factor of native C, though at the cost of a somewhat larger
flashed image than either interpreter. That result leaves two kinds of open work behind it.
The first is performance and footprint, where several concrete, already-identified changes
would plausibly narrow the gaps Chapter 5 measured. The second is correctness under
conditions this thesis did not test, trapping, exhaustion, and a handful of range checks that
are currently either absent or inconsistently applied. We discuss both in turn, then close with
algorithmic directions from the valent-block paper that neither port has implemented and a
few structural gaps tied to RISC-V specifically.

Native C’s role in this comparison is a ceiling, not a competitor. It carries no runtime at all,
and static ahead-of-time compilation with a full optimizing backend will always outperform
a lightweight, single-pass, on-target translator, so the gap in Section 5.1 reflects that structural
difference rather than a shortfall in the valent-block approach itself. What C lacks for the
problem this thesis actually addresses is portability: a native build has to be recompiled and
reflashed for every target, exactly the deployment friction Chapter 1 sets out to avoid. The
real competition is the interpreted scripting engines, MicroPython and Lua, which share that
same portability, and against those our output is consistently and substantially faster.
## 6.1 Closing the Code-Size Gap
The code-size comparison in Section 5.1 is the least flattering result in this thesis: our kernel
output ranges from two and a half to nearly twenty-six times the size of the equivalent native
C function. A few compiler limitations, already identified independently of that measurement,
would plausibly narrow it.

The most direct is immediate-operand folding. When one operand of an arithmetic
instruction is an unresolved constant, the compiler currently still materializes it into a register
before emitting a register-register instruction, rather than folding it into a register-immediate
form the way addi allows. This was partly implemented for the TriCore port but not carried
over to RISC-V.

f32/f64 min/max are a smaller but concrete case: both go through the same out-of-line
helper-call mechanism used for operations with no direct RISC-V translation, purely to
patch over the mismatch between RISC-V’s NaN-suppressing fmin/fmax and WASM’s NaN-propagating min/max. The fix-up needed is small enough to inline directly at the call site,
which would remove a full call and return for an operation that appears often in floating-

<!-- source-pdf-page: 50 -->

point-heavy code like the benchmarks in Chapter 5.

Freeing more of the seven permanently reserved general-purpose registers, discussed below,
would also help indirectly: more registers available to locals and scratch means fewer spills,
and every spill costs both cycles and code size.

Section 5.3 points at a second, narrower target: br_table dispatch. It is currently a linear
scan of equality comparisons, each costing roughly two instructions since the case value has
to be materialized into a register first. A jump table was already built for the TriCore port but
never carried over, in favor of the simpler linear scan, which reuses the same placeholder-and-fixup mechanism every other branch already relies on. Past a small number of cases a
jump table plausibly costs less code as well as fewer cycles, not a size-for-speed tradeoff at all,
though this has not been benchmarked directly.

The compiler’s own footprint, measured in Section 5.2, raises a separate question from the
kernel-size gap above. Being larger than MicroPython and Lua is expected given the AOT
approach itself, not a fixed cost we are stuck with: some of that size is plausibly recoverable
through ordinary cleanup, distinct from anything discussed here, though by how much is not
something this thesis attempts to estimate.
## 6.2 Register Allocation and ISA Usage
Chapter 4 reserves seven general-purpose registers for fixed roles: the return address, the
stack pointer, the frame pointer, and four base addresses, for WASM globals, linear memory,
the table, and the stack’s own base, the last of which exists only to reconstruct the frame
pointer after a call. This is a structural legacy of TriCore, where address registers were a
separate register file from data registers, so dedicating one to each fixed address cost nothing
against the budget actually used for computation. On RISC-V’s single unified register file
that is no longer true: these compete directly with locals and scratch, and Section 5.4 shows
performance is sensitive to exactly that budget.

Not all seven carry equal weight, though. The linear-memory base is touched by nearly
every WASM memory access, so keeping it permanently resident clearly earns its place. The
other three base addresses are a different story: the stack base only exists to rebuild the
frame pointer after a call, the table base is only needed by call_indirect, and the global
base only by global.get/global.set, all comparatively rare next to ordinary arithmetic and
local access. Loading any of these three on demand instead of keeping them permanently
resident would trade a small, occasional performance cost for three registers permanently
available to locals and scratch instead. Whether that tradeoff is worth taking depends on
how rare those operations actually are in practice, which this thesis does not measure, but
the asymmetry between the memory base and the other three is itself informative: not every
fixed-role register earns its permanent reservation equally, and TriCore’s register-file split
made that distinction free to ignore.

The helper-call mechanism used for the handful of WASM operations with no direct
RISC-V translation, mostly float-to-i64 conversions, is a maintainability compromise more
than a performance one. It moves arguments into four fixed scratch registers, the same ones

<!-- source-pdf-page: 51 -->

call_indirect’s type check and memory.grow borrow when they need working space, uses
Zcmp’s cm.push to save a few more and free up room, calls a hand-written naked-assembly
routine, then cm.pop to restore. Naked assembly means none of the usual guarantees a
compiler gives around register usage or calling convention apply here: the routine and its
caller agree on exactly which registers hold what by convention alone, checked by nothing.
It works, and Section 6.1 already covers one case, min/max, where this mechanism is used
for something small enough that it should not need an out-of-line call at all. But even for
the operations that genuinely need it, a cleaner design would be easier to extend and audit:
a conservative routine that saves everything a callee could need, followed by a proper call
under the standard C ABI, at the cost of saving some registers unnecessarily on each call.

These same four registers are unavailable to ordinary allocation for a function’s entire
body, not just while a helper call is actually in flight, purely so they are guaranteed free
whenever one is needed. An alternative would treat them like ordinary scratch instead: let
the allocator use them freely, and spill whatever they currently hold to memory only at the
point a helper call actually needs them, the same on-demand spilling already used to free
floating-point registers elsewhere. This only matters under the current mechanism, though. If
the helper-call mechanism above were replaced with a proper call under the standard C ABI,
the calling convention itself would already decide what gets saved and where, and a dedicated
spill-on-demand scheme for these four registers specifically would become unnecessary.

x31 is a related but separate case: it exists solely to save the runtime’s own stack pointer
once, immediately before the single call from the runtime into compiled code, and restore it
once after the compiled function returns, untouched for the entire function body in between.
It sits outside both the four registers above and the seven fixed-role registers discussed earlier,
a fifth kind of special-purpose reservation alongside them. Restoring it through some other
means at that single entry and exit point instead, either by computing it directly or by pushing
it to the stack alongside everything else the prologue already saves, would remove the need
for a register dedicated to it at all.
## 6.3 Algorithmic Directions Not Yet Explored
Two ideas from Scheidl’s original valent-block paper [1] were never implemented by either
the TriCore port or this one. The paper proposes an offboard pass that ranks local variables
by hotness or lifetime so the on-target compiler can greedily assign registers in that order.
Both ports instead use the simpler, workload-tuned fixed budget described in Chapter 4 and
measured in Section 5.4. Whether hotness-based allocation would outperform that fixed
budget, and by how much, is untested.

The second is fused multiply-add: when resolving a floating-point multiplication whose
child is itself an unresolved addition, the two could in principle be replaced by a single
ternary valent-block resolving directly to one fmadd instruction instead of separate multiply
and add code. WASM 1.0 requires mul and add to be rounded separately per IEEE 754, so any
implementation of this would need to preserve that rounding behavior to stay spec-compliant.
That is why it was not attempted here.

<!-- source-pdf-page: 52 -->

## 6.4 Architectural Gaps
Two gaps follow directly from moving to RISC-V rather than from the compiler’s own design.
Zifencei is needed for instruction-cache coherency after writing freshly compiled code to
memory, but is not used by this port. It works on the current target without it, but that is a
property of this specific implementation, not something the architecture guarantees elsewhere.
Address masking, implemented for the TriCore port, has only a partial basis in place for
RISC-V and needs substantially more work to match.

A third architectural gap is the range limit on branch and jump displacements, a genuine
limit rather than a missing feature. Every branch and jump starts out as a placeholder, patched
with its real displacement once the target address is known, by the fixup pass described in
Chapter 4’s account of why branches and jumps are excluded from automatic compression.
That pass panics outright if the real displacement does not fit the instruction’s immediate
field: roughly four kilobytes for BEQ/BNE, one megabyte for JAL. The fixup only ever spans
a single WASM function, never the whole module, and has not been checked against real
function sizes, so whether this is a realistic risk or a purely theoretical one is unclear.

Scheidl’s paper describes a fix with the right shape for this: fall back to an indirect jump
through a table whenever the direct form cannot reach. Its own trigger for that fallback is
different from ours, a flash page already committed and no longer directly editable, rather
than an instruction encoding that is simply too narrow, but the mechanism itself would apply
equally well to either cause. No equivalent fallback exists here: today the fixup pass either
succeeds or panics.
## 6.5 Correctness Under Conditions Not Yet Tested
Trap handling is not implemented in this port, a scope decision stated throughout this thesis.
Two concrete consequences follow. RISC-V’s floating-point-to-integer conversion instructions
saturate on out-of-range or NaN input, where WASM’s equivalent instructions are specified
to trap instead. And nothing in the translator currently checks call depth or remaining stack
space during recursion, so deep or unbounded recursion consumes stack without a trap of
our own raising it first, relying on whatever guard the surrounding deployment provides. The
frame mechanics underneath both cases are sound: stack usage grows exactly as recursion
requires, so a runaway recursive workload behaves predictably even without a dedicated trap,
it just does not get the clean, spec-defined trap WASM programs may expect.

Finally, the forward-branch placeholder mechanism itself has only ever been exercised on a
simulator: it writes a zeroed target field and patches the real address in later. On real NOR
flash, patching a zeroed field with a real, generally non-zero address is close to the worst
case for that memory technology, since flash can only flip bits from one to zero without a full
erase cycle. Whether this behaves correctly on actual hardware is untested, a limitation this
port shares with the TriCore compiler it originates from.

<!-- source-pdf-page: 53 -->

## 6.6 Evaluation Coverage
PolyBench/C’s four kernels are dense numerical loops, chosen for direct comparability
with Scheidl’s paper and the TriCore port (Chapter 1), not because they resemble what this
compiler would actually run in an automotive deployment. Sensor polling, telemetry parsing,
and small event-driven routines look nothing like matrix-style numerical kernels in their
control flow, memory access patterns, or function-call frequency. Testing against real payloads
like these instead would validate the register budget, the branch-dispatch strategy, and the
compression mechanism examined throughout this thesis under the conditions that actually
motivate the approach, rather than under a general-purpose numerical proxy for them.
## 6.7 Conclusion
This thesis ports the valent-block ahead-of-time compilation approach from TriCore to RISC-V.
The result, measured in Chapter 5, runs an order of magnitude faster than MicroPython,
several times faster than Lua, and within a single-digit factor of native C, the appropriate
ceiling for this comparison rather than its target. That speed comes at a real cost: a larger
flashed image than either interpreter, and a handful of concrete size and correctness gaps this
chapter names but does not fix, most notably trap handling, left out of scope entirely. None of
these are architectural dead ends. The register-budget tradeoffs, the compression mechanism,
and the branch-dispatch strategy examined throughout this thesis all carry over cleanly to
whatever comes next, whether that means closing the gaps named here or extending the
compiler to a wider set of WASM features. Most importantly, this thesis demonstrates that
this lightweight, single-pass, on-target approach to compiling WebAssembly is not tied to the
architecture it was first built for, and that RISC-V is a fully viable target for it.
