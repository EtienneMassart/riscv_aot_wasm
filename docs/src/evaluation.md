# 5 Evaluation

<!-- source-pdf-page: 43 -->

We evaluate the RISC-V port on the same four PolyBench/C 4.2.1 kernels introduced in
Chapter 1: jacobi-1d and correlation for single-precision floating-point performance, floyd-warshall and nussinov for integer performance. These four benchmarks are the same ones
used to evaluate the original valent-block paper [1] and the TriCore port before it [5], both
drawn from PolyBench/C, a widely used numerical-kernel suite, and we follow that choice
for direct comparability. Like the TriCore port, we use PolyBench’s MINI dataset, its smallest
available problem size.

The MicroPython and Lua versions of each kernel are produced by a pair of deterministic,
rule-based source-to-source translators built on the same underlying tooling: the system C
preprocessor plus pycparser, walking the parsed AST with a fixed set of rewrite rules rather
than reinterpreting the algorithm. Given the same input, each always produces the same
output, and neither introduces loop restructuring, fusion, or any other optimization beyond
what each language’s own runtime does internally. The interpreter comparison therefore
reflects the same computation as the C and WASM versions, not an independently rewritten
one.

All cycle counts are measured on internal available simulator, simulating the riscv32imafd-ifx-none-elf
target, an Infineon-configured RV32IMAFD core built through the internal toolchain.
The native C baseline is compiled with `-O3`. The WASM modules
are the same ones used for the TriCore port’s own evaluation [5], compiled via Emscripten
at `-O3`, and are translated to RISC-V by our compiler unchanged. `-O3` is unproblematic for
the C baseline specifically because it compiles to just the kernel itself, with no surrounding
engine to keep within a size budget, and stays small regardless of optimization level. Our
own runtime, MicroPython, and Lua, the three engines the kernels actually run under, are a
different matter: each ships as a whole interpreter or runtime on the target, so we build all
three for size rather than speed instead. Our own runtime uses rustc’s opt-level = "z", the
more aggressive of its two size settings, roughly equivalent to Clang’s `-Oz`. We initially built
MicroPython with ccac at both `-Os` and `-Oz`, for toolchain consistency with the C baseline, but
both were bigger and slower than the same source built with gcc, which only offers `-Os` in the
first place. We benchmark the gcc `-Os` build instead, and did not repeat the ccac attempt for
Lua given that result. Unless a section says otherwise, all results use the compiler’s default
configuration: instruction compression enabled, a budget of twenty general-purpose and
sixteen floating-point local registers.

<!-- source-pdf-page: 44 -->

## 5.1 Execution Time and Code Size

Table 5.1 gives the raw cycle count for each benchmark’s kernel under our compiler, MicroPython, Lua, and native C. Table 5.2 restates the same data as ratios: our overhead against native
C, and our speedup over each interpreter.

**Table 5.1: Kernel execution cycles by engine, measured on internal simulator.**

| Benchmark      | AOT WASM   | MicroPython | Lua         | Native C  |
| -------------- | ---------- | ----------- | ----------- | --------- |
| jacobi-1d      | 53,806     | 2,210,722   | 426,758     | 18,246    |
| correlation    | 718,327    | 42,490,515  | 6,615,609   | 184,719   |
| floyd-warshall | 24,837,737 | 413,461,774 | 107,402,881 | 6,020,027 |
| nussinov       | 6,124,333  | 78,139,735  | 15,002,522  | 886,818   |

**Table 5.2: Relative performance: overhead against native C, speedup over each interpreter.**

| Benchmark      | Slowdown vs. C | Speedup vs. MicroPython | Speedup vs. Lua |
| -------------- | -------------- | ----------------------- | --------------- |
| jacobi-1d      | 2.95×          | 41.1×                   | 7.9×            |
| correlation    | 3.89×          | 59.2×                   | 9.2×            |
| floyd-warshall | 4.13×          | 16.6×                   | 4.3×            |
| nussinov       | 6.91×          | 12.8×                   | 2.5×            |

Across all four benchmarks, our AOT-compiled WASM consistently and substantially
outperforms both interpreters, between twelve and fifty-nine times faster than MicroPython,
and between two and nine times faster than Lua. Overhead against native C stays within
a factor of three to seven, closest for jacobi-1d and furthest for nussinov. The two integer
benchmarks (floyd-warshall, nussinov) sit at a higher overhead than the two floating-point ones,
though with only two benchmarks per category this is a suggestive pattern rather than a
conclusive one.

