# 16. The Assembly Layer: Instancing, Large-Assembly Management, and Lightweight Representation

## Scope

This file investigates the structural and data layer that sits between a B-rep solid modeling kernel and the application: the assembly tier. The kernel produces and edits individual bodies. The assembly tier composes thousands of those bodies into product structure, positions them with transforms, manages memory and loading at scale, detects clash across the whole product, and feeds lightweight visualization pipelines. The questions for Keel are: what must a Parasolid-class kernel provide so that an assembly tier can be built cleanly on top, and what should deliberately stay above the kernel boundary.

This file covers: assembly data models (part/instance/occurrence, DAGs not trees, transform propagation, the academic assembly modeling tradition, STEP AP242 NAUO structure); how kernels actually handle assemblies (Parasolid partitions and application-layer instancing, OCCT XCAF/OCAF with shared shapes and TopLoc_Location); lightweight formats (JT/ISO 14306, glTF for CAD, the Tech Soft 3D HOOPS ecosystem, on-demand tessellation); large-assembly rendering literature (GigaWalk, HLODs, Boeing 777 ray tracing, occlusion culling); clash at assembly scale (Navisworks-class streaming and culling); simplification (shrinkwrap and envelope generation for IP protection); versioning and collaboration (Onshape microversions and in-context design); and coordinate precision in large models (relative-to-center rendering). Clash-detection BVH internals and Kramer-style assembly constraint solving are covered in other files; this file is the structural/data layer.

---

## Theme 1: Assembly data models

The recurring structural insight across every system surveyed is the same: an assembly is a directed acyclic graph (DAG) of definitions and instances, not a tree, because a subassembly definition can be instanced many times. The data model must separate the definition of a part or subassembly (the geometry, stored once) from the occurrence of that definition in a parent context (a reference plus a transform). Industry terminology distinguishes part (the definition), instance or component (a single placed reference with its own transform), and occurrence (a node in the fully expanded usage path, identified by the chain of instances from the root). The same physical bolt definition can appear as thousands of occurrences; each occurrence has a unique path-based identity even though they share one definition.

### Source 1.1: Shah and Rogers, assembly modeling as an extension of feature-based design

**Citation:** Shah, J. J., and Rogers, M. T., "Assembly Modeling as an Extension of Feature-Based Design," Research in Engineering Design, vol. 5, 1993. https://link.springer.com/article/10.1007/BF01608364 and https://www.sciencedirect.com/topics/engineering/assembly-model

**Content:** This is the seminal formalization of assembly modeling in the academic CAD literature. Shah and Rogers proposed a representation that encapsulates relationships at every level of the assembly: subassembly, parts, form features, and the feature-producing volumes beneath them. They define five basic structures usable to describe relationships between assemblies, parts, features, feature volume primitives, and evaluated boundaries: Part-of, Structuring relation, Degrees of freedom, Motion limits, and Fit. On top of these, "generic relations" facilitate constraint specification between a target entity and a reference entity. Assembly features are defined as an association between two form features on different parts, with mutual constraints encoded from shape, dimensions, position, and orientation. The paper explicitly contrasts procedural (history-based) versus declarative (constraint-based) approaches to product modeling and argues for a uniform set of structures spanning all levels of product relations.

**Limitations:** The work predates modern cloud and microversion architectures and is oriented toward single-CAD-system modeling. The "feature-producing volume" granularity is finer than most assembly tiers need and ties the model to a feature-based history representation.

**Kernel relevance:** It establishes the conceptual vocabulary Keel's assembly tier inherits: Part-of (the DAG edge), Degrees of freedom and Motion limits (constraint/joint data, which belongs above the kernel), and Fit (mating). The key takeaway for the kernel boundary is that mating relationships reference form features on parts, so the kernel must expose stable identifiers for sub-shapes (faces, edges) that the assembly tier can attach mates to. This connects directly to persistent naming (file 07).

### Source 1.2: STEP assembly structure (NAUO chains)

**Citation:** STEP Tools, "STEP Assembly Functions," STIX documentation. https://steptools.com/docs/stix/stix_asm.html and http://www.steptools.com/docs/stp_aim/stix_asm.html

**Content:** STEP encodes assembly structure through `next_assembly_usage_occurrence` (NAUO), a subtype of `product_definition_relationship`. Each NAUO links a relating `product_definition` (the parent assembly) to a related `product_definition` (the child component), establishing one parent-child edge. A product participates in many NAUOs: it has back pointers as parent (its `child_nauos`) and as child (its `parent_nauos`), which is exactly the DAG adjacency. Crucially, AP242 changed `product_definition_relationship`'s relating and related attributes from a bare `product_definition` to a select type that can take several entity types, generalizing the relationship. Geometric placement is separate from structure: a `mapped_item` references a `shape_representation` and carries an `item_defined_transformation` built from `axis2_placement_3d`, so the same component shape can be positioned differently in each occurrence. The documentation notes a product can have a parent even with no NAUO, because some data carries a `mapped_item` between two shape_representations without an NAUO relating the products; assembly traversal code must therefore tag and orient relationships first (the `stix_tag_asms()` step) to determine correct parent-child direction.

**Limitations:** STEP's entity-relationship model is verbose and exchange-oriented, optimized for completeness over runtime efficiency. The separation of product structure from shape (mapped_item) means an importer must reconcile two parallel graphs. The "NAUO sometimes absent" wrinkle shows real-world data is messy.

**Kernel relevance:** STEP is the canonical interoperability target, so Keel's assembly tier must be able to lossless-map its internal occurrence DAG onto NAUO chains plus mapped_item transforms, and back. The clean lesson: structure (who contains whom) and placement (the transform) are orthogonal axes and should be modeled separately. The kernel itself need not know NAUO; it must provide bodies with stable shape representations that the tier can wrap in mapped_item-equivalent transform references.

