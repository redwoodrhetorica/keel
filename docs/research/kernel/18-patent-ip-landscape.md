# Patent and IP Landscape for CAD Geometry Kernel Algorithms

Research review supporting the design of Keel, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition. This document surveys the public patent and intellectual-property landscape around CAD geometry kernel algorithms, for the defensive purpose of identifying which published algorithms carry active patent risk and which rest on expired or never-patented prior art. It complements the technical research files (00 through 17) by mapping each planned Keel subsystem to its IP exposure.

## Disclaimer

This is an informational survey of public records (Google Patents, USPTO, company announcements, and open-source project discussions). It is NOT legal advice. Patent claim scope is determined by the exact language of the independent claims as construed under the law of the relevant jurisdiction, by the file history, and by any later litigation. Expiry dates here are estimates based on the standard 20-years-from-earliest-non-provisional-filing rule and do not account for terminal disclaimers, patent term adjustment (PTA), patent term extension, or maintenance-fee lapses. Legal-status labels reflect what the cited sources showed at the time of research and can change. Before relying on any conclusion here for a shipping product, retain qualified patent counsel for a freedom-to-operate (FTO) analysis. Nothing here should be read as an assertion that any specific Keel feature does or does not infringe any specific patent.

Scope note on style: this document avoids em-dashes by deliberate house rule. Each significant patent or source carries a structured entry.

---

## 1. Patent basics relevant to this survey

A short orientation so the rest of the document reads cleanly.

**Term and expiry math.** A US utility patent filed on or after June 8, 1995 expires 20 years from its earliest non-provisional (or PCT) filing date, not from grant. A provisional application sets a priority date but does not itself start the 20-year clock; the clock runs from the later non-provisional. Practical consequence for Keel: anything with an earliest effective filing date before roughly mid-2006 is now (2026) expired or about to expire. Foundational 1980s and 1990s CAD patents are all long dead. The live risk window is filings from roughly 2007 onward, which expire 2027 through 2045.

**Patent term adjustment and extension.** USPTO delays can add days to years of PTA, pushing real expiry past the nominal 20-year date. Treat any "expires around 2028" estimate near the present as soft.

**US versus EP coverage.** Patents are territorial. A US patent does not block activity in Europe and vice versa. Many CAD families are filed only in the US, or in the US plus EP plus JP. An OSS project distributed globally is exposed wherever a corresponding patent is in force, so the relevant question is per-jurisdiction. European patents also tend to be harder to obtain for pure software because the EPO requires a "further technical effect."

**Algorithm patentability after Alice.** In Alice Corp. v. CLS Bank International, 573 U.S. 208 (2014), the US Supreme Court held that implementing an abstract idea (including a mathematical relationship or formula) on a generic computer is not patent-eligible under 35 U.S.C. 101. The two-step Mayo/Alice test asks (1) is the claim directed to an abstract idea, law of nature, or natural phenomenon, and (2) if so, is there an "inventive concept" beyond the abstract idea. A bare mathematical algorithm (for example "compute a NURBS basis function") is weak post-Alice. However, CAD patents are routinely drafted to recite a specific technical improvement to a computer-graphics or manufacturing process, which often survives Alice. So pure-math claims are weak, but "method of editing a solid model such that the system infers constraints and re-solves" style claims can be valid.

**Defensive publication.** Publishing an algorithm (paper, preprint, blog, or a dated public code commit) before anyone files a patent makes it prior art that can invalidate or prevent a later patent. This is the single most powerful and cheapest defensive tool available to an OSS project, and it cuts both ways: it protects Keel's own techniques and it means any algorithm already in the literature before a competitor's filing date is safe to use.

---

## 2. Expired foundational patents (now safe to use)

### 2.1 PTC parametric, feature-based modeling (Pro/ENGINEER era)

**Patent family / status.** PTC (Parametric Technology Corporation) shipped Pro/ENGINEER in 1988 and was first to market with parametric, associative, feature-based solid modeling, founded by Samuel Geisberg and Mike Payne. Any US patents arising from that late-1980s and early-1990s work have a maximum 20-year term and are therefore expired (filings from that era would have lapsed by roughly 2008 to 2012 at the latest). No active 1980s PTC parametric-modeling patent could survive to 2026.

**Claims summary (era, generic).** Feature-based construction (extrude, revolve, hole, round driven by named parameters), dimensional constraints that regenerate downstream geometry, and parent-child feature dependency graphs.

