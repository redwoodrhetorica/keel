# Parasolid Capability Map

An authoritative inventory of the functional capabilities exposed by the Siemens Parasolid geometric modeling kernel through its PK (Parasolid Kernel) interface. This serves as a coverage checklist against which the open-source Keel kernel can be measured. The inventory is faithful to Parasolid's published functionality and does not editorialize about Keel.

## Sources and method

This map is assembled from Siemens' published Parasolid documentation and product material, cross-checked across multiple independent sources to reflect real shipped functionality rather than marketing. Primary sources include the Parasolid Functional Description and Overview (V13 and V35 chapter mirrors at q-solid.com), the PK Interface Programming Reference, Siemens PLM Components product pages and release-highlight blogs, the Tech Soft 3D Parasolid distributor pages, HOOPS Visualize integration documentation (which wraps the PK interface), CAD-interoperability vendor write-ups, and encyclopedic summaries. Where I am uncertain whether a capability is provided by Parasolid itself rather than by the host application or by Siemens' separate D-Cubed components, I say so explicitly. PK function-family names are given in parentheses for specificity; some names follow Parasolid's standard `PK_<CLASS>_<verb>` convention and reflect documented families. Version-introduced features (convergent modeling at v26, tolerant modeling at v16, synchronous/direct editing at v20, lattices in the v32/33 era) are noted where visible.

A general caveat on scope: Parasolid is a geometry and topology kernel. It does NOT provide a constraint solver, a feature/history tree, dimensional/parametric sketching, or assembly mating logic. Those are host-application concerns, with 2D/3D constraint solving supplied by Siemens' separate D-Cubed (DCM/2D DCM, 3D DCM, AEM, PGM, CDM) components, not by Parasolid. Free-form interactive deformation in the "DSM" sense is likewise largely a D-Cubed/host concern; see the Deformation section for what Parasolid itself does and does not do.

---

## 1. Model creation

- Primitive solids: create block, cylinder, cone, sphere, torus, and prism solids directly from dimensional parameters (`PK_BODY_create_solid_block`, `PK_BODY_create_solid_cyl`, `PK_BODY_create_solid_cone`, `PK_BODY_create_solid_sphere`, `PK_BODY_create_solid_torus`, `PK_BODY_create_solid_prism`).
- Sheet primitives: create planar and other sheet bodies directly (for example a sheet from a single surface bounded by a profile).
- Wire bodies: create wireframe bodies from curves and points; minimal/empty bodies.
- Topology-from-geometry construction: build a body by supplying geometry and topology directly (the lower-level `PK_BODY_make_*` / topology-creation families and the "create bodies from geometry" path), allowing direct construction of an exact body when the application already has surfaces, curves, and connectivity.
- Empty/acorn bodies: create a minimal body (a single vertex, or an empty body) as a seed for incremental construction via Euler operations.
- Geometry creation: create exact curves (line, circle, ellipse, parabola, hyperbola, B-curve/NURBS, intersection curves) and surfaces (plane, cylinder, cone, sphere, torus, blended/swept/spun surfaces, B-surface/NURBS) as standalone geometric entities (`PK_CURVE_*`, `PK_SURF_*`, `PK_BCURVE_*`, `PK_BSURF_*`).
- Helix creation: create helical curves (documented helix-creation capability; helix geometry is supported as a construction curve for sweeping threads/springs).

## 2. Body types and topology

- Four body types: solid (manifold closed), sheet (open or closed shells of faces with no enclosed volume claimed as solid), wire (edges and vertices only), and general/mixed bodies (any combination, including non-manifold).
- General bodies: a single body may simultaneously contain solid regions, sheet faces, and wire edges, and may be non-manifold (more than two faces meeting at an edge, or wires embedded in solids).
- Topology hierarchy: body, region (a connected portion of space, solid or void), shell (a connected set of faces/edges bounding a region), face, loop, fin/half-edge, edge, and vertex. Parasolid exposes navigation and query across the entire hierarchy (`PK_BODY_ask_*`, `PK_FACE_ask_*`, `PK_TOPOL_*`).
- Regions: explicit modeling of solid and void regions, including the infinite background void; region-level queries and operations.
- Non-manifold support: edges shared by more than two faces, embedded wires and laminae, and mixed-dimensional bodies are first-class.
- Orientation and senses: faces carry an outward sense relative to their region; fins carry edge senses for consistent loop traversal.