### Source 1.3: Recommended Practices for AP242 Domain Model XML assembly structure

**Citation:** mbx-if.org (CAx Implementor Forum), "Recommended Practices for AP242 Ed.3 Domain Model XML: Assembly Structure," v3.2, 2024. https://www.mbx-if.org/home/wp-content/uploads/2024/05/rec_prac_ap242xml_assy_struct_v3.2.pdf and https://www.ap242.org/

**Content:** AP242's Business Object Model (BO Model), expressed in XML, carries standardized product metadata and the assembly structure of a complex product, layered over the same product/product_definition/NAUO foundation. AP242 is positioned for Model Based Definition (MBD): Product and Manufacturing Information (PMI, including GD&T) is directly associated with the 3D model rather than living on drawings. The recommended-practices documents are the implementation contract that CAD vendors follow so that assembly trees, transforms, colors, and PMI round-trip between systems. AP242 also serves as one of the standard "structure" wrappers for partitioned JT (PLMXML / AP242 XML describing how shattered JT part files compose into a product).

**Limitations:** AP242 is a moving standard (editions 1 through 3) and the BO Model XML is distinct from the AP242 SDAI/EXPRESS form, so "AP242 support" is ambiguous without specifying which serialization. Full conformance is large.

**Kernel relevance:** Confirms the assembly tier (not the kernel) owns PLM-grade product structure: effectivity, configurations, revision and version metadata, and PMI association. The kernel exposes geometry and stable IDs; the tier maps those into AP242 product structure for exchange. Keel should treat AP242 BO Model XML as the long-term exchange spec for the assembly tier and keep the kernel agnostic.

---

## Theme 2: How kernels handle assemblies

The dominant industry pattern is that the geometry kernel deliberately stops at the single-body (or partition-of-bodies) level and leaves occurrence/instance management to the application. Parasolid exemplifies the minimalist approach; OCCT's XCAF is the notable exception that builds a full assembly document framework on top of the kernel.

### Source 2.1: Parasolid partitions, bodies, and assemblies

**Citation:** Siemens, "Introduction to Parasolid," Parasolid documentation (functional description). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.02.html and Spatial glossary https://www.spatial.com/glossary/parasolid

**Content:** Parasolid's top-level container is the partition. A partition can hold one or more bodies and can be stored and rolled back as a unit, so each partition has independent undo/redo of operations on its bodies. Parasolid does have an assembly concept: an assembly is a collection of instances of bodies or assemblies, optionally with construction geometry, and an assembly carries a Transform field (null implies identity). However, the practical reality reported is that Parasolid's instancing is limited; full shared-instance modeling with positioning matrices is thin, and applications commonly store each occurrence as an independent body rather than relying on kernel-level instancing. Rendering is delegated: Parasolid calls application-supplied Graphical Output (GO) functions, generating many GO calls per PK rendering call, which shows the kernel emits geometry piecemeal and the application owns the scene graph and display.

**Limitations:** Because kernel-level instancing is weak, the heavy lifting of occurrence identity, shared-definition memory savings, and transform propagation lands on the application layer. This is by design but means the kernel offers little assembly help.

**Kernel relevance:** This is the central architectural precedent for Keel. The market-leading kernel intentionally keeps assemblies thin: bodies live in partitions (rollback units), and the application builds the occurrence DAG. Keel should mirror this: provide bodies, partition-like rollback/transaction units, attachable transforms, and stable IDs, but keep occurrence/instance management and product structure in the assembly tier. The GO callback pattern also validates keeping tessellation/display outside the core.

### Source 2.2: OCCT XDE/XCAF assembly document framework

**Citation:** Open CASCADE Technology, "Extended Data Exchange (XDE)" user guide. https://dev.opencascade.org/doc/overview/html/occt_user_guides__xde.html and XCAFDoc_ShapeTool reference https://dev.opencascade.org/doc/occt-7.5.0/refman/html/class_x_c_a_f_doc___shape_tool.html

**Content:** OCCT is the major open kernel that does provide a built-in assembly framework. XDE uses XCAF, built on OCAF (the Open CASCADE Application Framework), storing everything in a `TDocStd_Document` organized as a labeled tree of `TDF_Label`s. Shapes live on sub-labels of label `0:1:1`; colors at `0:1:2`; GD&T at `0:1:4`; views, clipping planes, and notes at further labels. The pivotal design choice: "shapes are simple OCAF objects without a location definition." A component is a sub-label that references an original shape's label via a `TDataStd_TreeNode` (GUID `XCAFDoc::ShapeRefGUID()`) and encapsulates its own `TopLoc_Location` (the transform). Therefore many components reference one and the same shape with different locations: this is structural sharing of geometry with per-occurrence transforms. The `XCAFDoc_ShapeTool` API distinguishes free shapes (`AddShape`, top-level roots), references/components (`AddComponent`, `IsReference`, `GetReferredShape`, `GetLocation`), and simple shapes (`IsSimpleShape`). Because a label can appear under multiple parents, a label alone cannot give a final world transform: the full placement is the composition of `TopLoc_Location`s along the occurrence path. Attributes (`TDataStd_Name`, `XCAFDoc_Color`, `XCAFDoc_Centroid/Area/Volume`, `XCAFDoc_Dimension/GeomTolerance/Datum`) attach to labels down to face/edge level.

**Limitations:** OCAF/XCAF is a heavyweight in-memory document model with its own persistence; integrating it implies adopting OCCT's data conventions. TopLoc_Location composition along deep paths must be done carefully. The label-can-appear-under-multiple-parents property is exactly the DAG, and naive code that assumes a tree will compute wrong transforms.

