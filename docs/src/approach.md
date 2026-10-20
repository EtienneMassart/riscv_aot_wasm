# 3 Approach

<!-- source-pdf-page: 22 -->

## 3.1 The Valent-Block Method
Translating WebAssembly’s stack-based instruction set directly to a register-based target
is straightforward but slow: a naive translator can simply materialize the operand stack
in memory, pushing and popping every intermediate value exactly as the WebAssembly
bytecode specifies. The valent-block approach avoids this by building on Google’s V8 Liftoff
baseline compiler, which keeps operand locations on a compile-time virtual stack instead
of a physical one, delaying code generation for a value until the instruction that actually
needs it is reached. It generalizes this from tracking individual locations to recording whole
expression trees, so that entire chains of side-effect-free computation can be resolved into
machine code at once rather than instruction by instruction. The performance difference is
substantial: for a recursive Fibonacci benchmark on x86-64, a non-optimizing translator that
materializes every value on a real runtime stack took 9.03 seconds, valent-block compilation
took 1.98 seconds, and native compilation via Clang took 1.82 seconds, within 9% of native
and more than four times faster than the naive baseline [1].
### 3.1.1 Provenance
The valent-block method originates with Scheidl’s paper [1], which describes the algorithm
but provides no implementation. It classifies instructions by valence and side effects, and
lazily resolves expression trees into machine code. Its first implementation was developed
internally at Infineon for the TriCore architecture, by a team the author of this thesis was also
part of prior to this thesis. That implementation includes the tree-structured intermediate
representation, the visitor-based construction of valent-blocks, and the tree-zipper-based
resolution mechanism described below [5].
### 3.1.2 Classifying Instructions
The valent-block approach classifies WebAssembly instructions along two independent properties. The first is an instruction’s valence, the number of stack elements it consumes: i32.add
has a valence of two, while i32.sqrt has a valence of one. Some instructions have a valence
of zero: i32.const and other constant instructions, as well as local.get and global.get,
consume nothing and simply push a new leaf onto the virtual stack. select is the only
WebAssembly instruction with a valence of three. The second is whether an instruction has
side effects beyond the operand stack. Numeric instructions, conversions, and the parametric
instructions drop and select only affect the stack and are side-effect free. Control-flow

<!-- source-pdf-page: 23 -->

instructions such as br, br_if, and call, memory-modifying instructions, and instructions
that write to a local or global variable all have effects beyond the stack and are classified as
side-effecting [1]. Together, the two properties determine what happens when an instruction
is parsed: side-effect status decides whether it triggers resolution at all, and valence decides
how many valent-blocks that resolution consumes.
### 3.1.3 Constructing and Resolving Valent-Blocks
Side-effect-free instructions do not immediately produce machine code. The valent-block
stack is a virtual stand-in for what a naive translator’s real operand stack would hold: at each
position where a naive translator would keep a concrete value, we instead keep a tree of the
operations still needed to compute it. Each side-effect-free instruction pops the number of
elements matching its valence off this virtual stack, combines them with itself into a single
node representing that unresolved expression, and pushes the resulting node, a valent-block,
back on. The stack can hold several such valent-blocks side by side, not just one: an earlier
expression may already have collapsed into its own block while later instructions push fresh
values or build another block on top of it, so the compile-time stack is generally a small forest
of independent trees rather than a single tree.

Only when a side-effecting instruction is encountered is anything resolved, and how much
depends on the instruction. An instruction like local.set has a valence of just one, so it
only resolves the single valent-block on top of the stack to store its value in the local space,
whatever else is sitting further down is left untouched and still unresolved. Branching and
entering or leaving a block are different: these are exactly the points where we need a clean,
fully materialized snapshot of every value in scope, not a mix of concrete locations and
pending trees, so we have to resolve the entire stack of valent-blocks. In either case, resolution
walks the tree depth-first, resolving each valent-block only once its children are available, all
the way down to the leaves, and working back up toward the block that triggered it. Each
resolved block is replaced with a single value representing where its result now lives, in
a register or on the runtime stack. This laziness is what avoids materializing the operand
stack: a chain of purely arithmetic instructions compiles directly into the machine code that
computes their combined result, without ever writing an intermediate value to memory only
to immediately read it back [1]. It also means we already know where a result needs to end
up by the time we emit any code for it, so that code can be generated directly into the right
register or stack slot instead of computing into a scratch location and then moving the value
into place.