**Keel subsystem affected.** The history/feature/regeneration model and persistent naming (file 07). Conclusion: the foundational parametric-modeling concepts are unencumbered prior art. Keel can build a feature tree, dimensional parameters, and regeneration freely. Caution applies only to specific modern refinements (see Synchronous Technology, section 3).

### 2.2 T-splines original patent (Sederberg, BYU)

**Patent number.** US 7,274,364 B2. Related: US 2004/0189633 A1 (application), EP 1,606,692 A2 (European counterpart).

**Title.** System and method for defining T-spline and T-NURCC surfaces using local refinements.

**Assignee.** Filed by Brigham Young University (March 26, 2004). Assigned to T-Splines, Inc. (2010), then to Autodesk, Inc. (2014).

**Dates and status.** Priority/filing March 26, 2004; granted September 25, 2007; nominal expiry March 26, 2024. EXPIRED. Autodesk acquired T-Splines in 2011/2012.

**Claims summary.** Defines bicubic spline surfaces (T-splines and T-NURCCs) that permit T-junctions in the control grid, so control points can be inserted locally without propagating an entire row or column as classic NURBS requires. The independent claims cover the local-refinement construction and the associated knot-interval representation.

**Keel subsystem affected.** Surface representation and local refinement (files 09, 15). Conclusion: the original T-spline construction is now EXPIRED and safe to implement. This is a meaningful win: classic T-splines with local refinement are usable. Watch only for distinct follow-on filings (section 4.1).

### 2.3 Viewpoint adaptive mesh subdivision

**Patent number.** US 6,356,263 B2.

**Title.** Adaptive subdivision of mesh models.

**Assignee.** Originally Viewpoint Corp; later Andreas Acquisition LLC.

**Dates and status.** Filed January 27, 1999; granted March 12, 2002; nominal expiry January 27, 2019. EXPIRED.

**Claims summary.** A method for adaptively refining triangle meshes by testing each triangle's edges against criteria (edge length, normal-angle difference) and subdividing two, three, or four ways depending on how many edges qualify, computing new points with an interpolating extrusion formula.

**Keel subsystem affected.** Tessellation and adaptive faceting (file 05). Conclusion: expired; adaptive tessellation by edge criteria is safe.

### 2.4 Pixar Catmull-Clark approximation by Bezier patches

**Patent number.** US 6,950,099 B2 (related grants in the same family include US 7,170,516).

**Title.** Approximation of Catmull-Clark subdivision surfaces by Bezier patches.

**Assignee.** Pixar (originally).

**Dates and status.** Early-2000s family. The base Catmull-Clark subdivision algorithm itself (1978) was never patented and is public-domain prior art. The patented pieces were specific refinements (GPU evaluation, semi-sharp creases, Bezier-patch approximation), and the early-2000s grants are now at or past their 20-year terms (expiring around 2023 to 2026). See also the OpenSubdiv patent grant in section 7.4.

**Claims summary.** A method to convert a Catmull-Clark subdivision surface into a set of bicubic Bezier patches for efficient (hardware) evaluation, including handling of extraordinary vertices.

**Keel subsystem affected.** Subdivision surfaces and tessellation (files 05, 09). Conclusion: the core subdivision math is free; Pixar additionally granted a royalty-free patent license through OpenSubdiv (section 7.4), so even the refinements are practically safe if their algorithms are used as published.

### 2.5 Classic NURBS / B-rep / Euler-operator era

**Patent family / status.** The mathematical and topological foundations laid in the 1970s through mid-1990s are expired or were never patented. NURBS evaluation (Cox-de Boor recursion, knot insertion, the Oslo algorithm), the boundary-representation data model, Euler operators (Baumgart's winged-edge, Mantyla's Euler operators), classic Boolean set operations via boundary classification, and the Bezier-clipping intersection method are all pre-2000 published literature. The canonical reference, Piegl and Tiller's The NURBS Book (1995/1997), documents algorithms that are prior art today.

**Keel subsystem affected.** Core NURBS, topology/Euler operators (file 01), classic Booleans, curve/surface interrogation (file 06). Conclusion: the entire classical core of Keel rests on expired or never-patented art. This is the safe foundation.

---

## 3. Siemens Synchronous Technology and variational direct editing (ACTIVE RISK)