**Kernel relevance:** XCAF is the closest open-source blueprint for what Keel's assembly tier should look like, and it strongly validates the structural-sharing model: geometry stored once, location stored per reference, transform = path composition. OCCT's `TopLoc_Location` is itself worth studying as the precedent for an efficient, hashable, composable location type with structural sharing of the location chain. For Keel, the lesson is to give the kernel a first-class, cheap, shareable transform/location primitive and let the tier compose it along occurrence paths.

---

## Theme 3: Lightweight representation formats

Lightweight formats decouple visualization and downstream consumption from the heavyweight native CAD model. They combine multi-LOD tessellation (always present) with optional exact B-rep (sometimes embedded), structure, metadata, and PMI, plus aggressive compression. JT is the dominant industrial standard; glTF is the web/mesh default with significant CAD caveats; the Tech Soft 3D stack is the dominant commercial toolchain.

### Source 3.1: JT format and ISO 14306

**Citation:** "JT (visualization format)," Wikipedia. https://en.wikipedia.org/wiki/JT_(visualization_format) ; ISO 14306:2017 https://www.iso.org/standard/62770.html ; ISO 14306-3:2025 (Part 3, Version 2) https://www.iso.org/standard/89233.html

**Content:** JT (Jupiter Tessellation) is the leading neutral 3D visualization format, formalized as ISO 14306. Version 1 (ISO 14306:2012) was based on Siemens JT v9.5; Version 2 (ISO 14306:2017) added STEP B-rep as an additional exact-geometry segment alongside the legacy XT (Parasolid) B-rep; current Siemens JT is v10.x (10.6, 2024). The data model is a Logical Scene Graph (LSG) of nodes: Partition Nodes, Part Node Elements, Group Nodes, Shape Nodes, Instance Nodes, and MetaData Nodes. JT supports flexible node-to-file mappings: monolithic (one file), per-part, fully shattered, or structured by PLMXML / STEP AP242 XML, enabling progressive and demand loading. Geometry can range from facet-only (tiny) to full NURBS B-rep with structure, attributes, and PMI. Three B-rep flavors exist: native JT B-rep, XT B-rep (recommended exact form, Parasolid lineage), and STEP B-rep (V2). Tessellation supports multiple Shape LODs (vertex, tri-strip, polyline, point set). Compression has a standard lossless mode and an advanced domain-specific lossy geometry mode; the JTIAP profile (ProSTEP iViP/VDA, 2015) mandates LZMA for a fully neutral royalty-free implementation. JT files are commonly cited at roughly 1 to 10 percent of native CAD size and the format is engineered for assemblies of tens of thousands of components with streamable architecture.

**Limitations:** JT is rooted in the Siemens/Parasolid ecosystem; the recommended exact form (XT B-rep) historically depended on Parasolid semantics. The standard is large and versioned, and the lossy compression mode trades exactness for size. Full PMI and B-rep round-trip fidelity varies by writer.

**Kernel relevance:** JT is the format Keel-based applications will most need to read and write for digital mock-up and supplier exchange. The XT B-rep segment is directly relevant: it embeds Parasolid-class exact geometry inside a lightweight wrapper, so Keel's serialization should aim for a B-rep representation that can map into JT's exact-geometry segment (XT or STEP B-rep). The LSG with Partition/Instance nodes mirrors the occurrence DAG, confirming the assembly tier's structure. Multi-LOD tessellation generation from B-rep is a kernel-adjacent responsibility (file 05, tessellation) feeding the tier.

### Source 3.2: glTF for CAD use and its limitations

**Citation:** CAD Interop, "CAD interoperability around the glTF mesh format." https://www.cadinterop.com/en/formats/mesh/gltf.html ; Novedge, "Deterministic Interoperability: Units, Precision, and Semantics." https://novedge.com/blogs/design-news/deterministic-interoperability-units-precision-and-semantics-for-governed-cad-cae-cam-pipelines ; google/draco https://github.com/google/draco

**Content:** glTF 2.0 is the web-native runtime 3D format and is "viz-first": efficient for scene composition with PBR materials and Draco mesh compression (often 70 to 95 percent size reduction), but fundamentally a tessellated-mesh format. Converting B-rep CAD (STEP, CATIA V5, NX, SolidWorks, Creo) to glTF requires triangulation, which loses precision; expected losses include construction history, parametric features, PMI, and 3D annotations, leaving only mesh plus basic materials and (with care) assembly hierarchy. A documented interoperability hazard: if a target viewer lacks an extension such as `KHR_draco_mesh_compression`, the result is silent material loss or load failure. Engineering meaning, if needed, must be attached as discipline-specific payloads (extensions or sidecar metadata). Guidance is explicit: avoid glTF for precision CAD archiving (prefer STEP AP242), simulation, or 3D printing (prefer STL/3MF).

**Limitations:** No native exact geometry, no native PMI/GD&T semantics, float-based precision, and extension fragmentation. Assembly structure support exists but is shallow relative to JT/AP242.

**Kernel relevance:** glTF is the right output for browser viewers and AR/web review but the wrong format for the kernel's authoritative model. For Keel, glTF is a downstream tessellation export target produced by the tier, not a kernel data model. The precision warning reinforces Theme 8 (coordinate management) and the rule that exact geometry stays in the kernel/JT-AP242 layer while glTF carries only display meshes.

### Source 3.3: Tech Soft 3D HOOPS ecosystem (Exchange, Communicator/Visualize Web, Stream Cache)

**Citation:** Tech Soft 3D, "HOOPS Communicator / Visualize for Web" and "HOOPS Exchange." https://www.techsoft3d.com/products/hoops/communicator/ , https://www.techsoft3d.com/products/hoops/exchange/ , technical overview https://docs.techsoft3d.com/hoops/visualize-web/overview/technical-overview.html , CAD model streaming lab https://labs.techsoft3d.com/project/cad-model-streaming/

