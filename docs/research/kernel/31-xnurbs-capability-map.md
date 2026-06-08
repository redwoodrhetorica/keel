# XNURBS Capability Map

**Purpose.** An authoritative, faithful catalog of what XNURBS (the commercial NURBS surfacing product) actually does, assembled as a coverage checklist for the open-source Keel kernel. The emphasis is completeness and fidelity to XNURBS's real behavior, not prose.

**Sources and method.** This catalog cross-checks XNURBS's own materials (xnurbs.com homepage, product page, "What's New" version notes for V6.2 and V7.0), the engineering.com product announcement, two Novedge write-ups (the "Meet xNURBS" overview and a "comprehensive guide" walking the UI), the official Rhino blog V6.1 release post, structural option descriptions from the Plasticity manual's XNurbs page, and independent practitioner discussion on the McNeel (Rhino) forum and SolidWorks forums. Where sources conflict, the forums are treated as the corrective to marketing copy.

**Verification labels.** Each capability is tagged:
- **[Verified]** stated consistently in XNURBS materials and corroborated by an independent source or multiple sources, or directly observable in the UI/option documentation.
- **[Claimed]** asserted by XNURBS marketing and plausible, but not independently corroborated (e.g., "milliseconds regardless of complexity", "fixes virtually all surfacing issues").
- **[Unconfirmed]** could not be confirmed from public sources; flagged so Keel does not over-scope.
- **[Caveat]** an independently reported limitation or honest qualification.

A recurring honesty note: XNURBS marketing language ("unlimited", "flawless", "fixes virtually all issues", "milliseconds regardless of complexity") is sweeping. Experienced reviewers confirm the tool is genuinely powerful for blends/patches with boundary continuity, but dispute the universality and especially the internal fairness claims. Both views are recorded below.

---

## 1. Core surfacing

- **Single-surface solve from N boundary curves/edges.** XNURBS takes a set of boundary curves or solid/surface edges and produces one NURBS surface satisfying them. Marketed as handling essentially any number of boundaries ("blending dozens of edges with one watertight G2 NURBS surface"). **[Verified]** the multi-edge blend is the headline use case; the "dozens" figure is **[Claimed]**.
- **Boundary plus internal constraint curves.** A generated surface can be constrained by internal (non-boundary) curves in addition to its boundary, pulling the interior to pass through or near them. XNURBS describes "boundary and non-boundary constraints." **[Verified]**
- **Point and point-set constraints.** Input can be points (not only curves); the surface is solved to satisfy point constraints. Users "select curves or points." **[Verified]**
- **Internal points/curves inside four-sided / untrimmed surfaces.** Added in the V6.2 / V7.0 generation: internal constraints (points and curves) can be placed inside an untrimmed four-sided surface. **[Verified]**
- **Curve-network-style input.** Because it accepts an arbitrary mix of boundary and internal curves plus points simultaneously, it functions like a network-surface tool but solved as one optimization rather than an interpolation grid. **[Verified]** (behavior); the "network" framing is inference, see Technical Approach.
- **Blend / fill / loft / multi-blend modes.** Demonstrated operations include surface blending across edges, hole/area filling, lofting (including a "Y-lofting" case), and multi-blend workflows, all through one tool/UI. **[Verified]**
- **Two-, three-, four-sided and "L"-shaped untrimmed surfaces.** The quad-sided path explicitly supports 2-, 3-, and 4-sided plus "L" configurations as untrimmed surfaces. **[Verified]**
- **One UI for all of the above.** "One simple UI for all kinds of NURBS modeling" - the same dialog handles blend, fill, loft, network-like, and point-fit cases. **[Verified]**

## 2. Continuity control