Synchronous Technology (Siemens, launched 2008 in NX and Solid Edge) combines direct (history-free) editing with a constraint-inference engine ("Live Rules") that detects relationships among faces before an edit and enforces them during the edit. The relevant filings cluster around 2007 to 2014, so several are ACTIVE into the late 2020s and 2030s.

### 3.1 Local behavior in a variational system

**Patent number.** US 9,235,659 B2.

**Title.** Local behavior in a variational system.

**Assignee.** Siemens Industry Software Inc.

**Dates and status.** Priority/filing March 17, 2014; granted January 12, 2016; estimated expiry March 17, 2034. ACTIVE.

**Claims summary.** A CAD editing method: the system receives a model with multiple geometric elements, accepts a user selection and move, applies predefined "basic conditions" (mandatory rules) and user-configurable "optional conditions," builds a constraint system incorporating those rules, runs a solver to compute new geometry positions, and stores the result. In plain terms, it claims intelligent direct editing where connected geometry automatically responds to a move according to inferred and user-toggled rules.

**Keel subsystem affected.** Direct editing with constraint inference (file 03), constraint solving (file 04). Conclusion: this is a directly relevant ACTIVE patent. A Keel "Live Rules"-style feature that infers relationships (coplanarity, concentricity, symmetry) among faces and re-solves during a push/pull edit could read on this claim. HIGH-CAUTION area.

### 3.2 Related Synchronous Technology / variational patents

**Patent numbers (family).** US 10,140,389 (Modifying constrained and unconstrained curve networks), US 10,176,291 (Ordering optional constraints in a variational system), plus other Siemens variational-system grants.

**Assignee.** Siemens Industry Software Inc.

**Dates and status.** Mid-2010s filings; grants 2018 to 2019; estimated expiry roughly 2034 to 2037. ACTIVE.

**Claims summary.** US 10,140,389 covers inferring relationships among selected, connected, and neighboring curves before a modification and enforcing those relationships across the network during the modification (synchronous curve editing). US 10,176,291 covers prioritizing/ordering the optional inferred constraints when the solver cannot satisfy all of them simultaneously.

**Keel subsystem affected.** Direct editing and 2D/curve constraint inference (files 03, 04). Conclusion: ACTIVE family covering the inference-plus-reorder behavior central to modern direct modeling. Keel should treat constraint-inference-during-direct-edit as patent-sensitive and design around it (see risk matrix and safe alternatives).

Note on a false hit: US 8,103,629 ("Bi-directional data modification with synchronization") surfaced in searches for "synchronous" but is a Microsoft data-sync patent unrelated to CAD; it is not relevant to Keel.

---

## 4. T-splines and U-splines follow-on (MIXED: original expired, refinements active)

### 4.1 Analysis-suitable T-splines and local-refinement follow-ons (Autodesk)

**Patent family / status.** Beyond the now-expired original (section 2.2), the academic line on "analysis-suitable T-splines" (Scott, Li, Sederberg, Hughes, circa 2011 to 2013) and local-refinement algorithms produced follow-on filings. Any Autodesk filings from roughly 2010 onward would still be ACTIVE (expiring around 2030 to 2033).

**Claims summary (era).** Construction of T-spline spaces guaranteed to be analysis-suitable (linearly independent, partition of unity) and bounded local-refinement algorithms that keep the basis well-behaved for isogeometric analysis.

**Keel subsystem affected.** Spline refinement and any isogeometric-analysis-facing surface representation (files 09, 15). Conclusion: the classic T-spline is free, but specific analysis-suitable refinement algorithms may carry live claims; treat IGA-grade local refinement as a check-before-implement area.

### 4.2 U-splines (Coreform)

**Patent / application number.** WO 2018/237067 A1 (PCT), from US application 16/012,128, published as US 2019/0130058 A1.

**Title.** U-splines: splines over unstructured meshes.

**Assignee.** Coreform LLC. Inventor: Derek Thomas.

**Dates and status.** Priority June 20, 2017; PCT filed June 20, 2018; published December 27, 2018. The cited PCT legal status showed "Ceased," and no granted US patent number was evident from the PCT record at research time. Coreform publicly markets U-splines as proprietary, patent-protected technology, so the family should be assumed to include at least one pending or granted US member regardless of the PCT branch status. Treat as ACTIVE/PENDING until a cleared FTO says otherwise. If granted off the 2017/2018 priority, a member would run to roughly 2037/2038.

