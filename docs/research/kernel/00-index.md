# Kernel-Layer Literature Research: Index

Deep literature research on the topics a CAD geometry kernel needs beyond curve/surface mathematics: topology, operations, system engineering, and adjacent representations. Companion to the NURBS collection in `docs/research/nurbs/` and the original six tracks in `docs/research/`. Roughly 250 cited sources across ten files, in the same full-findings format (per-source citation, explicit mechanism, limitations, kernel relevance).

## Files

| File | Scope | Sources |
|---|---|---|
| [01-topology-euler-operators.md](01-topology-euler-operators.md) | B-rep data structures (winged-edge, half-edge, quad-edge), Euler operators and Mantyla's completeness result, Euler-Poincare invariants, Requicha's r-sets and regularized booleans, boundary evaluation, STEP topology, kernel hierarchy comparisons | 27 |
| [02-nonmanifold-cellular.md](02-nonmanifold-cellular.md) | Weiler radial edge, partial entity structure, SGC, NOODLES, cellular topology, non-manifold Euler operators, G-maps and combinatorial maps, LAR chain complexes, how Parasolid/ACIS/OCCT each handle non-manifoldness | 24 |
| [03-local-operations-direct-editing.md](03-local-operations-direct-editing.md) | Tweak, delete-face with wound healing, blend recognition and removal, defeaturing, direct modeling (Synchronous Technology, variational direct modeling), surface extension, local-op robustness, kernel API anatomies | 25 |
| [04-constraint-solving-2d-profiles.md](04-constraint-solving-2d-profiles.md) | Geometric constraint solving (Owen, Fudos-Hoffmann, DOF analysis, witness configuration), 3D assembly constraints, solver landscape (DCM, PlanGCS, SolveSpace); 2D curve arrangements, region extraction, planar booleans, curve offsetting, 2D robustness | ~26 |
| [05-tessellation-faceting.md](05-tessellation-faceting.md) | Trimmed-surface triangulation, CDT and Delaunay refinement, sag/normal/edge-length criteria, metric-driven anisotropic meshing, watertight faceting across shared edges, industrial faceters (Parasolid, ACIS, OCCT BRepMesh), tolerant remeshing | 26 |
| [06-interrogation-hlr.md](06-interrogation-hlr.md) | Fundamental forms and curvature fields, umbilics, isophotes/reflection/highlight lines, silhouettes and draft analysis, mold separability, geodesics (exact and discrete), flattening, hidden line removal and quantitative invisibility | 24 |
| [07-persistent-naming-sessions.md](07-persistent-naming-sessions.md) | The topological naming problem: Kripac, Capoyleas-Chen-Hoffmann, Marcheix-Pierra, Farjana-Han review, FreeCAD/realthunder fix, OnShape query model, Cascaval lineage DSL; Parasolid partitions/pmarks/deltas, ACIS bulletin boards, undo, journaling, transactionality, concurrency | 24 |
| [08-kernel-engineering.md](08-kernel-engineering.md) | Spatial acceleration (BVH variants, box filters, incremental refit), testing methodology (ABC/Thingi10K corpora, differential and metamorphic testing, fuzzing, FP reproducibility), API design study (PK, ACIS, OCCT, Rust handle arenas) | 25 |
| [09-mesh-implicit-hybrid.md](09-mesh-implicit-hybrid.md) | Exact mesh booleans (CGAL, libigl, EMBER, Cherchi/Attene indirect predicates), tolerant repair (TetWild family), winding numbers, convergent modeling; implicit/SDF modeling (R-functions, ADFs, OpenVDB, dual contouring), lattices/TPMS, conversion boundaries | 27 |
| [10-minkowski-medial-midsurface.md](10-minkowski-medial-midsurface.md) | Minkowski sums (exact and convolution), morphology view of filleting, swept volume envelope theory (grazing condition, SDE, continuation), 5-axis envelopes; medial axis theory and computation, pruning, power crust, Q-MAT, midsurface extraction, thickness analysis | 28 |

## Headline findings across the collection