- **G0 (position) to boundary.** Supported, with a precision slider; default-quality target reported around G0 < 0.001 mm. **[Verified]**
- **G1 (tangent) to boundary and adjacent surfaces.** Supported, with a precision slider; reported target G1 < 0.05 degree, with later versions reducing tangent deviation (one source cites < 0.04 degree). **[Verified]**
- **G2 (curvature) to boundary and adjacent surfaces.** Central marketed capability; "watertight G2 NURBS surface" matching surrounding faces. **[Verified]** at boundaries; internal G2 fidelity is disputed, see Caveats.
- **G3 (curvature-rate) continuity.** XNURBS materials describe G3 (position + tangent + curvature + curvature change rate). Listed as a supported continuity level. **[Claimed/Verified-leaning]** stated by XNURBS; G3 is less prominent in demos than G2.
- **Per-edge continuity selection.** Continuity condition is set per boundary edge (different edges of the same surface can carry different G-levels), so a patch can be G2 to some neighbors and G0 to others ("smartly blends G2 and G0 continuity"). **[Verified]**
- **Match to surrounding existing surfaces.** Continuity is taken against adjacent faces, not only against free curves, so the new surface blends into an existing model with tangency/curvature. **[Verified]**
- **Adjustable continuity precision (sliders).** G0/G1 (and quality) tolerances are user-set via sliders rather than fixed, trading accuracy against surface simplicity. **[Verified]**

## 3. Messy-input tolerance

- **Gapped / non-contiguous boundary input.** A defining marketing claim: input curves need not form a clean closed loop; XNURBS solves across gaps and disjoint curves to a single surface. **[Claimed]** as a general guarantee; **[Verified]** that it tolerates imperfect input far better than classical loft/network tools.
- **Open boundaries.** Surfaces can be solved from open (non-closing) boundary sets. **[Verified]**
- **Overlapping / intersecting constraints.** It accepts redundant or intersecting constraint curves and resolves them through optimization rather than failing. **[Claimed]**
- **Auto-trimming of boundary curves.** From V5.2 onward it can automatically trim/clean boundary curves as part of the solve. **[Verified]**
- **Conflicting-constraint reporting.** V7.0 adds direct on-screen display of conflicting constraints, implying the solver detects over/under-constrained or contradictory input and surfaces it to the user. **[Verified]**
- **"Fixes virtually all surfacing issues."** Blanket robustness claim. **[Claimed]** and explicitly disputed by reviewers who document cases where the quad-sided solver refuses clean G2 input. **[Caveat]**

## 4. Gap filling and hole filling

- **Hole / patch filling in existing surfaces.** Fills holes or missing patches bounded by existing faces, with continuity (up to G2) to the surrounding faces, producing a watertight result. **[Verified]**
- **Watertight output across many edges.** The fill/blend is solved as one surface so the result closes the region without sliver gaps; "watertight" is repeatedly emphasized (bumper, jet-ski hull, hull examples). **[Verified]**
- **Surrounding-geometry patching.** Patching into existing geometry (not just free curves) is an explicit demonstrated workflow. **[Verified]**

## 5. Editing and dynamic update

- **Re-solve on input change (history / rebuild).** Surfaces support editing and rebuilding; changing inputs re-runs the optimization. SolidWorks/Rhino integration carries feature history so the XNURBS feature updates parametrically. V7.0's XNurbsSquare command adds history support. **[Verified]**
- **Interactive slider adjustment.** Continuity and quality sliders update the result; reviewers note that getting a good surface often means tuning sliders, not just clicking OK. **[Verified]** (and see Caveat on it not being purely one-click).
- **Tension and "Align to Next" controls.** V7.0 adds a tension option and an "Align to Next" option for shaping/aligning the solve. **[Verified]**
- **Dragging / real-time manipulation.** Real-time interactive editing/dragging is implied by the dynamic-update framing. **[Unconfirmed]** as a true drag-the-surface interaction versus re-solve-on-edit.

## 6. Quality and fairness

- **Energy-minimization fairing.** Among all surfaces meeting the constraints, it returns "the smoothest" by minimizing surface (bending) energy, analogized to a wooden batten minimizing bending energy. This is the quality engine. **[Verified]** as the stated method; see Technical Approach.
- **Curvature/quality emphasis and Class-A intent.** Marketed for high-quality, smooth surfaces; V7.0's XNurbsSquare is explicitly pitched for "Class A surfacing." **[Claimed]** (Class-A positioning).
- **Curvature analysis driven by host tools.** Quality is judged with the host CAD's curvature/zebra tools rather than a bespoke analyzer. **[Unconfirmed]** that XNURBS ships its own curvature analysis.
- **Control-point reduction for cleaner output.** V6.1 reduced the number of control points needed to hit the target precision, improving surface quality. **[Verified]**
- **[Caveat] Internal fairness disputed.** Independent reviewers report that while G1/G2 is met at boundaries, the surface interior can be wavy, and the patches carry dense control-point grids (inherited from dense input edges) that are heavier and less fair than a well-built native sweep/rebuild. Class-A-grade interior is not guaranteed.