**Claims summary.** A computational method to construct spline basis functions over an arbitrary (unstructured) mesh by solving localized constraint systems and normalizing to a partition of unity, allowing simultaneous local adaptivity in element size (h), polynomial degree (p), and smoothness (k) with no restriction on T-junction placement. The novelty over T-splines is the unrestricted-topology, simultaneous h/p/k local adaptivity.

**Keel subsystem affected.** Advanced spline refinement / unstructured-mesh splines (files 09, 15). Conclusion: U-splines are the clearest "do not implement the patented construction" item. Prior research already flagged this. Keel should rely on the expired T-spline construction or on hierarchical B-splines (THB, see 4.3) for local refinement and avoid the specific U-spline constraint-solve construction.

### 4.3 Safe refinement alternative: hierarchical and truncated hierarchical B-splines

**Status.** Hierarchical B-splines (Forsey and Bartels, 1988) and truncated hierarchical B-splines (THB-splines, Giannelli, Juttler, Speleers, 2012) are published in the academic literature. The 1988 work is decades old; the 2012 THB construction was published openly and is widely implemented in open research code (for example the G+Smo library). These provide local refinement without touching the T-spline or U-spline patent constructions.

**Keel subsystem affected.** Local refinement (files 09, 15). Conclusion: THB-splines are a documented, published, patent-safe path to local refinement and are the recommended alternative to U-splines for Keel.

---

## 5. Convergent / hybrid mesh-and-B-rep modeling (ACTIVE RISK)

### 5.1 Siemens Convergent Modeling

**Patent family / status.** Siemens announced Convergent Modeling in 2016 and shipped it in NX and later in Parasolid (v30+). It performs B-rep-style operations (Booleans, blends, offsets) directly on bodies that mix classic B-rep faces with facet (mesh) "faces." The supporting patents are from roughly 2015 to 2018 filings and are ACTIVE (expiring around 2035 to 2038). Specific public patent numbers were not pinned down in this survey; the family should be assumed to exist and to be live.

**Claims summary (era, generic).** Methods for representing a single body containing both analytic/spline B-rep faces and facet (triangle-mesh) faces in one topological structure, and for performing modeling operations (Boolean unite/subtract/intersect, blend, offset, shell) across the mixed representation while maintaining a valid body.

**Keel subsystem affected.** Hybrid mesh/B-rep "convergent" modeling (file 09). Conclusion: ACTIVE-RISK area. A Keel feature that puts mesh faces and B-rep faces in one body and runs Booleans/blends across them may read on Siemens convergent-modeling claims. Safe alternative: keep mesh and B-rep as separate bodies and convert at well-defined boundaries (the mesh-boolean approach in file 09 using published exact-arrangement algorithms), rather than maintaining a single mixed-topology body operated on as one.

---

## 6. Direct modeling, blends, healing, lattice, and cloud (mixed)

### 6.1 SpaceClaim / Ansys direct modeling (the "Pull" tool)

**Patent family / status.** SpaceClaim Corp (founded 2005, first CAD app 2007) pioneered a four-tool direct-modeling UX (Pull, Move, Fill, Combine). Ansys acquired SpaceClaim in 2014. Core direct-modeling-UX patents would derive from 2005 to 2012 filings, putting nominal expiry around 2025 to 2032; the earliest are expiring now, later ones remain ACTIVE.

**Claims summary (era).** Gesture-driven direct editing where a single "pull" interaction context-sensitively offsets, extrudes, revolves, sweeps, drafts, blends a face, or rounds/chamfers an edge depending on selection, without a feature history.

**Keel subsystem affected.** Direct editing UX and local operations (file 03). Conclusion: the general idea of history-free push/pull is broadly practiced and much of the foundational art is expiring; specific UI-method claims from later filings warrant a check. The geometric operations themselves (offset, extrude, fillet) are classical and free; only specific inference-rich UX claims (overlapping with section 3) are sensitive.

### 6.2 Variable-radius and setback vertex blends

**Patent family / status.** Setback vertex blends, variable-radius blends, and the smooth "patch" that fills the gap where three or more blends meet at a vertex were heavily developed inside ACIS (Spatial) and Parasolid in the late 1980s and 1990s. Patents from that era are EXPIRED. The features are documented in 1990s/2000s ACIS and Parasolid manuals (public reference material).

**Claims summary (era).** Methods for trimming each edge blend back by a setback distance near a common vertex and constructing a smooth multi-sided patch to fill the resulting gap; methods for blends whose radius varies along the spine.