The following function computes ( a + b) × ( a − b) and illustrates both how valent-blocks
nest and how several of them can sit on the stack at once:

Listing 3.1: A WAT function whose result requires resolving two nested valent-blocks

```wat
(func $diffprod (param $a i32) (param $b i32) (result i32)
  local.get $a ;; pushes a leaf valent-block: $a
  local.get $b ;; pushes a leaf valent-block: $b
  i32.add ;; combines them into one valent-block: (a + b)

  local.get $a
  local.get $b
  i32.sub ;; combined into a second valent-block: (a - b)
  i32.mul ;; both blocks become children of a third
  return ;; side-effecting: triggers resolution
)
```

<!-- source-pdf-page: 24 -->

![Figure 3.1: The virtual stack of valent-blocks across three steps of `diffprod`](images/figure-3-1-valent-block-stack.svg)
*Figure 3.1: The virtual stack of valent-blocks across three steps of `diffprod`.*

Up to i32.mul, nothing is emitted: even local.get $a and local.get $b are themselves
valence-zero instructions that construct their own leaf valent-blocks, rather than simply being
values that are already there. i32.add and i32.sub then each combine two such leaves into
their own valent-block. Figure 3.1 shows all three steps: after i32.sub, the stack briefly holds
these two blocks side by side, a forest of two independent trees, and only i32.mul combines
them into the single tree shown in the last panel. Only return is side-effecting, and resolving
the i32.mul block it triggers first requires resolving its two children: machine code for a + b
is emitted first, then machine code for a − b, each leaving its result in a register. Only then
is the multiplication itself emitted, using those two registers as operands, directly into the
register return expects its result in, since we already know by then that this value is what the
function returns, rather than into a scratch register that would then have to be moved. Every
step from i32.add to i32.mul costs nothing but stack bookkeeping. The entire expression is
compiled in one pass over three instructions’ worth of machine code, triggered by a single
side-effecting instruction at the end.
### 3.1.4 Concurrent Validation
Resolving valent-blocks requires tracking exactly the same information WebAssembly’s
own validation algorithm needs: the type and number of values a valent-block’s contained
instructions produce and consume. If a resolution ever finds a type mismatch, a stack
underflow, or a wrong number of return values, the module is malformed and translation
aborts [1]. In our implementation, that validation is performed by the wasmparser crate
itself, not by us: its Parser emits a sequence of payloads as it reads through the binary, and
its Validator confirms each one is well-typed as it arrives [6]. Through the visitor pattern

<!-- source-pdf-page: 25 -->

described in the next section, we consume that same validated payload stream to emit code,
so validation and code generation happen together rather than as two separate passes.
### 3.1.5 Branches
WebAssembly selects a branch’s target with a relative label index rather than an address: br 0
targets the block the branch is directly inside of, br 1 targets the next one out from there, and
so on. Resolving that target differs depending on what kind of instruction it is. A backward
branch to the start of a loop is trivial: the loop’s starting offset in the emitted machine code is
already known by the time a branch to it is parsed, since the loop was necessarily emitted
first. As discussed in Chapter 2, a backward branch to a loop always targets an empty type in
WebAssembly 1.0. Resolving it therefore discards whatever valent-blocks are still pending
above the loop’s entry height, restoring the stack to exactly that height rather than keeping
any of them. Resolving the loop’s own natural end is different, since it may need to preserve
a declared result instead. A forward branch to the end of a block is harder, since the target
address is not yet known when the branch itself is compiled.

br_table selects one of several branch targets at runtime, based on an index popped from
the stack, falling back to a mandatory default target if the index matches none of the cases.
Each individual target it can select is a branch like any other, and is resolved the same way,
trivially if backward, or once its address becomes known if forward.