## 7. Automation / one-click

- **Minimal setup workflow.** Select curves/edges/points, pick continuity, solve; no manual surface topology layout required. This is the central usability claim ("one super powerful NURBS tool"). **[Verified]**
- **Auto-solve to smoothest result.** The optimizer chooses the surface; the user does not hand-place control points. **[Verified]**
- **[Caveat] Not literally one-click for quality.** Reviewers stress that good results require understanding NURBS and tuning sliders/options; "you can't just click OK." So "automatic" is true for getting *a* surface, less true for getting an optimal one.

## 8. Output and conversion

- **Native NURBS output, no translation.** Output is standard NURBS usable directly by the host CAD "without any geometry translation"; it is the kernel's native surface type, not an imported mesh/foreign body. **[Verified]**
- **Trimmed or untrimmed output (user choice).** A "generated surface" checkbox controls whether the result is trimmed; the quad-sided path yields an untrimmed surface. The trim toggle changes the boundary only, not interior points. **[Verified]**
- **Single surface per solve.** Each solve yields one NURBS surface spanning the whole region (rather than a stitched patchwork), which is what enables watertight multi-edge blends. **[Verified]**
- **Degree / knots.** Output surface degree and knot vector are chosen by the optimizer to meet precision; control-point/span count varies with required accuracy. Specific fixed degree is not published. **[Unconfirmed]** (exact degree); **[Verified]** that CP/span count is solver-determined and accuracy-driven.
- **Untrimmed quad-sided spline surface.** The quad-sided option produces a clean untrimmed spline patch suitable for further native operations. **[Verified]**

## 9. Integrations and host round-tripping

- **xNURBS kernel + plugins architecture.** A core engine ("xnkernel.dll") drives the products; the kernel is licensed to ISVs and not sold to end users. **[Verified]**
- **SolidWorks add-in.** Full plugin with feature history, native SolidWorks surface output. **[Verified]**
- **Rhino plugin.** Commands include XNurbs and XNurbsSquare (quad-sided/Class-A). Distributed via Food4Rhino; free upgrades across V4/V5/V6 noted. **[Verified]**
- **Standalone application.** A standalone version exists in addition to the plugins. **[Verified]**
- **Other CAD hosts.** Marketed as available for additional CAD systems via the ISV kernel. **[Claimed]** (breadth of additional hosts not enumerated here).
- **Licensing models.** Sold as standalone (node-locked) and cloud-based licenses. **[Verified]**
- **Parametric round-trip in host.** Because the feature lives in the host's feature tree with history, edits round-trip within SolidWorks/Rhino. **[Verified]**

## 10. Claimed advantages over native and classical tools

- **vs classical Coons/Gordon/loft/network surfaces.** Those interpolate a clean, topologically regular boundary/grid; XNURBS instead solves a constrained energy-minimization over arbitrary, possibly messy input, yielding one surface where classical tools need clean contiguous loops or rectangular grids. **[Verified]** (architectural difference) / **[Claimed]** (universal superiority).
- **vs SolidWorks native surfacing.** Positioned to "fix virtually all surfacing issues" SolidWorks users hit (multi-edge blends, fills that native fill/boundary surfaces choke on). **[Claimed]**, broadly corroborated as a real pain reliever for hard blends. **[Verified-leaning]**
- **vs Rhino / Alias native tools.** Marketed as faster/easier for complex multi-edge blends and fills. **[Caveat]** McNeel-forum reviewers find Rhino's Sweep-2-Rails + MatchSurface + Rebuild + MoveUVN can beat XNURBS on fairness for simpler cases, and call some XNURBS marketing comparisons rigged. So the advantage is real for hard/messy/many-edge cases, contested for clean simple ones.
- **Speed.** "Solves virtually any NURBS surface in milliseconds regardless of constraint complexity." **[Claimed]** (speed is genuinely fast; "regardless of complexity" is marketing).

## 11. Reverse engineering / point clouds