**Content:** Tech Soft 3D supplies the dominant commercial toolchain for CAD viewing and data access (and is also the distributor of Parasolid and ACIS). HOOPS Exchange is a single-API SDK that reads 30+ CAD formats and writes 10+, giving applications uniform access to geometry, assembly structure, and PMI. HOOPS Visualize for Web (formerly HOOPS Communicator) is a high-performance graphics kernel purpose-built for engineering: massive-model support, complex-assembly navigation, intelligent streaming, and server-side rendering, exposed through a JavaScript API for browser apps. The HOOPS Stream Cache Server prioritizes streaming so the data most relevant to the current view arrives first, letting users interact before the whole model has loaded. This is the productized, documented version of the streaming/demand-loading techniques the research literature describes.

**Limitations:** Proprietary and commercial; the documentation describes capabilities rather than internal algorithms. It is a reference for the boundary's behavior, not an open implementation to copy.

**Kernel relevance:** HOOPS shows the clean separation Keel should target: a data-access layer (Exchange-like: read/write formats, expose structure) distinct from a visualization/streaming layer (Communicator-like). Both sit above the kernel. The Stream Cache "most-important-data-first" model is the architecture an open Keel viewer should emulate. It also confirms the kernel only needs to emit geometry and stable IDs; structure-aware streaming is a tier concern.

### Source 3.4: On-demand tessellation (Onshape and the general pattern)

**Citation:** Onshape, "Architecture." https://onshape-public.github.io/docs/api-intro/architecture/

**Content:** Onshape does not persist tessellated data: meshes are generated on demand for client display or REST requests and may be cached for performance. The authoritative model is the exact (server-side kernel) geometry plus the microversion history; display geometry is derived. This is the cloud-native generalization of the JT/HOOPS LOD approach: rather than baking and shipping LODs, regenerate tessellation at the needed fidelity from the exact model when a view requests it.

**Limitations:** Requires fast server-side tessellation and good caching; round-trips add latency versus pre-baked LOD files. Works best with a server-resident kernel.

**Kernel relevance:** Validates that tessellation is a derived, regenerable product, not a stored asset. For Keel this argues for a fast, deterministic, parameter-driven tessellator (file 05) callable at multiple chord/angle tolerances, with the assembly tier deciding when and at what LOD to invoke it. Exact geometry and stable IDs are the durable artifacts; meshes are ephemeral.

---

## Theme 4: Large-assembly performance and massive-model rendering

The massive-model rendering literature from the late 1990s and early 2000s (much of it from the UNC GAMMA group) established the core toolkit still used today: precomputed geometric LODs, hierarchical LODs that collapse whole subtrees, conservative occlusion culling, spatial clustering, out-of-core/streaming data management, and parallel rendering. CAD aircraft and ship models (Boeing 777, Double Eagle Tanker) were the canonical stress tests.

### Source 4.1: HLODs (Hierarchical Levels of Detail)

**Citation:** Erikson, C., Manocha, D., and Baxter, W. V., "HLODs for Faster Display of Large Static and Dynamic Environments," Symposium on Interactive 3D Graphics (I3D), March 2001. http://gamma.cs.unc.edu/POWERPLANT/papers/erikson2001.pdf and https://www.cs.unc.edu/Research/ProjectSummaries/hlods.pdf

**Content:** The foundational HLOD paper. A geometric dataset is represented as a scene graph; conventional LODs are computed for each node, then augmented with automatically generated hierarchical LODs (HLODs) that are drastic simplifications of entire branches of the scene graph, not just single objects. At runtime the traversal can substitute one HLOD proxy for a whole subtree of distant geometry, collapsing thousands of small parts into a single simplified mesh. Demonstrated on the Double Eagle Tanker: 126,630 objects and 82,361,612 triangles rendered at 1 to 8 frames per second on an SGI Infinite Reality, "more than two orders of magnitude" speedup with little quality loss.

**Limitations:** Static-scene oriented (dynamic extensions are harder); simplifying across part boundaries can merge distinct parts visually, which is acceptable for navigation but not for measurement. Building HLODs is an expensive preprocess.

**Kernel relevance:** HLOD is the canonical answer to "how do you render an assembly of thousands of parts." It maps naturally onto the occurrence DAG: each subassembly node gets an HLOD proxy. For Keel, the kernel provides per-part tessellation and merge/simplify operations (defeaturing, file 03/10); the assembly tier builds and caches HLOD proxies per subassembly node. The DAG-aligned structure means shared subassemblies can share HLOD proxies.

### Source 4.2: GigaWalk

**Citation:** Baxter, W. V. III, Sud, A., Govindaraju, N., and Manocha, D., "GigaWalk: Interactive Walkthrough of Complex Environments," 13th Eurographics Workshop on Rendering (EGRW), 2002. http://gamma.cs.unc.edu/GIGAWALK/Paper/GigaWalk-EGRW02.pdf and project page http://gamma.cs.unc.edu/GIGAWALK/

**Content:** GigaWalk is a parallel architecture for interactive walkthrough of gigabyte-sized environments of tens of millions of polygons. It uses a single unified scene graph for multiple acceleration techniques, precomputes geometric LODs, and at runtime computes a potentially visible set (PVS) per frame by combining view-frustum culling with a two-pass hierarchical Z-buffer occlusion-culling algorithm, in conjunction with the precomputed LODs. It performs spatial clustering of geometry and load-balances across two graphics rasterization pipelines and one or more CPU processors. The two-pass occlusion culling alone improves frame rate by a factor of 3 to 4. It renders the Double Eagle Tanker (over 82 million triangles) at 11 to 50 fps on an SGI with two IR2 pipelines and three 300 MHz R12000 CPUs.

