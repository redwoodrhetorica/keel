# Sheet Metal Modeling as Kernel-Level Operations

Research review supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition.

## Scope and framing

Sheet metal is the application domain that most directly stresses developable-surface support in a solid modeling kernel. Unlike free-form surfacing (where approximate flattening is acceptable) or machining (where the part is rigid), sheet metal demands an **exact, invertible round-trip** between a folded 3D solid and a flat 2D blank, because the flat pattern drives a physical cutting and bending process. This file treats the sheet-metal-specific requirements:

- Feature semantics and published taxonomies (base/edge/contour flange, hem, jog, gusset, lance/form, corner relief).
- Bend mathematics: neutral axis/fiber theory, K-factor and Y-factor, bend allowance and bend deduction, DIN 6935, material and tooling dependence (air bend vs bottoming vs coining), springback.
- Unfold/refold algorithms: exact flattening of cylindrical bend regions, bend-graph traversal, deformation-feature handling, collision-checked refold and bend sequencing.
- Bend-graph topology: connected flange graph, cycles (closed-box over-constrained unfold), tab/slot self-intersection.
- Non-developable transitions (lofted/square-to-round flanges) and the stamping one-step inverse boundary.
- Recognition of sheet metal semantics from dumb imported solids (thickness-pair detection, bend-region recognition, midsurface).
- Manufacturing constraints (DFM checkers) and the nesting interface.
- How existing systems (SolidWorks, Inventor, NX, FreeCAD, OCCT) do it as documented.

General flattening theory (Gaussian curvature obstruction, ARAP, mesh parameterization) is covered in a separate file; here it appears only as a boundary to the sheet-metal-specific exact-developable case.

The core kernel insight, repeated throughout the literature, is this: **a sheet metal part is a hinge model.** It is a graph of nominally planar flanges connected by exactly-developable (cylindrical) bend regions. Topology is invariant between folded and flat states; only geometry (the embedding of each face) changes. Everything else, the math, the algorithms, the recognition, the DFM, follows from making that hinge model a first-class kernel object.

---

## Theme 1: Feature semantics and published taxonomies

### Source 1: Solid Edge sheet metal training (Siemens, spse01546)

**Citation.** Siemens PLM Software. (2012). *Solid Edge sheet metal design (spse01546-s-1050)*. Siemens Industry Software training manual. Retrieved from support.industrysoftware.automation.siemens.com.

**Content.** This vendor training manual is one of the cleaner published feature taxonomies. It distinguishes the *base feature* (the first wall, established from a closed or open profile that sets the global material thickness), from secondary *flange* features grown off existing edges. It formalizes the family of contour/lip flanges (a swept profile run along an edge), and the system-level coupling whereby thickness and default bend radius are global part attributes rather than per-feature ones. Crucially for a kernel, it documents that every wall/flange operation simultaneously creates a *thickness face* (the side wall of magnitude equal to the gauge) and that bends carry a *bend radius* and *neutral factor*. The manual treats the flat pattern as a derived, associative representation of the folded model.

**Limitations.** Vendor documentation, not peer-reviewed; describes a specific implementation's UI semantics rather than the underlying algorithms. No formulas for bend compensation are given in the design chapters (they are deferred to gauge tables).

**Kernel relevance.** Establishes the canonical feature set Keel must represent and confirms that **thickness and bend radius behave as part-global attributes** with per-bend overrides, which argues for an attribute-propagation system at the part level, not just per-face.

### Source 2: Advanced Features in Sheet Metal (Springer)

**Citation.** (2020). Advanced Features in Sheet Metal. In *A practical guide to sheet metal design* (Chapter 9). Springer Nature. https://link.springer.com/chapter/10.1007/978-3-030-38901-7_9

**Content.** Gives textbook definitions of the harder feature classes. A **hem** is a flange folded 180 degrees or more back onto its parent, used to stiffen an edge and remove the sharp burr edge; subtypes include closed, open (teardrop), and rolled. A **jog** (offset bend) is a pair of equal-and-opposite bends producing a Z profile, with the defining kernel behavior that it *preserves the position of any features on the offset face* (holes ride along with the jog). A **gusset** is a formed (not bent) stiffening rib pressed into an existing bend region. **Lance/louver/form** features are local deformations that punch and form material without removing it. **Corner relief** and **bend relief** are notches added at the junction of adjacent bends to prevent tearing and material bunching during forming.

**Limitations.** Pedagogical; does not specify geometric tolerances or recognition rules.

**Kernel relevance.** Two features deserve special kernel handling: the **jog**, which demands that face-local attributes (holes) be transported rigidly through a compound bend, and the **gusset/louver/lance** family, which are *non-developable* deformation features the unfolder cannot flatten exactly (see Theme 4).

### Source 3: SolidWorks Lofted Bends documentation (Bent vs Formed)

**Citation.** Dassault Systemes SolidWorks Corp. (2021). *Lofted Bends* and *Lofted Bend manufacturing methods*. SOLIDWORKS Help; with practitioner commentary, GoEngineer and CadShift blogs.