Table 5.3 compares kernel code size (the function body alone, not the surrounding runtime)
between our output and native C.

**Table 5.3: Kernel code size in bytes, our output versus native C.**

| Benchmark      | AOT WASM | Native C | AOT / C |
| -------------- | -------- | -------- | ------- |
| jacobi-1d      | 3,706    | 144      | 25.74×  |
| correlation    | 4,636    | 488      | 9.50×   |
| floyd-warshall | 302      | 122      | 2.48×   |
| nussinov       | 926      | 314      | 2.95×   |

Here the gap is much larger than for execution time, and much less consistent: our kernel
code is between two and a half and nearly twenty-six times the size of the equivalent native

<!-- source-pdf-page: 45 -->

function, with jacobi-1d by far the worst case. We have not profiled which specific instructions
account for this gap, so we only note it plainly here rather than attribute it to a specific cause.
Chapter 6 lists several already-identified compiler limitations (unfolded immediate operands,
the fixed local register budget) that would plausibly narrow it.

## 5.2 Total Compiler and Runtime Footprint

Table 5.4 compares the .text section size of our AOT compiler and its supporting runtime
against the MicroPython and Lua interpreter binaries. We compare .text specifically rather
than the whole flashed image because the rest largely reflects statically allocated arrays, linear
memory among them, and their size is a parameter the runtime’s user sets, not a property
of the runtime itself, so including it would make the comparison depend on an arbitrary
choice rather than on the code we actually wrote. The compiled kernel itself is not part of
this figure: our translator generates its machine code at runtime rather than linking it in
statically, so .text reflects the engine alone, the same regardless of which benchmark is run.
MicroPython and Lua are in the same position, since they interpret the benchmark script
rather than compiling it into their own .text.

**Table 5.4: Flashed `.text` section size by engine.**

| Engine                        | Size (bytes) |
| ----------------------------- | ------------ |
| AOT WASM (compiler + runtime) | 197,674      |
| MicroPython                   | 128,238      |
| Lua                           | 136,770      |

Our image is roughly 1.5 times the size of either interpreter. This is a structural consequence
of the AOT approach rather than an incidental inefficiency: we ship the translator itself on
the target, not just a runtime for already-compiled code, while MicroPython and Lua ship
only their interpreter loop and standard library. The compiler itself accounts for most of that
figure, not the supporting runtime around it, since MicroPython and Lua never need anything
like a compiler on the target at all. On a platform where flash is the binding constraint, this
fixed cost is a genuine tradeoff against the per-kernel execution speed shown above, not one
we can resolve away without abandoning on-target compilation entirely.

## 5.3 Effect of Instruction Compression

Chapter 4 describes automatic compression of eligible standard-width instructions into their
16-bit Zca equivalents. We isolated its effect by disabling the compression pass entirely at
emission and rerunning all four benchmarks unchanged otherwise. Table 5.5 gives cycles and
code size with and without it.

The size improvement (13% to 24%) is unsurprising: that is exactly what compression is
for. The cycle improvement is the more interesting result. Instruction count is identical in

<!-- source-pdf-page: 46 -->

**Table 5.5: Cycles and code size with and without instruction compression.**

| Benchmark      | Cycles impr. | Size impr. | Cycles (on) | Size (on) |
| -------------- | ------------ | ---------- | ----------- | --------- |
| jacobi-1d      | 0.53%        | 13.41%     | 53,806      | 3,706     |
| correlation    | 11.90%       | 12.79%     | 718,327     | 4,636     |
| floyd-warshall | 9.93%        | 23.74%     | 24,837,737  | 302       |
| nussinov       | 9.73%        | 13.94%     | 6,124,333   | 926       |

both configurations, compression only changes instruction width, and yet three of the four
benchmarks still run measurably faster with it enabled, up to almost 12% for correlation.
jacobi-1d is the outlier, with a negligible 0.5% cycle improvement despite a substantial size
reduction. A plausible explanation is reduced instruction-fetch pressure: denser code needs
fewer fetch-width reads to pull the same instruction stream off memory. We have not verified
this against the actual memory or fetch model of internal available simulator, though, so we offer it only as the most
likely explanation, not a confirmed one. This result also illustrates why branches and jumps
are deliberately excluded from this mechanism (Chapter 4). The benefit compression provides
here is real, so the range limitation that rules it out for branches is a genuine cost, not a
hypothetical one.