**Limitations:** Tied to specialized multi-pipeline SGI hardware of its era; the two-pass HZB approach predates modern GPU hardware occlusion queries and compute-based culling. Conservative occlusion can still over-draw.

**Kernel relevance:** GigaWalk confirms the trio of techniques (LOD + frustum culling + occlusion culling on a clustered scene graph) and the value of a single shared scene-graph representation feeding all of them. For Keel this is a tier-level renderer concern, but it dictates what the tier needs from the kernel: per-part bounding volumes, conservative bounds for occlusion, and tessellation at multiple LODs. Spatial clustering of occurrences is a tier responsibility built over kernel-provided bounds.

### Source 4.3: Boeing 777 ray tracing (massive CAD data)

**Citation:** Stephens, A., Boulos, S., Bigler, J., Wald, I., and Parker, S., "An Application of Scalable Massive Model Interaction using Shared-Memory Systems" (the "Boeing 777" interactive ray tracing work), EG Symposium on Parallel Graphics and Visualization, 2006; related "Exploring a Boeing 777: Ray tracing large-scale CAD data." https://www.researchgate.net/publication/221357168 and https://www.researchgate.net/publication/5819131

**Content:** This line of work renders the full Boeing 777 CAD dataset, on the order of 300+ million triangles, interactively via ray tracing on shared-memory systems rather than rasterization. Ray tracing scales sub-linearly with scene size given a good acceleration structure (BVH/kd-tree), which suits massive CAD where most geometry is occluded or off-screen. The approach relies on out-of-core/large-memory data management and cache-coherent traversal to keep the working set tractable. It demonstrated that interactive frame rates on full-aircraft CAD were achievable without aggressive LOD simplification, trading rasterization-plus-LOD for ray tracing over the full-resolution model.

**Limitations:** Required large shared-memory machines; full-resolution ray tracing of huge CAD remained hardware-intensive for its time. ResearchGate full text is access-restricted (HTTP 403 on direct fetch); details here are from the abstract and the broader UNC/Utah massive-model literature.

**Kernel relevance:** Establishes the alternative to LOD: a good spatial acceleration structure (BVH) over the full assembly geometry, traversed cache-coherently. This ties to clash detection (shared BVH) and to coordinate management (precision under huge extents). For Keel, the kernel/tier should be able to build and persist a BVH over tessellated assembly geometry that serves both interrogation/clash and ray-traced visualization.

### Source 4.4: Occlusion culling and HLOD refinement via hardware occlusion queries

**Citation:** Various, e.g. "Coherent Hierarchical Level-of-Detail (HLOD) Refinement Through Hardware Occlusion Queries" and "Optimized HLOD Refinement Driven by Hardware Occlusion Queries." https://diglib.eg.org/items/2a693a47-8dd8-4258-8d26-1b34f58394ea and https://link.springer.com/chapter/10.1007/978-3-540-76858-6_11 ; SWITCH (Govindaraju et al.) http://gamma.cs.unc.edu/SWITCH/Paper/i3d03.pdf

**Content:** LOD and occlusion culling are orthogonal accelerations, and the major thrust of this literature is combining them: drive HLOD refinement (when to swap a subtree proxy for its detailed children) using visibility information from hardware occlusion queries (HOQs) on the bounding volumes of scene-graph nodes. SWITCH ("occlusion switches") uses multiple GPUs to compute occlusion for interactive culling in complex environments. The unifying idea: a node that is occluded need not be refined, so HOQ results feed the LOD error metric and the traversal order.

**Limitations:** HOQ latency and the read-back stall are classic pitfalls (addressed by coherent/temporally-amortized query schemes). Conservative results still over-draw; query overhead can dominate for many small nodes.

**Kernel relevance:** Modern equivalents (GPU compute culling, two-phase Hi-Z culling) are tier/renderer concerns, but they need exactly what the kernel can cheaply provide: per-occurrence and per-subassembly bounding volumes. Keel should ensure every body and node exposes a tight, fast bounding box (and optionally a conservative simplified hull) so the tier can do frustum + occlusion culling over the DAG.

---

## Theme 5: Clash and interference at assembly scale

At assembly scale, clash detection is dominated by the combinatorial broad phase: avoiding the O(n squared) pairwise test across thousands of parts. The kernel provides exact pairwise interference; the tier provides spatial culling, incremental update, and result management. (BVH internals are covered in the dedicated clash file; this section is the assembly-scale framing.)

### Source 5.1: Navisworks-class streaming clash and broad-phase culling

**Citation:** United-BIM, "All About Clash Detection with Navisworks." https://www.united-bim.com/get-to-know-all-about-clash-detection-with-navisworks/ ; ViBIM workflow https://vibimglobal.com/blog/navisworks-clash-detection/ ; Novatr 2026 guide https://www.novatr.com/blog/navisworks-for-clash-detection

**Content:** Navisworks is the canonical large-scale coordination/clash tool. It classifies clashes as hard (physical overlap), soft/clearance (insufficient gap), and workflow/4D (time-sequencing). Its engine streams geometry, loading only what is visible so multi-gigabyte aggregated models from Revit, Inventor, and SolidWorks run on standard workstations, with advanced occlusion culling keeping walkthroughs fluid over millions of polygons. The documented practical workflow is itself a broad-phase strategy: group the model into sets (by trade/system/zone) and run clash tests between specific sets rather than everything-versus-everything; start with large/critical elements and wide tolerances to catch redesign-level conflicts, then tighten tolerances for minor interferences. Without such grouping, a naive run yields thousands of ungroupable, unmanageable clash results.