**Keel subsystem affected.** Blending/filleting (local operations, files 03, 06). Conclusion: classical blends, including setback vertex blends and variable-radius blends as described in the 1990s literature and manuals, are EXPIRED art and safe. Only genuinely novel modern blend methods (a specific recent filing) would be a concern, and those are rare and narrow.

### 6.3 CAD healing and translation (ITI/CADfix, Spatial)

**Patent family / status.** Model-healing and translation/repair pipelines (ITI's CADfix, Spatial's repair service, Elysium) were commercialized through the late 1990s and 2000s. Foundational healing patents (gap stitching, tolerance management, geometry repair, defeaturing) from that period are EXPIRED or expiring. Specific public patent numbers were not isolated in this survey.

**Claims summary (era).** Automated detection and repair of model defects introduced by translation (gaps between faces, sliver faces, near-tangent edges), stitching faces into a watertight shell within tolerance, and defeaturing/simplification for downstream CAE.

**Keel subsystem affected.** Import/healing/tolerant topology (files 06, 08). Conclusion: the canonical healing operations are old enough to be largely free; the algorithms (stitching within tolerance, sliver removal) are also published in the academic literature. Low risk for classical healing; check only for narrow recent filings on specific automated-repair heuristics.

### 6.4 Implicit / lattice modeling (nTopology, Carbon, Materialise)

**Patent family / status.** nTopology markets "patent-pending" GPU-accelerated implicit-modeling and field-driven design (latticing, texturing, filleting, shelling via signed-distance/implicit fields). These are recent (roughly 2016 onward) filings and are PENDING or newly ACTIVE (expiring around 2036+). Carbon and Materialise hold additive-manufacturing and lattice-related patents of similar vintage.

**Claims summary (era, generic).** Methods for representing geometry as implicit fields and driving field operations (lattice generation, variable-thickness shelling, conformal texturing) by scalar/vector fields, with real-time CPU+GPU evaluation.

**Keel subsystem affected.** Implicit/SDF modeling and lattice generation (file 09). Conclusion: ACTIVE-RISK area for any "field-driven" feature that mirrors nTopology's specific methods, especially real-time GPU field evaluation and field-driven latticing as claimed. Safe alternatives: the underlying implicit/SDF mathematics (CSG on distance fields, TPMS surfaces such as gyroids defined by their classical analytic equations, marching cubes for extraction) are published prior art. Keel can implement implicit modeling on published algorithms; it should avoid copying nTopology's specific claimed field-driven workflows and any patented GPU-evaluation method.

### 6.5 OnShape / PTC cloud CAD

**Patent number.** US 11,170,134 B2 (and family members; example application US 2016/... "Multi-user cloud parametric feature-based 3D CAD system with sketching"); related PCT WO 2016/135674 A2.

**Title.** Multi-user cloud parametric feature-based 3D CAD system (with sketching).

**Assignee.** Onshape Inc. (acquired by PTC, 2019).

**Dates and status.** Filings from roughly 2015 to 2016; grants in the 2019 to 2021 range; estimated expiry around 2035 to 2037. ACTIVE.

**Claims summary.** Architecture and methods for a server-hosted parametric, feature-based CAD system supporting multiple simultaneous users editing one model in real time over the web, with automatic versioning/branching of the model document.

**Keel subsystem affected.** Sessions, collaboration, persistence (files 07, 08). Conclusion: relevant only if Keel ships a cloud multi-user real-time co-editing service. The kernel itself (geometry/topology) is unaffected. If a hosted collaborative editor is ever built on Keel, this family is a check-before-build item. The kernel core has no exposure here.

---

## 7. Open-source precedents and IP practice

### 7.1 OpenCASCADE licensing history

**Source.** Open CASCADE Technology licensing pages and FreeCAD forum discussions.

OpenCASCADE shipped under the GPL-incompatible "Open CASCADE Technology Public License" (OCTPL) for years, which made GPL programs that linked it (FreeCAD, Netgen, Elmer, Gmsh, Salome) hard or impossible to distribute cleanly. From version 6.7.0 (December 18, 2013) OCCT moved to LGPL-2.1 with minor additional permissions, becoming GPL-compatible. OCC has publicly stated it has not heard of patent issues from its users over many years. Lesson for Keel: license choice (not just patents) can break the downstream ecosystem; pick a permissive, patent-grant-bearing license from the start to avoid the years of friction OCCT caused.