An unconditional branch (br, br_table, return, or unreachable) leaves the rest of its
enclosing block provably unreachable, the same stack-polymorphism property discussed in
Chapter 2. We act on that directly: once an unconditional branch has been translated, the
remaining instructions in that block are treated as dead code and are not translated at all,
rather than being compiled into machine code that can never execute.
## 3.2 Implementation
### 3.2.1 Construction and Resolution of Valent-Blocks
Parsing uses the wasmparser crate’s visitor pattern [6], a trait with one method per WebAssembly instruction that must be implemented exhaustively, even for instructions that end
up trivial. A side-effect-free instruction’s method pops its child valent-blocks off the virtual
stack, attaches them to a new valent-block node, and pushes that node back, emitting nothing.
A side-effecting instruction’s method instead resolves the valent-blocks it needs and emits
machine code directly, within that same visit.

Resolving a valent-block always proceeds in the same order: from the node currently
being resolved, descend into its leftmost unresolved child, and keep descending into leftmost
children until a leaf is reached. Once a node’s result is known, resolution returns to its parent,
whether that result came from directly reading a leaf or from a subtree whose code has just
been emitted. If that parent still has an unresolved child, resolution descends into that child’s
own leftmost descendants next. Otherwise, it continues up to the grandparent instead. This
depth-first, leaves-first walk also needs to mutate the tree as it goes: each time a child is

<!-- source-pdf-page: 26 -->

resolved, it must be replaced in its parent by the location where its result now lives, a register
or a runtime stack slot.

A recursive implementation gets this traversal order for free: each call descends into one
child, and returning from it is exactly what moves resolution back up to the parent. But we
run directly on the microcontroller we target rather than on a desktop host, so stack memory
is scarce, and a call depth proportional to how deeply an expression is nested is a real cost,
not just an abstract inefficiency.

Replacing the recursion with an explicit loop avoids that cost, but only shifts it: something
still has to remember the path back to the root so it can climb back up and rewrite each
parent once its child is resolved, and in Rust that means holding a mutable reference to every
ancestor still on that path. A recursive call gets this safely for free too, since each child’s
reference is dropped before its parent’s is used again, but an explicit loop needs several such
references alive at once, one per level still open on the path, exactly the kind of simultaneous
mutable access Rust does not allow.

We instead implement resolution as a tree zipper [5], an adaptation of Huet’s zipper [7]. It
generalizes the more familiar list zipper, a cursor that moves back and forth through a list,
to trees. A tree zipper is centered on the node currently being visited, its focus, and carries
a path of breadcrumbs alongside it, one per ancestor between the focus and the root, each
recording which child position the focus descended from and what that ancestor’s other,
still-unvisited subtree was.

For example, resolving i32.mul in Figure 3.1 begins with the focus on that node itself.
Moving into its first child, i32.add, leaves one breadcrumb behind: it records that the focus
descended from i32.mul’s left position, and that its other child, the i32.sub subtree, is still
waiting to be visited. Each further step down adds one more breadcrumb the same way, and
each step back up consumes the most recent breadcrumb to reassemble the parent around the
child just resolved, continuing outward one breadcrumb at a time until the whole expression
is resolved.

Moving into a child, resolving it, and moving back out are therefore all local operations
on this breadcrumb path: each step takes ownership of the current zipper and produces a
new one in its place, rather than mutating the tree through a live reference, avoiding the very
problem recursion and manual iteration ran into above. Both the visitor-based construction
described above and this tree-zipper-based resolution are unchanged from the existing TriCore
compiler [5].
### 3.2.2 Branches
We turn the relative label indices described above into actual addresses using a stack of
currently open block, loop, and if labels, each carrying a label index that is unique across
the whole function. A branch’s relative index maps directly onto a position in this stack: br
0 targets the label at the very top, the block the branch itself sits inside of, and br n targets
the label n positions below that.

Alongside each label, we also store the height the stack should be unwound to once that
label is reached. A block’s or an if’s label carries just one such height, its own declared result,

<!-- source-pdf-page: 27 -->