**Limitations:** Heavy reliance on user-driven set selection to tame the combinatorics; result triage at scale is a known pain point. It operates largely on tessellated/aggregated geometry, so clash is approximate to the mesh tolerance rather than exact B-rep unless backed by exact geometry.

**Kernel relevance:** Confirms the division of labor for Keel. Broad phase = spatial partitioning/BVH plus rule-based set selection (tier). Narrow phase = exact pairwise interference between two bodies (kernel, via boolean/distance interrogation, file 06). Keel should expose an efficient exact pairwise interference and minimum-distance query, plus stable occurrence IDs so the tier can group occurrences into clash sets and report clashes by occurrence path. Incremental clash after a transform change means the tier only re-tests the BVH-adjacent pairs of moved occurrences, so the kernel's pairwise test must be fast and the BVH cheaply updatable.

---

## Theme 6: Simplification for assemblies (shrinkwrap and envelopes)

Per-part defeaturing and assembly-level envelope/shrinkwrap generation reduce data for visualization and, importantly, strip proprietary internal detail before sharing with suppliers and partners (IP protection).

### Source 6.1: Shrinkwrap and envelope generation

**Citation:** CAD Interop, "Create a shrinkwrap from a complex model." https://www.cadinterop.com/en/your-needs/cad-weight-reduction/create-a-shrinkwrap-from-a-complex-model.html ; CoreTechnologie geometry simplification https://coretechnologie.com/find-out-more/geometry-simplification/ ; USPTO patent on best-fit approximate 3D B-rep https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11429756

**Content:** Shrinkwrap/envelope generation produces a simplified outer-shell representation of a part or whole assembly. Techniques span a spectrum: bounding box, convex hull, and tighter concave-hull/envelope shapes. The cited patent describes determining a concave hull for each face of the B-rep model via a concave hull algorithm, then combining those hull shapes with Boolean or stitching operations to form a closed envelope; the result is a valid B-rep solid of the outer shell with the interior removed, exportable in neutral B-rep formats (STEP, Parasolid) at much smaller size. The driving use case is contractual confidentiality: a shipyard or OEM sends suppliers a shrinkwrap so they can verify integration/fit without receiving the construction tree, functional parameters, or internal definition dimensions, definitively protecting IP against reverse engineering.

**Limitations:** Envelopes lose internal features, so they are unsuitable where internal interfaces matter; a too-loose envelope (bounding box) gives false clearance, a too-tight one is expensive to compute. Producing a watertight valid B-rep envelope from messy input is nontrivial.

**Kernel relevance:** Shrinkwrap is a kernel-adjacent operation: it needs faceting, hull computation, Boolean union, and shelling/healing, all of which the kernel provides (files 03, 06, 10). Keel should expose the primitives (convex/concave hull over faces, Boolean union of per-occurrence shapes, outer-shell extraction, healing) so the tier can offer assembly shrinkwrap. The output being a clean B-rep solid (not just a mesh) is what distinguishes a kernel-grade shrinkwrap from a mesh decimation and is a differentiator Keel should target.

---

## Theme 7: Versioning and collaboration at assembly scale

Cloud-native CAD reframes the assembly tier as a version-controlled database. Onshape is the published reference: microversions, branchable workspaces, immutable versions, and managed in-context references that keep top-down design stable.

### Source 7.1: Onshape microversions, workspaces, and in-context design

**Citation:** Onshape, "Architecture" https://onshape-public.github.io/docs/api-intro/architecture/ ; "Cloud-Native CAD Enables Top-Down Design" https://www.onshape.com/en/blog/top-down-design-cloud-cad-software ; "Managed In-Context Design" https://www.onshape.com/en/resource-center/tech-tips/tech-tip-using-managed-in-context-design

**Content:** Onshape models a document as elements (Part Studios, Assemblies, etc.) under a Git-inspired four-level version model: microversions (every edit creates one; internal, immutable, never edited directly), workspaces (branches; edits land here and spawn microversions; "Main" is default), versions (named immutable snapshots of a microversion), and linked references that always target a specific version and keep that version alive as long as anything references it. IDs (document, workspace, version, microversion, element) are 24-character strings. Topology identity is explicitly handled: Part/Face/Edge IDs are variable-length and session-scoped, can change/appear/disappear/merge/split across microversions, but the system preserves topology IDs across microversions to track identity through such transformations. In-context design lets a part be modeled referencing the assembly context; "Managed In-Context Design" pins those references to a captured context so that merely moving components to test motion does NOT update the dependent parts, eliminating the broken-link/unexpected-update failures of traditional top-down design. Tessellation is regenerated on demand, never persisted (Source 3.4).

**Limitations:** The model is cloud/server-resident and assumes a central authoritative store; offline and peer-to-peer scenarios are out of scope. Conflict resolution for truly concurrent edits to the same element is constrained by the workspace/branch model (changes serialize through microversions on a workspace).

**Kernel relevance:** This is the gold standard for Keel's collaboration tier and it pins down the kernel's hardest obligation: stable topology identity across edits. Onshape preserving Face/Edge IDs across microversions is exactly the persistent-naming problem (file 07), and it is what makes in-context references and assembly mates survive model changes. For Keel the boundary is clear: the kernel must provide stable, edit-surviving entity IDs and a deterministic regeneration; the tier builds microversion history, branching/merging, configurations, and in-context reference management on top. Managed in-context design (decoupling "move to test motion" from "edit that propagates") is a tier policy enabled by the kernel separating transform changes from geometry edits.

---

## Theme 8: Coordinate system management in large models

Parts positioned far from the world origin expose 32-bit float precision limits, causing visible jitter and inaccurate interrogation. The standard remedy is relative-to-center / relative-to-eye rendering and double-precision emulation, plus disciplined local frames.