## 5.4 Register Budget Sensitivity

Chapter 4 states that the local register budget was tuned empirically rather than derived,
since the best value is workload-dependent. By default, MAX_LOCAL_REGISTERS is twenty
and MAX_LOCAL_FP_REGISTERS is sixteen. General-purpose allocation starts at x8 rather than
x0, since x0 through x7 are reserved (Chapter 4), so the actual usable count is eight less
than MAX_LOCAL_REGISTERS itself, twelve registers at the default value of twenty. Floating-point allocation starts at f0 with no such offset, so MAX_LOCAL_FP_REGISTERS already is the
usable count, sixteen by default. The general-purpose registers above the locals pool are
not all general scratch either: five of them, x27 through x31, are carved out for helper calls,
call_indirect’s type check, memory.grow, and the one-time save of the runtime’s own stack
pointer on entry (Chapter 4), leaving the rest as the actual scratch pool valent-block resolution
draws from. Table 5.6 and Table 5.7 give the underlying sweep data, labeled by each constant’s
own swept value.

The floating-point benchmarks (jacobi-1d, correlation) are essentially flat across the whole
swept range, twelve to twenty-two: the general-purpose budget barely matters for them. That
is expected, and a useful sanity check that the sweep isolates the right variable. floyd-warshall
is highly sensitive: cycles roughly halve from budget 12 to budget 19, then plateau, with no
further gain out to 22. nussinov is also sensitive and still improving slightly at 22, the top of
the range we tested, so 20 may sit slightly below its true optimum, though close to it.

Here the two integer benchmarks are flat as expected, again a sanity check, but the two
floating-point benchmarks pull in opposite directions. jacobi-1d improves monotonically as

<!-- source-pdf-page: 47 -->

**Table 5.6: Cycles by MAX_LOCAL_REGISTERS (general-purpose local budget, usable registers are
eight less than the value shown).**

| MAX_LOCAL_REGISTERS | jacobi-1d | correlation | floyd-warshall | nussinov  |
| ------------------- | --------- | ----------- | -------------- | --------- |
| 12                  | 53,799    | 735,678     | 50,698,952     | 6,434,828 |
| 14                  | 53,821    | 719,280     | 45,876,636     | 7,181,960 |
| 16                  | 53,824    | 717,646     | 31,008,401     | 6,921,281 |
| 18                  | 53,813    | 718,379     | 26,830,166     | 6,131,240 |
| 20                  | 53,806    | 718,327     | 24,837,737     | 6,124,333 |
| 22                  | 53,745    | 718,155     | 24,838,387     | 6,046,629 |

**Table 5.7: Cycles by MAX_LOCAL_FP_REGISTERS (floating-point local budget, equal to the usable
register count).**

| MAX_LOCAL_FP_REGISTERS | jacobi-1d | correlation | floyd-warshall | nussinov  |
| ---------------------- | --------- | ----------- | -------------- | --------- |
| 4                      | 58,595    | 689,293     | 24,836,473     | 6,124,289 |
| 8                      | 56,756    | 694,478     | 24,838,539     | 6,124,631 |
| 12                     | 55,206    | 702,552     | 24,840,350     | 6,124,680 |
| 16                     | 53,806    | 718,327     | 24,837,737     | 6,124,333 |
| 20                     | 52,192    | 725,718     | 24,838,283     | 6,123,035 |
| 24                     | 51,115    | 749,453     | 24,840,856     | 6,126,107 |
| 28                     | 49,991    | 741,549     | 24,837,793     | 6,124,607 |

<!-- source-pdf-page: 48 -->

the floating-point budget grows, still improving at 28, the top of the tested range, so it would
likely benefit from an even larger budget than that. correlation does the opposite: it is best
at the smallest budget tested (4) and gets steadily worse as the budget grows, worst around 24. The likely explanation is that a larger local budget takes registers away from the scratch
pool valent-block resolution depends on (Chapter 4), so correlation loses more from additional
scratch spilling than it gains from having more locals in registers. This has not been confirmed
by further profiling.

jacobi-1d wants a larger floating-point budget, correlation wants a smaller one, and this
conflict is measured directly rather than assumed. Sixteen sits as a compromise between the
two. Four benchmarks are enough to demonstrate that this tradeoff is real and to justify a
defensible compromise value, but not enough to claim sixteen is a true optimum. That would
need a much larger and more representative benchmark suite to average over.