**Content.** A *lofted bend* transitions between two open profiles (e.g., square-to-round). SolidWorks exposes two manufacturing methods. **Bent** produces a flat pattern with discrete, valid bend lines; the developed length of each facet is computed with the part K-factor. The loft must be a *developable ruled surface* for this to work, and the documentation explicitly warns the user that it is their responsibility to author developable input. **Formed** treats the loft as a smoothly curved surface from a non-traditional process (stamping, roll-forming, hydroforming); there are no bend lines, and the flat pattern is a *developable approximation of the curved surface to a specified maximum deviation tolerance*.

**Limitations.** Documentation describes capability and constraints but not the flattening algorithm. The "you must make it developable yourself" caveat reveals the kernel does not robustly check developability of the input loft.

**Kernel relevance.** Confirms two distinct flattening paths Keel must support: **exact developable unwrapping** (ruled-developable lofts) and **tolerance-bounded approximate flattening** (non-developable formed surfaces). It also implies a kernel-level **developability predicate** is needed, both as a precondition check and as a user-facing diagnostic.

---

## Theme 2: Bend mathematics

### Source 4: K-Factor formulas and empirical ranges (SheetMetal.Me)

**Citation.** SheetMetal.Me. (n.d.). *K-Factor; Formulas and Functions*. Retrieved 2026. https://sheetmetal.me/formulas-and-functions/k-factor/

**Content.** Defines the K-factor as the ratio of the neutral-axis offset *t* to material thickness *MT*: `K = t / MT`. The inverse identity used to back out K from a measured bend allowance is `K = (180·BA)/(π·B∠·MT) − (IR/MT)`, where BA is bend allowance, B∠ the bend angle in degrees, IR the inside radius. The page tabulates empirical K by material hardness and radius bucket: for **air bending**, K rises from about 0.33 (soft/aluminum, IR 0..MT) through 0.40..0.45 (IR MT..3MT) to a saturating 0.50 for IR > 3MT across all materials; for **bottoming**, K is higher (0.42..0.48); **coining** sits between (0.38..0.44). The neutral axis shifts toward the inside radius as the bend tightens, which is why K is below 0.5 for tight bends.

**Limitations.** Aggregated shop tables, not a controlled study; the buckets are coarse and the values are advisory.

**Kernel relevance.** Gives the canonical equations Keel's unfold engine must implement and the realistic **K range (0.3..0.5)**. The dependence of K on the IR/MT ratio means K cannot be a single scalar; it must be **looked up per bend** from a material/tooling table keyed on (material, radius, thickness, forming method).

### Source 5: K-factors, Y-factors, and press-brake precision (The Fabricator)

**Citation.** Benson, S. (n.d., reprinted). *K-factors, Y-factors, and press brake bending precision* and *Analyzing the k-factor in sheet metal bending*. The Fabricator. https://www.thefabricator.com

**Content.** Defines the **Y-factor** as a derived companion to K used in bend-allowance equations: `Y = K · (π/2)`. The bend allowance (arc length of the neutral fiber) for a bend of complementary angle is `BA = θ · (IR + K·MT)` with θ in radians, equivalently `BA = (π/180)·θ·(IR + K·MT)` in degrees. **Bend deduction** is the amount subtracted from the sum of the outside flange dimensions: `BD = 2·OSSB − BA`, where outside setback `OSSB = tan(θ/2)·(IR + MT)`. The article stresses that real-world K drifts with tonnage, die width (the 8-times-thickness die-opening rule of thumb), grain direction, and material lot, so a fixed K is only a starting estimate.

**Limitations.** Trade-press explanation; gives the standard model but not lot-specific empirical fits.

**Kernel relevance.** Pins down the exact **BA / BD / OSSB / Y-factor** equation set. The OSSB term `tan(θ/2)·(IR+MT)` is the link between flat lengths and folded dimensions, and the kernel must compute it from bend geometry directly. Confirms K is a **process variable**, so Keel should treat the bend-compensation function as a pluggable strategy, not a hard-coded formula.

### Source 6: DIN 6935 cold bending of flat-rolled steel

**Citation.** Deutsches Institut fuer Normung. *DIN 6935: Cold bending of flat rolled steel*. Summarized via ModulusMetal and Scribd reproductions of the standard's tables.

**Content.** DIN 6935 gives the developed length as `L = a + b + v`, where *a* and *b* are the outside leg lengths and *v* is the **bend-compensation value** (Ausgleichswert), which may be negative. The standard defines a correction factor *k* (analogous to but not identical to the American K) that scales with the ratio of inside radius *r* to thickness *s*. The rounded table is: r:s 0.65..1 -> k 0.6; >1..1.5 -> 0.7; >1.5..2.4 -> 0.8; >2.4..3.8 -> 0.9; >3.8 -> 1.0 (and for r:s > 5 the correction vanishes, k = 1). The compensation *v* is computed piecewise by bend angle: a formula in (π, r, s, k) for β in 0..90 degrees, a second for 90..165, and v = 0 for 165..180 (compensation negligible near flat).