### Source 8.1: Relative-to-center rendering and large-coordinate precision

**Citation:** Re:Earth Engineering, "Rendering Models with High Precision in Global Scenes." https://reearth.engineering/posts/high-precision-rendering-en/ ; mlightcad, "Precision-Safe Rendering of Large-Coordinate CAD Drawings in Three.js." https://medium.com/@mlightcad/precision-safe-rendering-of-large-coordinate-cad-drawings-in-three-js-c49c299b3afc ; Andrieu, "The Problem of Floating Point Precision in OpenGL, Vulkan and 3D, Part 2." https://medium.com/@thibautandrieu/the-problem-of-floating-point-precision-in-opengl-vulkan-and-3d-in-general-part-2-52c1efad78ab

**Content:** GPUs compute in Float32, which cannot faithfully represent real-world large coordinates (e.g. Earth-centered values around 6,371,000 m, or a plant model kilometers from origin), producing vertex jitter. The relative-to-center (RTC) technique computes a tile/cluster center C in world (double) coordinates and uploads each vertex as p_local = P - C, keeping the model matrix holding C so the GPU only ever sees small local values. The related relative-to-eye (RTE) expresses positions relative to the camera. Cesium emulates Float64 with a pair of Float32 values to restore near-53-bit precision on a Float32 GPU. The common principle: never let the GPU see large absolute coordinates; keep everything small and local, with the large offset carried in double precision on the CPU side.

**Limitations:** RTC/RTE add bookkeeping (per-cluster centers, double offsets) and complicate shaders; choosing cluster granularity is a tradeoff. Float64 emulation costs extra GPU work.

**Kernel relevance:** This is decisive for an assembly tier rendering large products and for accurate interrogation. The architectural lesson maps directly onto the occurrence DAG: each occurrence already has a local frame, and its world transform is the path composition; rendering and computation should be done in local frames with the large offset kept in double precision and applied last. For Keel, the kernel should (a) store geometry in part-local coordinates (it already does), (b) use double-precision transforms/locations, and (c) provide cluster/center extraction so the tier can do RTC. The kernel must never bake a far-from-origin world transform into a body's coordinates. This connects to the transform/location primitive (Theme 2) being double-precision and composable.

---

## Assembly tier boundary for Keel

The consistent verdict across Parasolid (minimalist), OCCT XCAF (full document framework), JT/AP242 (exchange), and Onshape (cloud) is that the assembly tier is a layer above the kernel, and the kernel's job is to make that layer cheap and correct to build. The boundary:

**What the kernel must provide (so the tier is possible):**

- **Instancing-friendly structural sharing.** A body/shape definition is stored once; the kernel must let many references point at one definition without copying geometry. The OCCT model (shapes have no location; components are references plus a `TopLoc_Location`) is the proven blueprint. Keel should give bodies an identity that can be referenced by many occurrences.
- **A first-class transform/location primitive.** Cheap to copy, hashable, composable along a path, and double-precision. OCCT's `TopLoc_Location` with structural sharing of the location chain is the precedent. Transforms are an axis orthogonal to structure (mirroring STEP's mapped_item vs NAUO split).
- **Stable, edit-surviving entity IDs.** Stable IDs for bodies, faces, and edges that persist across regeneration are the linchpin for assembly mates, in-context references, persistent clash grouping, and Onshape-style microversioning. This is the persistent-naming contract (file 07) viewed from the assembly side. Without it, the entire tier above is fragile.
- **Partition / transaction units with rollback.** Parasolid partitions (independent rollback units holding bodies) are the model: the tier needs to load, edit, and roll back subassemblies independently.
- **Fast derived data on demand:** tessellation at arbitrary LOD (regenerable, not stored, per Onshape), tight bounding volumes per body, conservative hulls, exact pairwise interference and minimum-distance queries, and the simplification primitives (Boolean union, hulls, shelling, healing) needed for shrinkwrap. These feed the tier's rendering, culling, clash, HLOD, and envelope workflows.
- **Local-coordinate geometry with double-precision placement.** The kernel stores parts in local frames and never bakes large world offsets into coordinates, enabling RTC rendering and precise interrogation of far-from-origin assemblies.

**What stays above the kernel (the assembly tier owns):**

- The occurrence/instance DAG itself: nodes, edges, occurrence-path identity, and world-transform composition along paths.
- PLM-grade product structure: effectivity, configurations, revisions, versions, and PMI association mapped to STEP AP242 / JT structure for exchange.
- Lightweight representation and streaming: JT and glTF export, LOD/HLOD proxy generation and caching, demand/lazy loading, and stream-cache prioritization (HOOPS-style).
- Massive-model rendering: scene-graph traversal, frustum + occlusion culling, HLOD refinement, spatial clustering, parallel/GPU rendering.
- Clash at scale: broad-phase spatial culling and clash-set grouping, incremental re-test after transforms, result triage (kernel only does the exact narrow-phase pairwise test).
- Collaboration: microversions, branchable workspaces, immutable versions, in-context reference management and conflict handling.
- Coordinate policy: RTC/RTE clustering and double-precision offset bookkeeping at render and compute time.

In short, Keel should follow the Parasolid precedent of a thin assembly footprint in the kernel, but borrow OCCT XCAF's structural-sharing-plus-location model and Onshape's stable-ID/regeneration discipline as the concrete contract the kernel exposes. Get instancing-friendly sharing, a composable double-precision location type, and stable entity IDs right in the kernel, and a full Parasolid-class assembly tier (including JT/AP242 exchange, HLOD massive-model viewing, scaled clash, shrinkwrap, and Onshape-style collaboration) becomes a tractable layer on top rather than a kernel rewrite.