but a loop’s carries two, since a backward branch and a fall-through to its natural end do not
target the same height. As established above, a backward branch to a loop always restores the
stack to the height it had when the loop was entered, discarding any declared result, while
falling off the loop’s own end naturally uses the other height instead, the one that accounts
for that result. A branch consults whichever of these two heights its target label actually
needs before unwinding to it.

We defer every branch to a second pass this way, backward and forward alike, rather than
resolving backward branches immediately and only deferring forward ones. Unlike a block’s
or an if’s, a loop’s own address is in fact already available by the time a backward branch
to it is compiled, but treating both cases identically keeps this part of our compiler to a
single mechanism instead of two. For every branch, we record the target label’s index and
the address of the branch or jump instruction itself as a pair in a list of pending jumps, and
emit it as a placeholder with its target field left blank. Separately, each label’s own address is
recorded into a map from its index to that address as soon as it becomes known: a loop’s the
moment it is entered, since a backward branch targets its start, and a block’s or an if’s once it
closes, since a forward branch targets its end. Once the function has been fully compiled, a
short fixup pass walks the list of pending jumps, looks up each one’s target label in that map
to find its real address, and patches the corresponding placeholder instruction with it. This is
the only part of the scheme that happens outside the single streaming pass, but it is minimal
compared to a full pass over the function.

This differs from the original TriCore implementation, where the placeholder instruction
itself was made to hold the target label’s index in place of a real address, rather than being
left blank [5]. Patching a branch then required first reading the placeholder instruction back
out to recover that index, before it could be looked up in the label map. We instead record
the label index directly in the pending-jump list at the time the branch is compiled, so the
placeholder itself carries no information at all, and the second pass never needs to read an
instruction back to know what to patch it with.
### 3.2.3 Dead Code
Tracking dead code follows the same principle as the label stack. A parallel stack holds one
flag per currently open block, loop, or if, recording whether code at that nesting level is
currently dead. Entering a new block pushes a flag onto this stack, initialized to whatever the
enclosing block’s own flag currently is, so that a block opened inside already-dead code starts
out dead itself. Reaching an unconditional branch, br, br_table, return, or unreachable,
sets the flag at the top of the stack to true, marking the rest of the current block as dead from
that point on. Exiting a block pops its flag back off. Whatever flag is left at the top of the
stack at any point determines whether the instruction being parsed constructs a valent-block
and emits code, or is skipped entirely.

An if complicates this slightly, since its then and else bodies are two alternatives, not a
sequence, and must not share whether they are dead the way ordinary nested code would.
Entering then pushes a flag the same way as any other block, propagated from whatever
was already on the stack before the if. Reaching else pops that flag back off, revealing the

<!-- source-pdf-page: 28 -->

enclosing flag from before the if was ever entered, and it is that value that else inherits, not
whatever then left behind. What happens next depends on that revealed value and on what
then did with its own:
- If the if construct was already dead before it was entered, else stays dead and there is
     nothing further to do, since dead code never had anything pending to resolve in the
     first place.
- If then was never dead, reaching else requires the same resolution any other block exit
     does, flushing whatever valent-blocks are still pending from then before continuing.
- If then started alive but became dead partway through by hitting an unconditional
     branch, no further resolution is needed at else, since that unconditional branch already
     triggered a full resolution of its own the moment it was compiled, and nothing was
     added to the stack afterward.
### 3.2.4 Architecture Independence
This chapter has described several mechanisms so far: instruction classification, valent-block
construction and resolution, branch patching, and the visitor-based traversal that implements
them. All of them only inspect the structure of the WebAssembly module itself. None of it
refers to a target instruction set, a register file, or a calling convention. Most of it is inherited
unchanged from the existing TriCore compiler, and none of it is specific to TriCore or RISC-V,
so we reuse the algorithm as it stands today without needing to adapt it for the new target.

The main place the algorithm meets the target architecture is register allocation: deciding
which local variables live in registers, which are spilled to the runtime stack, and how many
registers are available to allocate from in the first place. How many registers are available,
which ones, and what it costs to spill to the stack all depend on the target. This is where
TriCore and RISC-V diverge, and where our actual porting work is concentrated, described in
Chapter 4: how functions are called, where and why values are saved, and how the runtime
stack itself is managed.