**Limitations.** Public summaries give the rounded k table and the L = a+b+v structure but defer the exact piecewise *v* algebra to Supplements 1 and 2 of the paid standard.

**Kernel relevance.** DIN 6935 is the European convention FreeCAD and others expose alongside ANSI; Keel must support **both conventions** because the DIN k references the sheet center (roughly twice the ANSI inside-referenced K). The piecewise-by-angle structure and the legal negative compensation value are important edge cases for the unfold arithmetic.

### Source 7: Autodesk Inventor unfold rule equations

**Citation.** Autodesk / KETIV. (2019). *Sheet Metal Unfold Rule Equations*. KETIV AVA technical reference PDF.

**Content.** Documents the three unfold-rule families Inventor exposes: a **linear** rule (`BA = A·(π/180)·(K·T + R)` style), a **bend-table** rule (interpolated lookup of compensation by angle and radius), and a **custom-equation** rule (user supplies an arbitrary expression in angle, radius, thickness). This generalizes the single-K model into a per-rule strategy attached to the part.

**Limitations.** The fetched PDF was image-encoded and not fully machine-readable; the rule structure is corroborated by Autodesk's published help and the companion search summary.

**Kernel relevance.** Strong precedent that the bend-compensation function should be a **first-class, swappable rule object** (linear / table / custom expression), bound to the part and overridable per bend, exactly the abstraction Keel should adopt.

### Source 8: Springback FEA and the elastic-recovery boundary

**Citation.** Tekaslan, O., et al., and related works including Ouakdi et al.; e.g., *Finite element analysis of springback in bending of aluminium sheets*, Materials & Design (Elsevier). https://www.sciencedirect.com/science/article/abs/pii/S0261306901000620

**Content.** Springback is the elastic recovery of the bend on unloading; the bend opens by a springback factor reported in the range K_sb ≈ 0.5..0.8 for R/t from 9 to 17, increasing with yield strength and decreasing with elastic modulus. FEA (and ML surrogates such as knowledge-based neural nets coupling RSM with FEM) predict the final angle and drive die-angle overbend compensation. The practical consequence is that the *as-formed* angle differs from the *commanded* angle, and the flat-pattern math (which assumes the nominal angle) is only one input to a tooling decision.

**Limitations.** Springback is a forming-physics problem, outside a geometry kernel's remit; FEA is expensive and material-model dependent.

**Kernel relevance.** Defines a clean **boundary**: Keel computes exact developable geometry and nominal bend compensation; springback compensation is an *attribute layer* (a per-bend angle correction supplied by a process model or FEA) that adjusts the commanded geometry without changing the unfold topology. The kernel must carry such per-bend process attributes through unfold/refold but should not attempt to compute them.

---

## Theme 3: Unfold/refold algorithms and the bend graph

### Source 9: Analysis Situs, cylindrical bend flattening (Part 2)

**Citation.** Analysis Situs (Quaoar). *On sheet metal recognition and unfolding, Part 2*. quaoar.su/blog. (OCCT-based.)

**Content.** The exact-developable core. A cylindrical bend face is flattened by exploiting its natural parameterization: U is the angular parameter, V is the height. The flat map is `X_flat = R·U`, `Y_flat = V`, i.e., the arc length `R·U` is laid down as a straight distance and the axial height is preserved isometrically (`Result[0] = m_fR * P.X(); Result[1] = P.Y()`). Curves on the bend face are reconstructed by rule: a 2D parametric curve parallel to the U or V axis maps to a straight 3D line in the flat; any other curve is approximated by a B-spline (via `AdvApprox_ApproxAFunction` and `GeomLib_MakeCurvefromApprox`). Holes and carved edges are preserved by reconstructing every face boundary edge through this UV-to-XYZ map. The base unwrap is geometric and **K-factor-free**; K scaling is applied separately so the developable geometry and the material compensation are decoupled.

**Limitations.** The clean `X = R·U` map is exact only for a true cylinder; non-cylindrical bends (conical, toroidal corners, spline) need different or approximate handling. Independent per-face flattening accumulates floating-point planarity error.

**Kernel relevance.** This is precisely the operation Keel needs as a primitive: **isometric unwrap of a cylindrical surface using its (R·U, V) parameterization**, with boundary-curve transport and a U/V-aligned straight-line fast path. Keel's surface evaluator and curve-approximation must support this map natively, and K scaling must be a separable post-step.

### Source 10: Analysis Situs, the unfolding tree and border trihedron (Part 5)

**Citation.** Analysis Situs (Quaoar). *On sheet metal unfolding, Part 5*. analysis-situs.medium.com.