### 7.2 Patent grants in OSS licenses (Apache 2.0 vs MIT vs GPLv3)

**Source.** Standard license texts and FOSS legal guidance.

MIT and BSD are silent on patents: they grant copyright permission but no explicit patent license, leaving users exposed to patent claims from contributors. Apache License 2.0 includes an express patent grant (Section 3) from every contributor and a patent-retaliation termination clause: if you sue an Apache project over patents in the work, your license terminates. GPLv3 (Section 11) likewise has an explicit patent grant and anti-tivoization/retaliation provisions; GPLv2 is largely silent on patents (an "implied license" is debated). Lesson for Keel: choosing Apache 2.0 (or dual MIT/Apache, the Rust ecosystem norm) gives contributors' patent grants to all users and adds retaliation protection, which is materially safer than MIT alone for a kernel that may attract corporate contributors holding CAD patents.

### 7.3 Rust ecosystem dual MIT/Apache-2.0 norm

**Source.** Rust project licensing convention.

The Rust standard library and the overwhelming majority of crates are dual-licensed MIT OR Apache-2.0. This lets downstream users pick MIT's simplicity or Apache's patent grant, and it means contributions come with Apache's patent grant available. Lesson for Keel: follow the ecosystem default (MIT OR Apache-2.0). It maximizes adoption and bakes in a patent grant path without extra legal work.

### 7.4 Pixar OpenSubdiv patent grant

**Source.** Pixar OpenSubdiv documentation and fxguide coverage.

The base Catmull-Clark algorithm (1978) was always free, but Pixar's refinements (semi-sharp creases per DeRose 1998, GPU evaluation, texture evaluation) were patented and licensable. When Pixar open-sourced OpenSubdiv (2013, moved to the Apache license that year), it included a free license to the relevant patents, and OpenSubdiv was then adopted by Blender, Maya, Houdini, 3ds Max, Cinema 4D, and Modo. Lesson for Keel: a vendor patent can be neutralized by an explicit royalty-free grant tied to an OSS release; if Keel ever wants subdivision-surface parity, building on OpenSubdiv (which carries the grant) is safer than reimplementing the patented creasing method from scratch.

### 7.5 Blender's patent-avoidance practice

**Source.** Blender developer discussions and OpenSubdiv adoption.

Blender has a documented practice of routing around patented algorithms: it adopted OpenSubdiv (with its patent grant) rather than risk an independent creasing implementation, and the project has historically been cautious about features touching known patents (for example deferring certain codec or algorithm features until patents expired or grants existed). Lesson for Keel: an OSS kernel can and should consciously choose published, pre-patent or patent-granted algorithms over reimplementing a patented method, and document that choice.

### 7.6 The academic-publication safe-harbor pattern

**Principle, with per-subsystem mapping.** An algorithm published in the open literature before any patent's earliest priority date is prior art and cannot be validly claimed afterward; it is therefore safe to implement. For Keel, the canonical algorithm for most core subsystems predates any live patent:

- NURBS evaluation, knot insertion, degree elevation, surface/surface intersection: Piegl and Tiller, The NURBS Book (1995/1997). Safe.
- Euler operators and B-rep topology: Baumgart (1972 winged-edge), Mantyla, An Introduction to Solid Modeling (1988). Safe.
- Boolean operations by boundary classification: 1980s solid-modeling literature (Requicha, Voelcker). Safe.
- Bezier clipping for intersections: Nishita, Sederberg, Kakimoto (1990). Safe.
- Catmull-Clark subdivision: Catmull and Clark (1978). Safe.
- Mesh arrangements / robust Booleans: Zhou et al. (2016), Cherchi et al. (2020) (file 09), published with reference code. Safe to reimplement.
- THB-splines for local refinement: Giannelli et al. (2012). Safe.

Conclusion: for every classical Keel subsystem, a published pre-patent algorithm exists. The discipline is simply to implement the published version and cite it, not to clone a vendor's specific modern claimed method.

### 7.7 Defensive publication for Keel's own work

**Principle.** If Keel develops a genuinely novel algorithm, publishing it (a dated commit plus a short technical note or preprint) before any competitor files creates prior art that prevents the competitor from patenting it and blocking Keel later. This is cheap insurance and aligns with OSS norms. It does not stop a pre-existing patent, but it protects the project's own innovations from future enclosure.

---

## 8. Risk matrix by Keel subsystem