## 3. Boolean operations

- Unite, subtract, intersect: full Boolean set operations between bodies (`PK_BODY_boolean_2`, formerly `PK_BODY_unite`/`PK_BODY_subtract`/`PK_BODY_intersect`).
- Booleans across body types: operate on solids, on sheets (sheet-solid and sheet-sheet), and on general bodies; sheets can be used as tools to divide solids.
- Multiple tool bodies: a single Boolean call can apply a list of tool bodies to a target.
- Local (selective) Booleans: comparison of selected face pairs between target and tool rather than whole bodies, faster than a global Boolean but without a global topological-consistency guarantee (`PK_FACE_boolean_2`).
- Imprint option: Booleans can imprint resulting intersection curves as edges while optionally keeping bodies separate.
- Coincident-face and tangent handling: robust treatment of coincident, tangent, and partially overlapping faces.
- Mixed-model Booleans: Booleans between classic B-rep bodies and facet (mesh) bodies, and within mixed bodies, with automatic topological matching (convergent modeling).

## 4. Local operations

- Tweak face / change surface: move the surface attached to one or more faces to a new surface (or move/rotate it), recomputing adjacent edges and faces (`PK_FACE_tweak_surf`, `PK_FACE_change`).
- Move faces: translate, rotate, or otherwise transform a selected set of faces, regenerating the surrounding topology (`PK_FACE_transform` / move-faces family).
- Offset face: offset selected faces by a distance, recomputing intersections with neighbors (`PK_FACE_offset`, `PK_FACE_offset_2`).
- Taper / draft faces: apply single-sided or double-sided taper (draft angle) to faces about a parting reference (`PK_FACE_taper`); extended in recent versions to mix classic and facet faces.
- Replace / swap surface: substitute the underlying surface of a face.
- Delete face with healing: remove faces and heal the wound by extending and re-intersecting neighbors, or by capping (`PK_FACE_delete_2` with heal options).
- Local Booleans on faces: see selective Booleans above.

## 5. Hollowing and offsetting

- Hollow / shell: convert a solid into a thin-walled shell of specified wall thickness, optionally piercing selected faces to leave them open (`PK_BODY_hollow`, `PK_BODY_hollow_2`).
- Thicken sheets: offset a sheet body to both sides (or one side) to produce a solid of given thickness (`PK_BODY_thicken`, `PK_BODY_thicken_2`).
- Offset body: offset all faces of a body outward or inward by a distance, with self-intersection resolution (`PK_BODY_offset`, `PK_BODY_offset_2`).
- Offset face (local): see local operations.
- Variable/face-specific thickness: per-face wall thickness in hollow operations.
- Mixed classic/facet: hollow, thicken, and body-offset extended to operate on bodies mixing classic and facet faces.

## 6. Blending and chamfering

- Edge blends, constant radius: replace edges with rolling-ball constant-radius blend faces (`PK_EDGE_set_blend_constant`, `PK_BODY_fix_blends` / `PK_BODY_make_blends`).
- Edge blends, variable radius: radius varying along the edge by a law or by control values (`PK_EDGE_set_blend_variable`).
- Rolling-ball and other cross-sections: circular rolling-ball blends plus conic and other cross-section blends.
- Face-face blends: blend between two sets of faces that need not share an edge (`PK_FACE_make_blend` / face-face blend family), including blends with a chosen spine.
- Vertex blends / setback: blend the corner where multiple blended edges meet, including setback vertex blends.
- Chamfers: replace edges with flat chamfer faces, by equal offsets, two offsets, or offset-and-angle; apex-range chamfers on mixed models (`PK_EDGE_set_chamfer` / chamfer family).
- Hold lines and ranges: blends defined to a hold line (tangent-to or through a curve) and range-controlled / apex-range blends.
- Blend networks: simultaneous creation of interacting blends across many edges, with mitring and overflow handling where blends run off the body or meet other blends.
- Blend overflow control: documented handling of blends that overflow onto adjacent faces or off edges.
- Blend recognition and removal: detect existing blend faces and remove (unblend) them, restoring the underlying sharp edges (blend recognition / `PK_FACE_*` unblend family).
- G2/curvature-continuous blends: curvature-continuous blend improvements in recent versions.
- Blends onto facet faces: classic blends applied where one side is facet geometry (convergent modeling).