**Content.** Describes two generations of the global unfold structure. The original **unfolding tree** has planar flanges as nodes and parent-child bend links as edges; each node stores its collected bends and a local flattening transformation. Flattening proceeds by rotating each child flange about its bend line, translating by the bend compensation, and filling residual gaps with connecting trapezia, with translations computed from a **border trihedron** (a local frame on the unrolled image of the anchor edge). The revised, more robust method abandons the tree for a **graph with anchor points**: each shared edge between two folded faces generates two images on the two unrolled faces, and the algorithm stitches faces by aligning those anchor images. Each graph node stores the unrolled geometry of its face.

**Limitations.** The tree method fails on parts without planar flanges and on consecutive cylinders lacking planar support, and is sensitive to planarity defects from independent rotations. It requires prior feature detection to label bends vs rolls.

**Kernel relevance.** Argues that Keel's flat-pattern data structure should be the **anchored bend graph**, not a tree, so it can express loops (closed boxes). Stitching by shared-edge anchor images is the robust way to keep flanges coincident, and the per-node "unrolled geometry" cache is the natural place to hang flat-pattern associativity.

### Source 11: Analysis Situs, bend sequences and refold collision (Part 6)

**Citation.** Analysis Situs (Quaoar). *On sheet metal unfolding, Part 6: bend sequences*. analysis-situs.medium.com.

**Content.** Refold (forming-order) feasibility. Each bend operation must be collision-checked for tool-part interference; the article recommends inexact, mesh-based collision detection (OCCT mesh) because a yes/no contact answer is enough and is far cheaper than exact contact zones. Two refold simulation strategies: an **origami** method that re-imprints fold lines and rotates flanges by half the bend angle (simple but slow because workpiece topology must be recovered after each fold), and an **unfolding-graph** method that reuses the existing flat-pattern topology to avoid expensive imprint/sew operations. Bend ordering follows the **Duflou** framework of hard precedence constraints plus heuristic weights: a distance weight (bend line to bounding rectangle), a length weight (shorter bends often precede longer), and a parallel weight; a permutation search combines sorting, parallel grouping, weight perturbation, and random shuffling over hundreds of iterations.

**Limitations.** The search is heuristic with no convergence guarantee; collision checking is approximate.

**Kernel relevance.** Refold is where the kernel meets manufacturing: Keel needs efficient **swept/positioned collision queries** (mesh-based is acceptable) and must keep the refold using the *same bend graph* rather than re-deriving topology each step. The ordering heuristics are application-layer, but they consume kernel-provided bend attributes (axis, length, position).

### Source 12: Analysis Situs, dynamic unfolding and associativity (Part 9)

**Citation.** Analysis Situs (Quaoar). *On sheet metal unfolding, Part 9: dynamic unfolding*. analysis-situs.medium.com.

**Content.** Establishes the central invariant: **topology is unchanged between folded and unfolded states**, so a direct correspondence exists between every flat and folded face. Bend attributes (angle, inner radius, direction, axis) persist through unfold. The system introduces a lightweight **Folded State** representation, a "hinge model of pairs of sheet faces," that reuses all the topological relationships from the unfolding graph and avoids re-running geometric modeling during interactive folding; each bend becomes an interactive handle that can be driven to a custom angle frame-by-frame. The author warns that grouping bends by their *final-state* alignment is unsafe because bends that coincide when fully folded can diverge at intermediate angles, so grouping needs dynamic validation.

**Limitations.** The hinge model is a simulation overlay; producing watertight final B-rep at an arbitrary intermediate angle still requires real modeling.

**Kernel relevance.** This is the **associativity design Keel should copy**: a persistent hinge graph in which flat and folded faces are the same topological entities at different embeddings, with bends as parametric joints. It makes flat-to-folded associativity, attribute round-tripping, and interactive fold angles fall out of one structure.

### Source 13: Analysis Situs, bend grouping and over-constrained loops (Part 11)

**Citation.** Analysis Situs (Quaoar). *On sheet metal unfolding, Part 11: bend grouping*. quaoar.su/blog.

**Content.** Tackles the closed-box problem. Property-based grouping (matching angle, axis, radius, up/down direction) works only when bends sit on independent flanges; it fails when bends belong to a **loop of flanges** where each flange depends on the previous one, forming a topological circuit. Detecting such situations is graph-circuit detection, which is well understood but computationally heavy on the full feature-topology graph (FTG). The proposed shortcut: a closed **cutout** contour is itself a loop in the sheet-face adjacency, so cutouts locally encode the topological circuits; grouping can exploit cutout topology instead of exhaustively searching for cycles.

**Limitations.** The cutout heuristic is an indirection that works because cutouts happen to bound flange loops; it is not a general cycle solver.

**Kernel relevance.** Confirms that the bend graph **contains cycles** for closed boxes and that the unfold is **over-constrained** there (the loop cannot be flattened without a controlled cut or strain). Keel must (a) detect cycles in the bend graph, (b) choose a spanning tree / cut edge to make the unfold well-posed, and (c) report residual gap/overlap at the cut as a manufacturability signal.

---

## Theme 4: Deformation (non-developable) features and the stamping boundary

### Source 14: Classification and extraction of deformation features (Kannan and Shunmugam)