- **Point and point-set fitting.** Genuinely accepts points/point-sets as constraints, so small scanned point sets can drive a surface. **[Verified]**
- **Dense point-cloud / scan-to-surface pipeline.** XNURBS is **not** marketed as a point-cloud reverse-engineering tool (no segmentation, decimation, or automatic mesh-to-NURBS pipeline found). For true scan reconstruction, dedicated tools (RhinoResurf, Autoshaper, etc.) occupy that space. **[Unconfirmed/Negative]** treat dense-cloud reverse engineering as out of XNURBS scope.

---

## Technical approach (as publicly described)

**Stated fact.** XNURBS frames surface creation as a constrained optimization: given a set of constraints (boundary curves/edges, internal curves, points, and per-edge continuity conditions G0/G1/G2/G3, plus continuity to adjacent surfaces), it searches over NURBS surfaces and returns the one that minimizes a surface "energy" while satisfying the constraints. The published analogy is a wooden batten that, when bent, settles into the minimum-bending-energy shape; the surface analog is the smoothest (fairest) surface among all feasible solutions. Continuity is enforced as constraints with user-set tolerances (G0 ~< 0.001 mm, G1 ~< 0.05 degree, tightened in later versions), and the optimizer chooses the control-point count / knotting needed to hit those tolerances, with later versions explicitly minimizing control points. Output is one native NURBS surface, trimmed or untrimmed at the user's choice.

**Reasoned inference (not stated verbatim).** The "energy" being minimized is almost certainly a fairness functional, plausibly a thin-plate / bending-energy style integral of squared (second) derivatives or curvature over the surface (the batten analogy is exactly the 1D thin-beam energy whose 2D analog is thin-plate energy). The continuity conditions read as hard or tolerance-bounded constraints, and the solve as a (likely nonlinear, iteratively reweighted) least-squares / variational optimization over the control net, since arbitrary G2/G3 boundary matching against existing surfaces is nonlinear. This variational, single-global-solve nature is what lets it absorb gapped, open, overlapping, and non-contiguous input that breaks classical constructors: where a Coons/Gordon/loft/network builder *interpolates* a clean, topologically regular boundary or grid and fails on dirty input, XNURBS *optimizes* a single surface to best satisfy whatever constraint soup it is given, treating gaps and inconsistencies as slack in the constraint set rather than as fatal topology errors. The known failure mode follows from the same nature: the optimizer can satisfy boundary continuity tightly while leaving the interior wavy or control-point-heavy, because interior fairness is one term traded off against constraint satisfaction rather than an independently guaranteed property. The exact functional, weighting, solver, and degree/knot strategy are proprietary and not published. **[Inference, clearly not a quoted XNURBS statement.]**

---

## Capability checklist (flat, coverage audit)

- Solve a single NURBS surface from an arbitrary number of boundary curves
- Solve from solid/surface edges (not just free curves) as boundaries
- Accept boundary plus internal constraint curves in one solve
- Accept point constraints (single points)
- Accept point-set constraints (multiple points)
- Place internal points inside an untrimmed four-sided surface
- Place internal curves inside an untrimmed four-sided surface
- Network-surface-like solve from mixed curves + points simultaneously
- Surface blending across many edges in one operation
- Hole filling bounded by existing faces
- Patch/area fill into surrounding existing geometry
- Lofting (including Y-loft topology)
- Multi-blend workflow in one tool
- 2-sided untrimmed surface
- 3-sided untrimmed surface
- 4-sided untrimmed surface
- "L"-shaped untrimmed surface
- Quad-sided ("XNurbsSquare") Class-A-oriented surface command
- Single unified UI for all surfacing modes
- G0 (position) continuity to boundaries
- G1 (tangent) continuity to boundaries and adjacent surfaces
- G2 (curvature) continuity to boundaries and adjacent surfaces
- G3 (curvature-rate) continuity (stated)
- Per-edge selection of continuity level on one surface
- Mixed continuity on one patch (e.g., G2 some edges, G0 others)
- Continuity matched against existing adjacent surfaces, not only curves
- User-set G0 precision tolerance (slider; ~< 0.001 mm)
- User-set G1 precision tolerance (slider; ~< 0.05 degree)
- Tightened tangent deviation in later versions (~< 0.04 degree)
- Quality/fairness slider control
- Tension control (V7.0)
- "Align to Next" control (V7.0)
- Tolerate gapped boundary input
- Tolerate non-contiguous / disjoint boundary curves
- Tolerate open (non-closing) boundaries
- Tolerate overlapping / intersecting constraint curves
- Automatic trimming/cleanup of boundary curves
- On-screen reporting of conflicting constraints
- Energy-minimization (bending-energy) fairing of the surface
- Return the smoothest surface among all feasible solutions
- Solver-chosen control-point / span count to meet precision
- Control-point-count reduction for cleaner output
- Watertight multi-edge output (no slivers)
- Output as native host NURBS with no geometry translation
- Trimmed-output option
- Untrimmed-output option
- User toggle between trimmed and untrimmed result
- Single surface spanning the whole region per solve
- Parametric feature with history in host (re-solve on input change)
- Editable / rebuildable result
- Interactive slider-driven re-solve
- Fast solve (sub-second / "milliseconds" claimed)
- SolidWorks add-in integration
- Rhino plugin integration (XNurbs, XNurbsSquare commands)
- Standalone application
- ISV-licensable kernel (xnkernel.dll)
- Standalone and cloud-based licensing options
- Free point-set fitting for small scanned point inputs
- Positioned to fix hard multi-edge blends native CAD struggles with
- Faster/easier complex blends than classical loft/boundary/network tools
- Dark-mode / Rhino 8 compatibility (V7.0)
- Free upgrade path across major versions (V4/V5/V6)

