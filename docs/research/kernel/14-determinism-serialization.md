# 14. Determinism / Reproducibility Engineering and Kernel Serialization / Schema Evolution

Research dossier supporting the design of **Keel**, an open-source Rust B-rep solid modeling kernel with Parasolid-class ambition.

## Scope and motivation

Determinism surfaced three independent times in prior Keel research, which is why it is treated here as a first-class engineering concern rather than an implementation detail:

1. OnShape's persistent-naming model is only sound if regeneration is **absolutely deterministic**: the same feature tree must produce the same topology with the same persistent IDs every time, on every machine.
2. Compiler FMA (fused multiply-add) fusion can silently break the **anticommutativity of orientation predicates**, corrupting the very sign tests that mesh, Boolean, and intersection algorithms depend on.
3. Parallel tessellation needs **deterministic ordering** so that the triangle list, vertex indices, and any derived hashes are reproducible.

Serialization surfaced via Parasolid XT's role as the de facto B-rep interchange standard and the requirement for save/load-stable entity references (a precondition for persistent naming surviving a round-trip to disk).

Both determinism and a stable file format are **API promises**, not internal conveniences. A kernel that cannot reproduce its own output cannot offer reliable persistent naming, replay-based debugging, hash-based regression testing, or trustworthy interchange. This file collects the literature, format documentation, and engineering accounts that inform Keel's determinism contract and file-format doctrine.

This dossier is organized into two parts. Part 1 covers determinism and reproducibility (floating point, parallelism, testing, geometry-specific and non-FP sources of non-determinism). Part 2 covers serialization and schema evolution (the major kernel formats, evolution strategy, robust deserialization, and geometry-specific serialization concerns). Each significant source gets a Citation / Content / Limitations / Kernel relevance entry. A closing synthesis states the doctrine Keel should adopt.

---

# PART 1: Determinism and reproducibility

## 1.1 Floating-point reproducibility across platforms and compilers

### Source: "Beware of fast-math" (Simon Byrne)

**Citation:** Simon Byrne, "Beware of fast-math," personal technical notes, https://simonbyrne.github.io/notes/fastmath/