**Citation.** Kannan, T. R., and Shunmugam, M. S. (2009/2013, related series). *Classification, representation, and automatic extraction of deformation features in sheet metal parts*. Computer-Aided Design, Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0010448513001188 (IISc ePrints 47508).

**Content.** Builds a hierarchy of **Basic Deformation Features (BDFs)**, the atomic Wall and Bend, and defines **compound deformation features** (flange, jog, dimple, rib/bead, louver, lance) as characteristic combinations of Walls and Bends encoded as a **Basic Deformation Features Graph (BDFG)**. Extraction is three-phase from a STEP B-rep: (1) extract internal and boundary **thickness-face chains**, (2) extract sub-chains from the boundary chain, (3) match sub-chains against BDFG templates to identify deformation features. A thickness face is identified as a face bounded by a pair of parallel, equal-length straight edges separated by the gauge.

**Limitations.** Template-graph matching is brittle for unanticipated feature combinations; the approach assumes clean uniform thickness and well-formed STEP.

**Kernel relevance.** Provides the **graph grammar** Keel can use to recognize and represent sheet metal features, and the precise **thickness-face criterion**. It also names the features (louver, lance, dimple, bead) that are *non-developable* and therefore cannot be unfolded by the cylindrical map; Keel must flag these and either keep them rigid or flatten them approximately with a tolerance.

### Source 15: One-step inverse isogeometric analysis for stamping flattening

**Citation.** (2017/2019). *One-step inverse isogeometric analysis for the simulation of sheet metal forming* and *Initial solution estimation for one-step inverse isogeometric analysis in sheet metal stamping*. Computer Methods in Applied Mechanics and Engineering, Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0045782519301306

**Content.** For genuinely non-developable stamped parts, flattening is a forming-mechanics problem, not isometry. The one-step inverse method unfolds a non-developable NURBS surface into the plane by minimizing deformation energy (an isogeometric membrane element under total-deformation plasticity), then a Newton-Raphson solve evaluates thickness change and equivalent strain/stress. A "cutting-stitching" algorithm handles extremely curled parts (square box, flower box, L shapes). The output is a strained blank contour, not an isometric flat pattern.

**Limitations.** Heavy numerics, requires a material model, predicts an *approximate* blank; not suitable for the bend-line-based fabrication a press brake uses. This is the stamping boundary, not core bend modeling.

**Kernel relevance.** Marks the limit of the kernel's responsibility. Keel should produce exact developable flat patterns and tolerance-bounded approximate flattening (for formed lofts and deformation features); strain-based one-step inverse flattening for stamping is an **external solver** that consumes Keel's surface and mesh but lives outside the kernel. The shared need is a **strain/deviation map** attribute over the flat pattern.

### Source 16: Unfolding non-developable surfaces by energy minimization

**Citation.** (2014). *Unfolding Method of Non-Developable Surface for Sheet-Metal Design*. Advanced Materials Research, Vol. 1022, p. 60. Scientific.Net. https://www.scientific.net/AMR.1022.60

**Content.** A meshing-plus-energy approach: triangulate the curved surface, then find the planar embedding that minimizes the deforming energy of the triangle set, suitable for non-developable surfaces with large bending. This is the discrete cousin of ARAP applied in the sheet metal setting and is the practical fallback for square-to-round and free-form transitions when an exact developable map does not exist.

**Limitations.** Mesh-resolution dependent, approximate, loses exactness and watertight parametric boundaries.

**Kernel relevance.** Confirms Keel needs a **discrete approximate flattener** (mesh + energy) as a complement to the exact cylindrical unwrap, gated by a developability test that chooses the exact path when available and the energy path otherwise, with a reported deviation tolerance.

---

## Theme 5: Recognition from dumb geometry and midsurface

### Source 17: A staged approach for feature extraction from sheet metal models

**Citation.** Gupta, R. K., and Gurumoorthy, B. (2013, related). *A staged approach for feature extraction from sheet metal part models*; and *Process Information Model for Sheet Metal Operations* (arXiv:1605.02514).

**Content.** A two-stage recognizer operating on neutral STEP (AP203) B-rep. Stage 1 extracts **thickness faces as chains**: pairs of planar end faces define a Wall, pairs of non-planar (cylindrical) end faces define a Bend; the reference face is the largest-area planar face and the gauge is inferred from the thickness-face separation. Stage 2 classifies each chain by topological and geometric signature against preset feature attributes. The Process Information Model layers manufacturing data (bend angle, radius, allowance) onto the recognized features and computes flat patterns from them.

**Limitations.** Depends on clean uniform thickness and exact parallel offset faces; struggles with chamfered edges, variable thickness, and imperfect imported geometry.

**Kernel relevance.** Defines the **import-to-feature pipeline** Keel needs for "dumb solid in, sheet metal model out": detect the gauge, find offset face pairs, chain them, label walls vs bends, build the bend graph. The reference-face and thickness-pair heuristics are directly implementable on Keel's adjacency graph.