Risk levels: LOW = rests on expired or never-patented art; MEDIUM = mostly safe but specific modern refinements may carry live claims, check before implementing those; HIGH = a known active patent family directly covers the obvious implementation, design around it.

| Keel subsystem | Risk | Why | Safe alternative / mitigation |
|---|---|---|---|
| Core NURBS (eval, knot insert, degree elevate) | LOW | Piegl-Tiller 1997, all pre-2000 | Implement from The NURBS Book; cite it |
| Topology / Euler operators (file 01) | LOW | Baumgart 1972, Mantyla 1988 | Implement classical operators |
| Classic Booleans (B-rep) | LOW | Requicha/Voelcker 1980s | Boundary-classification method |
| Curve/surface intersection | LOW | Bezier clipping, Nishita 1990 | Published clipping/subdivision |
| Classic blends/fillets, setback vertex, variable radius | LOW | ACIS/Parasolid 1990s patents expired | Implement per 1990s literature/manuals |
| Tessellation / adaptive faceting (file 05) | LOW | Viewpoint patent expired 2019; methods published | Published adaptive criteria |
| Subdivision surfaces | LOW | Base algo 1978 free; OpenSubdiv patent grant | Use OpenSubdiv or published methods |
| CAD healing / translation repair (file 06) | LOW-MED | Foundational patents expiring; methods published | Published stitch/sliver/defeature methods |
| Classic T-spline local refinement | LOW | US 7,274,364 expired March 2024 | Implement the now-expired construction |
| History/feature/regeneration model (file 07) | LOW | PTC 1980s patents expired | Build feature tree freely |
| Direct push/pull (geometry only) | MEDIUM | Operations classical; early UX patents expiring | Use classical offset/extrude/fillet |
| Direct editing WITH constraint inference / Live Rules (files 03, 04) | HIGH | Siemens US 9,235,659 (exp ~2034), US 10,140,389, US 10,176,291 active | Avoid auto-infer-and-resolve during edit; require explicit user constraints, or solve only user-specified relations |
| Hybrid mesh + B-rep in one body (convergent) (file 09) | HIGH | Siemens convergent-modeling family active (~2035+) | Keep mesh and B-rep as separate bodies; convert at boundaries via published mesh-arrangement Booleans |
| Analysis-suitable T-spline / IGA local refinement | MEDIUM | Possible Autodesk follow-on filings post-2010 | Use THB-splines (Giannelli 2012, published) |
| U-spline-class unstructured-mesh refinement | HIGH | Coreform U-spline family pending/active (~2037) | Use THB-splines or expired T-splines; do not implement the U-spline constraint-solve construction |
| Implicit / SDF modeling, field-driven lattices (file 09) | MEDIUM-HIGH | nTopology/Carbon/Materialise filings pending/active | Use published implicit math (CSG on SDF, analytic TPMS/gyroid, marching cubes); avoid claimed field-driven workflows and patented GPU eval |
| Cloud multi-user real-time co-editing | MEDIUM | Onshape/PTC family active (~2035+) | Only relevant if a hosted editor is built; kernel core unaffected |
| Project licensing | LOW (risk-reducing) | Apache patent grant + retaliation | Dual MIT OR Apache-2.0 (Rust norm) |

---

## 9. Summary of actionable conclusions

1. The entire classical core of Keel (NURBS, Euler operators, classic Booleans, classic blends including setback/variable-radius, tessellation, subdivision, healing) rests on expired or never-patented art. Implement the published versions and cite them.
2. Three HIGH-risk areas directly threaten obvious implementations of planned advanced features: (a) Siemens Synchronous Technology variational/Live-Rules direct editing (US 9,235,659 active to ~2034, plus US 10,140,389 and US 10,176,291); (b) Siemens convergent modeling (mixed mesh/B-rep in one body); and (c) Coreform U-splines. For each, a published patent-safe alternative exists: explicit-constraint direct editing, separate-body mesh/B-rep conversion via published mesh-arrangement Booleans, and THB-splines for local refinement.
3. The original T-spline patent (US 7,274,364) EXPIRED in March 2024, so classic T-splines with local refinement are now safe; this is a notable recent change in the landscape.
4. Implicit/lattice (nTopology et al.) and cloud co-editing (Onshape/PTC) are live patent areas, but the implicit underlying math is free and the cloud patents touch only an optional hosted service, not the kernel.
5. License Keel as dual MIT OR Apache-2.0 to inherit the Rust ecosystem norm and the Apache patent grant plus retaliation clause.
6. Adopt a documented defensive-publication habit for any novel Keel algorithm.

