# Kinematics and Motion Layer for CAD Assemblies

Research file 21 for the Keel kernel. This file covers the motion tier that sits above the static assembly layer (occurrence DAGs, transforms, clash) and the static constraint solver (Kramer DOF analysis, 3D DCM). The question this file answers is: what kinematic and motion capability must a Parasolid-class B-rep kernel provide as services, and what belongs in a separate motion or simulation layer above it?

The short framing: a geometry kernel is not a motion solver and is not a multibody dynamics engine. But the kernel is the sole authority for the geometric and inertial facts that every motion solver consumes: mass properties, joint axes derived from faces, swept volumes, and continuous collision queries. The boundary is a data contract, and getting that contract right is the design task.

## Scope

- Mate to joint semantics: how CAD mates map to kinematic joints, published mappings, joint limits, the mate-versus-joint modeling debate (Onshape's explicit-joint rationale).
- Kinematic analysis foundations: Kramer's degrees-of-freedom analysis, Gruebler/Kutzbach mobility counting and its failure cases, screw theory (twists, wrenches, se(3)).
- Kinematic loop solving: open chains versus closed loops, loop closure equations, four-bar through Stewart platform, Newton on loop equations, singularity handling.
- Drag and manipulation: real-time constraint-consistent dragging, warm starting, continuation, D-Cubed DCM kinematic mode and AEM.
- Multibody dynamics boundary: reduced versus maximal coordinates, Featherstone, what CAD motion studies do, the data contract.
- Motion envelopes and swept volumes of mechanisms, continuous collision detection (conservative advancement).
- Joint geometry from B-rep: deriving joint axes from faces, kinematic pair recognition, cam and gear and follower contact.
- Tolerance and clearance effects on kinematics.
- Open implementations: MuJoCo, Drake, Bullet, FreeCAD/Ondsel, SolveSpace.

---

## Theme 1: Mate semantics versus joint semantics

The central modeling decision is whether the assembly is described by low-level mates (pairwise geometric constraints between faces and edges, the SolidWorks tradition) or by explicit joints (a named kinematic pair between two coordinate frames, the Onshape and robotics tradition). These are two front-ends to the same underlying degree-of-freedom algebra, but they imply very different kernel-level data structures.

### Source 1.1: Onshape mates and kinematics

**Citation.** Onshape (PTC). "Kinematics: Assemblies, Mates, and Simulation." Onshape Blog. Retrieved 2026. https://www.onshape.com/en/blog/kinematics-assemblies-mates-simulation . Supporting: Onshape Help, "Mates," https://cad.onshape.com/help/Content/Assembly/mates.htm .

**Content.** Onshape made an explicit architectural choice: every mate is defined between two *mate connectors*, which are local coordinate systems (implicit ones snapped to circle centers, midpoints, corners, or explicit ones placed by the user in the Part Studio). Mating is "aligning two coordinate systems." Because both sides are full frames, each mate type is exactly a kinematic joint and removes a known set of the six DOF between the frames. The published mate set and the DOF each leaves free:

| Mate (Onshape) | Free DOF | Equivalent kinematic joint |
|---|---|---|
| Fastened | none (0) | rigid weld |
| Revolute | 1 rotation (about Z) | revolute / hinge |
| Slider | 1 translation (along Z) | prismatic |
| Cylindrical | 1 rotation + 1 translation (Z) | cylindrical |
| Pin Slot | 1 rotation (Z) + 1 translation (X) | pin-in-slot |
| Planar | 2 translation (XY) + 1 rotation (Z) | planar |
| Ball | 3 rotation | spherical |
| Parallel | 3 translation + 1 rotation | (alignment, not a classic pair) |

The base Fastened mate removes all six DOF in one operation, whereas legacy CAD typically needs three coincidence/concentric mates to fully locate a part. The decisive claim for Keel: "your Mates *are* your Simulation connections." Because each mate is already a joint with an axis and a frame, motion limits and kinematic relations (gear ratios, rack-and-pinion couplings) attach directly to mates, and motion analysis becomes interactive rather than a separate post-modeling step.

**Limitations.** Onshape's mate-connector model pushes complexity onto connector placement; the public blog is marketing-level and omits the solver. The "Parallel" mate is an alignment constraint, not a single lower kinematic pair, which shows the model is not purely a clean joint taxonomy.

**Kernel relevance.** This is the cleanest published mate-to-joint mapping and a strong default for Keel's API: define joints between two frames, where a frame is anchored to B-rep geometry. The eight-row table is essentially the joint enum the kernel should expose, plus weld. The mate-connector idea (a frame attached to a face/edge feature, recomputed when geometry regenerates) is exactly the persistent-naming-plus-frame primitive the kernel must supply.

### Source 1.2: SolidWorks mechanical mates (higher pairs)

**Citation.** Dassault Systemes SolidWorks. "Mechanical Mates." SOLIDWORKS Help (2025). https://help.solidworks.com/2025/English/SolidWorks/sldworks/c_Mechanical_Mates.htm . Supporting: GoEngineer, "Introduction to SOLIDWORKS Mates."

**Content.** SolidWorks splits mates into Standard (coincident, concentric, parallel, perpendicular, tangent, distance, angle), Advanced (limit, width, symmetric, path, linear coupler), and Mechanical (cam-follower, gear, rack-and-pinion, screw, slot, hinge, universal joint). The Standard mates are pure geometric constraints that remove DOF but do not by themselves name a joint: three of them together (e.g. concentric + coincident) realize a revolute. The Mechanical mates are *higher kinematic pairs* and coupling relations:
- Gear mate: couples two rotations with a ratio (a 1-DOF holonomic coupling), optionally with reverse.
- Rack-and-pinion: couples a rotation to a translation by pitch diameter (rotation to linear).
- Cam-follower: keeps a point/line/plane tangent to a series of tangent faces (a contact constraint that varies with configuration).
- Screw: couples rotation to translation by pitch.
- Slot, hinge, universal: convenience composites of lower pairs with limits.

**Limitations.** The Standard-mate philosophy means joint identity is *implicit*: the kernel/solver must infer "these three coincident/concentric mates equal a revolute" to do mobility analysis, which is harder than Onshape's explicit joints. Cam and gear mates are configuration-dependent (gear ratio is constant, but cam contact point moves), so they are not constant-Jacobian constraints.

**Kernel relevance.** Keel must support *both* idioms because users come from both traditions. The takeaway: the kernel should provide the geometric constraint primitives (coincident, concentric, tangent, distance, angle) and the coupling primitives (ratio coupling between two scalar joint variables, screw coupling, cam tangency). Gear/rack/screw are scalar algebraic couplings the kernel can express as a constraint between two joint coordinates; cam-follower needs a tangency constraint evaluated against actual B-rep faces, which ties to contact-geometry services.

### Source 1.3: kinematic pair recognition from B-rep (MCAD2Sim)

**Citation.** Thongnuch, S., and Fay, A. (2020). "MCAD2Sim: Towards Automatic Kinematic Joints Recognition." *Computer-Aided Design and Applications*, 17(1), 44-60. https://www.cad-journal.net/files/vol_17/CAD_17(1)_2020_44-60.pdf . Related: Wang et al., "Recognition of Kinematic Joints of 3D Assembly Models Based on Reciprocal Screw Theory."

**Content.** This work derives kinematic joints automatically from assembly B-rep contact, without any user-authored mate. It analyzes the *mating surfaces* between two parts and computes two descriptors: Independent Principal Vectors (IPV), the independent directions of the contacting geometry, and Intersection of Mating Geometries (IMG), the geometric type of the contact region. The count of IPVs and the IMG type together fix the rotational and translational DOF, and thus the joint:
- Cylinder-cylinder coaxial contact yields a revolute (if axially located) or cylindrical (if free to slide) pair.
- Plane-plane contact yields a planar or, when bounded, a prismatic/slider pair.
- Sphere-sphere contact yields a spherical pair.
- Full conformal contact yields a fastened pair.
The seven recognized categories match the Onshape joint set (fastened, revolute, planar, slider, cylindrical, parallel, ball, pin-slot). The reciprocal-screw variant computes the wrench system reciprocal to the contact twists to identify the residual freedoms.

**Limitations.** Recognition is geometry-driven and brittle for partial or non-conformal contact, fillets, and clearance gaps; it cannot disambiguate intended motion from incidental contact. It assumes clean mating faces.

**Kernel relevance.** This is squarely a kernel service: "given two solids in contact, return candidate joint axes and types." The kernel already has the faces, surface types, and adjacency. Exposing a `derive_joint(face_a, face_b)` that returns axis + joint kind from cylinder/plane/sphere pairing is high-value and uniquely kernel-positioned (only the kernel knows the exact surface geometry). The IPV/IMG and reciprocal-screw formulations are the algorithms to implement.

---

## Theme 2: Kinematic analysis foundations

### Source 2.1: Kramer, degrees-of-freedom analysis (foundational)

**Citation.** Kramer, G. A. (1992). *Solving Geometric Constraint Systems: A Case Study in Kinematics*. MIT Press (AI series). ISBN 9780262111645 (reissue 9780262515399). https://mitpress.mit.edu/9780262515399/solving-geometric-constraint-systems/ . Companion paper: Kramer, G. A. (1992). "Using Degrees of Freedom Analysis to Solve Geometric Constraint Systems." *ACM Symposium on Solid Modeling*. https://dl.acm.org/doi/10.1145/112515.112566 .

**Content.** Kramer replaces brute-force algebraic or numerical equation solving with symbolic reasoning about *where a geometric body can still move*. Each rigid body starts with its full set of DOF (a 3D body has 6: 3 translation, 3 rotation). Constraints are applied one at a time. For each constraint, the method reasons about the *locus* on which the remaining freedom keeps the entity, then incrementally places the body by intersecting loci. The architecture is a plan-and-execute system often summarized as: a plan generator picks an ordering of constraint actions that monotonically reduces DOF, and an action analyzer (the locus-intersection step) executes each placement in closed form (point on a sphere, on a circle, on a line, etc.). Because each step has a closed-form geometric meaning, the solver runs roughly an order of magnitude faster than iterative Newton on the full system and, crucially, it *understands* the remaining DOF, which is exactly what an assembly needs to allow dragging. The canonical worked domain is mechanical linkage assembly and simulation.

**Limitations.** Pure DOF/locus analysis handles *serial* placement well but struggles with *closed loops* where no single body can be placed by locus intersection alone; loops fall back to numerical solving. The action repertoire must be enumerated for each constraint pairing, and exotic constraints fall outside the canned actions. Overconstraint and redundancy need separate handling.

**Kernel relevance.** Kramer is the intellectual basis of D-Cubed DCM and SolveSpace and is the reference design for Keel's constraint layer. The relevant inheritance for the *motion* tier: DOF analysis is what tells the dragger which directions are free, and the plan/action structure gives constant-time placement during drag for the serial portions of an assembly. Keel should keep an explicit, queryable DOF count per body and per joint, computed by this style of analysis, with numerical loop closure layered on top.

### Source 2.2: Gruebler/Kutzbach mobility and its failures

**Citation.** Survey via Huang, Z., Li, Q., Ding, H. (2013) and "Applicability and generality of the modified Grübler-Kutzbach criterion," *Chinese Journal of Mechanical Engineering*, 26(2), 257. https://cjme.springeropen.com/articles/10.3901/CJME.2013.02.257 .

**Content.** The Kutzbach-Gruebler formula counts mobility (net DOF) of a mechanism: M = d(n - 1 - j) + sum(f_i), where d is 6 for spatial / 3 for planar, n is the number of links (including ground), j is the number of joints, and f_i is the DOF of joint i. It is a simple constraint-counting accountant. Its famous failure mode: it counts every joint constraint as independent, so it fails for *overconstrained but movable* mechanisms where constraints are redundant. The textbook counterexample is the planar parallelogram four-bar (and the Bennett and Sarrus linkages): naive counting predicts mobility 0 (rigid), yet the mechanism moves with 1 DOF because the parallel-link constraints are redundant in the special geometry. The modified criterion subtracts the number of redundant (passive) constraints and adds back any local/idle freedoms, typically computed via screw-system rank.

**Limitations.** The basic formula is geometry-blind: it sees topology, not the special alignments that create redundancy. The corrections require screw-theoretic rank analysis at a configuration, so mobility can be configuration-dependent (a mechanism can gain instantaneous freedom at a singular pose).

**Kernel relevance.** A naive DOF counter in Keel will report parallelogram and other overconstrained linkages as locked. The motion layer must therefore compute *effective* mobility from the rank of the constraint Jacobian (screw/twist system) at the current configuration, not from joint counting. This is a warning to design the DOF query as rank-based numerical analysis, with counting only as a fast pre-check.

### Source 2.3: screw theory for mobility (twists, wrenches, se(3))

**Citation.** Dai, J. S., Huang, Z., Lipkin, H. "A Unified Methodology for Mobility Analysis Based on Screw Theory," in *Advances in Robot Kinematics* (Springer, 2008), ch. 3. https://link.springer.com/chapter/10.1007/978-1-84800-147-3_3 . Foundational: Ball's screw theory; Lynch and Park, *Modern Robotics*.

**Content.** A *twist* is a 6-vector (angular + linear velocity) representing an instantaneous rigid-body motion; it is an element of the Lie algebra se(3) of the rigid-motion group SE(3). A *wrench* is the dual 6-vector (force + moment) representing a constraint or load. The freedoms a joint permits form a *twist system* (a subspace of se(3)); the constraints it imposes form the *reciprocal wrench system*. Mobility analysis becomes linear algebra on these subspaces: the system mobility equals 6 minus the rank of the combined constraint-wrench system, and redundant constraints show up as rank deficiency. Twist and wrench systems are reciprocal (their reciprocal product is zero), which gives a clean way to find exactly the constraints a joint applies. The screw (twist) representation also gives the constraint Jacobian rows directly.

**Limitations.** Screw systems are *instantaneous* (first-order): they describe the configuration at one pose and can mispredict finite mobility near singularities (the parallelogram at a flat pose gains an instantaneous freedom it does not keep). Second-order (curvature) analysis is needed for some degenerate mechanisms.

**Kernel relevance.** se(3)/twist algebra is the right internal representation for joints and DOF in Keel's motion tier: a joint stores its twist basis (its allowed motion subspace), the dragger projects requested motion onto the feasible twist space, and mobility comes from wrench-system rank. This unifies joint definition, DOF counting, drag projection, and singularity detection under one linear-algebra framework, and it interoperates cleanly with the maximal-coordinate constraint formulation used by physics engines.

---

## Theme 3: Kinematic loop solving

### Source 3.1: open chains versus closed loops (Modern Robotics ch. 7)

**Citation.** Lynch, K. M., and Park, F. C. (2017). *Modern Robotics: Mechanics, Planning, and Control*, Chapter 7 "Kinematics of Closed Chains." Cambridge University Press. https://www.cambridge.org/core/books/abs/modern-robotics/kinematics-of-closed-chains/719FCB2974C25DEF35489C2FB6C247B7 .

**Content.** An *open chain* (serial) has trivial forward kinematics: compose the joint transforms outward from the base, and the end pose is a closed-form product of exponentials. There are no constraints to satisfy. A *closed loop* (the four-bar, the Delta robot, the Stewart-Gough platform) imposes *loop-closure constraints*: going around the loop must return to identity, giving a system of nonlinear equations g(theta) = 0 in the joint variables. Forward kinematics of a closed chain (given actuated joints, find the pose) generally has no closed form and is solved numerically; inverse kinematics (given pose, find joints) is often the easy direction for parallel mechanisms. The Stewart platform's forward kinematics is the canonical hard case: given six leg lengths, find platform pose, which can have up to 40 solutions.

**Limitations.** Closed-chain solving is multi-valued (branch/assembly-mode ambiguity), and the correct branch must be tracked across motion. Singular configurations make the constraint Jacobian rank-deficient.

**Kernel relevance.** Keel's motion tier needs two code paths: cheap forward composition for the tree (serial) part of an assembly, and a numerical loop solver for the cycles. The DAG of occurrences plus joints forms a graph; the kernel/motion layer must find the spanning tree and treat the remaining joints as loop-closure constraints. Branch tracking (staying on the same assembly mode while dragging) is a continuation problem, see 3.3.

### Source 3.2: numerical loop solving and singularity handling

**Citation.** "Forward kinematics modeling of spatial parallel linkage mechanisms based on constraint equations and the numerical solving method," *Robotica* (Cambridge). https://www.cambridge.org/core/journals/robotica/article/abs/forward-kinematics-modeling-of-spatial-parallel-linkage-mechanisms-based-on-constraint-equations-and-the-numerical-solving-method/ . Geometric real-time approach: "A Geometric Approach for Real-Time Forward Kinematics of the General Stewart Platform," PMC9269243.

**Content.** Loop-closure equations g(q) = 0 are solved by Newton-Raphson: linearize to J dq = -g, solve, update, iterate. Convergence is fast (quadratic) but depends heavily on the initial guess, which in an interactive assembly is the *previous* solved configuration (warm start). Near singularities J becomes ill-conditioned; the standard fix is the damped least-squares / Levenberg-Marquardt step (J^T J + lambda I) dq = -J^T g, which stays numerically stable through rank deficiency at the cost of a small motion error. For tracking solutions through singular poses, pseudo-arc-length homotopy continuation parameterizes the path by arc length rather than by a coordinate, so the solver does not stall when a joint variable reverses.

**Limitations.** Damping introduces error and slows convergence; choosing lambda is heuristic. Continuation adds bookkeeping. Newton can jump branches if the step is too large.

**Kernel relevance.** If Keel offers any loop-aware motion service, it needs robust Newton with damped least-squares and warm starting from the last pose. This is the same numerical core as the static constraint solver, reused with time as the continuation parameter. Singularity detection (monitoring the smallest singular value of J) should be a first-class signal surfaced to the UI layer.

---

## Theme 4: Drag and interactive manipulation

### Source 4.1: D-Cubed 3D DCM kinematic dragging

**Citation.** Siemens Digital Industries Software. "D-Cubed 3D DCM" component page and release notes. https://plm.sw.siemens.com/en-US/plm-components/d-cubed/3d-dcm/ . Release detail: "D-Cubed 3D DCM Version 57.0," https://blogs.sw.siemens.com/plm-components/d-cubed-3d-dcm-version-57-0/ .

**Content.** The 3D DCM (Dimensional Constraint Manager) is the constraint solver embedded in most commercial CAD (it is the engine behind many systems' assembly mating). Its defining motion feature: it "solves dimensions and constraints as parts are being moved," interactively, with the mouse. This is *dragging*: the user grabs a part and the solver continuously re-satisfies all constraints while respecting the remaining DOF, at interactive frame rates. This lets a designer "study how the motion of a part is influenced by the various geometric rules in an assembly" and "explore physical assembly/disassembly processes." The DCM is descended directly from Kramer-style DOF analysis (Kramer was at D-Cubed), so dragging exploits the symbolic DOF understanding: the free directions are known, so the drag is a constrained projection plus a fast re-solve, not a from-scratch solve each frame.

**Limitations.** Documentation is product-level, not algorithmic. The DCM is proprietary and not available to an open kernel. Performance at interactive rates relies on warm starting and incremental re-solve that the public docs do not detail.

**Kernel relevance.** This is the behavior Keel's motion layer must replicate as an open component: given a dragged handle and a target, project onto the feasible motion space and re-solve constraints incrementally per frame. The DCM proves the architecture works and sets the performance bar (interactive, sub-frame). Keel's equivalent should warm-start from the previous frame and use the twist-space projection (Theme 2.3) so only the residual loop constraints need a Newton step.

### Source 4.2: D-Cubed Assembly Engineering Manager (AEM)

**Citation.** Siemens. "D-Cubed Assembly Engineering Manager." https://www.siemens.com/en-us/products/plm-components/d-cubed/assembly-engineering-manager/ .

**Content.** AEM is the higher-level component that sits above the 3D DCM and adds assembly-specific services: rigid-body collision detection during motion, contact constraints, and motion limits, so that dragging an assembly stops at interferences and respects joint stops. Where the DCM handles the constraint algebra, AEM adds the *physical plausibility* layer (parts collide and stop) and manages the assembly graph. It is the commercial reference for the "drag with collision and limits" behavior.

**Limitations.** Proprietary; public material is feature-level. AEM is a motion/assembly manager, explicitly *above* the geometry kernel, which is itself the architectural lesson.

**Kernel relevance.** AEM's existence as a *separate component above* the constraint solver, which is itself above the geometry kernel (Parasolid), is a direct model for Keel's layering: kernel (geometry + queries) -> constraint/DOF solver -> assembly motion manager (drag, limits, collision-stop). Keel should provide the queries AEM consumes (CCD, distance, joint limits as data) but the motion manager is a layer above the kernel, not inside it.

---

## Theme 5: Multibody dynamics boundary

The motion *kinematics* layer (does it move, where, without interference) is distinct from *dynamics* (forces, mass, acceleration, contact response). CAD "motion studies" cross into dynamics. The kernel must understand this boundary because it supplies the data both consume.

### Source 5.1: reduced versus maximal coordinates; Featherstone

**Citation.** Featherstone, R. (2008). *Rigid Body Dynamics Algorithms*. Springer. Algorithm overview: "Featherstone's algorithm," Wikipedia, https://en.wikipedia.org/wiki/Featherstone%27s_algorithm . Divide-and-conquer variant: Featherstone, R. (1999), *Int. J. Robotics Research*, 18(9-10).

**Content.** Two formulations dominate multibody dynamics. *Maximal (Cartesian) coordinates*: each of m bodies carries its full 6-DOF state (6m variables), and joints are enforced as explicit constraints solved each step; this is what game/physics engines (Bullet, PhysX) historically use, simple but with constraint drift and stiffness issues. *Reduced (generalized) coordinates*: the system is parameterized by the actual joint variables only, so constraints are satisfied by construction and there is no drift. The Featherstone Articulated-Body Algorithm (ABA) computes forward dynamics for a reduced-coordinate tree in O(n) by recursively propagating articulated inertias and bias forces inward then accelerations outward. Loops break the pure tree, so loop joints are reintroduced as a small set of explicit constraints (the hybrid approach). Constraint drift in maximal coordinates is controlled with Baumgarte stabilization or projection.

**Limitations.** Reduced coordinates need a tree topology and special handling for loops; maximal coordinates need stabilization and are stiffer. Neither is a kernel concern directly; both consume the same geometric/inertial data.

**Kernel relevance.** Keel does not implement dynamics, but the choice of coordinate formulation in the layer above dictates what the kernel must hand over. Both formulations need: per-body mass, center of mass, and full inertia tensor (the kernel's mass-properties query), plus joint definitions (axis, type) and contact geometry. Keel should provide mass properties about an arbitrary frame (not just the centroid) and the principal-axis inertia, since reduced-coordinate solvers want inertia in a body frame and maximal solvers want it at the CoM.

### Source 5.2: MuJoCo computational model (reference joint and data contract)

**Citation.** Todorov, E., Erez, T., Tassa, Y. (2012). "MuJoCo: A Physics Engine for Model-Based Control." *IEEE/RSJ IROS*. Docs: https://mujoco.readthedocs.io/en/stable/computation/index.html and https://mujoco.readthedocs.io/en/stable/overview.html .

**Content.** MuJoCo uses *reduced (generalized) coordinates*. Joint types are minimal: hinge (1 rotational DOF), slide (1 translational), ball (3 rotational, quaternion), and free (6 DOF, floating). Position coordinates q can exceed velocity coordinates v because quaternions use 4 numbers for 3 DOF; velocities live in the tangent space (se(3) again). The equation of motion is M(q) v_dot + c(q,v) = tau + J^T f, with M the joint-space inertia (built by Composite Rigid Body), c the bias forces (built by Recursive Newton-Euler), J the constraint Jacobian. Crucially for CAD: *bodies carry mass and inertia but no geometry*; geometry lives in massless *geoms* (sphere, capsule, box, mesh, plane) used only for collision and contact. Closed loops and welds are added as *equality constraints* (connect = ball between two bodies, weld = full 6-DOF lock, joint = scalar polynomial coupling for gears), evaluated as extra Jacobian rows, so loops do not require restructuring the tree. Contact is soft (convex optimization), allowing slight penetration for stability.

**Limitations.** MuJoCo's soft contact trades physical exactness for solver robustness; its collision geoms are coarse primitives, not B-rep. It is a control/RL engine, not an engineering-accuracy multibody tool.

**Kernel relevance.** MuJoCo cleanly separates *inertial body* from *collision geom* from *joint*, which is the exact data contract Keel should target: the kernel supplies (a) mass + inertia per body, (b) collision geometry (the B-rep, or a derived mesh/convex hull), (c) joint axes and types. The equality-constraint mechanism for gears (scalar polynomial coupling) is how Keel's gear/screw/rack mates should be expressed for any downstream dynamics engine. The joint enum (hinge/slide/ball/free) is the dynamics-side minimal set the CAD joint set must lower to.

### Source 5.3: Drake MultibodyPlant (frames, mobilizers, spatial inertia)

**Citation.** Drake (Toyota Research Institute / MIT). "MultibodyPlant," "Joint," "RigidBody" class references. https://drake.mit.edu/doxygen_cxx/classdrake_1_1multibody_1_1_multibody_plant.html and pydrake.multibody.tree, https://drake.mit.edu/pydrake/pydrake.multibody.tree.html .

**Content.** Drake formalizes the data contract precisely. A `RigidBody` holds a `SpatialInertia` (mass, center-of-mass offset, rotational inertia, all as one 6x6-structured object). A body's allowed motion relative to its parent is set by a *Mobilizer* (tree joint): RevoluteJoint, PrismaticJoint, BallRpyJoint/QuaternionFloating, ScrewJoint, UniversalJoint, PlanarJoint, WeldJoint, each granting 0-6 DOF. Extra constraints beyond the tree (loop closure) are `Constraint` objects that remove further DOF. The `MultibodyPlant` "is responsible for defining the dynamics and kinematics... mass properties, joint types, joint limits, and contact models." This is the most complete open enumeration of the joint set a CAD assembly must map onto, and it includes ScrewJoint (matching the CAD screw mate) and PlanarJoint.

**Limitations.** Drake is research-grade and heavyweight; its API is the contract, not a kernel. SpatialInertia must be supplied by the modeler, i.e. by the kernel's mass-properties computation.

**Kernel relevance.** Drake's `SpatialInertia` is the precise object Keel's mass-properties query should produce: mass, CoM, and rotational inertia in one structure, expressed about a named frame. Drake's joint enum (revolute, prismatic, ball, screw, universal, planar, weld) plus loop `Constraint` objects is a validated lowering target for Keel's mate/joint set. Keel's contract: emit a body per occurrence with SpatialInertia, a joint per mate, and a constraint per loop closure.

### Source 5.4: Bullet joints (maximal-coordinate reference)

**Citation.** Coumans, E., et al. Bullet Physics SDK, btTypedConstraint and btMultiBody documentation. https://pybullet.org/ and Bullet User Manual.

**Content.** Bullet historically uses *maximal coordinates* with constraints: each rigid body has full 6-DOF state, and joints are `btTypedConstraint` subclasses (point2point = spherical, hinge = revolute, slider = prismatic, generic 6-DOF, gear) solved by a sequential-impulse / projected Gauss-Seidel iterative solver with Baumgarte error reduction. Bullet later added `btMultiBody`, a Featherstone reduced-coordinate articulation for drift-free chains. Bodies need a collision shape (primitives or convex hull / triangle mesh) and a mass + local inertia diagonal.

**Limitations.** Maximal-coordinate constraints drift and need ERP/CFM tuning; iterative solver is fast but inexact. Collision shapes are meshes/primitives, never B-rep.

**Kernel relevance.** Bullet shows the *other* coordinate choice and confirms the data contract is the same: mass, diagonal inertia (so Keel should provide principal-axis inertia, which diagonalizes), collision shape (derived mesh/convex hull from B-rep). The generic-6-DOF and gear constraints again map to CAD mates. Confirms Keel should ship a "to convex hull" and "to triangle mesh" tessellation service for any physics consumer.

### Source 5.5: clearance-joint dynamics (tolerance effect on motion)

**Citation.** Several; representative: "Influence of two kinds of clearance joints on the dynamics of planar mechanical system based on a modified contact force model," *Scientific Reports* 13 (2023), 20264. https://www.nature.com/articles/s41598-023-47315-1 . Foundational model: Lankarani, H. M., Nikravesh, P. E. (1990), contact force model.

**Content.** Real joints have clearance (gap between pin and bore) from manufacturing tolerance, assembly need, and wear. An *ideal* kinematic joint removes DOF exactly; a *clearance* joint instead lets the pin float within the bore and contact intermittently, so the constraint becomes a *contact force* (normal via Lankarani-Nikravesh Hertzian-with-damping, tangential via LuGre friction) rather than a hard constraint. This produces measurable departures from ideal motion: vibration, position error, and impact loads, growing with clearance size. Equations of motion become strongly nonlinear and are integrated with Baumgarte stabilization.

**Limitations.** This is firmly a *dynamics* phenomenon requiring a force model and time integration; it cannot be captured by pure kinematic constraint solving. Highly parameter-sensitive.

**Kernel relevance.** Mostly informational for the boundary: clearance kinematics is above the kernel (it needs a dynamics solver and tolerance/GD&T data). The kernel's contribution is the *nominal geometry plus the tolerance zone* (ties to the GD&T research file): the kernel can report the clearance gap from nominal pin/bore radii and tolerance, which the motion layer turns into a contact model. Keel should expose nominal-versus-toleranced dimensions so a downstream solver can model clearance, but should not itself model clearance dynamics.

---

## Theme 6: Motion envelopes, swept volumes, and continuous collision

### Source 6.1: mechanism workspace and swept-volume computation

**Citation.** Sprecher, A., et al. "Voxel-Based Motion Bounding and Workspace Estimation for Robotic Manipulators," *IEEE ICRA* 2012. https://www.cs.cmu.edu/~reids/papers/SprecherICRA12.pdf . Plus analytical workspace methods (Gröbner/resultant boundary surfaces) surveyed in *Robotica*.

**Content.** A mechanism's *workspace* (reachable set of its moving point) and its *swept volume* (region occupied by its links over a motion) are computed two ways. Discretized: sweep point-cloud or mesh representations of the links through their joint ranges, voxelize the union, and extract a bounding volume; building the workspace incrementally link by link. Analytical: eliminate joint variables from the forward-kinematics equations using resultants or Gröbner bases to get implicit boundary surfaces of the workspace; this is exact and 10-100x faster than discretization but only tractable for simpler mechanisms. The swept volume of a mechanism is the time-parameterized union of each rigid link's swept volume, where each link's sweep is the existing swept-volume problem with a *time-varying* rigid transform supplied by the joint motion.

**Limitations.** Voxel methods trade accuracy for generality and produce approximate boundaries; analytical methods do not scale to general spatial linkages. Both need the joint motion as input, i.e. the kinematics must be solved first.

**Kernel relevance.** This is the precise tie to Keel's existing swept-volume research: *motion adds a time-parameterized rigid transform on top of static sweep.* The kernel already computes a solid's swept volume along a path; mechanism sweep is that service called per link with the transform trajectory the motion layer produces. So the kernel obligation is "sweep this solid along this time-varying rigid transform"; assembling per-link sweeps into a mechanism envelope and feeding the transforms is the motion layer's job. Keel should make swept volume accept a sampled or analytic SE(3) trajectory, not just a curve.

### Source 6.2: continuous collision detection (conservative advancement, C2A)

**Citation.** Tang, M., Kim, Y. J., Manocha, D. (2009). "C2A: Controlled Conservative Advancement for Continuous Collision Detection of Polygonal Models." *IEEE ICRA*. https://graphics.ewha.ac.kr/C2A/C2A.pdf . Foundational: Redon, S., Kheddar, A., Coquillart, S. (2002). "Fast Continuous Collision Detection between Rigid Bodies." *Eurographics*. Extension: Zhang, X., Kim, Y. J. et al., articulated CCD.

**Content.** *Discrete* collision detection checks for overlap at sampled times and misses thin/fast collisions (tunneling). *Continuous* collision detection (CCD) finds the first time of contact (TOC) within a time step for bodies under continuous rigid motion. *Conservative advancement* (CA) computes a lower bound on the time the bodies can safely advance before they could possibly touch: it bounds the maximum projected motion of the bodies along the closest-approach direction over the interval, advances by that safe amount, recomputes the closest distance, and repeats until distance hits a tolerance, yielding the TOC. C2A (Controlled CA) extends CA from convex polytopes to general non-convex polygonal models using a swept-sphere bounding-volume hierarchy and a controlled scheme that picks the BVH nodes to advance, computing motion bounds tightly; it runs CCD in a few milliseconds on models of tens of thousands of triangles. Redon et al. (2002) gave the rigid-body interval-arithmetic formulation; Zhang/Kim extended CA to articulated bodies with Taylor-expansion motion bounds for long chains.

**Limitations.** CA needs a distance/closest-feature query and bounded motion; tight motion bounds for general rotation are the hard part. Works on meshes/convex hulls, not directly on B-rep. TOC is for one step; tracking through a long motion repeats it.

**Kernel relevance.** This is the algorithm behind "interference along the motion path" and "drag stops at collision." The kernel obligation is the underlying *distance query* (closest distance and closest features between two solids) and a BVH over tessellated B-rep; given those plus a motion bound, CA/C2A runs above. Keel should provide: minimum-distance query between two bodies, a swept-sphere or AABB BVH, and a "maximum projected motion over this SE(3) interval" helper. The CCD loop itself (advance, re-query, repeat) is motion-layer code, but it is useless without the kernel's distance and BVH services, so those must be designed as fast, CCD-ready primitives.

---

## Theme 7: Open assembly solvers as reference implementations

### Source 7.1: FreeCAD built-in Assembly + OndselSolver

**Citation.** Ondsel. "Ondsel added integrated assembly to the FreeCAD core." Ondsel Blog (2024). https://www.ondsel.com/blog/assembly-workbench-preview/ . FreeCAD docs: "Assembly Objects and Joints," https://deepwiki.com/FreeCAD/FreeCAD/3.7.1-assembly-objects-and-joints .

**Content.** FreeCAD 1.0's built-in Assembly workbench uses the open-source OndselSolver, built on "MbD" (a multibody dynamics library). Ondsel hired Dr. Aik-Siong Koh, who had decades of experience writing kinematic and multibody-dynamics solvers; his existing solver, originally in Smalltalk, was ported to C++ and released under LGPL 2.1+. The key architectural point: FreeCAD chose a *multibody-dynamics-based* solver for assembly, not a pure geometric-constraint solver. The workbench is *joint-based* (explicit joints between parts, like Onshape and robotics), referencing geometry via App::Link. This is notable because the prior FreeCAD ecosystem had three competing approaches: Assembly2/A2plus (constraint-based), and Assembly3 by "realthunder," which wrapped the SolveSpace solver via a Python binding.

**Limitations.** OndselSolver is young; the MbD lineage means it leans toward dynamics formulation, which may be heavier than needed for pure kinematic positioning. Public material is blog-level on the solver internals.

**Kernel relevance.** Strong validation of Keel's intended split and the joint-based front-end. It also shows a real, LGPL, C++ open solver exists (OndselSolver) that Keel's motion layer could interoperate with or learn from. The lesson: the assembly solver is a *separate library* layered on the geometry kernel, and a multibody/dynamics formulation can double as the kinematic positioner. For Keel (Rust), this argues for a clean FFI-able joint+inertia data contract so such a solver can be bound.

### Source 7.2: SolveSpace constraint and mechanism solver

**Citation.** SolveSpace. "Technology," https://solvespace.com/tech.pl ; DeepWiki overview, https://deepwiki.com/solvespace/solvespace . Author: Jonathan Westhues.

**Content.** SolveSpace is an open (GPLv3) parametric 2D/3D CAD with a 3D constraint solver that doubles as a mechanism solver. Constraints are symbolic equations; the system is solved numerically by a *modified Newton's method*: each nonlinear constraint is linearized about the current estimate, the linear system is solved, and the estimate is improved iteratively to required accuracy. It tracks DOF explicitly and nudges the user toward zero remaining DOF per group, but leaving DOF free is exactly how it models mechanisms (drag the under-constrained part to see it move). Its solver is small and embeddable and has been reused (the SolveSpace solver library `slvs` is bound from Python and was used in FreeCAD's Assembly3). It handles 3D constraints and demonstrates real mechanism motion (linkages, suspensions).

**Limitations.** Pure numerical Newton with no DOF/locus pre-analysis can be slower and less robust on large systems than the Kramer/D-Cubed approach; redundant constraints need explicit detection. Not Parasolid-class on geometry.

**Kernel relevance.** SolveSpace is the closest existing *open* analog to Keel's constraint+mechanism layer and proves a small embeddable numerical solver suffices for interactive 3D mechanism dragging. Its design (symbolic equations, modified Newton, explicit DOF tracking, under-constraint = motion) is a viable baseline for Keel's motion tier. The `slvs` library is a concrete reference for the constraint-solver API surface a kernel should pair with.

---

## Motion tier boundary for Keel

The recurring lesson across D-Cubed (DCM under AEM under Parasolid), FreeCAD (OndselSolver above the geometry core), and the robotics engines (MuJoCo/Drake/Bullet consume a fixed data contract) is a three-layer split. Keel is the bottom layer. It must be an excellent *data and query provider* and must not try to be the motion solver.

### Kernel obligations (what Keel must provide)

1. **Mass properties as a first-class query.** Mass, center of mass, and full rotational inertia tensor, computable about an arbitrary named frame and in principal axes (diagonalized). This is the single most-consumed datum: MuJoCo bodies, Drake `SpatialInertia`, and Bullet local inertia all need it. Provide it for single solids and for composed occurrences.

2. **Joint-axis and joint-type extraction from B-rep.** Given two faces or two solids in contact, return candidate joint axis/frame and kind (revolute, prismatic, cylindrical, planar, spherical, fastened), using the IPV/IMG or reciprocal-screw analysis (MCAD2Sim, source 1.3). Only the kernel knows the exact surface geometry, so this service is uniquely kernel-positioned. Also expose mate-connector frames anchored to geometry features that survive regeneration.

3. **Swept volume under a time-parameterized rigid transform.** Extend the existing static swept-volume service to accept a sampled or analytic SE(3) trajectory, so the motion layer can build mechanism motion envelopes by sweeping each link along its joint trajectory (source 6.1).

4. **CCD-ready geometric queries.** Minimum-distance and closest-feature queries between two bodies, a BVH (swept-sphere or AABB) over tessellated B-rep, and a "maximum projected motion over an SE(3) interval" bound, so a conservative-advancement CCD loop (Redon/C2A, source 6.2) can run in the layer above for interference-along-path and drag-stop-at-collision.

5. **Tessellation/convex-decomposition export.** Triangle mesh and convex hull / convex decomposition of any solid, since every downstream physics engine (Bullet, MuJoCo, Drake) collides on meshes/primitives, not B-rep.

6. **Nominal-plus-tolerance dimensions.** Expose toleranced versus nominal radii/dimensions (ties to the GD&T file) so a downstream solver can model clearance-induced motion (source 5.5); the kernel reports the gap, it does not simulate the contact.

7. **DOF/twist algebra primitives.** A queryable, rank-based effective-mobility computation per body and per joint, using the twist/wrench (se(3)) representation, so overconstrained-but-movable mechanisms (parallelogram, source 2.2/2.3) are reported correctly and the dragger can project requested motion onto the feasible twist space.

### What stays above the kernel

- **The constraint/DOF solver** (Kramer-style plan/action plus numerical loop closure with damped least-squares and warm starting). This is a separate library (the DCM/SolveSpace/OndselSolver analog). It consumes the kernel's joint frames and DOF primitives.
- **Interactive drag management** (warm-started incremental re-solve, twist-space projection, branch/assembly-mode tracking through singularities, joint limits, collision-stop). This is the AEM analog.
- **Multibody dynamics** entirely: reduced or maximal coordinate formulation, Featherstone ABA, contact-force models (Lankarani-Nikravesh), clearance dynamics, time integration. Keel never integrates equations of motion; it hands over inertia, joints, and collision geometry and lets an Adams-class or MuJoCo/Drake-class solver do the physics.
- **Higher-pair coupling semantics** (gear ratio, rack-and-pinion, cam profile following). The kernel supplies the contact faces and the scalar coupling can be expressed as a constraint between two joint coordinates (MuJoCo equality / Drake constraint), but enforcing the coupling over time is the motion layer's job. Cam-follower tangency is the one case that reaches back into the kernel each frame for an updated contact point.

### One-line contract

Keel emits, per assembly: one rigid body (with SpatialInertia) per occurrence, one joint (axis + type + limits, derived from or authored on B-rep faces) per mate, one constraint per loop closure or coupling, plus on-demand distance/BVH/swept-volume/tessellation queries. Everything that turns that contract into motion lives above the kernel.

---

## References

1. Onshape (PTC). "Kinematics: Assemblies, Mates, and Simulation." Onshape Blog. https://www.onshape.com/en/blog/kinematics-assemblies-mates-simulation
2. Onshape. "Mates." Onshape Help. https://cad.onshape.com/help/Content/Assembly/mates.htm
3. Dassault Systemes SolidWorks. "Mechanical Mates." SOLIDWORKS Help 2025. https://help.solidworks.com/2025/English/SolidWorks/sldworks/c_Mechanical_Mates.htm
4. Thongnuch, S., Fay, A. (2020). "MCAD2Sim: Towards Automatic Kinematic Joints Recognition." Computer-Aided Design and Applications 17(1), 44-60. https://www.cad-journal.net/files/vol_17/CAD_17(1)_2020_44-60.pdf
5. Kramer, G. A. (1992). Solving Geometric Constraint Systems: A Case Study in Kinematics. MIT Press. https://mitpress.mit.edu/9780262515399/solving-geometric-constraint-systems/
6. Kramer, G. A. (1992). "Using Degrees of Freedom Analysis to Solve Geometric Constraint Systems." ACM Symposium on Solid Modeling. https://dl.acm.org/doi/10.1145/112515.112566
7. "Applicability and generality of the modified Grübler-Kutzbach criterion." Chinese Journal of Mechanical Engineering 26(2), 257 (2013). https://cjme.springeropen.com/articles/10.3901/CJME.2013.02.257
8. Dai, J. S., Huang, Z., Lipkin, H. "A Unified Methodology for Mobility Analysis Based on Screw Theory." Advances in Robot Kinematics (Springer, 2008), ch. 3. https://link.springer.com/chapter/10.1007/978-1-84800-147-3_3
9. Lynch, K. M., Park, F. C. (2017). Modern Robotics, ch. 7 "Kinematics of Closed Chains." Cambridge University Press. https://www.cambridge.org/core/books/abs/modern-robotics/kinematics-of-closed-chains/719FCB2974C25DEF35489C2FB6C247B7
10. "Forward kinematics modeling of spatial parallel linkage mechanisms based on constraint equations and the numerical solving method." Robotica (Cambridge). https://www.cambridge.org/core/journals/robotica/article/abs/forward-kinematics-modeling-of-spatial-parallel-linkage-mechanisms-based-on-constraint-equations-and-the-numerical-solving-method/CF65BBA18E17525B0645CCAFFC70A20F
11. "A Geometric Approach for Real-Time Forward Kinematics of the General Stewart Platform." PMC9269243. https://pmc.ncbi.nlm.nih.gov/articles/PMC9269243/
12. Siemens. "D-Cubed 3D DCM." https://plm.sw.siemens.com/en-US/plm-components/d-cubed/3d-dcm/
13. Siemens. "D-Cubed Assembly Engineering Manager." https://www.siemens.com/en-us/products/plm-components/d-cubed/assembly-engineering-manager/
14. Featherstone, R. (2008). Rigid Body Dynamics Algorithms. Springer. (Overview: https://en.wikipedia.org/wiki/Featherstone%27s_algorithm )
15. Todorov, E., Erez, T., Tassa, Y. (2012). "MuJoCo: A Physics Engine for Model-Based Control." IEEE/RSJ IROS. Docs: https://mujoco.readthedocs.io/en/stable/computation/index.html
16. Drake (TRI/MIT). MultibodyPlant / Joint / RigidBody references. https://drake.mit.edu/doxygen_cxx/classdrake_1_1multibody_1_1_multibody_plant.html
17. Coumans, E., et al. Bullet Physics SDK (btTypedConstraint, btMultiBody). https://pybullet.org/
18. "Influence of two kinds of clearance joints on the dynamics of planar mechanical system based on a modified contact force model." Scientific Reports 13, 20264 (2023). https://www.nature.com/articles/s41598-023-47315-1
19. Sprecher, A., et al. (2012). "Voxel-Based Motion Bounding and Workspace Estimation for Robotic Manipulators." IEEE ICRA. https://www.cs.cmu.edu/~reids/papers/SprecherICRA12.pdf
20. Tang, M., Kim, Y. J., Manocha, D. (2009). "C2A: Controlled Conservative Advancement for Continuous Collision Detection of Polygonal Models." IEEE ICRA. https://graphics.ewha.ac.kr/C2A/C2A.pdf
21. Redon, S., Kheddar, A., Coquillart, S. (2002). "Fast Continuous Collision Detection between Rigid Bodies." Eurographics / Computer Graphics Forum.
22. Ondsel. "Ondsel added integrated assembly to the FreeCAD core." Ondsel Blog (2024). https://www.ondsel.com/blog/assembly-workbench-preview/
23. FreeCAD. "Assembly Objects and Joints." https://deepwiki.com/FreeCAD/FreeCAD/3.7.1-assembly-objects-and-joints
24. SolveSpace. "Technology." https://solvespace.com/tech.pl