---

## References

1. Shah, J. J., Rogers, M. T. "Assembly Modeling as an Extension of Feature-Based Design." Research in Engineering Design, vol. 5, 1993. https://link.springer.com/article/10.1007/BF01608364 ; overview https://www.sciencedirect.com/topics/engineering/assembly-model
2. STEP Tools. "STEP Assembly Functions" (STIX). https://steptools.com/docs/stix/stix_asm.html ; AIM variant http://www.steptools.com/docs/stp_aim/stix_asm.html
3. CAx Implementor Forum (mbx-if.org). "Recommended Practices for AP242 Ed.3 Domain Model XML: Assembly Structure," v3.2, 2024. https://www.mbx-if.org/home/wp-content/uploads/2024/05/rec_prac_ap242xml_assy_struct_v3.2.pdf ; AP242 portal https://www.ap242.org/
4. Siemens. "Introduction to Parasolid" (functional description). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.02.html ; Spatial glossary https://www.spatial.com/glossary/parasolid
5. Open CASCADE Technology. "Extended Data Exchange (XDE)" user guide. https://dev.opencascade.org/doc/overview/html/occt_user_guides__xde.html ; XCAFDoc_ShapeTool reference https://dev.opencascade.org/doc/occt-7.5.0/refman/html/class_x_c_a_f_doc___shape_tool.html
6. "JT (visualization format)." Wikipedia. https://en.wikipedia.org/wiki/JT_(visualization_format)
7. ISO 14306:2017, "JT file format specification for 3D visualization." https://www.iso.org/standard/62770.html ; ISO 14306-3:2025 (Part 3, V2) https://www.iso.org/standard/89233.html
8. CAD Interop. "CAD interoperability around the glTF mesh format." https://www.cadinterop.com/en/formats/mesh/gltf.html
9. Novedge. "Deterministic Interoperability: Units, Precision, and Semantics for Governed CAD/CAE/CAM Pipelines." https://novedge.com/blogs/design-news/deterministic-interoperability-units-precision-and-semantics-for-governed-cad-cae-cam-pipelines
10. google/draco. https://github.com/google/draco
11. Tech Soft 3D. "HOOPS Communicator / Visualize for Web." https://www.techsoft3d.com/products/hoops/communicator/ ; "HOOPS Exchange" https://www.techsoft3d.com/products/hoops/exchange/ ; technical overview https://docs.techsoft3d.com/hoops/visualize-web/overview/technical-overview.html ; CAD model streaming https://labs.techsoft3d.com/project/cad-model-streaming/
12. Onshape. "Architecture." https://onshape-public.github.io/docs/api-intro/architecture/
13. Erikson, C., Manocha, D., Baxter, W. V. "HLODs for Faster Display of Large Static and Dynamic Environments." I3D 2001. http://gamma.cs.unc.edu/POWERPLANT/papers/erikson2001.pdf ; project summary https://www.cs.unc.edu/Research/ProjectSummaries/hlods.pdf
14. Baxter, W. V. III, Sud, A., Govindaraju, N., Manocha, D. "GigaWalk: Interactive Walkthrough of Complex Environments." EGRW 2002. http://gamma.cs.unc.edu/GIGAWALK/Paper/GigaWalk-EGRW02.pdf ; project page http://gamma.cs.unc.edu/GIGAWALK/
15. Stephens, A., et al. "An Application of Scalable Massive Model Interaction using Shared-Memory Systems" / "Exploring a Boeing 777: Ray tracing large-scale CAD data." EG PGV 2006. https://www.researchgate.net/publication/221357168 ; https://www.researchgate.net/publication/5819131
16. "Coherent / Optimized HLOD Refinement Through Hardware Occlusion Queries." https://diglib.eg.org/items/2a693a47-8dd8-4258-8d26-1b34f58394ea ; https://link.springer.com/chapter/10.1007/978-3-540-76858-6_11 ; SWITCH (Govindaraju et al., I3D 2003) http://gamma.cs.unc.edu/SWITCH/Paper/i3d03.pdf
17. United-BIM. "All About Clash Detection with Navisworks." https://www.united-bim.com/get-to-know-all-about-clash-detection-with-navisworks/ ; ViBIM workflow https://vibimglobal.com/blog/navisworks-clash-detection/ ; Novatr 2026 guide https://www.novatr.com/blog/navisworks-for-clash-detection
18. CAD Interop. "Create a shrinkwrap from a complex model." https://www.cadinterop.com/en/your-needs/cad-weight-reduction/create-a-shrinkwrap-from-a-complex-model.html ; CoreTechnologie geometry simplification https://coretechnologie.com/find-out-more/geometry-simplification/ ; USPTO patent 11429756 (best-fit approximate 3D B-rep) https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11429756
19. Onshape. "Cloud-Native CAD Enables Top-Down Design." https://www.onshape.com/en/blog/top-down-design-cloud-cad-software ; "Managed In-Context Design." https://www.onshape.com/en/resource-center/tech-tips/tech-tip-using-managed-in-context-design
20. Re:Earth Engineering. "Rendering Models with High Precision in Global Scenes." https://reearth.engineering/posts/high-precision-rendering-en/ ; mlightcad, "Precision-Safe Rendering of Large-Coordinate CAD Drawings in Three.js." https://medium.com/@mlightcad/precision-safe-rendering-of-large-coordinate-cad-drawings-in-three-js-c49c299b3afc ; Andrieu, "The Problem of Floating Point Precision in OpenGL, Vulkan and 3D, Part 2." https://medium.com/@thibautandrieu/the-problem-of-floating-point-precision-in-opengl-vulkan-and-3d-in-general-part-2-52c1efad78ab