### Source 18: Topological validation of midsurface from sheet metal parts

**Citation.** (related, IISc). *Topological Validation of Midsurface Computed from Sheet Metal Part*. academia.edu/50106259.

**Content.** Addresses extracting the **midsurface** (the single-skin idealization halfway between the two thickness faces) and validating that the resulting midsurface model is topologically faithful to the solid. The midsurface is the natural neutral-fiber representation: bends become single cylindrical strips, walls become single planar patches, and the whole part collapses to the hinge graph.

**Limitations.** Midsurface extraction is fragile near junctions, T-intersections, and variable thickness; topological validation is needed precisely because naive offset-pairing produces gaps and overlaps.

**Kernel relevance.** The midsurface *is* the hinge model. Keel should be able to compute and store a validated midsurface as the canonical neutral-fiber representation, with the two offset thickness faces as **first-class paired faces** linked to it. This unifies recognition, unfolding (which acts on the neutral fiber), and DFM (which measures from the midsurface).

### Source 19: Recognition of features in progressive-die parts

**Citation.** (2021). *Recognition of features in sheet metal parts manufactured using progressive dies*. Computer-Aided Design, Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0010448521000026

**Content.** Extends recognition to progressive-die parts where blanking, piercing, bending, and forming features co-occur. Uses an attributed graph of the B-rep faces, classifying cutting features (holes, notches, cutouts as loops in the sheet-face adjacency), bend features, and formed features. (Abstract-level; full text gated.)

**Limitations.** Full text behind paywall; method specifics inferred from abstract and the companion graph-recognition literature.

**Kernel relevance.** Reinforces that **cutting features are loops in the face-adjacency graph** (echoing the cutout-loop insight of Source 13) and that recognition must jointly handle cut, bend, and formed features on the same graph, the unified representation Keel's recognizer should target.

---

## Theme 6: DFM, nesting, and existing-system implementations

### Source 20: DFMPro sheet metal checks

**Citation.** HCL / Geometric. (n.d.). *DFMPro for Sheet Metal Design Guidelines*. dfmpro.com.

**Content.** An automated DFM checker enumerating geometric rules and, implicitly, the kernel queries behind them: extruded-hole-to-edge distance (>= 3T), hole-to-hole spacing (>= 6T), minimum hole diameter (>= T), maximum emboss depth (<= 3T), minimum bend radius (>= 1T), curl outside radius (>= 2T), hem inside-diameter and bend-spacing limits, and notch width (>= 1.5T) with corner radius (>= 0.5T). Each maps to a kernel capability: point/edge distance, face adjacency, feature dimension extraction, bend-radius measurement, and feature recognition.

**Limitations.** Rules are advisory thresholds and "highly customizable"; the page does not expose the recognition internals.

**Kernel relevance.** Defines the **geometric query surface** Keel must expose for a DFM layer: robust minimum-distance between features (hole-edge, hole-hole, feature-bend), bend-radius and gauge readout, and recognized-feature dimensions. These are read-only queries over the recognized feature model and the midsurface.

### Source 21: FreeCAD SheetMetal workbench (open-source implementation)

**Citation.** Saxena, S. (shaise) et al. *FreeCAD SheetMetal workbench*. GitHub shaise/FreeCAD_SheetMetal; SheetMetalUnfolder.py; DeepWiki. (LGPL, OCCT-backed.)

**Content.** The most studied open-source sheet metal implementation. It supports both **ANSI** K (inside-referenced) and **DIN** K (center-referenced, roughly 2x ANSI), with K specified manually, per-bend, or via a material-definition spreadsheet keyed by R/T bucket. It ships two unfolders: **V1 (original)** for typical parts and **V2 (NetworkX graph-based)** for complex parts with branching/looping bend connectivity, traversing a bend graph of faces and carrying holes/cutouts to the flat pattern. An "Engineering UX mode" forces explicit entry of fabrication-critical, visually-hidden parameters such as K.

**Limitations.** OCCT-bound and thus inherits OCCT's robustness issues on degenerate geometry; V1 fails on complex topologies (hence V2); no exact handling of non-developable formed features.

**Kernel relevance.** A direct, readable reference design for Keel. Confirms the **graph-based unfolder is necessary for non-trivial connectivity**, the **dual ANSI/DIN K conventions**, the **material-table K lookup**, and the requirement to **transport cutouts/holes** through unfold. Keel can study its face-traversal and gap-filling logic as prior art.

### Source 22: OCCT Sheet Metal Operations / Unfolding components

**Citation.** Open Cascade SAS. (n.d.). *Sheet Metal Operations Component* and *Unfolding Component*. opencascade.com; occt3d.com. (OCCT 7.6.1.)

**Content.** The commercial OCCT add-on recognizes primary features (flanges, bends, jogs/silks, holes, cutouts, bridges) including abnormal/crushed/hem bends without construction history, then unfolds and validates flat patterns using the K-factor. The general Unfolding component maps points and lines from 3D curved models to 2D with mesh-controlled accuracy. The unwrap uses the cylinder's (U=angle, V=height) parameterization (per Source 9).

