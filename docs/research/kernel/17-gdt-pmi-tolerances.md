# GD&T, PMI, and Tolerance Semantics in CAD Systems

Research dossier for the Keel B-rep kernel (Rust, Parasolid-class ambition).

## Scope and why this matters before the attribute and reference layers freeze

Geometric Dimensioning and Tolerancing (GD&T) is the engineering language that says how much a manufactured part is allowed to deviate from its nominal shape and still function. Product Manufacturing Information (PMI) is the digitized carrier of that language: the dimensions, geometric tolerances, datums, surface finishes, notes, and material conditions, attached directly to a 3D model rather than to a 2D drawing. Model-Based Definition (MBD) is the practice of making the annotated 3D model the single authoritative product definition, with the drawing demoted or eliminated. When MBD is in force, PMI is not decoration: it is a first-class deliverable that flows downstream into CAM, CMM inspection programming, and supplier exchange.

This matters to a kernel for one structural reason. PMI does not attach to coordinates; it attaches to *entities*: "the flatness of this face", "the position of this hole relative to datums A, B, C", "the profile of this freeform surface". Those entity references must survive edits, regeneration, and round-trip exchange, which loops directly back to the persistent-naming research (file 07): a PMI annotation is exactly the kind of long-lived downstream reference that the topological-naming machinery exists to protect. Furthermore, tolerance *semantics* (what a callout actually means geometrically) demand specific geometric services from the kernel: generation of offset and virtual-condition boundaries (ties to offset research, file 10 neighborhood), least-squares and minimum-zone fitting of substitute features to faces, and signed distance evaluation of measured points against NURBS faces (ties to interrogation/distance research, file 06). A kernel that cannot expose stable references, rich attributes, and these geometric primitives cannot support MBD, no matter how good its Booleans are.

This file collects roughly 22 sources spanning five themes: (1) the standards that define GD&T semantics (ASME, ISO GPS, the math standard Y14.5.1); (2) formal/computational tolerance semantics (Requicha, Srinivasan, Shah, T-Maps, validation); (3) tolerance analysis methods (stack-up, SDT, Jacobian-torsor, Monte Carlo, CAT tools); (4) PMI in and between CAD systems (semantic vs graphical, NIST testing, AP242, JT, QIF); and (5) the inspection/freeform connection (CMM planning, profile evaluation, datum fitting). Each entry gives Citation, Content, Limitations, and Kernel relevance. It closes with a synthesis of PMI readiness requirements for Keel and a references list.

---

# PART 1: GD&T semantics foundations (the standards)

## 1.1 ASME Y14.5 (2009 / 2018): the dimensioning and tolerancing language

**Citation.** American Society of Mechanical Engineers. (2018). *ASME Y14.5-2018: Dimensioning and Tolerancing* (revision of ASME Y14.5-2009). New York: ASME.

**Content.** Y14.5 is the U.S. national standard that defines the symbology and rules of GD&T: the fourteen geometric characteristics grouped into form (flatness, straightness, circularity, cylindricity), orientation (parallelism, perpendicularity, angularity), location (position, concentricity, symmetry), profile (profile of a line, profile of a surface), and runout (circular, total). It defines the feature control frame (the rectangular callout box carrying the characteristic symbol, the tolerance value, optional material-condition modifiers, and the datum reference frame), the datum reference frame (an ordered set of up to three datums, primary/secondary/tertiary, that establishes a coordinate system), and the material-condition modifiers MMC (maximum material condition), LMC (least material condition), and RFS (regardless of feature size). The single most consequential default rule is **Rule #1 (the envelope principle, also called the Taylor principle)**: for a feature of size, the surface shall not violate a boundary of perfect form at MMC, so a single size dimension imposes *two* requirements simultaneously, a size limit and a perfect-form-at-MMC envelope. The 2018 revision tightened datum-feature definitions, clarified profile tolerancing, and added dynamic profile and a continuous-feature concept.

**Limitations.** Y14.5 is prose plus pictures: it is precise enough for human draftsmen but not formally machine-interpretable. Its companion math standard (Y14.5.1, section 1.3) exists precisely because Y14.5 alone leaves edge cases (irregular datum features, candidate datums, MMC boundary computation) underspecified for software.

**Kernel relevance.** Y14.5 fixes the vocabulary Keel's PMI attribute schema must encode: characteristic enum (14 values), tolerance magnitude, material-condition modifier enum, datum reference frame as an *ordered* list of datum references each with its own modifier. Rule #1 tells Keel it must be able to generate a perfect-form-at-MMC envelope for a size feature, that is, an offset/virtual boundary solid, on demand.

## 1.2 ISO GPS system (ISO 1101, ISO 8015, ISO 5459) and the ASME/ISO philosophical split

**Citation.** International Organization for Standardization. (2017). *ISO 1101:2017: Geometrical product specifications (GPS), Geometrical tolerancing, Tolerances of form, orientation, location and run-out*. Geneva: ISO. Related: ISO 8015 (fundamental concepts, independency principle), ISO 5459 (datums and datum systems). See also Pawel Marczak's comparative analyses and GD&T Basics, "A Comparison of GD&T Standards: ISO GPS vs. ASME Y14.5."