1. **The topology hierarchy is a solved consensus.** Parasolid, ACIS, OCCT, and STEP ISO 10303-42 describe essentially the same body-shell-face-loop-fin-edge-vertex model; fin = coedge = half-edge = oriented edge-use under four names. Adopt it without re-derivation (file 01).
2. **Go non-manifold from day one, via the partial entity structure.** PES gives radial-edge expressiveness at roughly half the storage; Parasolid makes regions first-class (the model to emulate), ACIS bolts cellular topology on (cautionary), OCCT refuses non-manifold boolean results (the gap) (file 02).
3. **Tweak is the foundation local operation.** Parasolid and ACIS both implement offset/move/taper/delete-heal/blend-removal as clients of change-face-surfaces-and-reintersect. OCCT lacks it entirely: a clear differentiator target for Keel (file 03).
4. **Delete-face cannot be decomposed** into simpler edits and must remain a dedicated primitive (Zou-Feng-Gao 2023 variational direct modeling) (file 03).
5. **Constraint solver above the kernel, profile geometry inside.** Siemens ships DCM beside Parasolid; one arrangement-plus-winding-number engine unifies region extraction, curved planar booleans, and offset cleanup inside the kernel (file 04).
6. **Watertight faceting is a by-construction property:** discretize each model edge once, have both adjacent faces consume the same polyline. All three industrial faceters converge on this (file 05).
7. **One multivariate spline solver unifies interrogation.** Silhouettes, draft isoclines, isophotes, curvature extrema, umbilics, and projection are all zero-set queries against the same subdivision solver already mandated by the intersection research (file 06, ties to nurbs/04).
8. **Persistent naming reduces to per-entity lineage.** Every scheme from Kripac (1995) to Cascaval's PLDI DSL (2023) is a variation on (operation id, operand ids, derivation kind: created/modified/split/merged/deleted). The kernel must emit these events from every operation, and must be deterministic, or no naming layer can be built above it. This must be settled before the topology layer freezes (file 07).
9. **Copy-on-write persistent topology buys three features at once:** undo, atomic failure rollback (a failed boolean must not corrupt the model), and cheap journaling snapshots. Parasolid's deltas and ACIS's bulletin boards are both approximations of what a Rust persistent data structure gives natively (file 07).
10. **Robustness claims need a corpus and a pass rate.** fTetWild's run-over-all-of-Thingi10K methodology plus the ABC dataset (1M B-reps) give Keel a concrete acceptance benchmark. Compiler FMA fusion can silently break orientation predicates; pin FP contraction and use Shewchuk predicates (file 08).
11. **Generational-index arenas are the Rust answer to entity references.** Parasolid tags, Fornjot's object store, and a generational arena are the same idea; it sidesteps borrow-checker pain on cyclic topology graphs and gives stable save/load references (file 08).
12. **Indirect predicates (Cherchi/Attene) give exact mesh booleans without bignum-everywhere.** EMBER's plane-based representation is architecturally sympathetic to B-rep (faces already carry carrier geometry). Parasolid Convergent Modeling's lesson is architectural: make mesh faces a first-class face type in the same topology (file 09).
13. **Lattices belong on the implicit side; winding number is the universal inside/outside oracle** across B-rep, mesh, and implicit, including leaky geometry (file 09, ties to nurbs/06).
14. **The grazing condition v.n = 0 unifies swept volumes and the medial axis** (envelope of moving solid; envelope of inscribed balls). One pruned MAT with a radius field feeds five services: midsurface, wall thickness, shelling validity, mesh sizing, defeaturing (file 10).
15. **Midsurfacing remains commercially unsolved** (2025 papers still propose new methods); treat it as a service with explicit failure reporting, not a guaranteed operation (file 10).

## Decisions this research forces before code freezes

- Manifold-only vs non-manifold topology core (file 02 says: non-manifold, PES-class)
- Lineage-event emission contract on every operation (file 07: day-one mandate)
- Determinism guarantees (stable iteration order, deterministic IDs) as API promises (files 07, 08)
- Mesh faces as first-class topology citizens vs separate representation (file 09)
- The in-kernel vs above-kernel line for constraint solving (file 04: above) and tessellation (file 05: inside)

## Verification caveats

As with the NURBS collection, several paywalled, scanned, or blocked sources (ACM/ScienceDirect PDFs, Siemens/Spatial documentation mirrors, old theses, one patent) could not be fetched in full text. Agents reconstructed mechanism detail from accessible mirrors, abstracts, secondary expositions, and established domain knowledge; citations (author, title, venue, year) were verified by search. Spot-check primary sources before relying on exact constants, page numbers, or operator-level details, in particular the Parasolid/ACIS session mechanics (file 07), the Baumgart and Requicha originals (file 01), and the blend-removal patent material (file 03).