**Content:** A careful enumeration of what `-ffast-math` (and GCC's `-Ofast`) actually does and why each sub-flag is dangerous. `-ffast-math` is not one optimization but a bundle: `-fno-math-errno`, `-funsafe-math-optimizations`, `-ffinite-math-only`, `-fno-rounding-math`, `-fno-signaling-nans`, `-fcx-limited-range`, and `-fexcess-precision=fast`. The three most damaging for a kernel: (1) `-ffinite-math-only` lets the compiler assume no NaN or Inf exists, so it deletes your explicit `isnan()` / `isinf()` guards entirely (the author calls this the single most common source of fast-math bugs). (2) `-fassociative-math` permits reassociation, turning `(a+b)+c` into `a+(b+c)`, which changes results and, critically, **optimizes away Kahan / compensated-summation error-correction terms**, producing catastrophically wrong sums. (3) Flush-to-zero (FTZ) for subnormals is set in a thread-wide control register, so merely linking a fast-math library can silently change the results of unrelated code in the same thread. The author recommends never hardcoding fast-math in library makefiles, enabling individual optimizations selectively with validation tests, and trying `-fno-finite-math-only` first when debugging.

**Limitations:** Does not cover FMA contraction (`ffp-contract`), x87 vs SSE excess precision, or cross-platform reproducibility in depth; it is a hazard catalog, not a reproducibility treatise.

**Kernel relevance:** Direct and decisive. Keel (and any C/C++ dependency it links, e.g. a libm or a mesh library) must never be built with `-ffast-math`. The FTZ-via-control-register hazard means even a transitively linked fast-math object can poison Keel's arithmetic. Rust does not enable fast-math by default (see below), which is a point in its favor, but Keel must audit every C dependency's build flags. The reassociation-kills-Kahan point is a warning to Keel's own robust-summation code: any compensated algorithm must be compiled with strict FP semantics or it is worthless.

### Source: Intel, "Consistency of Floating-Point Results using the Intel Compiler"

**Citation:** Intel Corporation, "Consistency of Floating-Point Results using the Intel Compiler" (white paper, fp-consistency-121918.pdf), https://www.intel.com/content/dam/develop/external/us/en/documents/pdf/fp-consistency-121918.pdf

**Content:** The canonical industry statement of where reproducibility comes from and how compilers break it. C and C++ explicitly permit (a) evaluation in higher precision than the declared type and (b) **contraction** of expressions, e.g. fusing `a*b + c` into a single FMA, or `1.0/sqrt(x)` into a reciprocal-sqrt instruction. GCC, Clang, MSVC, and ICC generally allow both by default because they are faster and usually more accurate. The `#pragma STDC FP_CONTRACT ON/OFF` directive and the `-ffp-contract` flag control fusion. The paper documents the standard compiler knobs (`/fp:strict`, `/fp:precise`, `/fp:fast` and Intel's `-fp-model`) and explains that bit-for-bit consistency across optimization levels, vectorization, and processor generations requires the strictest model, with a measurable performance cost.

**Limitations:** Vendor document focused on Intel toolchains; the precise default of `ffp-contract` differs between GCC, Clang, and MSVC (a recurring footgun), and the paper does not catalog those differences.

**Kernel relevance:** This is the single most important compiler fact for Keel's predicates. An FMA fusion of `a*b - c*d` (the heart of a 2D orientation determinant) changes the rounding and, as the predicates literature shows below, can break anticommutativity. Keel must compile predicate code with contraction **off** (`-ffp-contract=off` for C deps; in Rust, avoid letting the optimizer fuse, see the Rust section) or use explicit `f64::mul_add` only where mathematically intended, never as a silent optimizer choice.

### Source: "Floating-Point Determinism" (Bruce Dawson, Random ASCII)

**Citation:** Bruce Dawson, "Floating-Point Determinism," Random ASCII blog, 2013, https://randomascii.wordpress.com/2013/07/16/floating-point-determinism/

**Content:** A widely cited practitioner survey of what is and is not guaranteed. Within a single binary on a single CPU, IEEE 754 basic operations (+, -, *, /, sqrt) are correctly rounded and deterministic. Determinism breaks across: x87 80-bit excess precision (the historical curse, where intermediate values held 80 bits in registers but 64 bits in memory, so spilling changed results), SSE/SSE2 fixing this by computing in the declared precision, compiler reassociation and contraction, different vectorization choices, and transcendental functions (`sin`, `cos`, `exp`) which are not required to be correctly rounded and differ between libm implementations and even CPU vendors. Dawson's guidance: use SSE not x87, disable contraction and fast-math, avoid transcendentals in determinism-critical paths or provide your own implementation, and test.

**Limitations:** Blog-level rigor; some platform specifics (notably ARM, GPUs, and modern AVX-512) are lightly treated.

**Kernel relevance:** Confirms the x87-vs-SSE history is mostly behind us on x86-64 (SSE2 is baseline), but warns that **transcendentals remain the largest cross-platform reproducibility hole**. For Keel, this means surface evaluation that calls `sin`/`cos`/`atan2` for parameterization is a determinism risk across platforms unless Keel ships its own correctly-rounded or fixed implementation (see correctly-rounded libraries below).

### Source: Rust floating-point and HashMap determinism (Rust forum, internals, morestina.net)

**Citation:** "Determinism for floating point operations in Rust," users.rust-lang.org/t/determinism-for-floating-point-operations-in-rust/4426; "The stable HashMap trap," https://morestina.net/1843/the-stable-hashmap-trap; "Support turning off hashmap randomness," Rust Internals.

**Content:** Rust's relevant story. (1) Rust has **no fast-math by default** and the stable language does not expose fast-math flags, so the dangerous reassociation/contraction transforms of C are not silently applied; `f32`/`f64` `+ - * / sqrt` follow IEEE 754 and are deterministic on a fixed target. (2) The remaining caveats are the same as C: `mul_add` may or may not lower to a hardware FMA, transcendentals come from the platform libm and are not cross-platform reproducible, and `-C target-cpu` / autovectorization can change results across targets. (3) The `libm` crate provides a pure-Rust software implementation of libm functions that gives the **same bits on every platform**, trading speed for reproducibility, which is exactly the tradeoff a deterministic kernel wants for transcendentals. (4) Rust's `std::collections::HashMap` uses **SipHash with a per-process random seed** by default, so iteration order is non-deterministic across runs. This is a security feature (HashDoS resistance) and a determinism trap: any algorithm that iterates a `HashMap` and feeds the order into geometric results is non-reproducible. Fixes are to use an ordered container (`BTreeMap`), a fixed-seed hasher, or `IndexMap`, and to sort by stable IDs before any order-sensitive step.

**Limitations:** Forum/blog sources rather than a normative spec; Rust's exact FP guarantees are documented per-operation rather than in a single authoritative determinism statement.

**Kernel relevance:** Strongly favorable to Rust for Keel: the absence of default fast-math removes the worst C footguns. But Keel must (a) use the `libm` crate (or its own tables) for any transcendental on the determinism-critical path, (b) never iterate `std::HashMap` in a way that influences geometric output, defaulting to `BTreeMap`/`IndexMap` or a deterministic hasher with stable integer keys, and (c) pin `target-cpu` or accept tolerance-band rather than bitwise reproducibility across CPU generations.

## 1.2 Correctly-rounded math libraries

### Source: RLIBM project (Rutgers) and CR-LIBM

**Citation:** J. P. Lim and S. Nagarakatte, "An Approach to Generate Correctly Rounded Math Libraries for New Floating Point Representations," POPL 2021, https://people.cs.rutgers.edu/~sn349/papers/rlibm-popl-2021.pdf; CR-LIBM (Daramy, de Dinechin, et al.), "CR-LIBM: A correctly rounded elementary function library"; RLIBM-ALL (arXiv:2108.06756); RLibm-MultiRound (arXiv:2504.07409).

**Content:** A family of efforts to make elementary functions (`exp`, `log`, `sin`, `pow`, etc.) **correctly rounded**, meaning the result is the infinitely-precise value rounded once to the target type. Because there is exactly one correct answer per input, correctly-rounded functions are fully specified and therefore **bit-identical across platforms, OSes, and libm updates**, eliminating the largest source of cross-platform FP divergence. CR-LIBM (2000s) pioneered this for double precision, addressing the Table Maker's Dilemma (you cannot know in advance how many extra bits you need to round correctly). RLIBM reframes the problem as finding a polynomial that produces the correctly rounded result directly, achieving 1.1x to 1.6x speedups over glibc and Intel libm for 32-bit functions while guaranteeing correct rounding for all inputs. RLIBM-ALL and RLibm-MultiRound extend this to multiple representations and all rounding modes. IEEE 754 **recommends but does not mandate** correct rounding of elementary functions, which is why mainstream libms diverge. The CORE-MATH project carries this forward toward production-quality correctly-rounded routines.

**Limitations:** Coverage is still expanding (not every function at every precision is shipped as production code); correctly-rounded routines can be slower than aggressively tuned non-correct ones for double precision in some cases; integrating them means not using the system libm.

**Kernel relevance:** This is Keel's best answer to the transcendental reproducibility hole. If surface/curve parameterization, blends, and analytic intersections route every transcendental call through a correctly-rounded library (RLIBM/CORE-MATH ported to Rust, or the pure-Rust `libm` crate as a portable fallback), Keel achieves **bitwise cross-platform reproducibility for transcendentals**, which the standard libm cannot promise. This directly underpins the OnShape-style determinism that persistent naming requires.

## 1.3 Reproducible parallel computation

### Source: Demmel & Nguyen, "Parallel Reproducible Summation" and ReproBLAS

**Citation:** J. Demmel and H. D. Nguyen, "Parallel Reproducible Summation," IEEE Transactions on Computers, 2015; P. Ahrens, J. Demmel, H. D. Nguyen, ReproBLAS project, https://bebop.cs.berkeley.edu/reproblas/

**Content:** Floating-point addition is **not associative**, so a parallel sum whose grouping depends on the number of threads or the order of reduction gives different bits every run. Demmel and Nguyen give a reproducible summation algorithm that is independent of the order of operations and the number of processors: it first computes an absolute bound on the magnitude of the sum, then rounds each addend to a fixed grid (a small number of "bins" at fixed binary exponents) so that the partial sums are exact and order-independent. The result is bit-reproducible regardless of how the work is partitioned. The naive version costs roughly 2x (data and reduction done twice); an improved single-reduction variant cuts the overhead to about 20%. Distributed as ReproBLAS (reproducible BLAS-1 routines: sum, dot, asum, nrm2).

**Limitations:** Designed for BLAS-style dense reductions, not arbitrary geometric reductions; adds overhead and code complexity; the binning approach is one-pass-bounded and must know or bound the magnitude range.

**Kernel relevance:** Whenever Keel sums over a parallel set (mass properties, volume/area integrals, centroid accumulation, bounding-box union over many faces) the result must not depend on thread count. Keel should adopt a reproducible-reduction primitive (either Demmel-Nguyen binning or a simpler fixed-order tree reduction over stably-sorted inputs) for any quantity that feeds a geometric decision or a saved value.

### Source: ExBLAS and Kulisch superaccumulator

**Citation:** R. Iakymchuk, S. Collange, D. Defour, S. Graillat, "ExBLAS: Reproducible and Accurate BLAS Library," and "A Reproducible Accurate Summation Algorithm for High-Performance Computing," https://www-pequan.lip6.fr/~graillat/papers/SIAMEX14.pdf; U. Kulisch (long accumulator concept).

**Content:** A complementary approach that achieves both **reproducibility and full accuracy** (the exact rounded result of the true sum). Kulisch proposed a long fixed-point accumulator (about 4288 bits) wide enough to hold the exact sum of any number of IEEE doubles without rounding until the very end, making the result trivially order-independent. ExBLAS combines a fast filtering stage using vectorized floating-point expansions and error-free transformations (Knuth/Dekker TwoSum and TwoProduct) with a Kulisch long accumulator in a high-radix carry-save representation for the residual, getting near-exact and fully reproducible BLAS at competitive speed.

**Limitations:** The long accumulator has memory and latency cost; full ExBLAS is heavier than simple fixed-order reduction; most useful when correctness to the last bit matters, which is often stronger than a kernel needs.

**Kernel relevance:** Provides the gold-standard option for the rare Keel computations where exactness, not just reproducibility, matters (for example, a robust signed-volume or a determinant accumulated over many terms). Error-free transforms (TwoSum/TwoProduct) from this line of work are also the building blocks of Shewchuk's robust predicates (below), so the technique is doubly relevant.

### Source: Deterministic work-stealing and fixed-schedule parallelism (Tapir/OpenCilk, ADWS)

**Citation:** T. B. Schardl, W. S. Moses, C. E. Leiserson, "Tapir: Embedding Recursive Fork-Join Parallelism into LLVM's IR," PPoPP 2017 / ACM TOPC 2019; "Almost Deterministic Work Stealing" (ADWS), SC 2019; classic Cilk work-stealing.

**Content:** Standard work-stealing schedulers (Cilk, Rayon, TBB) are intentionally non-deterministic in *which* worker runs *which* task, so any reduction order that follows the dynamic schedule is non-reproducible. The literature offers two escapes. (1) Make the *algorithm* order-independent so the schedule does not matter (reproducible summation above). (2) Impose a deterministic schedule. Fork-join reductions can be made deterministic by combining results in a fixed reduction tree keyed on task indices rather than completion order. ADWS shows you can keep most of work-stealing's load balancing while making the planned schedule deterministic and cache-aware. Tapir/OpenCilk give the compiler first-class knowledge of parallel control flow, enabling deterministic reducer semantics.

**Limitations:** Deterministic scheduling can cost load-balancing efficiency; the cleanest answer (order-independent algorithms) is not always available for irregular geometric work.

**Kernel relevance:** Keel will likely use Rayon for parallel tessellation and parallel face/edge processing. The doctrine must be: **the scheduler may be non-deterministic, the results must not be.** Achieve this by (a) assigning each parallel unit a stable integer ID, (b) writing outputs into pre-sized index slots rather than appending in completion order, and (c) doing any cross-unit reduction with a fixed-order tree or reproducible-summation primitive. This makes the parallel tessellation deterministic regardless of thread count, satisfying the third place determinism surfaced in prior research.

## 1.4 Testing for reproducibility

### Source: FLiT (Sawaya, Bentley, et al.)

**Citation:** G. Sawaya, M. Bentley, I. Briggs, G. Gopalakrishnan, D. H. Ahn, "FLiT: Cross-Platform Floating-Point Result-Consistency Tester and Workload," IISWC 2017, https://pruners.github.io/pdf/iiswc2017-final43.pdf; maintained at University of Utah.

**Content:** FLiT is the first automated framework for measuring how much a collection of computational kernels varies across compilers, optimization levels, and architectures. It compiles each kernel under a matrix of compilers and flags (including the dangerous fast-math family), disperses the binaries across machines with different ISAs, runs them, and collects every result into a central SQL database to flag divergence. It can pinpoint which compiler/flag combination first introduces a bit difference, effectively a differential-testing harness for FP reproducibility. It can also search for the fastest flag set that preserves bitwise results.

**Limitations:** It detects divergence, it does not prove correctness; setting up the compiler/architecture matrix is heavyweight; results are about the tested kernels, not a general guarantee.

**Kernel relevance:** This is the model for Keel's reproducibility CI. Keel should run its predicate kernels, tessellation, and mass-property routines through a FLiT-style matrix (GCC/Clang for C deps, multiple `target-cpu` settings, debug vs release) and **fail the build on any unexpected bit divergence** in code paths declared bitwise-deterministic. This catches an accidental FMA fusion or a fast-math flag leaking in from a dependency before it corrupts persistent naming.

### Source: Verificarlo and Verrou (Monte Carlo / stochastic arithmetic)

**Citation:** C. Denis, P. de Oliveira Castro, E. Petit, "Verificarlo: Checking Floating Point Accuracy through Monte Carlo Arithmetic," IEEE ARITH 2016, https://arxiv.org/pdf/1509.01347; Verrou (F. Févotte, B. Lathuilière, EDF), Valgrind-based; CESTAC/Discrete Stochastic Arithmetic (Vignes, 1974).

**Content:** Where FLiT measures cross-platform divergence, these tools measure **numerical stability** of a single program. Verificarlo is an LLVM pass that replaces each FP operation with a Monte Carlo Arithmetic operator that injects controlled random rounding at a chosen virtual precision; running the program many times turns each execution into a Monte Carlo trial, and the spread of results estimates how many significant digits are actually trustworthy and where catastrophic cancellation occurs. Verrou does the same at the binary level via a Valgrind tool, randomizing the rounding mode and using delta-debugging to localize the unstable code region without recompiling. Both descend from CESTAC/DSA (Vignes).

**Limitations:** They estimate stability statistically rather than proving bounds; Monte Carlo runs are slow; they find instability, not the fix.

**Kernel relevance:** Complementary to FLiT in Keel's verification suite. Before declaring an algorithm "tolerance-band reproducible," run it under Verificarlo/Verrou to confirm the answer is numerically stable (enough significant digits survive) rather than accidentally correct on the test machine. This is how Keel can justify, per algorithm, whether it needs exact predicates, reproducible summation, or merely a tolerance band.

## 1.5 Determinism in geometry and simulation systems

### Source: OnShape determinism and deterministic regeneration

**Citation:** Onshape, "Under the Hood: How Collaboration Works," https://www.onshape.com/en/blog/under-the-hood-how-collaboration-works (and companion architecture posts).

**Content:** OnShape stores a Part Studio as an immutable chain of microversions; the **regeneration result is cached but always rebuildable from the definition**, and tessellation is generated on demand and never stored persistently. Features and entities are referenced by stable internal IDs, and a change like "set Extrude 1 depth to 4 in" is applied by internal feature ID so it works robustly across microversions. The whole model rests on the assumption that regenerating the same definition yields the same topology and the same persistent IDs, which is only true if regeneration is deterministic. (Discussed here for its determinism implications; the storage model recurs in Part 2.)

**Limitations:** Marketing-adjacent engineering blog; OnShape does not publish the bit-level determinism guarantees of its kernel or how it handles cross-platform FP. The determinism is asserted as a design premise rather than specified.

**Kernel relevance:** This is the primary evidence that **persistent naming requires absolute regeneration determinism**, the first reason determinism surfaced for Keel. If Keel ever caches regen results and rebuilds them elsewhere (a near-certainty for any cloud or distributed use), the rebuilt topology and IDs must match bit-for-bit, or cached references silently rot. Keel's determinism contract must be strong enough to support a cache-and-rebuild architecture.

### Source: Game-industry lockstep determinism (Gaffer On Games; Gas Powered Games)

**Citation:** Glenn Fiedler, "Floating Point Determinism" and "Deterministic Lockstep," https://gafferongames.com/post/floating_point_determinism/ ; gamedeveloper.com "Cross platform RTS synchronization and floating point indeterminism."

**Content:** Decades of hard-won RTS/lockstep experience. Lockstep simulations send only inputs and require every client to compute the identical world state, so a single divergent bit desyncs the game; a checksum of the full simulation state each frame must match across all clients. Fiedler's conclusion is "deterministic, yes, if": achievable on a **fixed compiler + fixed architecture** with FPU control set explicitly at startup and asserted every tick (Gas Powered Games did this across millions of Supreme Commander players), but **very hard cross-platform**. MotoGP found debug-build replays would not run on release builds due to compiler differences. Practitioners avoid SSE/x87 mismatches, disable optimization-driven reassociation (`/fp:strict`), avoid transcendentals (which differ between AMD and Intel), and many switch to **fixed-point math** to escape FP indeterminism entirely for cross-platform titles.

**Limitations:** Practitioner blogs and postmortems, not peer-reviewed; advice is x86/Windows-centric and somewhat dated; the fixed-point recommendation trades a hard problem for a different hard problem (range/precision management).

**Kernel relevance:** The most concrete real-world playbook for what Keel must control to be deterministic: pin/assert FPU state, forbid reassociation and contraction, route transcendentals through a portable correctly-rounded library, and accept that **same-platform bitwise determinism is realistic while cross-platform bitwise determinism requires extraordinary discipline** (or fixed-point/integer predicates). It also validates replay-based debugging and full-state checksums as a regression technique Keel should adopt: hash the topology + geometry after each operation and diff against a golden hash.

## 1.6 Robust geometric predicates and the FMA anticommutativity hazard

### Source: Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates"

**Citation:** J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates," Discrete & Computational Geometry 18:305-363, 1997, https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf ; project page https://www.cs.cmu.edu/~quake/robust.html

**Content:** The foundational work on exact geometric predicates (orientation/`orient2d`,`orient3d` and `incircle`/`insphere`). The predicate is a determinant whose **sign** must be exact, because mesh generation, Delaunay triangulation, convex hull, and Boolean operations branch on it; a wrong sign produces inconsistent topology and crashes. Shewchuk uses error-free transformations (TwoSum, TwoProduct) to represent intermediate results as exact floating-point expansions and evaluates the determinant **adaptively**: a fast approximate floating-point filter handles the common easy cases, and only inputs near zero (where the sign is uncertain) escalate to higher-precision exact arithmetic, so the average cost stays low while the answer is always correct.

**Limitations:** The reference implementation assumes IEEE 754 round-to-nearest with exact (un-fused, un-excess-precision) multiplication and addition; it is sensitive to compiler transformations that violate those assumptions; porting to new platforms requires care that the FP environment matches its assumptions.

**Kernel relevance:** Predicates are the bedrock of Keel's topology decisions, and they are exactly where the **FMA hazard bites** (next source). Keel should adopt Shewchuk-style adaptive exact predicates (the well-known `robust-predicates` ports exist in C and JS, and Rust equivalents exist) for all orientation/incircle/insphere tests, and must compile them with contraction off so the error-free transforms remain exact.

### Source: Bartels & Hemmer, "Fast Floating-Point Filters for Robust Predicates" (FMA anticommutativity)

**Citation:** T. Bartels and M. Hemmer, "Fast Floating-Point Filters for Robust Predicates," arXiv:2208.00497; FOSSGIS/FOSDEM talk "Fast Robust Arithmetics for Geometric Algorithms."

**Content:** Identifies a subtle and dangerous interaction between FMA and predicates. Plain IEEE multiplication is anticommutative in the sense the orientation determinant needs: `a*b - c*d = -(c*d - a*b)`. But when the compiler contracts `a*b - c*d` into `fma(a, b, -(c*d))`, the fused form computes the product with extra internal precision, and `fma(a,b,-c*d)` is **not** guaranteed to equal `-fma(c,d,-a*b)`. The consequence is catastrophic for geometry: **swapping two input points may fail to reverse the sign of the orientation result**, breaking the antisymmetry that mesh and Boolean algorithms assume, leading to inconsistent orientations even though each individual computation is "more accurate." The paper develops correct, fast floating-point filters that account for FMA behavior.

**Limitations:** Focused on the filter stage of predicates; the result depends on whether and how a given compiler chooses to contract, which is itself non-portable.

**Kernel relevance:** This is the **second** reason determinism surfaced for Keel, stated precisely. The fix is doctrinal: compile predicate code with `-ffp-contract=off` (and never `mul_add` inside a predicate unless the algorithm explicitly accounts for the fused semantics), or use a predicate implementation with FMA-aware filters. An accidental contraction here does not merely change the last bit, it can flip a topology decision and corrupt the model. Keel's reproducibility CI (FLiT-style) must specifically guard the predicate translation units.

## 1.7 Non-FP sources of non-determinism

### Source: Synthesis of container-ordering, pointer-ordering, and address-leak hazards (Rust HashMap trap and general practice)

**Citation:** "The stable HashMap trap," https://morestina.net/1843/the-stable-hashmap-trap; rust-fuzz/book issue #35 "Warn about hashmap randomization"; general determinism engineering practice.

**Content:** Beyond floating point, the classic determinism leaks are: (1) **hash-container iteration order**, non-deterministic in Rust by default (random SipHash seed) and unspecified in C++ `unordered_map`; (2) **pointer/address-based ordering**, where sorting or hashing by a pointer value or a memory address leaks the allocator's run-to-run nondeterminism into results; (3) **thread-scheduling order** feeding into accumulation (covered in 1.3); (4) iterating sets/maps to assign IDs, so IDs depend on insertion/hash order. The eliminating patterns are well established: assign **stable explicit IDs** to every entity at creation, use **ordered containers** (`BTreeMap`) or `IndexMap` for anything whose iteration influences output, **canonically sort** by stable ID before any order-sensitive step, and **never let a raw pointer or address reach a comparison, hash, or output**.

**Limitations:** This is engineering doctrine assembled from several sources rather than a single definitive paper; the discipline must be enforced continuously because a single careless `HashMap` iteration reintroduces nondeterminism.

**Kernel relevance:** Directly actionable for Keel. Every topological entity (vertex, edge, loop, face, shell, body) gets a stable, deterministically-assigned integer ID at creation. All maps that influence geometry or output use `BTreeMap`/`IndexMap` or a fixed-seed hasher keyed on those integer IDs. No algorithm ever sorts or hashes by `&T as *const _`. Tessellation outputs are written to ID-indexed slots, then read in ID order. This combined with the FP doctrine is what makes Keel's regeneration deterministic end to end.

## 1.8 Determinism versus performance, and bitwise versus tolerance-band

**Synthesis (drawn from all Part 1 sources):** The literature converges on a tiered policy. **Bitwise reproducibility** (every bit identical, every run, every platform) is the strongest and most expensive guarantee: it requires contraction off, no fast-math, correctly-rounded transcendentals, reproducible reductions, deterministic container/ID order, and pinned target-cpu, and it can cost 20% to 2x in the reduction-heavy paths and a constant factor in transcendentals. **Same-platform bitwise reproducibility** is much cheaper and is what game lockstep and OnShape's cache-rebuild realistically rely on. **Tolerance-band reproducibility** (results agree within an epsilon, validated as numerically stable via Verificarlo) is cheapest and acceptable for purely visual or non-branching outputs, but is **unsafe anywhere a result feeds a sign test or a persistent ID**, because a tolerance band still permits a topology flip. The decision rule: bitwise (and exact) for predicates and ID-determining computations; reproducible reduction for saved scalar quantities; tolerance-band only for display-only tessellation coordinates if at all.

---

# PART 2: Serialization and schema evolution

## 2.1 Parasolid XT: the de facto B-rep interchange standard

### Source: Parasolid XT Format Reference (Siemens) and CAD Exchanger overview

**Citation:** Siemens PLM Software, "Parasolid XT Format Reference" (Oct 2006), http://www.13thmonkey.org/documentation/CAD/Parasolid-XT-format-reference.pdf ; "Parasolid XT Format Manual," http://www.q-solid.com/Parasolid_Docs/xt_index.html ; CAD Exchanger, "3D formats overview: Parasolid," https://cadexchanger.com/blog/3d-formats-overview-parasolid/

**Content:** XT is a **schema-driven** format. A file is a list of **entity nodes**, each given a unique index, and each node carries a fixed set of fields (numeric, boolean, string, and **references to other nodes by index**, i.e. pointers serialized as integers) determined by the entity's type in the **schema**. The schema is the formal description of the data model, and **there is roughly one schema per kernel version**. Crucially, later versions support **embedding the schema in the file itself**, so a correctly-built importer can read a file from a newer kernel and extract at least the parts it understands, a built-in forward-compatibility mechanism. XT ships in two equivalent forms: **text** (`.x_t`, space-separated, not human-friendly) and **architecture-independent binary** (`.x_b`). It represents the full range of B-rep: solids, sheets, wireframe, mixed, non-manifold, and (in convergent modeling) facet bodies, plus model hierarchy/assembly structure. It stores procedural geometry implicitly (intersection curves, rolling-ball blends are kept as their defining recipe rather than flattened to NURBS), which is part of its fidelity and part of why a faithful importer is a large effort.

**Limitations:** The format is documented but the schema content is proprietary and version-specific; the implicit/procedural geometry makes third-party importers hard; the reference PDF is dated (2006) though the model is stable.

**Kernel relevance:** XT is the interchange target Keel must read and ideally write, because it is the lingua franca of the CAD industry (NX, SolidWorks, Inventor all use Parasolid). The architecture lessons are directly transferable to Keel's own format: **node-list + integer-index references + a versioned schema + embeddable schema for forward compatibility** is a proven design for a long-lived B-rep format. The integer-index reference scheme is exactly the **save/load-stable entity reference** prior research flagged, and it pairs naturally with Keel's stable IDs.

### Source: Parasolid in JT and STEP AP242 (Siemens, CAD Exchanger, Capvidia)

**Citation:** Siemens, "Parasolid 3D Geometric Modeling," https://www.siemens.com/en-us/products/plm-components/parasolid/ ; "Convert STEP to Parasolid," https://cadexchanger.com/step-to-parasolid/ ; Capvidia, "Top Neutral 3D CAD File Formats."

**Content:** XT's reach extends beyond `.x_t`/`.x_b` files. **JT** (ISO 14306), Siemens' visualization format, can carry exact B-rep, and when it does **that B-rep is stored in Parasolid XT form**. **STEP AP242** is the ISO neutral standard for exact geometry plus semantic PMI/GD&T; in practice exchanges pair an exact-geometry carrier with AP242 for annotations. This is why XT is treated as *the* exact B-rep payload across the ecosystem while STEP carries the standardized, vendor-neutral envelope and metadata.

**Limitations:** Mixed sources; the JT-contains-XT detail is documented but the precise embedding is proprietary; STEP and XT are not interchangeable in fidelity (STEP normalizes geometry, XT preserves Parasolid's native procedural forms).

**Kernel relevance:** Confirms Keel's interchange strategy should treat **STEP AP242 as the must-support open standard** (read and write) for vendor-neutral exchange, while recognizing that XT/JT carry the highest-fidelity exact geometry in the commercial world. Keel's own native format can be XT-inspired, but its **portable promise should be AP242 + a documented Keel format**, since AP242 is the only one Keel can fully implement without licensing.

## 2.2 ACIS SAT/SAB format

### Source: ACIS SAT/SAB Save File Format (Spatial/ARIZONA ACIS docs)

**Citation:** Spatial Corp / Dassault, "SAT Save File Format" (Kernel R10 Ch.9, R17 Tech Articles), http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/09SAT.PDF ; "SAT Save and Restore," q-solid.com ACIS R17 docs; ACIS, Wikipedia.

**Content:** ACIS offers two equivalent forms: **SAT** (Standard ACIS Text, `.sat`, human-readable ASCII) and **SAB** (Standard ACIS Binary, `.sab`); the data is identical, only the encoding differs. Structure: a **three-line header** (encoded version number, record count, product ID, ACIS version string, date, **modeling units and precision/tolerance values**), then a sequence of **entity records** (sequence number, entity type identifier, the entity data, terminator). Top-level entities (bodies) come first. **Pointers between entities are saved as integer index values** with `-1` for NULL; in SAT a pointer is prefixed `$`, in SAB it is a binary tag. Reserved characters structure the text: `{`/`}` delimit subtype definitions, `$` a pointer, `#` ends an entity record, `@` begins a string record (added in R7.0 to support arbitrary character sets, e.g. Japanese). The file may include an optional **history/rollback section** between begin-history and end-history markers holding old entity records.

**Limitations:** Format details are version-bound and partly proprietary; the documents are old kernel releases; full restore requires matching entity definitions in the reading kernel.

**Kernel relevance:** A second proven, well-documented design for Keel to learn from, and notable for two features XT documentation is quieter about: (1) an **explicit human-readable text variant** (excellent for debugging and diffing, which Keel should provide), and (2) a **history/rollback section in the file**, i.e. session state (undo/rollback) persisted alongside model state, which speaks to Keel's save/load of feature-tree rollback marks. The `$index` / `-1` pointer convention is the same save/load-stable reference idea as XT.

### Source: ACIS versioning and compatibility policy

**Citation:** Same ACIS docs; "Beginning with ACIS Release 4.0, the SAT save file format did not change with minor releases, only with major releases."

**Content:** ACIS codifies a **version-compatibility policy**: the SAT format changes only on **major** releases (where significant functionality changes and applications may need updates), never on **minor** releases. A "save version" mechanism lets a newer kernel write a SAT file readable by a specified older version (down-saving). The header's encoded version number tells the reader exactly which schema/format era the file belongs to.

**Limitations:** Down-saving necessarily drops features the older version cannot represent; policy is vendor-specific.

**Kernel relevance:** A concrete model for Keel's own compatibility promise: **freeze the on-disk format across minor versions, allow breaking changes only on major versions, embed an unambiguous version number in the header, and support explicit down-save to a target version**. This decades-proven policy is exactly the kind of API promise prior research said the file format must be.

## 2.3 OCCT BRep format and STEP as serialization

### Source: Open CASCADE Technology BRep Format specification

**Citation:** Open CASCADE Technology, "BRep Format," https://dev.opencascade.org/doc/overview/html/specification__brep_format.html ; OCCT wiki brep_format; OCCT STEP Translator user guide.

**Content:** OCCT's native serialization is produced by `BRepTools::Write/Read` (ASCII, header `CASCADE Topology V1/V2/V3`) and `BinTools::Write/Read` (binary). The ASCII format is **fully documented with a BNF-like grammar** and stores the complete topological and geometric model: vertices, edges, wires, faces, shells, solids, compsolids, compounds, plus the underlying curves/surfaces, **edge and face triangulations**, polygons on triangulation, and each entity's location (placement) and orientation. The `V1/V2/V3` header tag is the format version, and OCCT changed serialization details between releases (e.g. 7.5 to 7.6). Separately, OCCT translates shapes to/from **STEP** (manifold_solid_brep, brep_with_voids), using STEP as the neutral interchange serialization.

**Limitations:** The OCCT BRep format is OCCT-specific (not an interchange standard); cross-version reading is not always seamless (the 7.5 to 7.6 change broke some round-trips); it is open but not widely supported outside OCCT/FreeCAD.

**Kernel relevance:** As the leading **open-source** kernel format, OCCT's BRep is the closest precedent to what Keel needs and is fully readable, making it a study reference and a likely interop target (FreeCAD users). Two lessons: (1) a **documented ASCII grammar** is invaluable for debugging and tooling, so Keel should publish a grammar for its text format; (2) OCCT's between-version breakage is a cautionary tale, reinforcing the ACIS-style discipline of a versioned header and a stable minor-version contract.

## 2.4 Schema evolution strategy for a long-lived binary format

### Source: Protobuf and Cap'n Proto schema-evolution discipline

**Citation:** "Cap'n Proto: Schema Language," https://capnproto.org/language.html and "Encoding Spec," https://capnproto.org/encoding.html ; Cap'n Proto vs FlatBuffers vs SBE, https://capnproto.org/news/2014-06-17-capnproto-flatbuffers-sbe.html ; Confluent, "Schema Evolution & Compatibility Types," https://docs.confluent.io/platform/current/schema-registry/fundamentals/schema-evolution.html

**Content:** The serialization-systems world has crisp vocabulary Keel should adopt. **Backward compatible** = new reader reads old data; **forward compatible** = old reader reads new data; **full** = both (needed when readers and writers update independently, exactly the kernel-and-files situation). Protobuf achieves this by tagging every field with a **stable numeric field number** on the wire (names are free to change), never reusing a retired number (mark it `reserved`/deprecated rather than deleting), so old readers skip unknown fields and new readers supply defaults for missing ones. Cap'n Proto enforces the same via `@N` ordinals assigned consecutively, allowing new fields at larger ordinals and free renames as long as ordinals/type-IDs stay fixed; its **wire format is self-describing enough to copy a sub-object without its schema** (pointers encode struct-vs-list and size). The universal rule across all of them: **add, never repurpose; readers must skip unknown fields; new fields get defaults.**

**Limitations:** These are message/RPC formats, not B-rep formats; geometry has richer invariants (referential integrity among entities) than a flat message; Cap'n Proto's discipline requires governance.

**Kernel relevance:** This is the modern playbook to layer on top of the XT/ACIS lessons. Keel's format should use **numeric type/field tags that are never reused**, require readers to **skip unknown entity types and unknown fields** (mirroring XT's embedded-schema forward compatibility), provide **defaults for fields absent in older files**, and treat the schema as append-only across minor versions. Combining Cap'n Proto's "skip unknown, copy without full schema" property with XT's embedded schema gives Keel both forward and backward compatibility over decades.

## 2.5 Robust and safe deserialization

### Source: Assimp (Open Asset Import Library) CVE history and OCCT STEP-reader fragility

**Citation:** Multiple Assimp CVEs: CVE-2025-70067 (heap overflow, FBX material properties), CVE-2025-70069 (uncontrolled allocation from crafted face-index counts), CVE-2025-70070 (NULL deref, FBX mesh layer), CVE-2026-10198, plus use-after-free issues (assimp GitHub #5788, #6286); OCCT STEP parser buffer-overflow reports on very large entity definitions (dev.opencascade.org forum).

**Content:** The 3D-import ecosystem is a rich source of memory-safety vulnerabilities precisely because importers parse hostile/malformed files. Assimp's recent CVEs are textbook: **uncontrolled memory allocation** driven by an attacker-supplied count field (a crafted file claims a huge face-index count and the importer allocates gigabytes or overflows), **heap/stack buffer overflows** in property and name handling, **NULL-pointer dereferences** crashing the process, and **use-after-free** during callback/logging. These were largely found by **fuzzing with AddressSanitizer**. OCCT's STEP reader has likewise shown buffer-overflow behavior on files with abnormally large entity definitions. The common theme: trusting length/count fields and string sizes from the file without bounds checks.

**Limitations:** Most reports are against C/C++ importers; the specific bugs are implementation flaws, not format flaws; CVE lists are a snapshot.

**Kernel relevance:** A direct, sobering argument for Keel's memory-safety posture and parsing doctrine. Rust eliminates the buffer-overflow and use-after-free classes by default, but **not** the **uncontrolled-allocation / logic** class: Keel must still validate every count and size against the actual remaining file length before allocating, reject implausible magnitudes, verify that every entity index reference points to an existing in-range entity, and detect cycles/dangling references. Keel's importers (its own format, STEP, and any XT reader) should be **continuously fuzzed** (cargo-fuzz/AFL) as part of CI, and should aim for **partial recovery**: a damaged or truncated file should yield the recoverable entities and a clear error rather than a crash or a panic in a release build.

## 2.6 Geometry-specific serialization concerns

### Source: Exact double round-tripping (Ryu, Grisu-Exact)

**Citation:** U. Adams, "Ryu: Fast Float-to-String Conversion," PLDI 2018, https://dl.acm.org/doi/10.1145/3360595 ; J. Jeon, "Grisu-Exact," https://github.com/jk-jeon/Grisu-Exact and https://fmt.dev/papers/Grisu-Exact.pdf ; F. Loitsch, "Grisu" (PLDI 2010).

**Content:** Storing geometry as decimal text risks losing the exact `f64` value unless the printer produces the **shortest decimal string that round-trips** to the identical bits. Grisu (2010) made this fast but needed a slow fallback for some inputs; **Ryu** produces the shortest correctly-rounded representation using only fixed-precision integer arithmetic with no fallback, and is significantly faster; **Grisu-Exact** always yields shortest-and-correct output and is competitive with or faster than Ryu for short outputs. The alternative to decimal entirely is **hexadecimal floating-point** (C99 `%a`), which represents an `f64` exactly and compactly with no rounding question at all.

**Limitations:** Shortest-round-trip printing guarantees the value, not human readability; hex float is exact but unfamiliar to humans and some tools; parsers must also be correct (fast correct parsing is a separate problem, cf. fast_float).

**Kernel relevance:** Essential for Keel's text format and for any decimal in its files. Control points, knot vectors, and tolerances **must round-trip exactly**, or a save/load silently perturbs geometry and breaks bitwise determinism across a disk round-trip (which would, in turn, break persistent naming on reload). Keel should use Ryu/Grisu-Exact-class shortest round-trip printing (Rust's `ryu` crate) for human-facing decimal, and offer **hex-float** (or raw little-endian bits in the binary form) for the canonical, exact storage path where reproducibility is paramount.

### Source: Structural sharing, DAG serialization, persistent IDs, and control-point compression (synthesis)

**Citation:** Synthesized from the XT, ACIS, and OCCT format docs above plus general serialization practice; OnShape architecture posts for the cloud-storage model.

**Content:** A B-rep is a **DAG**, not a tree: many faces share an edge, many edges share a vertex, surfaces are reused. All three commercial/open formats handle this with **integer-index references** rather than inlined copies, giving structural sharing for free and avoiding duplication; serialization is a topological write of nodes plus index references, deserialization re-links indices to objects (with the bounds/cycle checks from 2.5). **Persistent IDs** in the file are what tie naming to disk: if the saved entity references are stable and survive reload, persistent naming survives a save/load cycle. **Tolerance values** belong in the file (ACIS stores them in the header) so a reader interprets geometry with the same epsilon the writer used. Large **control-point arrays** are the bulk of NURBS data and compress well (delta-encoding along a control net, or general-purpose compression of the binary block). For the **cloud model**, OnShape's lesson recurs: store the **immutable definition with structural sharing in the database** and treat tessellation/regeneration as rebuildable cache, never as primary stored state, which keeps storage small and consistency simple.

**Limitations:** A synthesis rather than a single citation; compression choices trade file size against load speed; structural sharing complicates partial loading and streaming.

**Kernel relevance:** Defines Keel's file doctrine for geometry specifically. **Serialize the topology DAG as a node list with integer-index references (matching the in-memory stable IDs), persist those IDs so naming survives reload, write tolerances into the file, store doubles exactly (hex/binary) or shortest-round-trip, and compress control-point blocks.** For session vs model state, follow ACIS (history/rollback section) and OnShape (immutable definition + rebuildable cache): Keel can save the feature tree with rollback marks and attribute schemas as the durable definition, and never depend on persisted tessellation.

---

# Determinism contract and file format doctrine for Keel

## Determinism contract (the API promise)

1. **Bitwise, same-platform reproducibility is mandatory.** Given the same feature tree and the same Keel build on the same target, regeneration must produce bit-identical topology, geometry, and persistent IDs. This is the floor that persistent naming and cache-and-rebuild require (OnShape, lockstep).
2. **Predicates are exact, never merely accurate.** All orientation/incircle/insphere tests use Shewchuk-style adaptive exact predicates, compiled with **FMA contraction off** (`-ffp-contract=off`; no implicit `mul_add`) so FMA cannot break anticommutativity and flip a topology decision (Bartels & Hemmer).
3. **No fast-math, anywhere, ever**, including transitively linked C dependencies; audit every dependency's build flags; beware the thread-wide FTZ control-register hazard (Byrne).
4. **Transcendentals route through a portable correctly-rounded (or pure-Rust `libm`) implementation**, not the system libm, so `sin`/`cos`/`exp` are bit-identical across platforms (RLIBM/CORE-MATH; Dawson; Rust `libm`). This is what upgrades same-platform determinism toward cross-platform determinism.
5. **Parallel results are order-independent.** Scheduler nondeterminism (Rayon) is fine; output nondeterminism is not. Assign stable IDs, write to index slots, and reduce with fixed-order trees or reproducible summation (Demmel-Nguyen / ExBLAS). Tessellation is deterministic regardless of thread count.
6. **No nondeterminism leaks from containers, pointers, or addresses.** Stable integer IDs on every entity; `BTreeMap`/`IndexMap` or fixed-seed hashers keyed on those IDs for anything influencing output; never hash/sort/compare by raw pointer or address; never iterate `std::HashMap` into geometric output.
7. **Reproducibility is tested in CI.** A FLiT-style cross-compiler/cross-target matrix fails the build on unexpected bit divergence in deterministic paths; Verificarlo/Verrou validate numerical stability before any path is declared tolerance-band; golden topology+geometry hashes give replay-based regression (lockstep practice).
8. **Tiered guarantee, declared per algorithm.** Bitwise/exact for predicates and ID-determining math; reproducible reduction for saved scalars; tolerance-band only for display-only coordinates, never where a result feeds a sign test or an ID.

## File format doctrine (the other API promise)

1. **Schema-driven node list with integer-index references** (XT/ACIS/OCCT consensus): serialize the topology DAG as numbered entity nodes; references are integer indices, NULL is a sentinel; deserialization re-links with strict bounds/cycle checks.
2. **Save/load-stable persistent IDs** written into the file, so persistent naming survives a disk round-trip.
3. **Versioned header + append-only schema.** An unambiguous version number in the header; format frozen across minor versions, breaking changes only on major versions (ACIS policy); embed/describe the schema for forward compatibility (XT) and require readers to **skip unknown entity types and fields** with defaults for missing ones (Protobuf/Cap'n Proto). Support explicit **down-save** to a target version.
4. **Two equivalent encodings:** a documented, grammar-published **text** form for debugging and diffing (ACIS SAT, OCCT BRep) and an **architecture-independent binary** form for size/speed (XT x_b, ACIS SAB).
5. **Exact numeric round-tripping.** Doubles stored as raw bits / hex-float in the canonical path, or shortest-round-trip decimal (Ryu/Grisu-Exact, Rust `ryu`) for human-facing text. Tolerances written into the file. Control-point arrays delta/compressed.
6. **Safe, fuzzed, recoverable deserialization.** Validate every count/size against remaining bytes before allocating (the Assimp uncontrolled-allocation class survives Rust's safety); verify referential integrity; continuously fuzz importers (cargo-fuzz); aim for partial recovery and clean errors over panics.
7. **Model state vs session state.** Durable definition is the feature tree with rollback marks and attribute schemas (ACIS history section); regeneration and tessellation are **rebuildable cache, never primary storage** (OnShape immutable-definition + structural-sharing model).
8. **Interchange:** treat **STEP AP242** as the must-implement open neutral standard (read/write), acknowledge XT/JT as the highest-fidelity commercial exact-B-rep carriers, and publish Keel's own format grammar so the ecosystem can build importers without the "monumental effort" XT's proprietary procedural geometry demands.

---

# References

1. Simon Byrne, "Beware of fast-math." https://simonbyrne.github.io/notes/fastmath/
2. Intel, "Consistency of Floating-Point Results using the Intel Compiler." https://www.intel.com/content/dam/develop/external/us/en/documents/pdf/fp-consistency-121918.pdf
3. Bruce Dawson, "Floating-Point Determinism," Random ASCII, 2013. https://randomascii.wordpress.com/2013/07/16/floating-point-determinism/
4. IEEE 754 standard overview. https://en.wikipedia.org/wiki/IEEE_754
5. "Determinism for floating point operations in Rust," Rust forum. https://users.rust-lang.org/t/determinism-for-floating-point-operations-in-rust/4426
6. "The stable HashMap trap," morestina.net. https://morestina.net/1843/the-stable-hashmap-trap
7. "Support turning off hashmap randomness," Rust Internals. https://internals.rust-lang.org/t/support-turning-off-hashmap-randomness/19234
8. J. P. Lim, S. Nagarakatte, "An Approach to Generate Correctly Rounded Math Libraries," POPL 2021. https://people.cs.rutgers.edu/~sn349/papers/rlibm-popl-2021.pdf
9. RLIBM-ALL, arXiv:2108.06756. https://arxiv.org/pdf/2108.06756
10. RLibm-MultiRound, arXiv:2504.07409. https://arxiv.org/html/2504.07409v1
11. CR-LIBM (Daramy, de Dinechin, et al.), "A correctly rounded elementary function library."
12. J. Demmel, H. D. Nguyen, "Parallel Reproducible Summation," IEEE TC 2015. https://www.netlib.org/utk/people/JackDongarra/WEB-PAGES/Batched-BLAS-2016/Day1/10_Demmel_ReproBLAS.pdf
13. ReproBLAS project. https://bebop.cs.berkeley.edu/reproblas/
14. Iakymchuk, Collange, Defour, Graillat, "ExBLAS: Reproducible and Accurate BLAS Library." https://hal.science/hal-01202396v2/file/exblas.pdf ; "A Reproducible Accurate Summation Algorithm for HPC." https://www-pequan.lip6.fr/~graillat/papers/SIAMEX14.pdf
15. T. B. Schardl, W. S. Moses, C. E. Leiserson, "Tapir," PPoPP 2017 / ACM TOPC 2019. https://dl.acm.org/doi/10.1145/3018743.3018758
16. "Almost Deterministic Work Stealing" (ADWS), SC 2019.
17. G. Sawaya, M. Bentley, et al., "FLiT: Cross-Platform Floating-Point Result-Consistency Tester," IISWC 2017. https://pruners.github.io/pdf/iiswc2017-final43.pdf
18. C. Denis, P. de Oliveira Castro, E. Petit, "Verificarlo," IEEE ARITH 2016. https://arxiv.org/pdf/1509.01347
19. Verrou (Févotte, Lathuilière); CESTAC/DSA (Vignes, 1974).
20. Onshape, "Under the Hood: How Collaboration Works." https://www.onshape.com/en/blog/under-the-hood-how-collaboration-works
21. Glenn Fiedler, "Floating Point Determinism," Gaffer On Games. https://gafferongames.com/post/floating_point_determinism/ ; "Cross platform RTS synchronization and floating point indeterminism," gamedeveloper.com.
22. J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates," DCG 18, 1997. https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf ; https://www.cs.cmu.edu/~quake/robust.html
23. T. Bartels, M. Hemmer, "Fast Floating-Point Filters for Robust Predicates," arXiv:2208.00497. https://arxiv.org/pdf/2208.00497
24. Siemens, "Parasolid XT Format Reference." http://www.13thmonkey.org/documentation/CAD/Parasolid-XT-format-reference.pdf ; XT Format Manual, http://www.q-solid.com/Parasolid_Docs/xt_index.html
25. CAD Exchanger, "3D formats overview: Parasolid." https://cadexchanger.com/blog/3d-formats-overview-parasolid/
26. Siemens, "Parasolid 3D Geometric Modeling." https://www.siemens.com/en-us/products/plm-components/parasolid/ ; "Convert STEP to Parasolid," https://cadexchanger.com/step-to-parasolid/
27. Spatial/Dassault, "SAT Save File Format" (ACIS Kernel R10 Ch.9). http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/09SAT.PDF ; "SAT Save and Restore," ACIS R17 docs.
28. Open CASCADE Technology, "BRep Format." https://dev.opencascade.org/doc/overview/html/specification__brep_format.html ; STEP Translator user guide.
29. Cap'n Proto, "Schema Language" and "Encoding Spec." https://capnproto.org/language.html , https://capnproto.org/encoding.html ; "Cap'n Proto, FlatBuffers, and SBE." https://capnproto.org/news/2014-06-17-capnproto-flatbuffers-sbe.html
30. Confluent, "Schema Evolution & Compatibility Types." https://docs.confluent.io/platform/current/schema-registry/fundamentals/schema-evolution.html
31. Assimp CVE history: CVE-2025-70067, CVE-2025-70069, CVE-2025-70070, CVE-2026-10198; assimp GitHub issues #5788, #6286.
32. U. Adams, "Ryu: Fast Float-to-String Conversion," PLDI 2018. https://dl.acm.org/doi/10.1145/3360595 ; J. Jeon, "Grisu-Exact." https://github.com/jk-jeon/Grisu-Exact ; F. Loitsch, "Grisu," PLDI 2010.