**Content.** ISO Geometrical Product Specification (GPS) is a layered system of dozens of interlocking standards, with ISO 1101 the central geometric-tolerancing document and ISO 8015 the foundational-principles document. The defining philosophical difference from ASME is the default coupling of size and form. **ASME defaults to the envelope principle (Rule #1)**, where size controls form via the perfect-form-at-MMC boundary. **ISO defaults to the independency principle (ISO 8015)**, where each size and each geometric specification is satisfied *independently* unless the envelope requirement is explicitly invoked with the circled-E modifier. The polarity is inverted: ASME has envelope by default and can relax it; ISO has independency by default and can add envelope. ISO 5459 treats datums with greater explicit mathematical rigor than legacy Y14.5, defining the simulated (associated) datum feature as a theoretically exact geometry fitted to the real datum feature under stated constraints.

**Limitations.** The two systems are not freely interconvertible. A drawing read under the wrong default can yield a different acceptance/rejection decision for the same physical part. ISO GPS is also large and fragmented, which complicates complete software coverage.

**Kernel relevance.** Keel's tolerance schema must carry a *standard flavor* flag (ASME vs ISO) and must NOT bake the envelope-vs-independency default into geometry generation. The same feature may need a perfect-form boundary under ASME defaults and not under ISO defaults. Datum simulation (fitting associated features) is required by both but with subtly different constraint rules, so the fitting service must be parameterizable.

## 1.3 ASME Y14.5.1-2019: the mathematical definition standard

**Citation.** American Society of Mechanical Engineers. (2019). *ASME Y14.5.1-2019: Mathematical Definition of Dimensioning and Tolerancing Principles* (revision of Y14.5.1M-1994). New York: ASME. See also CMM Quarterly, "Understanding ASME Y14.5.1."

**Content.** Y14.5.1 is the formal, math-based companion to Y14.5: it converts each tolerance class into precise point-set and constraint definitions so that software can compute conformance unambiguously. It defines tolerance zones as regions of space (for example, a position tolerance zone as a cylinder of diameter equal to the tolerance, located by the datum reference frame) and conformance as "the toleranced feature lies within the zone." Its most important contributions for a kernel are the **candidate datum set** and the **candidate datum reference frame set**: because a real (imperfect) datum feature does not uniquely determine a single datum (a slightly non-flat face can be touched by many valid contacting planes), the standard defines the *set* of all valid candidate datums establishable from a datum feature, and the set of all candidate datum reference frames. A part conforms if there *exists* a candidate datum reference frame under which all tolerances are met. The 2019 revision aligned with Y14.5-2018 and added a stabilization definition for irregular RMB datum features as an alternative to the full candidate-datum-set approach.

**Limitations.** The candidate-set definitions are existentially quantified ("there exists a valid datum frame"), which is mathematically clean but computationally a global optimization problem, not a closed-form evaluation. Implementations approximate it.

**Kernel relevance.** This is the single most important standard for Keel's geometric services. It tells Keel exactly what computations the kernel must support: fitting contacting/associated features to faces under constraints, enumerating or optimizing over candidate datum frames, and testing point-set containment in computed tolerance zones. The candidate-datum-set concept means datum establishment is an *optimization* the kernel's fitting routines must serve, not a one-shot least-squares fit.

---

# PART 2: Formal and computational tolerance semantics

## 2.1 Requicha (1983, 1986): toward a theory of geometric tolerancing

**Citation.** Requicha, A. A. G. (1983). Toward a theory of geometric tolerancing. *The International Journal of Robotics Research, 2*(4), 45-60. Companion: Requicha, A. A. G., & Chan, S. C. (1986). Representation of geometric features, tolerances, and attributes in solid modelers based on constructive geometry. *IEEE Journal of Robotics and Automation, 2*(3), 156-166.

**Content.** Requicha gave the first rigorous mathematical theory of tolerancing aimed at solid modeling. The core idea: a tolerance on a feature defines a **tolerance zone** as an *offset region*, the set of points between two offset surfaces (inner and outer offsets of the nominal surface at distances set by the tolerance). A part is acceptable if its actual toleranced surface lies entirely within that zone. He generalized this into the **variational class**: the set of *all* solids that satisfy a given nominal shape plus its full tolerance specification. The variational class is the formal meaning of a toleranced drawing, namely the family of acceptable parts. Requicha conjectured that variational classes are regular closed sets in a hyperspace whose elements are r-sets (regular point sets, the standard solid-modeling object). His framework attaches tolerances to *faces* of a solid model and treats size, position, orientation, and form tolerances as constraints carving out the zone. He noted that regardless-of-feature-size position tolerances yield bounded zones whereas MMC position tolerances naturally yield (generally unbounded) zones defined by offset solids, the latter fitting solid modeling more cleanly.

**Limitations.** The offset-zone model handles form and many position cases well but the variational-class hyperspace is an existence framework, not a directly computable object; building or testing membership in a variational class for general parts is open-ended. The theory predates and does not fully capture datum-precedence subtleties later formalized in Y14.5.1.

**Kernel relevance.** Requicha establishes the foundational requirement that *tolerance zones are offset geometry*. Keel must be able to generate inner/outer offset surfaces of a face at a given distance, which is the same offset machinery the kernel needs elsewhere (shelling, virtual condition). It also frames the conceptual model: a PMI annotation defines a zone; conformance is a containment test of actual geometry in that zone.

## 2.2 Srinivasan: virtual boundary requirement and GPS classification

**Citation.** Srinivasan, V. (2008/various). A geometrical product specification language based on a classification of symmetry groups. *Computer-Aided Design*. Related: Jayaraman, R., & Srinivasan, V. (1989). Geometric tolerancing: I. Virtual boundary requirements; II. Conditional tolerances. *IBM Journal of Research and Development, 33*(2). See also tolerance-synthesis work on the quantifier notion and virtual boundary (ScienceDirect, S0010448504001198).

**Content.** Srinivasan (with Jayaraman) introduced the **Virtual Boundary Requirement (VBR)** and conditional tolerancing, a functional reformulation of GD&T grounded in assembleability. The VBR states a part's controlled feature must not violate a *virtual boundary* (a perfect-form boundary offset by the combined size and geometric tolerance, the MMC virtual condition), so the part is guaranteed to assemble with its mating gauge. This reframes tolerancing around *gauging*: a virtual gauge (material or digital) of the virtual-condition boundary either admits or rejects the part. Srinivasan also contributed heavily to the *classification* of GPS standards, organizing them around symmetry groups of features (the seven invariance classes: spherical, planar, cylindrical, helical, revolute, prismatic, complex) which dictate which tolerances and datums are meaningful for each feature type.

**Limitations.** VBR is most natural for size features and assembly-driven tolerances (position at MMC); it is less direct for pure form or profile tolerances that are not assembly-bounding. The symmetry-class formalism is elegant but adds an abstraction layer not present in shop-floor GD&T.

**Kernel relevance.** The VBR tells Keel concretely what a virtual-condition boundary is: an offset solid at the virtual-condition size. To support functional gauging Keel must generate these virtual-boundary solids (offset by tolerance) and test containment, again the offset + interference machinery. The symmetry-class taxonomy is a useful organizing principle for the kernel's feature-recognition and datum-fitting code: a cylindrical face affords axis datums and cylindricity; a planar face affords plane datums and flatness.

## 2.3 Shah and collaborators: tolerance representation surveys

**Citation.** Hong, Y. S., & Chang, T. C. (2002). A comprehensive review of tolerancing research. *International Journal of Production Research, 40*(11), 2425-2459. Related surveys: Shah, J. J., Yan, Y., & Zhang, B.-C. (1998). Dimension and tolerance modeling and transformations in feature based design and manufacturing. *Journal of Intelligent Manufacturing, 9*; Roy, U., Liu, C. R., & Woo, T. C. (1991). Review of dimensioning and tolerancing: representation and processing. *Computer-Aided Design, 23*(7); Qin, Y., et al. (2018). A review of representation models of tolerance information. *International Journal of Advanced Manufacturing Technology, 95*.

**Content.** These surveys map the landscape of how tolerances are represented inside CAD/CAM. They distinguish the main schools: (a) attribute/parametric approaches that hang tolerance values off CAD feature parameters; (b) the offset-zone / variational approaches (Requicha lineage); (c) the degrees-of-freedom / kinematic approaches (TTRS, technologically and topologically related surfaces, of Clement et al.) that classify how each tolerance constrains the relative motion of feature pairs; (d) graph-based and ontology-based schemes linking features, datums, and tolerances. They consistently identify the same hard problems: complete and consistent representation, the gap between human GD&T and machine semantics, datum-system formalization, and tolerance *transfer* between design, manufacturing, and inspection views of the same part.

**Limitations.** Surveys describe rather than resolve; the field had (and has) no single dominant representation, which is itself the finding. Many representations are tied to a particular CAD feature model and do not survive neutral exchange.

**Kernel relevance.** The TTRS / degrees-of-freedom view is directly useful: it says each tolerance type constrains specific DOFs between feature pairs (a perpendicularity constrains two rotational DOFs, a position constrains translations). Keel's datum/tolerance schema should be able to express which DOFs a datum reference frame removes, which is the bridge to tolerance analysis. The surveys also confirm that the durable representation is feature-and-datum-graph based, reinforcing the persistent-reference requirement.

## 2.4 Davidson and Shah: Tolerance-Maps (T-Maps)

**Citation.** Davidson, J. K., Mujezinovic, A., & Shah, J. J. (2002). A new mathematical model for geometric tolerances as applied to round faces. *Journal of Mechanical Design, 124*(4), 609-622. Extensions: Mujezinovic, Davidson, & Shah (2004, polygonal faces); Ameta, Davidson, & Shah (2007, point-line clusters); Davidson, Shah et al. (2012-2016, line-profiles via Boolean intersection of T-Map primitives, e.g. *J. Computing and Information Science in Engineering, 12*(4), 041004).

**Content.** A **Tolerance-Map (T-Map)** is a hypothetical Euclidean point-space whose size and shape encode *all* the allowable geometric variations of a feature within its tolerance zone. Each point in the T-Map corresponds to one acceptable position/orientation/form state of the feature; the boundary of the T-Map is the locus of states that just reach the tolerance limit. Dimensionality depends on the feature and tolerance: a planar face's position+orientation T-Map is built in a multi-dimensional space, an axis T-Map is four-dimensional. T-Maps are constructed as convex (or piecewise) point-sets and combined by Minkowski-sum-like and Boolean operations to model stacked tolerances and assemblies; profile T-Maps for line-profiles are built by decomposing the profile into segments, making a primitive T-Map per segment, and intersecting them. Because a T-Map is a geometric object, statistical tolerance analysis can be done by integrating probability density over the T-Map volume, and worst-case analysis by examining its extremes.

**Limitations.** T-Maps grow in dimension and construction complexity with feature/tolerance richness; general freeform-surface profile T-Maps remain hard. Building them requires nontrivial computational geometry (high-dimensional convex hulls, Minkowski sums). They are a research representation, not a shipping CAD format.

**Kernel relevance.** T-Maps are downstream of the kernel (an analysis tool), but they reveal what analysis consumers want: a *geometric* (set-valued) model of allowable variation per toleranced feature. If Keel exposes clean offset-zone and DOF information per tolerance, T-Map and SDT tools can be built on top. The Minkowski-sum requirement (file 10) reappears here as the operation that composes tolerance variations along an assembly stack.

## 2.5 Tolerance specification validation (Anand and Jackman; commercial advisors)

**Citation.** Anand, A., & Jackman, J. (various). Validation model for detecting syntax and semantic errors in geometric dimensioning and tolerancing specifications. Related commercial: Sigmetrix GD&T Advisor (EnginSoft / Sigmetrix). See also "Interpreting the semantics of GD&T specifications of a product for tolerance analysis" (NIST-associated).

**Content.** This work treats GD&T as a formal language with *syntax* (the legal structure of a feature control frame and its datum references) and *semantics* (the geometric meaning of a callout). A validation model must (1) check completeness of a specification, (2) check that the toleranced feature is appropriate for the characteristic (you cannot apply cylindricity to a plane), (3) verify the datum reference frame is well-formed and non-ambiguous, and (4) check compatibility/consistency with the part's other tolerances and with assembly requirements. The authors present an XML-based scheme to classify and detect syntactic and semantic errors against ASME rules, including a datum evaluator that flags ambiguous datum referencing on a 3D model. Commercial tools (Sigmetrix GD&T Advisor) productionize this: they let users create only syntactically valid feature control frames, validate completeness (every controllable degree of freedom is constrained), and give function-oriented feedback.

**Limitations.** Completeness checking against *function* requires knowing design intent, which is hard to infer; tools check structural completeness, not whether the tolerance scheme actually guarantees the part works. Rule coverage is standard-version specific.

**Kernel relevance.** Validation is mostly an application-layer concern, but it dictates what the kernel must expose to enable it: feature-type queries ("is this face planar / cylindrical?"), DOF analysis of a datum frame ("which DOFs remain unconstrained?"), and the ability to detect that a referenced entity exists and is of the right kind. Keel's PMI attributes should therefore be typed and validated at attach time (characteristic must match feature type), preventing semantically impossible annotations from being stored.

---

# PART 3: Tolerance analysis (CAT)

## 3.1 Stack-up analysis: worst-case, RSS, Monte Carlo

**Citation.** Industry/standard practice as documented by Enventive, 3DCS (DCS), and Blackrock Engineering; see also ASME B4 dimensioning practices. Representative writeup: "What are Worst Case, RSS, and Monte Carlo Simulation Calculations for Tolerance Analysis?" (Enventive).

**Content.** Tolerance stack-up analysis predicts the accumulated variation of an assembly measurement from the tolerances of its contributing dimensions. **Worst-case** sums the absolute tolerance contributions (each at its extreme, with sensitivity sign), guaranteeing 100% conformance but yielding the largest, most expensive tolerances. **Root-sum-square (RSS)** statistical stacking adds contributions in quadrature (square root of the sum of squares), assuming independent, normally distributed contributions, giving a realistic spread for high-volume production and looser, cheaper individual tolerances at a small accepted defect rate. **Monte Carlo simulation** samples each contributor from its assumed distribution thousands of times, evaluates the assembly function each time, and builds an empirical distribution of the output, capturing nonlinearity, non-normal distributions, and geometric (non-1D) effects that RSS cannot.

**Limitations.** Worst-case is overly conservative for many real assemblies; RSS assumes independence and normality that may not hold and is unreliable for highly nonlinear or geometric (2D/3D) stacks; Monte Carlo is only as good as the assumed input distributions and sensitivity model.

**Kernel relevance.** Stack-up is an application built on the kernel's *sensitivity* outputs: how a measured assembly dimension changes as each contributing feature moves within its tolerance. Keel can support this by exposing exact distance/measurement evaluation and derivatives (Jacobians) between referenced features, so a CAT layer can compute sensitivities analytically rather than by finite differencing the whole model.

## 3.2 3D tolerance analysis: SDT, vector loop, matrix, Jacobian-torsor (survey)

**Citation.** Chen, H., Jin, S., Li, Z., & Lai, X. (2014). A comprehensive study of three dimensional tolerance analysis methods. *Computer-Aided Design, 53*, 1-13. Foundational: Bourdet, P., & Clement, A. (small displacement torsor, SDT); Desrochers, A. (Jacobian and unified Jacobian-torsor); Chase, K. W. (vector loop, ADCATS/CATS at BYU).

**Content.** This survey compares the four dominant 3D CAT models. (1) **Vector loop** (Chase) represents the assembly as chains of vectors (dimensions) closed into loops; tolerances perturb vector lengths/angles and the loop closure equations are linearized to propagate variation. (2) **Matrix model** uses homogeneous transformation matrices with small variational parameters to represent feature displacements. (3) **Small Displacement Torsor (SDT)** (Bourdet/Clement) represents the small rigid-body deviation of a real surface from its nominal as a torsor: three small translations and three small rotations (a screw), with components left *unconstrained* in the directions the surface is invariant (a plane's in-plane translations and normal rotation are free). Tolerance zones become bounds on the torsor components. (4) **Jacobian-torsor** (Desrochers) unifies the two: the torsor captures each feature's variability, and a Jacobian matrix (from robotics kinematics) propagates those small displacements through the assembly's functional element chain to the final requirement. The unified model supports worst-case (interval) and, combined with Monte Carlo, statistical analysis.

**Limitations.** All are *small-displacement* linearizations: accurate for small tolerances, degrading for large or coupled variations. Form errors (within-feature waviness) are poorly captured by rigid-feature torsors; the methods assume features are perfect and only their pose varies. Building the functional element chain and the constraint set is manual and error-prone.

**Kernel relevance.** The SDT formalism maps cleanly onto kernel services: each face/datum has an associated frame, and a tolerance bounds the allowed small screw displacement of that frame, with free components determined by the face's invariance class (Srinivasan's symmetry groups again). Keel should be able to report, per face, its invariance class and a local frame, and per datum reference frame, the transformation chain, so a CAT tool can assemble torsors and Jacobians from kernel data instead of reconstructing geometry.

## 3.3 Commercial CAT tools as documented: CETOL 6σ and 3DCS

**Citation.** Sigmetrix CETOL 6σ product documentation; Dimensional Control Systems (DCS) 3DCS Variation Analyst documentation (e.g., "Understanding Worst Case Tolerance Analysis", 3dcs.com; CATIA/Creo/NX integration docs).

**Content.** **CETOL 6σ** (Sigmetrix) is a CAD-integrated tolerance analysis tool that builds a sensitivity-based model from the CAD geometry and GD&T, computing how each tolerance contributes to each measurement using analytical sensitivities, and reporting worst-case and statistical results plus contributor ranking (which tolerances drive the variation). **3DCS** (DCS) is a Monte-Carlo-centric variation simulation suite integrated into CATIA, Creo, NX, and offered standalone; it models locating schemes, moves parts within their tolerances, simulates assembly, and reports measurement distributions, with options for worst-case, sensitivity (high-low-median), and a GeoFactor RSS computation. Both consume the CAD model's features and GD&T and rely on a defined assembly/locating scheme (datum-driven part placement).

**Limitations.** Both depend on correct, machine-readable GD&T in the model (semantic PMI), and on a correctly specified assembly locating scheme; garbage GD&T yields garbage analysis. They are proprietary and CAD-host-coupled.

**Kernel relevance.** These tools define the *consumer contract* for Keel's PMI: they need (a) stable references to the toleranced and datum features, (b) machine-readable tolerance values and modifiers, and (c) exact geometric evaluation of measurements between features. If Keel exposes these, an open-source CAT layer (or a bridge to existing tools via QIF/AP242) becomes feasible. The "contributor ranking" need confirms the value of analytic sensitivities from the kernel.

---

# PART 4: PMI in and between CAD systems

## 4.1 Semantic vs graphical (presentation) PMI: the core distinction

**Citation.** Synthesized from STEP AP242 documentation (CADInterop) and NIST PMI testing reports (below). See also Capvidia, "QIF Definitive Guide."

**Content.** PMI exists in two fundamentally different forms. **Presentation (graphical) PMI** is the visual rendering of an annotation: tessellated polylines and text positioned in 3D space so a viewer reproduces exactly what the author drew. It is for humans; it carries no machine-interpretable meaning, only appearance. **Representation (semantic) PMI** is the structured, computer-interpretable data: a position tolerance of value 0.1 at MMC on *this* face referencing datums A, B, C, stored as parametric entities with explicit links to the geometry. Semantic PMI is queryable by CAM, CMM, and inspection software; presentation PMI is not. A well-formed MBD model carries both, ideally consistent with each other, but they can diverge (the picture says one thing, the data another), which is a major source of downstream error. Best practice and the NIST methodology give *precedence to semantic representation* over presentation.

**Limitations.** Maintaining consistency between the two forms is a known failure point; some CAD systems author strong graphics but weak/absent semantics, or vice versa.

**Kernel relevance.** Keel must store PMI as *semantic* data first: typed tolerance objects bound to entity references, not as drawn geometry. Presentation (placement of the callout, leader lines, text) is a separate, optional layer that references the same semantic object. This separation is the kernel-level analog of the model/drawing split and is mandatory for MBD.

## 4.2 NIST PMI CAD model verification (Lipman): the fidelity studies

**Citation.** Lipman, R. R., & Lubell, J. (2015). Conformance checking of PMI representation in CAD model STEP data exchange files. *Computer-Aided Design, 66*, 14-23. Companion guide: Lipman, R. R. (2017). *Guide to the NIST PMI CAD Models and CAD System PMI Modeling Capability Verification Testing Results* (NIST Advanced Manufacturing Series 100-10). Gaithersburg, MD: NIST. Further: Lipman, R. R., & Filliben, J. J. (2020). Testing the reliability of model-based definition CAD systems and exchange. *Computer-Aided Design & Applications, 17*(6), 1241-1265.

**Content.** NIST built a public-domain suite of test-case CAD models with GD&T applied to representative geometry, and modeled them in the four major CAD systems (CATIA, Creo, NX, SolidWorks) as *semantic* PMI, then exported to STEP and checked fidelity. This was the first rigorous public testing of CAD GD&T implementations. Errors were collected in two families. **Presentation (graphical) quality** was scored on six aspects: visibility, layout, location, orientation, lines, and text. **Semantic representation** quality was scored on whether the exported data correctly captured the GD&T meaning. The verification testing found hundreds of issues: Lipman and Filliben report on the order of 411 PMI errors (about 98 unique) feeding the high-level results, across the four systems. Findings: CAD systems vary widely in PMI fidelity; semantic export through STEP frequently loses or distorts information (missing datum references, wrong material-condition modifiers, broken geometry associations); presentation and representation can disagree within the same model.

**Limitations.** The studies are snapshots of specific CAD/STEP versions; vendors improve over time. The test cases, while representative, do not exhaust real-world GD&T complexity (especially profile on freeform).

**Kernel relevance.** This is the empirical case for getting Keel's PMI foundations right: the dominant failure mode in industry is *loss of semantic PMI on attachment and on exchange*. Keel must (a) attach PMI to persistent, exact entity references so the geometry association does not break, (b) store all semantic fields (every datum reference, every modifier) losslessly, and (c) round-trip them through AP242/QIF without dropping data. The six presentation aspects are a checklist for any future Keel presentation layer.

## 4.3 STEP AP242 semantic PMI: the exchange representation

**Citation.** ISO 10303-242 (STEP AP242), "Managed model based 3D engineering." Documentation: CADInterop AP242 overview; NIST AMS 200-6, *STEP File Analyzer and Viewer* (Lipman). Application study: (gated) Springer IJIDeM (2023), "Using semantic GD&T information from STEP AP242 neutral exchange files for robotic applications."

**Content.** AP242 merges the former aerospace AP203 and automotive AP214 into one managed-model-based-engineering protocol and is the principal neutral format for semantic PMI. It carries both exact B-rep geometry and tessellated geometry, and both PMI **representation (semantic, parametric)** and PMI **presentation (polyline/tessellated graphics)**. Semantic PMI is built from EXPRESS entities: a `shape_aspect` (and subtypes) identifies the toleranced portion of a feature; `geometric_tolerance` and its subtypes (`flatness_tolerance`, `position_tolerance`, `geometric_tolerance_with_datum_reference`, etc.) carry the characteristic, magnitude, and modifiers; `datum`, `datum_feature`, and `datum_reference`/`datum_system` entities define the datum reference frame; dimensional tolerances use `dimensional_size`/`dimensional_location` with `tolerance` value entities. The crucial linkage is `geometric_item_specific_usage` (and the shape-representation relationship machinery), which *binds the semantic PMI to the actual B-rep faces/edges*. AP242 has dozens of shape_aspect-category entities to cover the GD&T vocabulary.

**Limitations.** The schema is large, and implementer interpretation varies, which is exactly why NIST testing found exchange losses. The geometry-to-PMI binding (`geometric_item_specific_usage`) is fragile across translators; if face identity is not preserved, the binding dangles.

**Kernel relevance.** AP242 defines the *data model Keel's PMI layer should mirror* so that export is a near-direct mapping: a tolerance object referencing a face id maps to `geometric_tolerance` + `shape_aspect` + `geometric_item_specific_usage` pointing at the exported B-rep face. To make this binding survive export, Keel must give each persistent face/edge a stable identity that the STEP writer can reference. This is the persistent-naming requirement expressed in the exchange layer.

## 4.4 JT PMI and the broader exchange landscape

**Citation.** ISO 14306 (JT). General positioning relative to AP242 from CADInterop and QIF documentation.

**Content.** JT (ISO 14306) is a lightweight visualization-and-collaboration format widely used in automotive (Siemens-originated). It carries tessellated geometry, product structure, and PMI, with strong support for *presentation* PMI and growing support for *semantic* PMI. JT is optimized for fast viewing of large assemblies rather than for full parametric exchange; it commonly travels alongside AP242 in PLM workflows (JT for visualization, AP242 for authoritative semantic data). The general landscape: AP242 is the authoritative neutral semantic/geometry format, JT the visualization format, and QIF (below) the quality/metrology format, with overlap and ongoing harmonization efforts.

**Limitations.** JT semantic PMI maturity historically lags AP242; relying on JT alone risks the same semantic-loss problems NIST documented, plus its tessellated-first geometry is lossy versus exact B-rep.

**Kernel relevance.** Keel does not need to *be* a JT producer, but the lesson is that a kernel must keep authoritative semantic PMI separate from any lightweight visualization derivative, so a JT-style tessellated export can be generated without becoming the source of truth.

## 4.5 QIF: the Quality Information Framework

**Citation.** ANSI/DMSC QIF (Quality Information Framework), Dimensional Metrology Standards Consortium; QIF 2.x/3.x. Surveys: Morse, E., et al. / NIST IR 8127 (2016), *End-to-End Quality Information Framework (QIF) Technology Survey*. Practitioner: Capvidia, "QIF Definitive Guide"; Action Engineering, "ANSI QIF 2.0 Revealed."

**Content.** QIF is an ANSI standard (sponsored by DMSC) defining an integrated set of XML information models that carry the digital quality thread end to end: MBD geometry plus semantic GD&T, measurement *plans*, measurement *resources/rules*, measurement *results*, and multi-part *statistics*, all in one queryable, schema-validated document family. Its ontology is **feature-based and characteristic-centric**: every controlled characteristic (a Bill of Characteristics) is an addressable object semantically linked back to the MBD geometry for full traceability. QIF's measurement plan supports graduated detail: what to measure (Bill of Characteristics), how to measure (inspection plan), which resources (CMM, probes), and where (sampling point locations). Because it is XML with built-in validation and explicit links to model geometry, QIF is designed to be machine-consumed by inspection-planning and CMM software with minimal human re-transcription.

**Limitations.** QIF requires upstream semantic PMI to exist and be correct; it inherits any authoring/exchange losses. Full adoption requires CAD, CAM, and metrology vendors to all implement it consistently.

**Kernel relevance.** QIF is the downstream destination that most clearly justifies Keel's PMI design: every QIF characteristic links to a model feature, so Keel's persistent face/edge references and semantic tolerance objects are exactly what a QIF exporter needs to populate the Bill of Characteristics with valid geometry traceability. If Keel can emit per-characteristic feature references, an open metrology pipeline (plan, measure, report) becomes buildable.

## 4.6 Model-Based Definition adoption studies

**Citation.** Goher, K., Shehab, E., & Al-Ashaab, A. (2021). Model-Based Definition and Enterprise: State-of-the-art and future trends. *Proc. IMechE Part B: J. Engineering Manufacture, 235*(14). Related: Quintana, V., et al. (2010). Will Model-based Definition replace engineering drawings throughout the product lifecycle? *Computers in Industry, 61*; Ruemler et al. (current state of MBD); Finnish manufacturing ecosystem case studies (Springer 2023).

**Content.** Adoption studies find MBD's benefits concentrate in the digital downstream thread: better communication between engineering and manufacturing, reduced re-interpretation, faster CMM/CAM programming when semantic PMI exists, and elimination of drawing/model divergence. Adoption is led by aerospace and automotive (the high-value, regulated, large-supply-chain industries) and lags in SMEs. Surveyed barriers: large capital investment (cited as the biggest risk, ~28%), legacy 2D drawing archives (~22%), lack of perceived business pull (~22%), software/IT infrastructure upgrades, and workforce training. A recurring theme is that MBD value is unlocked only when PMI is *semantic and consumable* downstream, not merely 3D-rendered.

**Limitations.** Survey populations skew to early adopters and large enterprises; numbers vary by study and region. Causality (does MBD cause the benefits) is hard to isolate.

**Kernel relevance.** The adoption literature is the business case for Keel investing in semantic PMI from day one: an open kernel that produces clean, consumable, exchangeable semantic PMI lowers exactly the barriers SMEs cite (tooling cost, interoperability friction). It confirms PMI is a first-class deliverable, not an afterthought.

---

# PART 5: Inspection, datum fitting, and freeform tolerancing

## 5.1 CMM programming and automated inspection planning from PMI

**Citation.** Nielsen, H. S., et al. CMM Automation from MBD (NIST-hosted case study). DMIS standard (ISO 22093). QIF inspection-plan tutorials (qifstandards.org). General: Hu, J., & Xiong, G. (automated inspection planning literature).

**Content.** The model-based inspection workflow consumes semantic PMI to generate CMM programs with minimal manual transcription. The chain: parse the semantic PMI/QIF model to extract the Bill of Characteristics (every toleranced characteristic and its feature); apply measurement rules and available resources to decide how each characteristic is measured (probe, strategy); compute sampling point locations on the feature geometry; and emit a DMIS (or vendor) program for the CMM. A documented pilot generated a CMM program in under three hours versus far longer manual programming. The key enabler is *semantic* PMI: presentational PMI cannot be queried for the characteristic's feature, value, and datum frame, so manual re-keying (and its errors) is unavoidable without it.

**Limitations.** Automation quality depends on completeness of semantic PMI and on accessibility/probe-reachability of features (a planning constraint the kernel does not directly solve). Measurement strategy selection still encodes shop knowledge.

**Kernel relevance.** Inspection planning needs three kernel services: (1) resolve a PMI reference to its actual face/edge geometry (persistent references), (2) sample points on a face (UV sampling of NURBS faces, file 05/06 neighborhood), and (3) provide the datum frame transformation to express measurements in the datum reference frame. Keel exposing these makes an open inspection-planning layer feasible.

## 5.2 Datum feature simulation and substitute-geometry fitting (Chebyshev/min-zone vs least-squares; ISO 10360)

**Citation.** Shakarji, C. M. (1998). Least-squares fitting algorithms of the NIST algorithm testing system. *Journal of Research of NIST, 103*(6). Shakarji, C. M., & Srinivasan, V. (2007). Reference algorithms for Chebyshev and one-sided data fitting for coordinate metrology. *CIRP Annals, 56*(1). Standard: ISO 10360 (acceptance/reverification of CMMs) and ISO/TS 10360-6 (testing of Gaussian/least-squares element software). See also TraCIM validation system.

**Content.** Establishing a datum from a real datum feature means fitting a *substitute (associated) geometric element* (plane, cylinder, cone, sphere, line) to the feature's points. The fitting objective matters: **least-squares (L2, Gaussian)** minimizes the sum of squared residuals, is stable and unique, and is the default many CMM packages use; **minimum-zone / Chebyshev (L∞)** minimizes the maximum residual (the peak-to-valley deviation), which is what form-tolerance definitions (flatness, cylindricity) and minimum-circumscribed/maximum-inscribed datum simulators actually require. Using least-squares where the standard demands min-zone *over-estimates* form error and gives wrong datums. NIST provides reference algorithms for Chebyshev fits of lines, planes, circles, spheres, cylinders, and cones, typically seeded by a least-squares fit then iterated to the L∞ solution. ISO 10360 governs CMM testing; ISO/TS 10360-6 specifically addresses testing of least-squares element-fitting software, while Chebyshev software standardization is less mature. The candidate-datum-set concept (Y14.5.1, section 1.3) is the formal reason a single least-squares fit is insufficient: valid datums are *constrained* contacting elements (e.g. a minimum-circumscribed cylinder, a tangent plane that does not penetrate material), and there can be a set of them.

**Limitations.** Min-zone/Chebyshev fits are non-smooth optimization (nonunique gradients at the solution), harder and slower than least-squares, and sensitive to outliers/sampling. Constrained datum fits (tangent, non-penetrating) add inequality constraints that make the problem a constrained optimization, not a plain fit.

**Kernel relevance.** This is a hard, mandatory kernel service for true GD&T support. Keel needs a fitting subsystem that can fit planes/cylinders/cones/spheres/lines to point sets (or to a face's sampled points) under *multiple objectives*: least-squares for nominal/measurement, minimum-zone (Chebyshev) for form-error evaluation, and constrained (min-circumscribed, max-inscribed, tangent non-penetrating) for datum simulation. It should follow NIST/ISO reference-algorithm definitions so results are standard-conformant. This subsystem is shared between datum establishment, form-tolerance evaluation, and feature recognition.

## 5.3 Profile tolerancing of freeform/NURBS surfaces: registration and deviation

**Citation.** Li, Y., & Gu, P. (2004/2005). Free-form surface inspection techniques state of the art review. *Computer-Aided Design, 36*. Min-zone NURBS: "Fast Evaluation of Minimum Zone form Errors of Freeform NURBS Surfaces" (CyberLeninka, open access). Registration: "A Registration Method for Profile Error Inspection of Complex Surface Under Minimum Zone Criterion" (*Int. J. Precision Eng. and Manufacturing*, 2019). PCA approach: "A quick deviation zone fitting in coordinate metrology of NURBS surfaces using PCA" (*Measurement*, 2016).

**Content.** Profile of a surface is the tolerance class that applies to freeform NURBS faces: it defines a 3D zone of width equal to the tolerance, bilaterally or unilaterally disposed about the true (nominal) surface, and requires the actual surface to lie within that zone. Evaluating it from measured points has two coupled subproblems: **registration** (aligning the measured point cloud to the nominal NURBS surface, since the part is measured in an arbitrary coordinate frame) and **deviation evaluation** (computing each point's signed distance to the nominal surface and the resulting profile error). State-of-the-art methods minimize the *maximum* deviation (minimum-zone criterion) rather than least-squares, because the profile tolerance is defined by the worst point. Reported approaches: decompose the NURBS surface into Bezier patches, run iterative-closest-point (ICP) for coarse registration, refine with orthogonal-distance least-squares, then solve a minimum-zone fit via interior-point or PSO/SQP optimization; or use PCA for a quick deviation-zone fit. The signed point-to-surface distance is the kernel primitive at the heart of all of these.

**Limitations.** Point-to-NURBS-surface distance is itself a nontrivial optimization (closest-point on a free-form surface) and must be robust to multiple local minima; registration and min-zone are jointly non-convex; the methods are point-cloud-density and noise sensitive.

**Kernel relevance.** Profile tolerancing makes Keel's NURBS distance/closest-point service (file 06) a tolerance-critical primitive: it must compute robust signed distance from arbitrary points to a NURBS face, which both profile evaluation and registration consume. Keel should also be able to generate the offset surfaces bounding the profile zone (offset of a NURBS face by ±t/2), again tying to offset research. Profile is the place where the kernel's freeform geometry and the tolerancing system meet most directly.

---

# PMI readiness requirements for Keel

Pulling the threads together, the kernel must provide the following to be MBD/PMI-capable, and most of these must be designed in before the topology, attribute, and reference layers freeze.

1. **Persistent, exact references for PMI attachment.** Every tolerance attaches to a face, edge, vertex, or feature. Those references must survive edit/regeneration (persistent naming, file 07) and survive export to AP242/QIF (stable ids the STEP/QIF writer can emit). This is the single hardest cross-cutting requirement; PMI is the canonical consumer that proves the naming system works.

2. **A semantic-first PMI attribute schema.** Store PMI as typed semantic objects, not as drawn geometry: characteristic enum (the 14 classes), tolerance magnitude(s), material-condition modifier (MMC/LMC/RFS), an *ordered* datum reference frame (each datum reference with its own modifier), and a *standard-flavor* flag (ASME vs ISO) so envelope-vs-independency defaults are not hard-coded. Presentation (callout placement, leaders, text) is a separate optional layer that references the semantic object. Validate at attach time that the characteristic is legal for the target feature type.

3. **Offset and virtual-condition geometry generation.** Tolerance zones are offset regions (Requicha); Rule #1 envelopes and MMC/LMC virtual boundaries (Srinivasan VBR) are offset solids. Keel must generate inner/outer offsets of faces (planar, quadric, and NURBS) at a given distance, and build virtual-condition boundary solids for containment/gauging tests. Shared with shelling and offset research (file 10 neighborhood).

4. **A substitute-geometry fitting subsystem with multiple objectives.** Fit plane/cylinder/cone/sphere/line to a face's points under: least-squares (L2) for measurement and nominal association, minimum-zone/Chebyshev (L∞) for form-error evaluation, and constrained fits (min-circumscribed, max-inscribed, tangent non-penetrating) for datum simulation. Follow NIST/ISO 10360 reference-algorithm definitions. This serves datum establishment, form tolerances, and feature recognition alike.

5. **Datum reference frame computation honoring candidate datum sets.** Per Y14.5.1, datum establishment is an existential optimization over candidate datums/frames, not a single fit. Keel must expose constrained fitting and the transformation chain so a tolerance evaluator can search candidate frames and test "exists a frame under which all tolerances pass." Report, per face, its invariance class (Srinivasan symmetry groups) and a local frame; per datum frame, which DOFs it removes (for SDT/torsor analysis).

6. **Exact measurement and signed-distance evaluation, with derivatives.** Stack-up and 3D CAT (vector loop, SDT, Jacobian-torsor) need sensitivities; profile evaluation and registration need robust point-to-NURBS-surface signed distance (file 06). Expose both exact distance/measurement between referenced features and, where possible, analytic Jacobians so CAT tools compute sensitivities without finite-differencing the model.

7. **Lossless exchange mapping to AP242 and QIF.** The PMI schema should mirror AP242's representation entities (`geometric_tolerance` subtypes, `shape_aspect`, `datum`/`datum_system`, `geometric_item_specific_usage`) and QIF's characteristic-centric, feature-linked model, so export is a near-direct mapping and the geometry-to-PMI binding does not dangle. NIST's testing shows semantic loss on attachment and exchange is the dominant industry failure mode; Keel's value proposition is to not lose it.

8. **Sampling and feature services for inspection planning.** Resolve a PMI reference to its geometry, sample points on a NURBS face, and supply datum-frame transforms, so an open CMM/inspection-planning and profile-evaluation pipeline can be built on the kernel.

In short: Keel must treat a tolerance as *a typed semantic object bound to a persistent entity reference that denotes an offset/zone geometry, evaluable by constrained fitting and signed-distance computation, and exportable losslessly to AP242/QIF*. Get the references, the attribute schema, the offset generator, and the fitting subsystem right early, and the rest of the MBD/PMI stack (analysis, inspection, exchange) can be layered on without re-cutting the kernel.

---

# References

1. American Society of Mechanical Engineers. (2018). *ASME Y14.5-2018: Dimensioning and Tolerancing*. New York: ASME.
2. International Organization for Standardization. (2017). *ISO 1101:2017: Geometrical product specifications (GPS), Geometrical tolerancing*. Geneva: ISO. (with ISO 8015, ISO 5459).
3. American Society of Mechanical Engineers. (2019). *ASME Y14.5.1-2019: Mathematical Definition of Dimensioning and Tolerancing Principles*. New York: ASME.
4. Requicha, A. A. G. (1983). Toward a theory of geometric tolerancing. *The International Journal of Robotics Research, 2*(4), 45-60.
5. Requicha, A. A. G., & Chan, S. C. (1986). Representation of geometric features, tolerances, and attributes in solid modelers based on constructive geometry. *IEEE Journal of Robotics and Automation, 2*(3), 156-166.
6. Jayaraman, R., & Srinivasan, V. (1989). Geometric tolerancing: I. Virtual boundary requirements; II. Conditional tolerances. *IBM Journal of Research and Development, 33*(2).
7. Srinivasan, V. (2008). Standardizing the specification, verification, and exchange of product geometry: Research, status and trends. *Computer-Aided Design, 40*(7-8). (and GPS classification work).
8. Hong, Y. S., & Chang, T. C. (2002). A comprehensive review of tolerancing research. *International Journal of Production Research, 40*(11), 2425-2459.
9. Qin, Y., et al. (2017). A review of representation models of tolerance information. *International Journal of Advanced Manufacturing Technology, 95*. https://link.springer.com/article/10.1007/s00170-017-1352-4
10. Davidson, J. K., Mujezinovic, A., & Shah, J. J. (2002). A new mathematical model for geometric tolerances as applied to round faces. *Journal of Mechanical Design, 124*(4), 609-622.
11. Ameta, G., Davidson, J. K., & Shah, J. J. (2007). Tolerance-Maps applied to a point-line cluster of features. *Journal of Mechanical Design*. (T-Map line-profile extensions, *JCISE 12*(4), 041004, 2012). https://link.springer.com/article/10.1631/jzus.A1400239
12. Anand, A., & Jackman, J. Validation model for detecting syntax and semantic errors in GD&T specifications. (and Sigmetrix GD&T Advisor). https://www.academia.edu/11611892/
13. Chen, H., Jin, S., Li, Z., & Lai, X. (2014). A comprehensive study of three dimensional tolerance analysis methods. *Computer-Aided Design, 53*, 1-13. https://www.sciencedirect.com/science/article/abs/pii/S0010448514000475
14. Desrochers, A., Ghie, W., & Laperriere, L. (2003). Application of a unified Jacobian-torsor model for tolerance analysis. *Journal of Computing and Information Science in Engineering, 3*(1). (SDT: Bourdet & Clement; vector loop: Chase, BYU ADCATS).
15. Enventive / DCS (3DCS) / Sigmetrix (CETOL 6σ) tolerance analysis documentation. https://www.3dcs.com/understanding-worst-case-tolerance-analysis ; https://enventive.com/tolerance-analysis-resources/
16. Lipman, R. R., & Lubell, J. (2015). Conformance checking of PMI representation in CAD model STEP data exchange files. *Computer-Aided Design, 66*, 14-23. https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=917105
17. Lipman, R. R. (2017). *Guide to the NIST PMI CAD Models and CAD System PMI Modeling Capability Verification Testing Results* (NIST AMS 100-10). NIST. https://nvlpubs.nist.gov/nistpubs/ams/NIST.AMS.100-10.pdf
18. Lipman, R. R., & Filliben, J. J. (2020). Testing the reliability of model-based definition CAD systems and exchange. *Computer-Aided Design & Applications, 17*(6), 1241-1265. https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=926080
19. ISO 10303-242 (STEP AP242), "Managed model based 3D engineering." STEP File Analyzer (NIST AMS 200-6). https://www.cadinterop.com/en/our-products/cadfix/320-step-ap242.html
20. ISO 14306 (JT). (PMI in JT; AP242/JT/QIF landscape.) https://www.cadinterop.com/en/formats/neutral-format/qif.html
21. ANSI/DMSC QIF (Quality Information Framework). NIST IR 8127 (2016), *End-to-End QIF Technology Survey*. https://nvlpubs.nist.gov/nistpubs/ir/2016/NIST.IR.8127.pdf ; https://qifstandards.org/about-qif/
22. Goher, K., Shehab, E., & Al-Ashaab, A. (2021). Model-Based Definition and Enterprise: State-of-the-art and future trends. *Proc. IMechE Part B, 235*(14). (with Quintana et al. 2010, *Computers in Industry, 61*). https://journals.sagepub.com/doi/10.1177/0954405420971087
23. Shakarji, C. M., & Srinivasan, V. (2007). Reference algorithms for Chebyshev and one-sided data fitting for coordinate metrology. *CIRP Annals, 56*(1). ISO 10360 / ISO/TS 10360-6. https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=822103
24. Li, Y., & Gu, P. (2004). Free-form surface inspection techniques state of the art review. *Computer-Aided Design, 36*. Min-zone NURBS form error and registration: *Int. J. Precision Eng. Manuf.* (2019); PCA deviation-zone fitting, *Measurement* (2016). https://cyberleninka.org/article/n/1278624
25. Lipman, R. R., & Filliben, J. J. / NIST. MBE PMI Validation and Conformance Testing Project. https://www.nist.gov/el/systems-integration-division-73400/mbe-pmi-validation-and-conformance-testing-project
26. Marczak, P. / GD&T Basics. A Comparison of GD&T Standards: ISO GPS vs. ASME Y14.5. https://www.gdandtbasics.com/iso-vs-asme-standards/ ; "Can ISO GPS and ASME Tolerancing Systems Define the Same Functional Requirements?" *Applied Sciences, 11*(17), 8269 (2021). https://www.mdpi.com/2076-3417/11/17/8269
27. Nielsen, H. S., et al. CMM Automation from MBD: optimized Model Based Inspection (NIST-hosted). DMIS (ISO 22093). https://www.nist.gov/document/4drp4nielsenroicmmautomationpdf