**Limitations.** Closed-source add-on on top of OCCT; community accounts (Sources 9-13) reveal robustness gaps OCCT's marketing does not: planarity defects from independent face rotations, difficulty with consecutive cylinders lacking planar support, and reliance on the (R·U, V) map that is exact only for true cylinders.

**Kernel relevance.** Confirms the feasibility and the **failure modes** of an OCCT-class kernel doing sheet metal. Keel can improve on this by (a) anchored-graph stitching to avoid accumulated planarity error, (b) a native developability predicate, and (c) treating thickness-paired faces and the bend graph as first-class persistent objects rather than re-recognized each time.

---

## Sheet metal support requirements for Keel

Synthesizing across all sources, the sheet-metal domain imposes the following concrete kernel requirements.

**1. Exact cylindrical bend surfaces and isometric unwrap.** Bends must be true cylinders (or cones for tapered bends) so the unwrap `X = R·U, Y = V` is exact (Sources 9, 22). Keel's surface evaluator and curve approximator must support this parameter-space map natively, with a fast path that turns U/V-aligned 2D curves into straight 3D lines and a B-spline approximation for the rest, transporting all boundary edges (holes, cutouts) faithfully.

**2. Thickness-paired faces as first-class objects.** Every wall and bend has two offset faces separated by the gauge. Keel should represent the **offset pair (and the midsurface neutral fiber) as a first-class linked entity** (Sources 17, 18), because recognition, unfolding (which acts on the neutral fiber), and DFM (which measures from it) all depend on it. The thickness-face criterion (parallel equal-length edges separated by gauge) is the recognition primitive.

**3. The bend graph as the canonical structure, with cycles.** Model the part as an **anchored bend graph**: nodes are flanges, edges are bends carrying (angle, radius, axis, direction, K/compensation rule), with shared-edge anchor images used to stitch unrolled faces (Source 10). The graph **must support cycles** for closed boxes; the kernel detects cycles, picks a spanning tree / cut edge to make the unfold well-posed, and reports residual gap/overlap at the cut as an over-constraint signal (Source 13).

**4. Topology-invariant flat/folded associativity.** Flat and folded states share one topology; bends are parametric joints (Source 12). Keel should keep a persistent **hinge model** so that flat-to-folded associativity, interactive fold angles, and refold are all the same structure at different embeddings. This makes attribute round-tripping (holes/features carried to and from the flat pattern with correct positions) automatic.

**5. Pluggable bend-compensation rules with dual conventions.** Bend allowance, deduction, and the Y-factor follow `BA = θ(IR + K·MT)`, `OSSB = tan(θ/2)(IR+MT)`, `BD = 2·OSSB − BA` (Source 5). K is a **process variable** (0.3..0.5 air, higher bottoming/coining) looked up per bend by (material, R/T, method) (Source 4). Keel must expose a **swappable compensation rule** (linear / table / custom expression, ANSI and DIN conventions) bound to the part and overridable per bend (Sources 6, 7, 21), with **K scaling decoupled** from the geometric unwrap.

**6. Developability predicate and a two-path flattener.** Keel needs a **developability test** that routes exactly-developable surfaces (cylinders, cones, developable lofts) to the isometric unwrap and non-developable surfaces (square-to-round, lofted-formed, deformation features) to a **tolerance-bounded approximate flattener** (mesh + energy minimization), reporting a deviation/strain map (Sources 3, 15, 16). Deformation features (louver, lance, dimple, bead) are flagged non-developable and either kept rigid or approximately flattened (Source 14).

**7. Recognition pipeline for imported solids.** A dumb-solid importer must infer gauge, find offset face pairs, chain thickness faces, label walls vs bends vs cuts (cuts are loops in face adjacency), build the bend graph, and validate the midsurface (Sources 14, 17, 18, 19). This converts STEP/dumb B-rep into the hinge model.

**8. Manufacturing query surface and refold checking.** Expose robust minimum-distance queries (hole-edge, hole-hole, feature-bend), bend-radius/gauge readout, and recognized-feature dimensions to drive a DFM layer (Source 20). For refold feasibility, provide efficient **positioned/swept collision queries** (mesh-based acceptable) that consume the bend graph and per-bend attributes for sequence checking (Source 11).

**9. Springback as an attribute layer, not kernel physics.** Keel computes nominal developable geometry and compensation; springback is a **per-bend angle-correction attribute** supplied by an external process model or FEA, carried through unfold/refold but not computed by the kernel (Source 8).

**10. Nesting and process attributes on the flat pattern.** The flat pattern is an output artifact that must carry bend lines (with up/down, angle, radius), feature positions, and grain direction, in a form a nesting/CAM tool can consume (Sources 1, 5, 21). The flat-pattern node's "unrolled geometry" cache (Source 10) is the natural carrier.