---

## References

Patents and applications (Google Patents / USPTO):

- US 7,274,364 B2, System and method for defining T-spline and T-NURCC surfaces using local refinements. https://patents.google.com/patent/US7274364B2/en
- US 2004/0189633 A1 (T-spline application). https://patents.google.com/patent/US20040189633A1/en
- EP 1,606,692 A2 (T-spline EP counterpart). https://patents.google.com/patent/EP1606692A2
- WO 2018/237067 A1, U-splines: splines over unstructured meshes (Coreform LLC). https://patents.google.com/patent/WO2018237067A1/en
- US 2019/0130058 A1 (U-splines application). https://www.freepatentsonline.com/y2019/0130058.html and https://uspto.report/patent/app/20190130058
- US 9,235,659 B2, Local behavior in a variational system (Siemens). https://patents.google.com/patent/US9235659B2/en
- US 10,140,389, Modifying constrained and unconstrained curve networks (Siemens). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/10140389
- US 10,176,291, Ordering optional constraints in a variational system (Siemens). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/10176291
- US 6,356,263 B2, Adaptive subdivision of mesh models (Viewpoint). https://patents.google.com/patent/US6356263B2/en
- US 6,950,099 B2, Approximation of Catmull-Clark subdivision surfaces by Bezier patches (Pixar). https://patents.google.com/patent/US6950099B2/en
- US 11,170,134 / WO 2016/135674 A2, Multi-user cloud parametric feature-based 3D CAD system (Onshape). https://patents.google.com/patent/WO2016135674A2/en

Legal and policy sources:

- Alice Corp. v. CLS Bank International, 573 U.S. 208 (2014). https://supreme.justia.com/cases/federal/us/573/208/ and https://en.wikipedia.org/wiki/Alice_Corp._v._CLS_Bank_International
- Electronic Frontier Foundation, "Saved by Alice." https://www.eff.org/alice

Open-source practice sources:

- Open CASCADE Technology licensing and FAQ. https://dev.opencascade.org/resources/licensing and https://dev.opencascade.org/resources/faq
- Open Cascade Technology (history). https://en.wikipedia.org/wiki/Open_Cascade_Technology
- Pixar OpenSubdiv initiative and patent grant. https://graphics.pixar.com/opensubdiv/blevins_opensubdiv.html and https://www.fxguide.com/fxfeatured/pixars-opensubdiv-v2-a-detailed-look/

Company and historical context:

- Autodesk acquires T-Splines (BYU). https://www.newswise.com/articles/autodesk-acquires-byu-prof-s-design-technology-t-splines
- Siemens Synchronous Technology and Live Rules. https://blogs.sw.siemens.com/solidedge/Synchronous-Technology-and-Live-Rules/
- Siemens convergent modeling (Parasolid). https://news.siemens.com/en-gb/parasolid-convergent-modeling-mixed-models/
- Ansys SpaceClaim and the Pull tool. https://en.wikipedia.org/wiki/SpaceClaim
- nTopology implicit / field-driven modeling. https://www.ntop.com/resources/blog/implicit-modeling-for-mechanical-design/
- ITI CADfix healing and translation. https://www.iti-global.com/interoperability-products/cadfix/
- PTC history (Pro/ENGINEER parametric). https://en.wikipedia.org/wiki/PTC_(software_company)
- Coreform U-splines technology page. https://coreform.tech/technology/u-splines

Academic prior-art anchors (full citations in files 01, 05, 09):

- Piegl, L., and Tiller, W. (1997). The NURBS Book (2nd ed.). Springer.
- Mantyla, M. (1988). An Introduction to Solid Modeling. Computer Science Press.
- Catmull, E., and Clark, J. (1978). Recursively generated B-spline surfaces on arbitrary topological meshes. Computer-Aided Design, 10(6).
- Nishita, T., Sederberg, T. W., and Kakimoto, M. (1990). Ray tracing trimmed rational surface patches (Bezier clipping). SIGGRAPH.
- Giannelli, C., Juttler, B., and Speleers, H. (2012). THB-splines: The truncated basis for hierarchical splines. CAGD, 29(7).
- Zhou, Q., Grinspun, E., Zorin, D., and Jacobson, A. (2016). Mesh arrangements for solid geometry. ACM TOG, 35(4).