## 7. Sweeping, spinning, and lofting

- Sweep (translate): create a swept body by translating a profile along a vector, producing lateral faces and edges (`PK_BODY_sweep`).
- Sweep along a path: sweep a profile along a 3D curve/path, including for CAM toolpath-style geometry (sweep-along-path family).
- Spin / revolve / swing: revolve a profile about an axis through an angle to create solids or sheets (`PK_BODY_spin`).
- Extrusion of bodies: extrude profiles/sheets into solids (creation-of-extruded-bodies family).
- Lofting / skinning: create a body or surface that passes through a sequence of profile sections, with continuity and guide-curve control (advanced surfacing / loft family).
- Surfaces from boundaries: build a face or sheet bounded by a set of curves (n-sided / boundary surface).
- Tabulated/ruled surfaces: ruled surfaces between two curves and extruded (tabulated) surfaces.

## 8. Sheet and surface operations

- Extend sheet: extend the boundary of a sheet body (extend its faces' surfaces) to a distance or up to other geometry (`PK_FACE_extend` / sheet-extend family).
- Knit / sew sheets: stitch a collection of sheet bodies/faces into a larger sheet or into a closed solid, within tolerance (`PK_BODY_knit_sheets` / sew family).
- Trim: trim sheets and faces against curves, surfaces, or other sheets (sheet-trimming family).
- Surface by boundaries: create surfaces filling boundary curve sets (see advanced surfacing).
- Thicken: see hollowing/offsetting.
- Make a face from a surface: attach an exact surface as a face within a sheet body.

## 9. Imprinting and sectioning

- Imprint curves: imprint curves onto faces to split them into new faces/loops (`PK_FACE_imprint_curve`, `PK_BODY_imprint_curve`).
- Imprint bodies: imprint one body onto another (the intersection graph), creating shared edges without uniting (general imprinting; `PK_BODY_imprint_body`).
- Section by surface/plane: non-destructive sectioning of a body by a plane or surface, returning section curves or splitting the body (`PK_BODY_section`, section family).
- Split body/face: split faces or bodies along imprinted or intersection curves.
- Slice: planar slicing, including the additive-manufacturing slicing path that takes a single surface tool plus a list of offsets to produce polyline results.

## 10. Tapering and draft (mold-related)

- Draft / taper faces: add draft about a parting plane or parting curve, single- and double-sided (`PK_FACE_taper`); see local operations.
- Parting reference: taper relative to a neutral/parting surface or pull direction.
- Note: Parting-line splitting and core/cavity separation are typically performed by combining sectioning, imprinting, splitting, and Boolean operations rather than by a single "split for mold" PK call; dedicated mold-splitting workflow logic generally lives in the host application.

## 11. Deformation

- What Parasolid provides: face/surface tweaks, offsets, tapers, and replacement of underlying surfaces (sections 4 and 6), which deform a model by changing the geometry attached to faces and re-solving topology. Parasolid also performs B-surface/B-curve manipulation at the geometry level.
- What Parasolid does NOT provide as a packaged feature: general free-form interactive deformation (control-cage push/pull, global bend/twist/taper-as-deformation, emboss, wrap-onto-surface). I am NOT aware of these being first-class Parasolid PK operations; free-form deformation (the "DSM"-style space deformation) is a Siemens D-Cubed / host-application capability rather than a Parasolid kernel function. Where a host offers emboss or wrap, it is typically built atop Parasolid's imprint, offset, sheet, and Boolean primitives.

## 12. Patterns and instancing

- Instancing model: the PK supports instances (`PK_INSTANCE`) and an assembly model (`PK_ASSEMBLY`) so that a part body can be referenced multiple times under transforms within an assembly structure, rather than copied.
- Assemblies: hierarchical assembly of parts and sub-assemblies with per-instance transforms (`PK_ASSEMBLY`, `PK_PART`).
- Pattern creation: replicate features/faces or bodies in linear, circular, and general patterns. Note: high-level "feature pattern" semantics often live in the host; at the kernel level patterning is realized by transformed copies plus Booleans/imprints, with instancing used where true references are wanted.
- Transform/copy: copy and transform bodies and entities (`PK_ENTITY_copy`, `PK_BODY_transform`, `PK_TRANSF_*`).

## 13. Convergent modeling and mesh

- Facet bodies as first-class geometry: meshes serve as surfaces and polylines serve as curves, usable across many PK functions just like classic geometry (convergent modeling, introduced at v26 and expanded through the v3x releases).
- Facet model structure: faces backed by meshes, edges by polylines, integrated into the same topology hierarchy as classic bodies.
- Classic-to-facet conversion: convert a classic B-rep body to a facet body (`PK_BODY_make_facet_body`).
- Mesh-to-B-rep / mixed: combine B-rep/NURBS faces and faceted faces within a single solid body; automatic topological matching between mesh faces and classic faces with variable overlap.
- Booleans, blends, tapers, offsets, hollow, thicken on facet and mixed bodies (see those sections), including blends from classic onto facet faces and mixed apex-range chamfers.
- Mesh repair: detect and repair mesh degeneracies and flat facets, improve edge merging in Booleans, and maintain mixed-body integrity.
- Lattice modeling (native, v32/33 era): lattice attachment, clipping, mesh generation from lattices, intelligent boundary handling, and transmission; improved facet-mesh creation from self-intersecting lattice bodies producing disjoint meshes.

## 14. Tessellation and rendering

- Faceting for display: tessellate a body or topology into facets within specified linear/angular tolerances for rendering (`PK_TOPOL_facet`, `PK_TOPOL_facet_2`).
- Render facets and lines: produce facets plus edge/silhouette line output and send to graphical output (`PK_TOPOL_render_facet`, `PK_TOPOL_render_line`).
- Hidden-line and wireframe rendering: precise hidden-line removal and wireframe output for drawing-quality views.
- Silhouette / outline curves: compute exact silhouette (visible) outlines and invisible (interior) outlines for a view direction, including spun outlines (visible-only or visible-plus-interior).
- Incremental/concurrent tessellation: incremental tessellation and concurrent rendering with topology/geometry matching options for performance.
- Sectioning for drawings: section output usable to generate section views in drawings (see sectioning).

## 15. Geometric and model interrogation

- Mass properties: volume, surface area, centroid, and moments/products of inertia for bodies, faces, and regions, to a requested accuracy (`PK_TOPOL_eval_mass_props`).
- Distance and clash: minimum/maximum distance between entities and clash/interference detection between bodies (min/max distance and clash family).
- Point classification: classify a point as inside, outside, or on a body/region (containment test).
- Intersections: curve-curve, curve-surface, surface-surface intersection evaluation, and entity-entity intersection curves.
- Bounding boxes: axis-aligned and precise bounding boxes for entities (`PK_TOPOL_find_box`, precise-box family).
- Curve/surface evaluation: evaluate position, derivatives, normals, parameters, and curvature on curves and surfaces (`PK_CURVE_eval`, `PK_SURF_eval`, curvature queries).
- Surface analysis: curvature analysis, draft analysis inputs, and other geometric evaluations.
- Entity comparison: compare two entities/bodies for geometric and topological equivalence.
- Topological inquiries: full navigation and counting queries across the topology hierarchy.
- Checking: model validity/consistency checking of bodies and geometry (`PK_BODY_check`, `PK_GEOM_check`, checker family) reporting self-intersections, inconsistencies, and invalid geometry.

## 16. Tolerant modeling

- Tolerant edges and vertices: edges/vertices carrying a local tolerance so that geometry that does not meet exactly can still form a valid body (introduced at v16-era).
- Tolerant import: accept imported geometry from foreign systems whose surfaces/curves are within tolerance rather than exact.
- Mixed precision: exact (machine-precision) and tolerant entities coexist in one body; operations propagate and manage tolerances.
- Local precision / session precision: control over precision settings governing operations.

## 17. Foreign geometry

- User-defined surfaces and curves: applications can supply their own surface and curve definitions through the foreign geometry interface, with callbacks for evaluation, so Parasolid can model bodies whose geometry it does not natively represent (`PK_SURF`/`PK_CURVE` foreign-geometry / evaluator callback family).
- Foreign-geometry operations: such geometry participates in modeling operations via the supplied evaluators, and can later be converted to native NURBS where needed.

## 18. Attributes and user data

- Attribute system: define attribute types and attach typed attributes (integers, reals, strings, vectors, etc.) to any entity (`PK_ATTDEF_create`, `PK_ATTRIB_create`, attribute family).
- System attributes: predefined attributes such as color/graphical attributes, density, names, and hatch.
- User fields: a block of bytes on every entity for application data (`PK_ENTITY_set_user_field`, `PK_ENTITY_ask_user_field`).
- Color and graphical attributes: per-face/per-body color and related display attributes.
- Attribute propagation: control over how attributes are inherited, split, merged, or deleted as topology changes through operations.

## 19. Sessions, partitions, rollback, and persistence

- Sessions: a Parasolid session holds the model and global settings; start/stop and configuration (`PK_SESSION_*`).
- Partitions: independent containers of bodies/assemblies that can be saved, loaded, and rolled back separately (`PK_PARTITION_*`).
- Pmarks and rollback: partition marks record states; roll a partition forward/backward between pmarks (`PK_PMARK_create`, `PK_PMARK_goto`), enabling undo/redo and history rollback.
- Transactions / deltas: operations recorded as deltas enabling rollback and incremental save; delta transmit/receive (`PK_PARTITION_transmit`, `PK_PARTITION_receive`, `PK_PARTITION_receive_deltas`).
- Save / restore (XT format): transmit and receive bodies, partitions, and assemblies to/from the neutral XT format, text `.x_t` and binary `.x_b`, storing exact B-rep at machine precision (`PK_PART_transmit`/`PK_PART_receive`, `PK_BODY_transmit`).
- Versioning: read older XT versions; control the version a model is written as for forward/backward interoperability.
- Journaling: a journal file can record API calls for replay/debugging.

## 20. Repair and simplification

- Body healing / geometry repair: repair invalid or inconsistent geometry and topology, including tolerant-modeling-based healing of imported data.
- Simplification / canonical conversion: simplify B-surfaces/B-curves to analytic geometry where possible (for example a NURBS that is really a cylinder becomes an exact cylinder), reducing data and improving robustness (geometry simplification / `PK_GEOM_simplify`-style family).
- Defeaturing: remove small or unwanted features such as small blends, holes, and bosses (often realized via blend removal, delete-face-with-heal, and redundant-entity removal).
- Blend removal: see blending; recognize and remove blend faces.
- Redundant entity removal: remove redundant edges/vertices and merge faces that share the same surface (face merging / clean-up family).
- Knit/merge clean-up: merge coincident or mergeable topology after operations.

## 21. Mid-surface and abstraction (CAE idealization)

- Mid-surface creation: derive mid-surfaces between pairs of opposing faces of thin-walled solids to produce a sheet idealization for CAE meshing (midsurface family; capability is documented for Parasolid-based idealization, with portions of the workflow sometimes completed by the host).
- Idealization support: thickness assignment and abstraction operations supporting downstream CAE.
- Note: I am confident Parasolid supports the geometric mid-surface and offset machinery; the precise extent of an automated, fully packaged "midsurface" PK call versus host-assembled workflow varies by release, so treat the fully automatic end-to-end midsurfacer as partly host-driven unless confirmed against a specific PK version.

## 22. Other documented capabilities

- Euler operations: low-level topology editing primitives (make/kill vertex-edge-face-shell-loop, etc.) for direct, controlled construction and modification (`PK` Euler-op family).
- NURBS conversion: convert any face/edge geometry to NURBS (B-surface/B-curve) form for export or downstream use, and convert NURBS back to analytic form via simplification.
- B-curve / B-surface toolkit: create, evaluate, split, and edit NURBS curves and surfaces with arbitrary degree and adaptive tolerance.
- Replace / general topology queries: general topology-replacement and query operations across the hierarchy.
- Cosmetic / thread features: cosmetic thread representation is generally carried as attributes/cosmetic geometry rather than modeled solids; Parasolid stores such data via its attribute system while the host defines the feature semantics.
- Symmetric multiprocessing: selected operations use SMP for performance; the kernel is thread-safe for concurrent use.
- Tolerant and exact coexistence, foreign geometry, and convergent geometry all interoperate within single operations as noted above.

---

## Capability checklist

A flat, terse inventory of distinct Parasolid capabilities for coverage auditing.

1. Create primitive block solid
2. Create primitive cylinder solid
3. Create primitive cone solid
4. Create primitive sphere solid
5. Create primitive torus solid
6. Create primitive prism solid
7. Create sheet primitive bodies
8. Create wire bodies from curves/points
9. Create empty / acorn (single-vertex) body
10. Create body directly from supplied geometry and topology
11. Create exact analytic curves (line, circle, ellipse, conics)
12. Create exact analytic surfaces (plane, cylinder, cone, sphere, torus)
13. Create NURBS B-curves (arbitrary degree)
14. Create NURBS B-surfaces (arbitrary degree)
15. Create helix curves
16. Solid body type
17. Sheet body type
18. Wire body type
19. General / mixed (non-manifold) body type
20. Region modeling (solid and void, incl. infinite background)
21. Full topology hierarchy (body, region, shell, face, loop, fin, edge, vertex)
22. Non-manifold edges/wires/laminae support
23. Topology navigation and counting queries
24. Boolean unite
25. Boolean subtract
26. Boolean intersect
27. Booleans on solids
28. Booleans on sheets (sheet-solid, sheet-sheet)
29. Booleans on general bodies
30. Multiple tool bodies per Boolean
31. Local / selective face-pair Booleans
32. Boolean with imprint-only option
33. Coincident/tangent face handling in Booleans
34. Mixed classic+facet Booleans (convergent)
35. Tweak face / change attached surface
36. Move faces (translate/rotate/transform)
37. Offset face (local)
38. Taper / draft faces (single- and double-sided)
39. Replace / swap underlying surface
40. Delete face with healing (extend-and-heal or cap)
41. Hollow / shell solid to wall thickness
42. Pierce selected faces open during hollow
43. Per-face variable wall thickness
44. Thicken sheet to solid
45. Offset whole body (with self-intersection resolution)
46. Hollow/thicken/offset on mixed classic+facet bodies
47. Constant-radius edge blends (rolling-ball)
48. Variable-radius edge blends
49. Conic / non-circular blend cross-sections
50. Face-face blends (no shared edge)
51. Vertex blends and setback vertex blends
52. Chamfers (equal offset, two offset, offset-angle)
53. Apex-range chamfers
54. Blends to hold line / curve
55. Range-controlled blends
56. Blend networks with mitring
57. Blend overflow handling
58. Blend recognition (detect blend faces)
59. Blend removal / unblend
60. G2 / curvature-continuous blends
61. Blends from classic onto facet faces
62. Sweep by translation (with lateral faces)
63. Sweep along 3D path
64. Spin / revolve / swing about axis
65. Extrude profiles/sheets into solids
66. Loft / skin through profile sections
67. Loft with guide curves and continuity control
68. Surface from boundary curves (n-sided)
69. Ruled and tabulated surfaces
70. Extend sheet (to distance or to geometry)
71. Knit / sew sheets into larger sheet or solid
72. Trim sheets/faces against curves/surfaces/sheets
73. Imprint curves onto faces
74. Imprint one body onto another
75. Section body by plane/surface (non-destructive)
76. Split body/face along curves
77. Planar slice (incl. additive-manufacturing slicing with offset list)
78. Draft about parting plane/curve/neutral surface
79. Mold core/cavity split (assembled from section+imprint+split+Boolean; host-driven workflow)
80. Surface/face geometry deformation via tweak/offset/taper
81. Free-form space deformation / bend / twist / emboss / wrap (NOT native Parasolid; D-Cubed/host)
82. Instances of bodies (PK_INSTANCE)
83. Assembly model with per-instance transforms (PK_ASSEMBLY)
84. Linear / circular / general pattern replication
85. Copy and transform bodies/entities
86. Facet (mesh) bodies as first-class geometry
87. Polylines as curves in facet bodies
88. Facet model integrated into topology hierarchy
89. Convert classic body to facet body
90. Mix B-rep and facet faces in one solid
91. Automatic topological matching (mesh-mesh, mesh-classic)
92. Mesh repair (degeneracies, flat facets, edge merging)
93. Native lattice attachment/clipping/mesh generation/transmission
94. Tessellate body/topology to facets within tolerance
95. Render facets plus edge/silhouette lines
96. Hidden-line removal and wireframe output
97. Exact silhouette / spun outline (visible and interior)
98. Incremental / concurrent tessellation
99. Section output for drawing views
100. Mass properties (volume, area, centroid, inertia)
101. Minimum/maximum distance between entities
102. Clash / interference detection
103. Point-in-body classification (inside/outside/on)
104. Curve-curve / curve-surface / surface-surface intersection
105. Axis-aligned and precise bounding boxes
106. Curve/surface evaluation (position, derivatives, normals, params)
107. Curvature and surface analysis
108. Entity / body equivalence comparison
109. Model validity and consistency checking
110. Tolerant edges and vertices
111. Tolerant import of foreign geometry
112. Mixed exact + tolerant precision in one body
113. Session / local precision control
114. Foreign (user-defined) surfaces and curves via evaluator callbacks
115. Foreign geometry participates in modeling operations
116. Convert foreign/NURBS geometry to native form
117. Typed attribute system on any entity
118. System attributes (color, density, name, hatch)
119. Per-entity user fields (raw bytes)
120. Color / graphical attributes per face/body
121. Attribute propagation control through operations
122. Sessions (start/stop/configure)
123. Partitions (independent model containers)
124. Pmarks and rollback (undo/redo, history rollback)
125. Transactions / deltas
126. Save/restore XT format (text .x_t and binary .x_b, exact B-rep)
127. Delta transmit/receive
128. XT version control (read old, write target version)
129. API journaling for replay/debug
130. Body healing / geometry repair
131. Geometry simplification / canonical (NURBS-to-analytic) conversion
132. Defeaturing (remove small blends/holes/bosses)
133. Redundant entity removal and face merging
134. Mid-surface creation for CAE idealization (geometry machinery; partly host-assembled)
135. Thickness/idealization support for CAE
136. Euler operators (low-level topology edit)
137. Convert any geometry to NURBS for export
138. General topology replace / query operations
139. Cosmetic / thread feature data via attributes
140. Symmetric multiprocessing and thread safety
141. Tessellation tolerance and rendering controls
142. Curve/surface split and edit (NURBS toolkit)
143. Bounding/precise box and locate utilities
144. Transform/orientation utilities (PK_TRANSF)

Note on exclusions (for audit accuracy): the following are NOT Parasolid kernel capabilities and are provided by Siemens D-Cubed or the host application: 2D/3D dimensional and geometric constraint solving (D-Cubed DCM/2D DCM, 3D DCM); assembly mating/positioning logic (D-Cubed AEM); collision/motion (D-Cubed CDM); free-form/variational deformation (D-Cubed DSM); feature/history trees and parametric sketching (host). Parasolid provides the geometry and topology operations those systems drive.

---

## References

- Parasolid 3D Geometric Modeling, Siemens PLM Components: https://plm.sw.siemens.com/en-US/plm-components/parasolid/
- Parasolid 3D modeling SDK, Siemens: https://www.siemens.com/en-us/products/plm-components/parasolid/3d-modeling-sdk/
- Parasolid, Tech Soft 3D (distributor product page): https://www.techsoft3d.com/developers/products/parasolid/
- Parasolid Functional Description and Overview, V35 chapter mirror (q-solid.com): http://www.q-solid.com/Parasolid_Docs_V35/pk_index.html
- Overview of Convergent Modeling, Parasolid V35 Functional Description: http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.083.html
- Facet Model Structure, Parasolid V35 Functional Description: http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.084.html
- Boolean Operations, Parasolid Functional Description: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.10.html
- General Bodies, Parasolid Functional Description: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.08.html
- Edge Blending Functions and Options, Parasolid Functional Description: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.31.html
- Partitions and Rollback, Parasolid Functional Description: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.42.html
- PK Interface Programming Reference (index): http://www.q-solid.com/Parasolid_Docs/pk_index.html
- Parasolid v33.1 release highlights, Siemens PLM Components blog: https://blogs.sw.siemens.com/plm-components/parasolid-v33-1-release-highlights/
- Parasolid with Convergent Modeling, Siemens PLM Components blog: https://blogs.sw.siemens.com/plm-components/parasolid-with-convergent-modeling/
- CAD interoperability around the Parasolid neutral format, CAD Interop: https://www.cadinterop.com/en/formats/neutral-format/parasolid.html
- Creating and Rendering Parasolid Entities, HOOPS Visualize documentation (Tech Soft 3D): https://docs.techsoft3d.com/3df/latest/prog_guide/misc/ps_entities.html
- Parasolid, Grokipedia overview: https://grokipedia.com/page/Parasolid
- Siemens Expands Convergent Modeling (Parasolid v33.0), Architosh: https://architosh.com/2021/01/siemens-expends-convergent-modeling-new-parasolid-v33-0/