Net: sheet metal does not ask Keel for one big feature; it asks for a small set of disciplined primitives, **exact developable unwrap, thickness-paired faces, an anchored bend graph with cycles, a topology-invariant hinge model, pluggable compensation rules, and a developability predicate**, from which the entire feature set, unfold/refold, recognition, and DFM follow.

---

## References

1. Siemens PLM Software. (2012). *Solid Edge sheet metal design (spse01546)*. Siemens Industry Software. https://support.industrysoftware.automation.siemens.com/training/se/en/ST5/pdf/spse01546-s-1050_en.pdf
2. Advanced Features in Sheet Metal. (2020). In *A practical guide to sheet metal design* (Ch. 9). Springer. https://link.springer.com/chapter/10.1007/978-3-030-38901-7_9
3. Dassault Systemes SolidWorks. (2021). *Lofted Bends; Lofted Bend manufacturing methods (Bent vs Formed)*. SOLIDWORKS Help; GoEngineer. https://www.goengineer.com/blog/solidworks-sheet-metal-lofted-bend-manufacturing-methods-bent-formed
4. SheetMetal.Me. (n.d.). *K-Factor*. https://sheetmetal.me/formulas-and-functions/k-factor/
5. Benson, S. (n.d.). *K-factors, Y-factors, and press brake bending precision*. The Fabricator. https://www.thefabricator.com/thefabricator/article/bending/k-factors-y-factors-and-press-brake-bending-precision
6. Deutsches Institut fuer Normung. *DIN 6935: Cold bending of flat rolled steel*. Summary: https://www.modulusmetal.com/din-6935-cold-bending-of-flat-rolled-steel/
7. Autodesk / KETIV. (2019). *Sheet Metal Unfold Rule Equations*. https://ketiv.com/wp-content/uploads/2019/04/KETIV-AVA-Sheet-Metal-Unfold-Rules-Equations.pdf
8. *Finite element analysis of springback in bending of aluminium sheets*. Materials & Design, Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0261306901000620
9. Analysis Situs (Quaoar). *On sheet metal recognition and unfolding, Part 2*. https://quaoar.su/blog/page/on-sheet-metal-unfolding-part-2
10. Analysis Situs. *On sheet metal unfolding, Part 5*. https://analysis-situs.medium.com/on-sheet-metal-unfolding-part-5-67cc0d5718f8
11. Analysis Situs. *On sheet metal unfolding, Part 6: bend sequences*. https://analysis-situs.medium.com/on-sheet-metal-unfolding-part-6-bend-sequences-11d06840c5e8
12. Analysis Situs. *On sheet metal unfolding, Part 9: dynamic unfolding*. https://analysis-situs.medium.com/on-sheet-metal-unfolding-part-9-dynamic-unfolding-067e7aa95f1e
13. Analysis Situs (Quaoar). *On sheet metal unfolding, Part 11: bend grouping*. https://quaoar.su/blog/page/on-sheet-metal-unfolding-part-11-bend-grouping
14. Kannan, T. R., and Shunmugam, M. S. *Classification, representation, and automatic extraction of deformation features in sheet metal parts*. Computer-Aided Design, Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0010448513001188
15. *One-step inverse isogeometric analysis for the simulation of sheet metal forming*. Comput. Methods Appl. Mech. Engrg., Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0045782519301306
16. *Unfolding Method of Non-Developable Surface for Sheet-Metal Design*. (2014). Advanced Materials Research, 1022, 60. https://www.scientific.net/AMR.1022.60
17. Gupta, R. K., and Gurumoorthy, B. *Process Information Model for Sheet Metal Operations* / *A staged approach for feature extraction from sheet metal part models*. arXiv:1605.02514. https://arxiv.org/pdf/1605.02514
18. *Topological Validation of Midsurface Computed from Sheet Metal Part*. https://www.academia.edu/50106259/Topological_Validation_of_Midsurface_Computed_from_Sheet_Metal_Part
19. *Recognition of features in sheet metal parts manufactured using progressive dies*. (2021). Computer-Aided Design, Elsevier. https://www.sciencedirect.com/science/article/abs/pii/S0010448521000026
20. HCL / Geometric. *DFMPro for Sheet Metal Design Guidelines*. https://dfmpro.com/manufacturing-processes/dfmpro-for-sheet-metal/
21. Saxena, S. (shaise) et al. *FreeCAD SheetMetal workbench*. GitHub. https://github.com/shaise/FreeCAD_SheetMetal ; https://deepwiki.com/shaise/FreeCAD_SheetMetal
22. Open Cascade SAS. *Sheet Metal Operations Component* and *Unfolding Component*. https://www.opencascade.com/components/sheet-metal-operations/ ; https://occt3d.com/components/unfolding-component/
23. Duflou, J. R., Van Oudheusden, D., Kruth, J.-P., and Cattrysse, D. (1999). *Methods for the sequencing of sheet metal bending operations*. Int. J. Production Research; and Duflou et al., *Design verification and automatic process planning for bent sheet metal parts*, CIRP Annals 48(1).
</content>
</invoke>