---

## Caveats reported by reviewers (for honesty)

- Interior surface flow can be wavy even when boundary continuity is met; G2 at edges does not guarantee fair interior.
- Output patches can be control-point-heavy (dense grids inherited from dense input edges), making later manual editing hard.
- "Optimize for quad-sided surface" historically disallowed internal constraints (eased only in the V6.2/V7.0 generation, which added internal points/curves for four-sided surfaces).
- Documented cases of the quad-sided solver refusing clean, valid G2 input (predictability gap).
- Real quality requires NURBS knowledge and slider tuning; not truly "click OK" for Class-A.
- For simple cases, native tools (Rhino Sweep-2-Rails + MatchSurface + Rebuild + MoveUVN) can produce fairer, lighter surfaces.
- Some XNURBS promotional comparisons were called rigged by experienced forum members (suboptimal native-tool setups used as the baseline).
- "Unlimited / milliseconds regardless of complexity / fixes virtually all issues" are marketing absolutes, not guarantees.

---

## References

- xNURBS homepage: https://www.xnurbs.com/
- xNURBS product page: https://www.xnurbs.com/product/
- xNURBS "What's New" (V7.0 / V6.2 notes): https://www.xnurbs.com/whats-new/
- engineering.com, "xNURBS releases NURBS software": https://www.engineering.com/xnurbs-releases-nurbs-software/
- Novedge, "Meet xNURBS": https://novedge.com/blogs/news/meet-xnurbs-the-most-powerful-and-flexible-nurbs-based-3d-modeling-solution-on-the-market
- Novedge, "Design Stunning 3D Models with xNURBS" (UI/options walkthrough): https://novedge.com/blogs/design-news/design-stunning-3d-models-with-xnurbs-a-comprehensive-guide-to-high-quality-surface-creation
- Rhino blog, "xNURBS Rhino Plugin V6.1 Released": https://blog.rhino3d.com/2023/03/xnurbs-rhino-plugin-v61-released.html
- Plasticity manual, XNurbs options page: https://doc.plasticity.xyz/solid/xnurbs
- Food4Rhino, XNurbs Rhino Plugin listing: https://www.food4rhino.com/en/app/xnurbs-rhino-plugin
- McNeel forum, "Unfair and set-up comparisons by xNURBS" (critical review): https://discourse.mcneel.com/t/unfair-and-set-up-comparisons-by-xnurbs/137130
- McNeel forum, "xNURBS releases a ground-breaking NURBS software": https://discourse.mcneel.com/t/xnurbs-releases-a-ground-breaking-nurbs-software/49049
- SolidWorks forum, "xNURBS SolidWorks Addin": https://forum.solidworks.com/thread/237602
- SolidWorks forum, "XNurbs - further investigations": https://forum.solidworks.com/thread/216269
