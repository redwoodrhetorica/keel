# 38. STEP AP242 Import: Entity-by-Entity Schema-to-Kernel Mapping

## Title and Scope

This dossier is the implementation-grade, entity-level mapping from STEP (ISO 10303) / EXPRESS entities to Keel's internal B-rep model, plus the parsing and robustness engineering needed to make import work on real foreign data. It is the one remaining literature gap on Keel's shippable path (flagged by `docs/research/03-completeness-audit.md`), and it is the concrete companion to material already in the corpus: STEP/Part 21 format anatomy in `kernel/14-determinism-serialization`, the import-healing pipeline in `kernel/13-import-healing-extension`, semantic PMI in `kernel/17-gdt-pmi-tolerances`, dirty-geometry handling in `kernel/29` and `kernel/30`, projection in `nurbs/04`, and AP203/214/242 geometry representations in `nurbs/06`. This file does NOT re-derive those. It delivers the entity-to-Keel mapping table as its centerpiece, the NURBS conversion specifics, the units/tolerance handling, the pcurve-reconstruction policy, the orientation/seam/degeneracy rules, validation-property round-trips, the heal-on-import integration, tessellation/PMI import, and an honest parser reuse-vs-build verdict for a pure-Rust kernel.

Keel's target model (recap, for mapping reference):
- Topology: `Body` / `Region` / `Shell` / `Face` / `Loop` / `Coedge` / `Edge` / `Vertex` on generational-handle arenas.
- Surface geometry enum: `{Plane, Cylinder, Cone, Sphere, Torus, NurbsSurface}`.
- Curve geometry enum: `{Line, Circle, Ellipse, NurbsCurve}`.
- NURBS stored in homogeneous 4D control points.
- A pcurve is MANDATORY on every coedge.
- Tolerances are per-entity, with a session resolution floor.

---

## 1. STEP Structure Essentials (only what import needs)

### Part 21 clear-text file (ISO 10303-21)

A STEP file begins with `ISO-10303-21;` and ends with `END-ISO-10303-21;`. It has two mandatory sections:

- `HEADER;` ... `ENDSEC;` carrying three fixed groups: `FILE_DESCRIPTION` (description text + implementation level), `FILE_NAME` (name, ISO-8601 timestamp, author, organization, preprocessor/originating system, authorization), and `FILE_SCHEMA` (the EXPRESS schema name(s) governing the data, e.g. `('AUTOMOTIVE_DESIGN { 1 0 10303 214 ... }')` for AP214 or `('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING {...}')` for AP242). Edition 3 (2016) adds optional `FILE_POPULATION`, `SECTION_LANGUAGE`, `SECTION_CONTEXT`, plus anchor / reference / signature sections for external references and digital signatures.
- `DATA;` ... `ENDSEC;` carrying entity instances.

Each instance is `#N=ENTITY_NAME(attr1, attr2, ...);` where `#N` is a unique positive integer id (local to the file, typically `< 2^63`). Attributes are: reals, integers, quoted `'strings'`, `.ENUM.` tokens, `#M` references to other instances, parenthesized `(...)` lists, `$` for an unset OPTIONAL value, and `*` for a derived/redeclared value. Example: `#16=ADVANCED_FACE('',(#42),#7,.T.);`.

**Forward references are legal and pervasive**: `#N` may reference an instance defined either before or after it in the file. This forces a two-pass parser (tokenize/record all instances, then resolve references), the single most important structural fact for import engineering.

**Complex (AND) instances** combine several leaf entity records for one id. In external mapping (the common case for the rational + b_spline combination) the syntax is one id mapping to a parenthesized concatenation of records in alphabetical leaf order:
```
#101=(BOUNDED_SURFACE() B_SPLINE_SURFACE(3,3,((#1,#2,...),...),.UNSPECIFIED.,.F.,.F.,.F.)
  B_SPLINE_SURFACE_WITH_KNOTS((4,1,4),(4,1,4),(0.,0.5,1.),(0.,0.5,1.),.UNSPECIFIED.)
  GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_SURFACE(((1.,0.7,1.),...))
  REPRESENTATION_ITEM('') SURFACE());
```
A parser must reassemble all leaf records sharing the id into a single logical entity. This is how STEP delivers a rational NURBS surface: `b_spline_surface_with_knots` carries the polynomial part, `rational_b_spline_surface` carries the weight grid, glued by the complex instance.

### EXPRESS (ISO 10303-11), the minimum to read an AP

EXPRESS is the schema language. Import only needs the read-level subset: `ENTITY name; attr : TYPE; ... END_ENTITY;`, `SUBTYPE OF`/`SUPERTYPE`, `OPTIONAL`, aggregates `LIST[lo:hi] OF`, `SET[lo:hi] OF`, `ARRAY`, `SELECT` types (a union of allowed entity types), `ENUMERATION OF`, and `TYPE name = ...;` defined data types. Inherited attributes appear in the Part 21 record in supertype-then-subtype order, flattened. Two practical consequences: (1) the on-disk attribute order of an instance equals the flattened EXPRESS attribute order, so a generated reader can positionally bind; (2) `SELECT` types mean an attribute slot can hold one of several entity kinds (e.g. a `trim_1` slot holds EITHER a `cartesian_point` OR a `parameter_value`), which the parser must dispatch on at runtime.

### Newer encodings (note only)

Part 21 edition 3 (2016) is still clear-text and is what most current AP242 files use. The JSON/Part 28 lineage (XML in Part 28 ed1, later JSON-oriented work) exists but is rare in mechanical CAD exchange. Keel's importer should target Part 21 clear-text (all editions) and may ignore XML/JSON encodings initially.

---

## 2. The Application Protocols and why AP242 is the target

- **AP203** (Configuration controlled 3D design, ISO 10303-203): the classic mechanical-CAD solid exchange AP. Geometry + topology + minimal config management. Schema name `CONFIG_CONTROL_DESIGN`.
- **AP214** (Core data for automotive mechanical design processes, ISO 10303-214): superset of AP203 geometry with colors, layers, assemblies, GD&T presentation. Schema name `AUTOMOTIVE_DESIGN`. The most common STEP flavor exported by mainstream CAD for two decades.
- **AP242** (Managed model-based 3D engineering, ISO 10303-242): the merger and successor of AP203 ed2 + AP214 ed3, adding **semantic PMI** (machine-readable GD&T), **tessellation** (ed2+), **kinematics**, composites, and (ed2) electrical. Schema name `AP242_MANAGED_MODEL_BASED_3D_ENGINEERING`.

**Edition history**: ed1 (2014) provides all AP203 ed2 + AP214 ed3 functionality and adds ~533 new EXPRESS entities; ed2 (2020) extends to electrical and adds richer tessellation; ed3 (2022) is corrective maintenance; ed4 is in development. The geometry/topology core (Part 42 entities) is essentially STABLE and IDENTICAL across AP203/214/242: an `advanced_brep_shape_representation` looks the same in all three. **The practical reality: Keel's importer must accept AP203, AP214, and AP242 files** and treat them through one geometry/topology mapping, branching only for the AP242-specific tessellation and semantic-PMI layers. Detect the AP from `FILE_SCHEMA`, but drive the B-rep mapping off entity names, which are shared.

---

## 3. The Part 42 Geometry-and-Topology Entity Model (as it appears in files)

These are the entities a B-rep importer actually encounters. Attribute lists below are the load-bearing fields (verified against ISO 10303-42 / the STEP Merged AP Library and the b_spline_geometry module).

### Topology entities

- `advanced_brep_shape_representation`: a `representation` whose `items` include the `manifold_solid_brep`(s) and an `axis2_placement_3d`, in a `geometric_representation_context`. This is the top-level container for a solid.
- `manifold_solid_brep(name, outer: closed_shell)`: one solid bounded by one outer closed shell.
- `brep_with_voids(name, outer: closed_shell, voids: SET OF oriented_closed_shell)`: a solid with internal cavities (voids), each an inward-oriented closed shell.
- `manifold_surface_shape_representation`: container for open/sheet bodies.
- `shell_based_surface_model(name, sbsm_boundary: SET OF shell)`: a sheet/surface body made of open or closed shells.
- `closed_shell(name, cfs_faces: SET OF face)`: a watertight shell (volume boundary).
- `open_shell(name, cfs_faces: SET OF face)`: a non-closed shell (sheet).
- `oriented_closed_shell(name, closed_shell_element, orientation: BOOLEAN)`: a shell reused with a sense flag (used for voids).
- `advanced_face(name, bounds: SET OF face_bound, face_geometry: surface, same_sense: BOOLEAN)`: the key face entity. `same_sense` tells whether the face normal agrees with the underlying surface normal.
- `face_bound(name, bound: loop, orientation: BOOLEAN)`: one boundary loop of a face; `orientation` flips the loop sense relative to the face.
- `face_outer_bound`: a `face_bound` subtype flagged as the outer boundary.
- `edge_loop(name, edge_list: LIST OF oriented_edge)`: an ordered loop of oriented edges (a wire).
- `oriented_edge(name, edge_start, edge_end, edge_element: edge, orientation: BOOLEAN)`: an edge reused in a loop with a direction flag. (`edge_start`/`edge_end` are usually `*` derived from `edge_element`.)
- `edge_curve(name, edge_start: vertex, edge_end: vertex, edge_geometry: curve, same_sense: BOOLEAN)`: the geometric edge. Two bounding vertices, a 3D curve, and a `same_sense` flag relating curve direction to the start to end direction.
- `vertex_point(name, vertex_geometry: point)`: a topological vertex carrying a `cartesian_point`.
- `vertex_loop(name, loop_vertex: vertex)`: a degenerate loop that is a single isolated vertex (e.g. the apex pole of a cone/sphere face boundary).
- `poly_loop(name, polygon: LIST OF cartesian_point)`: a polygonal loop (faceted-brep legacy).

### Surface entities

- `plane(name, position: axis2_placement_3d)`.
- `cylindrical_surface(name, position: axis2_placement_3d, radius: length)`.
- `conical_surface(name, position, radius, semi_angle)`. `radius` is the radius at the placement origin plane; `semi_angle` is the half-cone angle.
- `spherical_surface(name, position, radius)`.
- `toroidal_surface(name, position, major_radius, minor_radius)`.
- `degenerate_toroidal_surface(name, position, major_radius, minor_radius, select_outer: BOOLEAN)`: a self-intersecting "apple/lemon" torus where `minor_radius >= major_radius`; `select_outer` picks the outer or inner part.
- `b_spline_surface_with_knots(...)`: see section 5.
- `rational_b_spline_surface(weights_data: LIST OF LIST OF REAL)`: supplies the weight grid; appears only as part of a complex instance with `b_spline_surface_with_knots`.
- `surface_of_revolution(name, swept_curve, axis_position: axis1_placement)`.
- `surface_of_linear_extrusion(name, swept_curve, extrusion_axis: vector)`.
- `offset_surface(name, basis_surface, distance, self_intersect)`.
- `rectangular_trimmed_surface(name, basis_surface, u1, u2, v1, v2, usense, vsense)`.
- `curve_bounded_surface(name, basis_surface, boundaries: SET OF boundary_curve, implicit_outer)`.
- `bounded_surface`: abstract supertype flag entity, appears as `BOUNDED_SURFACE()` in the b_spline complex instance.

### Curve entities

- `line(name, pnt: cartesian_point, dir: vector)`. The vector's magnitude is the parameterization scale.
- `circle(name, position: axis2_placement (2d or 3d), radius)`.
- `ellipse(name, position, semi_axis_1, semi_axis_2)`.
- `parabola(name, position, focal_dist)`, `hyperbola(name, position, semi_axis, imaginary_axis)`: rare; Keel has no native conic-section-beyond-ellipse variant, convert to NURBS.
- `b_spline_curve_with_knots(...)`, `rational_b_spline_curve(weights_data)`: see section 5.
- `polyline(name, points: LIST OF cartesian_point)`.
- `trimmed_curve(name, basis_curve, trim_1, trim_2, sense_agreement, master_representation)`: see section 5.
- `composite_curve(name, segments: LIST OF composite_curve_segment, self_intersect)`.
- `pcurve(name, basis_surface: surface, reference_to_curve: definitional_representation)`: a 2D curve in a surface's parameter space; `reference_to_curve` wraps the actual 2D curve.
- `surface_curve(name, curve_3d: curve, associated_geometry: LIST[1:2] OF pcurve_or_surface, master_representation: preferred_surface_curve_representation)`. The `master_representation` enum is `{curve_3d, pcurve_s1, pcurve_s2}` telling which representation is authoritative.
- `seam_curve`: a `surface_curve` subtype where both `associated_geometry` pcurves lie on the SAME surface (the two sides of a seam).
- `intersection_curve`: a `surface_curve` subtype representing the exact intersection of two surfaces.

### Points and placement

- `cartesian_point(name, coordinates: LIST[1:3] OF length)`.
- `point_on_curve(name, basis_curve, point_parameter)`, `point_on_surface(name, basis_surface, u, v)`: rarely used for vertices.
- `direction(name, direction_ratios: LIST[2:3] OF REAL)` (a unit-ish vector, not necessarily normalized in file).
- `vector(name, orientation: direction, magnitude: length)`.
- `axis2_placement_3d(name, location: cartesian_point, axis: OPTIONAL direction, ref_direction: OPTIONAL direction)`. `axis` is the local +Z; `ref_direction` seeds local +X (projected orthogonal to axis); +Y is the cross product. When `axis`/`ref_direction` are `$`, defaults are used (Z=global Z, X=global X). This is the single most important placement entity: every analytic surface and conic curve carries one.
- `axis2_placement_2d(name, location, ref_direction)`: 2D analogue used inside pcurves.

---

## 4. THE ENTITY-TO-KEEL MAPPING TABLE (centerpiece)

Read top to bottom: containers, topology, then geometry. "Keel target" is the arena object or enum variant produced. "Notes" carries the conversion rule and the orientation algebra.

| STEP entity | Fields used | Keel target | Mapping / conversion rule |
|---|---|---|---|
| `advanced_brep_shape_representation` | items, context | `Body` (+ `Region`) | Create one `Body`; each `manifold_solid_brep` in items becomes a solid `Region`. Pull units + tolerance from the `context` (sec. 6). |
| `manifold_solid_brep` | outer (closed_shell) | `Region` with one outer `Shell` | One solid region; outer shell is the region's boundary shell, oriented outward. |
| `brep_with_voids` | outer, voids | `Region` with outer `Shell` + inner `Shell`s | Outer shell as above; each `oriented_closed_shell` void becomes an inward-oriented `Shell` in the same region. |
| `manifold_surface_shape_representation` / `shell_based_surface_model` | sbsm_boundary | `Body` of sheet `Region`(s) | Sheet/open body; each shell becomes a `Shell`. Open shells produce a non-solid region. |
| `closed_shell` | cfs_faces | `Shell` (closed) | Each face becomes a `Face`. Mark shell closed; expect watertight (heal if not, sec. 10). |
| `open_shell` | cfs_faces | `Shell` (open) | As above, shell marked open. |
| `oriented_closed_shell` | closed_shell_element, orientation | `Shell` with flipped sense | If `orientation=.F.`, invert all face senses of the referenced shell (used for voids: void shells point into the cavity). |
| `advanced_face` / `face_surface` | bounds, face_geometry, same_sense | `Face` | Face surface = convert(face_geometry) into a surface enum variant. Face orientation = surface natural orientation XORed with `same_sense=.F.` (see orientation algebra, sec. 8). Each `face_bound` becomes a `Loop`. |
| `face_bound` | bound (loop), orientation | `Loop` | Loop coedges come from the bound's `edge_loop`. If `face_bound.orientation=.F.`, reverse the loop's coedge order and flip each coedge sense. |
| `face_outer_bound` | bound, orientation | outer `Loop` of the face | Same as `face_bound` but tagged as the outer (peripheral) loop; inner `face_bound`s are holes. |
| `edge_loop` | edge_list (oriented_edges) | the `Coedge` cycle of a `Loop` | Each `oriented_edge` becomes one `Coedge`, in list order. |
| `oriented_edge` | edge_element, orientation | `Coedge` (+ orientation flag) | Coedge references the shared `Edge` from `edge_element`. Coedge `forward` flag = `oriented_edge.orientation` combined with `edge_curve.same_sense` and the enclosing `face_bound.orientation` (sec. 8). The coedge's pcurve is taken from the matching `surface_curve` pcurve or reconstructed (sec. 7). |
| `edge_curve` | edge_start, edge_end, edge_geometry, same_sense | `Edge` (shared) | Create one `Edge` per `edge_curve` (shared by the two adjacent coedges). Curve = convert(edge_geometry). Start/end vertices = the two `vertex_point`s. `same_sense=.T.` means the 3D curve's natural direction goes start to end; `.F.` means reversed: record this on the edge so coedge orientation can be derived consistently. |
| `vertex_point` | vertex_geometry (cartesian_point) | `Vertex` | One `Vertex` at the point coordinates. Deduplicate coincident vertices within session tolerance during stitching. |
| `vertex_loop` | loop_vertex | degenerate `Loop` (single vertex, no coedges) | Used at poles/apices. Keel: a loop bounding the face at a single point (cone apex, sphere pole). Mark as a degenerate/pole loop; no edge geometry. |
| `poly_loop` | polygon | `Loop` of line `Edge`s | Legacy faceted brep: synthesize straight `Line` edges between successive points. |
| `plane` | position (axis2_placement_3d) | `Surface::Plane` | Plane through `location`, normal = `axis` (local +Z), with `ref_direction` as the U axis. |
| `cylindrical_surface` | position, radius | `Surface::Cylinder` | Axis = placement +Z through `location`; `radius`. U is angular about axis (from `ref_direction`), V is along axis. |
| `conical_surface` | position, radius, semi_angle | `Surface::Cone` | Apex computed from `location`, axis, `radius`, `semi_angle`. Store half-angle. Watch the apex pole (degenerate). |
| `spherical_surface` | position, radius | `Surface::Sphere` | Center = `location`; `radius`. Poles along `axis` are degenerate. |
| `toroidal_surface` | position, major_radius, minor_radius | `Surface::Torus` | Center = `location`, axis = +Z, `major_radius` (R), `minor_radius` (r). |
| `degenerate_toroidal_surface` | + select_outer | `Surface::Torus` (degenerate) or NURBS | If Keel's torus cannot represent r >= R, convert to a NURBS surface; otherwise store with a degeneracy flag and the `select_outer` choice. |
| `b_spline_surface_with_knots` (+ `rational_b_spline_surface` via complex) | see sec. 5 | `Surface::NurbsSurface` (homogeneous 4D) | Full NURBS conversion, sec. 5. |
| `surface_of_revolution` / `surface_of_linear_extrusion` / `offset_surface` | swept geometry | `Surface::NurbsSurface` (or analytic when recognizable) | Evaluate/convert to NURBS unless the result is a recognizable analytic (e.g. revolving a line gives a cone/cylinder). Canonical recovery ties `kernel/24`. |
| `rectangular_trimmed_surface` | basis_surface, u1,u2,v1,v2 | trimmed `Face` on convert(basis_surface) | Use the underlying surface; the trim rectangle becomes the face's parametric extent / outer loop. |
| `line` | pnt, dir | `Curve::Line` | Point `pnt`, direction `dir.orientation`; parameter scale = `dir.magnitude`. |
| `circle` | position, radius | `Curve::Circle` | Center = `location`, plane normal = +Z, start angle from `ref_direction`, `radius`. |
| `ellipse` | position, semi_axis_1, semi_axis_2 | `Curve::Ellipse` | Center, axes from placement, two semi-axes. |
| `parabola` / `hyperbola` | position, params | `Curve::NurbsCurve` | No native variant; convert to rational NURBS (hyperbola/parabola arcs are rational/polynomial NURBS). |
| `b_spline_curve_with_knots` (+ rational) | see sec. 5 | `Curve::NurbsCurve` (homogeneous 4D) | Full NURBS conversion, sec. 5. |
| `polyline` | points | `Curve::NurbsCurve` (degree 1) or split into line edges | Degree-1 NURBS or a chain of line segments. |
| `trimmed_curve` | basis_curve, trim_1, trim_2, sense, master_representation | trimmed `Curve` variant | Convert basis; apply trim params (sec. 5). |
| `composite_curve` | segments | concatenated `Curve` (often NURBS) | Join segments; usually convert to one NURBS curve. |
| `pcurve` | basis_surface, reference_to_curve | `Coedge` pcurve (2D in surface param space) | The 2D curve in the face surface's UV space; attach to the coedge. |
| `surface_curve` / `seam_curve` / `intersection_curve` | curve_3d, associated_geometry, master_representation | `Edge` 3D curve + coedge pcurves | `curve_3d` becomes the edge curve; the two `associated_geometry` pcurves become the pcurves for the two adjacent coedges. `master_representation` selects authoritative source (sec. 7). `seam_curve`: both pcurves on the same surface (seam handling, sec. 8). |
| `cartesian_point` | coordinates | `Point3` (or vertex location) | Direct. |
| `direction` | direction_ratios | unit `Vector3` | Normalize on read (file may not normalize). |
| `vector` | orientation, magnitude | `Vector3` scaled | direction * magnitude. |
| `axis2_placement_3d` | location, axis, ref_direction | local frame `(origin, X, Y, Z)` | Z = normalize(axis) (default global Z if `$`); X = normalize(ref_direction projected orthogonal to Z) (default global X if `$`); Y = Z cross X. Feeds every analytic surface and conic. |
| `axis2_placement_2d` | location, ref_direction | 2D frame for pcurves | Analogous in UV space. |

---

## 5. NURBS Conversion Specifics (high stakes)

### `b_spline_surface_with_knots`

Flattened attribute order in the file (as a complex instance combining `b_spline_surface` + `b_spline_surface_with_knots` + optionally `rational_b_spline_surface`):

`B_SPLINE_SURFACE(u_degree, v_degree, control_points_list, surface_form, u_closed, v_closed, self_intersect)` then `B_SPLINE_SURFACE_WITH_KNOTS(u_multiplicities, v_multiplicities, u_knots, v_knots, knot_spec)`.

Fields:
- `u_degree`, `v_degree`: INTEGER.
- `control_points_list`: `LIST[2:?] OF LIST[2:?] OF cartesian_point`. **Ordering is ROW-MAJOR by U**: the outer list indexes U, the inner list indexes V. So `control_points_list[i][j]` is the control point at U-index i, V-index j. Grid is `(n_u+1) x (n_v+1)` where `n_u+1 = sum(u_multiplicities) - u_degree - 1`.
- `surface_form`: enumeration (`.UNSPECIFIED.`, `.PLANE_SURF.`, `.CYLINDRICAL_SURF.`, etc.), a hint, not authoritative; do not trust it for analytic recovery, validate independently.
- `u_closed`, `v_closed`, `self_intersect`: LOGICAL (`.T.`/`.F.`/`.U.`).
- `u_multiplicities`, `v_multiplicities`: `LIST OF INTEGER`, the multiplicity of each distinct knot.
- `u_knots`, `v_knots`: `LIST OF REAL`, the DISTINCT knot values (not the expanded vector).
- `knot_spec`: enum `{unspecified, quasi_uniform_knots, uniform_knots, piecewise_bezier_knots}` (file tokens `.UNSPECIFIED.` etc.). A hint; the multiplicities+knots are authoritative.

**Conversion to Keel:**
1. Expand the knot vector: repeat each distinct `u_knots[k]` value `u_multiplicities[k]` times to build the full clamped knot array. Same for V. The expanded length must equal `(n_u+1) + u_degree + 1`. Validate this invariant; mismatch means a malformed file.
2. STEP uses clamped/non-periodic knots with end multiplicity = degree+1 in the common case. If multiplicities indicate a periodic surface (`u_closed=.T.` with interior-style end knots), convert periodic to clamped by knot insertion / unwrapping, since Keel stores clamped.
3. Build the homogeneous 4D control grid: for the non-rational case, weight = 1, so `P_h[i][j] = (x, y, z, 1)`. For the rational case, the complex instance carries `RATIONAL_B_SPLINE_SURFACE(weights_data)` with `weights_data: LIST OF LIST OF REAL` in the SAME `[i][j]` row-major order. Then `P_h[i][j] = (w*x, w*y, w*z, w)` where `w = weights_data[i][j]`. **This pre-multiplication is the conversion to homogeneous 4D**: Keel stores `(wx, wy, wz, w)`, never the un-weighted Euclidean point plus a separate weight. Getting this wrong silently distorts every rational surface.
4. Surface evaluates as `S(u,v) = (sum_ij N_i(u) M_j(v) P_h[i][j]) / w(u,v)` with the projection back to 3D by dividing by the 4th coordinate. This matches Keel's homogeneous-4D evaluator.

### `b_spline_curve_with_knots`

`B_SPLINE_CURVE(degree, control_points_list, curve_form, closed_curve, self_intersect)` then `B_SPLINE_CURVE_WITH_KNOTS(knot_multiplicities, knots, knot_spec)`. Rational via complex `RATIONAL_B_SPLINE_CURVE(weights_data: LIST OF REAL)`. Conversion is the 1D analogue: expand knots from multiplicities, build `P_h[i] = (w*x, w*y, w*z, w)`.

### `circle`, `ellipse`, `line` parameterization

- `line(pnt, dir)`: `C(t) = pnt + t * dir.orientation * dir.magnitude`. The magnitude is the unit-parameter length; preserve it so trims by parameter convert correctly.
- `circle(position, radius)`: angle measured from `position.ref_direction` (local +X) toward local +Y, about local +Z. `C(theta) = center + radius*(cos(theta)*X + sin(theta)*Y)`, theta in radians.
- `ellipse(position, a, b)`: `C(theta) = center + a*cos(theta)*X + b*sin(theta)*Y`.

### `trimmed_curve`

Fields: `basis_curve`, `trim_1`, `trim_2`, `sense_agreement: BOOLEAN`, `master_representation: trimming_preference`.
- `trim_1`/`trim_2` are each a `SET[1:2] OF trimming_select`, where `trimming_select = SELECT(cartesian_point, parameter_value)`. So a trim endpoint may be given as a parameter value, a Cartesian point on the curve, or BOTH.
- `master_representation` (`.PARAMETER.`, `.CARTESIAN.`, `.UNSPECIFIED.`) tells which form is authoritative when both are supplied. **Policy**: if `.PARAMETER.`, use the parameter; if `.CARTESIAN.`, project the point onto the basis curve to get the parameter; if `.UNSPECIFIED.` and both present, prefer parameter but cross-check the point lies on the curve within tolerance.
- `sense_agreement`: `.T.` means the trimmed curve runs from `trim_1` to `trim_2` in the basis curve's natural direction; `.F.` means reversed.
- Convert: clip Keel's converted basis curve to `[t1, t2]` (swapping if `sense_agreement=.F.`).

### Analytic surface placement conversion

Every analytic surface (`plane`, `cylindrical_surface`, `conical_surface`, `spherical_surface`, `toroidal_surface`) carries an `axis2_placement_3d`. Convert it once to Keel's `(origin, X, Y, Z)` frame (Z=axis, X=ref_direction orthogonalized, Y=ZxX), then map into Keel's native parameter convention for each surface type. Beware: STEP's `direction` entities are NOT guaranteed unit-length; normalize. And `ref_direction` may not be orthogonal to `axis`; Gram-Schmidt project it before use. Both are classic import bugs.

---

## 6. Units, Context, and Tolerance

The `advanced_brep_shape_representation` lives in a `geometric_representation_context` that is usually a complex instance also being a `global_unit_assigned_context` and a `global_uncertainty_assigned_context`:

```
#10=(GEOMETRIC_REPRESENTATION_CONTEXT(3)
  GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#11))
  GLOBAL_UNIT_ASSIGNED_CONTEXT((#12,#13,#14))
  REPRESENTATION_CONTEXT('',''));
```

- `global_unit_assigned_context(units: SET OF unit)`: carries the length unit, plane-angle unit, solid-angle unit. A length unit is a `(named_unit, si_unit)` like `.MILLI. .METRE.`, or a `conversion_based_unit('INCH', #factor)` whose `conversion_factor` is a `length_measure_with_unit` giving `0.0254` metres per inch (or `25.4` mm). **Import must convert all coordinates to Keel's working unit (mm) using this factor.** Plane-angle unit is typically radians (`.RADIAN.`) but may be degrees via a conversion_based_unit; circles/cones store angles in the file's plane-angle unit, so convert.
- `uncertainty_measure_with_unit(value, unit, name, description)`, referenced from `global_uncertainty_assigned_context`, carries the file's **GLOBAL TOLERANCE** (typically `name='distance_accuracy_value'`). This is the exporting system's own statement of geometric accuracy: a STEP file tells you its own resolution.

**Mapping to Keel**: set the session resolution from `uncertainty_measure_with_unit.value` (converted to mm), but floor it at Keel's minimum resolution (do not trust an absurdly tight or loose declared value blindly). Each imported entity's per-entity tolerance starts at this global value; healing (sec. 10) may raise individual vertex/edge tolerances locally to absorb gaps, exactly as OpenCASCADE does with `read.precision.val` (default 0.0001 mm) and `read.maxprecision.val` (default 1.0). Keel's per-entity tolerance model is a strict superset of this single-global-plus-local-relaxation scheme.

---

## 7. Pcurves: provided vs reconstructed

Keel mandates a pcurve on every coedge. STEP's supply is inconsistent:

- **Best case**: the edge's geometry is a `surface_curve` (or `seam_curve`) whose `associated_geometry` provides one or two `pcurve`s. Each `pcurve(basis_surface, reference_to_curve)` is a 2D curve in a specific surface's UV space. Match the pcurve whose `basis_surface` equals the coedge's face surface, and attach it directly. `master_representation` (`curve_3d` / `pcurve_s1` / `pcurve_s2`) tells which representation the exporter considered authoritative: if `pcurve_s1`, trust the first pcurve over the 3D curve when they disagree.
- **Common case**: the edge's geometry is a bare `edge_curve` with only a 3D `curve` (line/circle/NURBS) and NO pcurve. Then Keel MUST reconstruct the pcurve by **projecting the 3D edge curve onto the face surface** (ties `nurbs/04` projection research): sample the 3D curve, invert each point through the surface's `(u,v)` inverse-evaluation, and fit a 2D NURBS pcurve through the parameter points, respecting seams and periodic wraps. For analytic surfaces the inverse is closed-form (cylinder: atan2 for U, axial distance for V); for NURBS surfaces use Newton point-inversion with the projection machinery.
- **Policy**: prefer the supplied pcurve when `master_representation` favors it and it passes a consistency check (3D curve evaluated at sampled params lies within tolerance of the surface point); otherwise reconstruct. Always reconstruct when the supplied pcurve is missing, off-surface, or self-intersecting. Record provenance (supplied vs reconstructed) for the validation pass.

---

## 8. Orientation, Seams, and Degeneracies

### The orientation algebra (the make-or-break detail)

Three booleans combine to give a coedge its final direction and a face its final normal:
- `advanced_face.same_sense`: face normal vs surface natural normal. Keel face normal = surface_normal if `.T.`, flipped if `.F.`.
- `face_bound.orientation`: loop sense vs face. If `.F.`, reverse the loop direction (and flip the contained coedge directions) so the loop runs counterclockwise about the (possibly flipped) face normal, keeping material on the left/inside by Keel's convention.
- `oriented_edge.orientation`: this coedge's traversal direction vs the underlying `edge_curve`'s direction.
- `edge_curve.same_sense`: the 3D curve's natural parameter direction vs the edge's start-to-end direction.

**Composite rule for a coedge's "forward" flag** (curve direction agrees with coedge traversal):
`coedge_forward = oriented_edge.orientation XOR (NOT edge_curve.same_sense) XOR face_bound.orientation_flip`
Implement as XOR of the three sense flips, then derive the pcurve direction to match. Validate by checking adjacent coedges across each edge traverse it in OPPOSITE directions (the half-edge consistency invariant); if not, the file has an orientation defect to heal.

### Seam edges on closed surfaces

On a cylinder/sphere/torus, the surface is periodic, and one `edge_curve` can bound the SAME face on BOTH sides (the seam where U wraps from 2pi back to 0). STEP marks this with a `seam_curve` whose two `associated_geometry` pcurves lie on the same `basis_surface` at U=0 and U=2pi. The single edge appears TWICE in the face's loop, once with each pcurve. Keel: create one `Edge`, two `Coedge`s in the loop (one with the U=0 pcurve, one with the U=2pi pcurve), both pointing at the same edge. Detect seams by: surface is periodic AND an edge appears twice in one loop AND the two pcurves differ by the period. Missing/mis-marked seams are a top cause of "imported cylinder won't close."

### Degeneracies and poles

- Sphere apex / cone apex: the surface degenerates to a point at a pole. The face boundary there is a `vertex_loop` (a single vertex, no edge) or a zero-length degenerate edge. Keel: model as a degenerate/pole loop; the pcurve runs along the iso-parameter edge of UV space at the pole (a horizontal segment at v=v_pole), so the parametric loop is still closed even though the 3D loop pinches to a point.
- `degenerate_toroidal_surface`: the apple/lemon torus with r >= R self-intersects; handle as in the table (flag or convert to NURBS).
- Tiny/zero-length edges and sliver faces: pass to the healing pipeline (sec. 10), do not reject the body.

---

## 9. Validation Properties (ties `kernel/22`)

The CAx-IF "Recommended Practices for Geometric and Assembly Validation Properties" (v4.6, 2023, MBx-IF) defines machine-checkable properties carried in the STEP file so the receiver can confirm the geometry survived translation:

- For a SOLID brep: **volume**, **surface area**, **centroid** (a point), and optionally bounding box.
- For a SHELL/surface: **surface area** and a **cloud of points** (sampled surface points) only.
- For a CURVE/wireframe: **curve length** and **centroid**.

Encoding: a `property_definition` (e.g. `name='volume'` / `'surface area'` / `'centroid'`) tied via `property_definition_representation` to a `representation` whose item is a `measure_representation_item` (a real measure with its unit) for scalars, or a `cartesian_point` for the centroid. These are bound to the shape through `geometric_validation_property_assignment` / `shape_aspect`. "Basic" properties attach to a single shape representation; "extended" properties attach to assembly instances.

**Round-trip check on import**: after building the Keel body, compute its volume, area, and centroid with Keel's mass-properties engine and compare against the stored values. If they agree within an agreed tolerance (the recommended practice and OpenCASCADE's `STEPConstruct_ValidationProps` use a small relative tolerance, e.g. fractions of a percent on volume/area and the distance accuracy on the centroid), the import is validated; if not, flag the body for review, the discrepancy localizes which faces/shells were mis-imported. The cloud-of-points (where present) gives a per-surface spot check: each stored point should lie on the corresponding imported face within tolerance. This is Keel's primary objective import-correctness oracle and should run automatically on every import.

---

## 10. Dirty STEP and Heal-on-Import (ties `kernel/13`, `kernel/29`, `kernel/30`)

STEP import is the FIRST place Keel's tolerant/healing machinery meets foreign data, and real exported STEP is reliably dirty because each exporting system's internal tolerance and surface conventions differ. Characteristic defects:

- **Gaps**: adjacent faces' edges do not actually coincide because the exporter's stitch tolerance differs from the file's declared `uncertainty`. Vertices that should be one point sit microns apart.
- **Missing or wrong pcurves**: many exporters omit pcurves (only 3D edge curves), or supply pcurves that drift off the surface; some set `master_representation` inconsistently.
- **Self-intersecting trim loops**: a face's boundary loop crosses itself in UV space.
- **Tiny edges and sliver faces**: sub-tolerance edges and needle/sliver faces from the source modeler.
- **Inconsistent orientation**: `same_sense`/`orientation` flags that do not yield a consistent half-edge structure.
- **Degenerate faces**: zero-area faces, faces collapsed to a line.
- **Per-vendor quirks**: well-known exporter-specific patterns (e.g. periodic surfaces emitted without seam marking, conics emitted as low-degree NURBS, redundant `trimmed_curve` wrapping, denormalized direction vectors).

**The stitch-then-heal pipeline an importer must run** (mirrors OpenCASCADE's post-translation `ShapeFix`/`ShapeHealing` sequence, which Keel should treat as the reference behavior to match or exceed):
1. **Build raw topology** from the entity graph (faces, loops, coedges, edges, vertices) without yet asserting watertightness.
2. **Vertex merge / stitch**: cluster vertices within the (possibly locally relaxed) tolerance; merge coincident edges so adjacent faces share one edge. This closes the gaps. Raise per-vertex/edge tolerance locally up to a max-precision cap (cf. `read.maxprecision.val`) rather than moving geometry when possible.
3. **Pcurve repair/reconstruction** (sec. 7): supply missing pcurves by projection; replace off-surface or self-intersecting pcurves (OCC's `FixSelfIntersection`, `FixLacking`, `FixConnected` are the model: cut out self-intersecting loops, insert degenerate edges to fill UV gaps, force shared vertices).
4. **Orientation repair** (sec. 8): recompute a globally consistent coedge orientation; flip faces so shell normals point consistently outward.
5. **Small-feature cleanup**: collapse sub-tolerance edges and remove sliver faces per Keel's degeneracy policy (`kernel/30`), only when it does not change the validated mass properties beyond tolerance.
6. **Shell closure check**: confirm every edge is used by exactly two coedges (closed shell) and the shell is watertight; otherwise downgrade to a sheet body and report.
7. **Validation-property round-trip** (sec. 9) as the acceptance gate.

Crucially, the healing must be tolerance-driven off the file's own `uncertainty_measure_with_unit` (sec. 6) combined with Keel's session floor, not a single hard-coded epsilon.

---

## 11. AP242 Tessellation and PMI Import (lighter)

### Tessellation (AP242 ed2+)

Entities (per the CAx-IF "Tessellated 3D Geometry" recommended practice, v1.1, 2019): `tessellated_shape_representation` is the container; `triangulated_face_set` and `complex_triangulated_surface_set` carry the facets; `coordinates_list` holds a flat array of `cartesian_point`s; index arrays (`pnindex`, the per-triangle `triangles` index triples, optional `normals` and `normal_index`) reference into the coordinate/normal lists. `tessellated_solid` / `tessellated_surface_set` group these. Mapping to Keel: build a facet/mesh body (the "convergent" representation, ties `kernel/09`): coordinates_list becomes the vertex buffer, the triangle index triples become the face index buffer, normals (if present) become per-vertex/per-face normals. A `tessellated_shape_representation` may accompany an `advanced_brep_shape_representation` for the same part (exact B-rep plus a lightweight tessellation); import both and link them.

### Semantic PMI (optional later layer)

AP242 semantic PMI (machine-readable GD&T) is carried via `datum`, `datum_feature`, `dimensional_size`, geometric-tolerance entities, and `shape_aspect`/`shape_aspect_relationship` linking them to faces/edges, per the CAx-IF "PMI Representation and Presentation" recommended practice (v4.1, 2024). This ties `kernel/17` (GD&T/PMI/tolerances). For import M-series milestones this is an OPTIONAL layer: bring the B-rep in first, then attach semantic PMI to the already-imported faces/edges via the `shape_aspect` references. Polyline (graphical) PMI presentation is lower priority still.

---

## 12. Parsing and Reuse Engineering

### The Part 21 parser

A correct importer needs: (1) a **tokenizer** for the Part 21 grammar (instance ids, keywords, reals, strings with `''` escaping and `\X2\...\X0\` extended encoding, enums, `$`, `*`, nested lists); (2) **two-pass forward-reference resolution**: pass one records every `#N = <record(s)>` into a table keyed by id (reassembling complex/AND instances into one logical entity); pass two resolves `#M` references to concrete objects, building the entity graph. The `ruststep` project models this exactly with its `Holder` derive (a deferred-reference placeholder) and `tables` module ("exchange structure graph" as tables) for second-pass resolution; (3) **schema awareness** sufficient to bind positional attributes to the right fields, either by generating structs from the EXPRESS schema (espr/fedex_plus style) or by a hand-written entity dictionary for the ~60 Part 42 entities Keel actually consumes.

### Robustness to malformed/hostile files (ties `kernel/14`)

Real and adversarial STEP files contain: cyclic references, dangling `#N` references, truncated instances, gigantic lists, deeply nested parentheses, duplicate ids, mixed line endings, and the full `\X2\`/`\X4\` unicode escape zoo in strings. The parser must be a streaming, allocation-bounded, panic-free tokenizer (no unbounded recursion on nested lists, ids validated before deref, every deref returns a Result), consistent with Keel's determinism/robustness posture. Never trust the file; validate every cross-reference and every aggregate length against the schema cardinality.

### Reuse-vs-build options for a pure-Rust kernel

- **`ruststep`** (ricosjp, MIT/Apache, pure Rust): EXPRESS-driven via the `espr` compiler; has the right architecture (tokenizer to AST, `Holder`/`tables` two-pass resolution, generated structs per schema). BUT it is explicitly experimental ("DO NOT USE FOR PRODUCT"), bundles only AP201/AP203 generated code behind feature flags (no AP214/AP242 schema generated out of the box), serialization is incomplete, and the IR/AST is not stabilized. Usable as a STARTING POINT for the parser/tokenizer and as a reference for the Holder/Table pattern, but Keel would need to generate AP242 structs with espr and harden it.
- **OpenCASCADE STEP reader** (`STEPControl_Reader` + `XSControl`/`Transfer` framework, LGPL, C++): the de-facto REFERENCE IMPLEMENTATION. Its entity mapping (manifold_solid_brep to solid, advanced_face to face, edge_curve to edge, etc.), its precision parameters (`read.precision.mode/val`, `read.maxprecision.val`, `read.surfacecurve.mode`), and its post-translation `ShapeFix`/`ShapeHealing` sequence (`FixSelfIntersection`, `FixLacking`, `FixConnected`, `FixSameParameter`) are the behavior Keel should study and match. Not reusable in a pure-Rust kernel (C++/LGPL, brings all of OCCT), but it is the gold-standard SPEC for what the importer must do.
- **STEPcode** (NIST/BRL-CAD, 3-clause BSD, C++/Python): mature EXPRESS-to-code generator (`fedex_plus`) + Part 21 read/write. Proven on real schemas including IFC. C++, so not a drop-in for Rust, but the `fedex_plus` generation approach and the SCL data-dictionary design are worth studying, and its BSD license is permissive.
- **IFC parsers** (e.g. IfcOpenShell, web-ifc): IFC is also ISO 10303-21 + an EXPRESS schema, so IFC parser architecture (tokenizer + two-pass) is a direct parallel; their performance engineering (memory-mapped, parallel parsing) is instructive for large STEP files.

**Honest verdict for Keel (pure-Rust kernel)**: do NOT take an FFI dependency on OpenCASCADE or STEPcode for a pure-Rust kernel; it would compromise the build, license, and determinism story. Instead, **build a focused, hand-written Part 21 tokenizer + two-pass resolver in Rust** (a few thousand lines, fully under Keel's control and determinism guarantees), with a **hand-curated entity dictionary for the ~60 Part 42 + AP242-tessellation entities that B-rep import actually needs** (rather than generating the full multi-thousand-entity AP242 schema, most of which Keel will never consume). Borrow `ruststep`'s `Holder`/`tables` two-pass pattern as the proven design, and treat OpenCASCADE's reader behavior (mapping rules + precision params + ShapeFix sequence) as the conformance reference. This is less work than wrangling generated code for an 1000+ entity schema, keeps the dependency surface zero, and matches Keel's robustness posture. If broad AP coverage is later needed, generate with espr; for shipping B-rep import, the curated dictionary is the faster, safer path.

---

## Per-Source Entries

### S1. ISO 10303-21 (Part 21 clear-text encoding): Wikipedia + LoC + ISO
- **Citation**: "ISO 10303-21" Wikipedia; Library of Congress FDD000448 STEP-file format description; ISO 10303-21:2016 (edition 3). https://en.wikipedia.org/wiki/ISO_10303-21 ; https://www.loc.gov/preservation/digital/formats/fdd/fdd000448.shtml ; https://www.iso.org/standard/63141.html
- **Content**: File begins `ISO-10303-21;`, ends `END-ISO-10303-21;`. HEADER groups `FILE_DESCRIPTION`, `FILE_NAME`, `FILE_SCHEMA` (+ optional `FILE_POPULATION`/`SECTION_LANGUAGE`/`SECTION_CONTEXT` in ed2+). DATA section of `#N=ENTITY(...)` instances. Instance ids are positive integers `< 2^63`, local to the file, used for forward and backward references. `$` = unset OPTIONAL, `*` = derived. Complex (AND) instances combine leaf records in alphabetical order under one id (external mapping). Edition 3 adds anchor/reference/signature sections for external refs and digital signatures. Example: `#16=PRODUCT('A0001','Test Part 1','',(#18));`.
- **Limitations**: Wikipedia/LoC are summaries; exact grammar productions need the ISO text. Does not cover entity semantics (that is Part 42).
- **Kernel relevance**: defines the exact tokenizer grammar and the forward-reference requirement that mandates Keel's two-pass parser, plus complex-instance reassembly for the rational+b_spline NURBS case.

### S2. EXPRESS (ISO 10303-11) read-level subset
- **Citation**: ISO 10303-11 (EXPRESS language reference); summarized via STEP Tools and JSDAI STEP-file docs. https://www.jsdai.net/support/about-step/step-file ; http://www.steptools.com/stds/step/
- **Content**: ENTITY/END_ENTITY, SUBTYPE/SUPERTYPE, OPTIONAL, aggregates (LIST/SET/ARRAY with `[lo:hi]`), SELECT (union of entity types), ENUMERATION, TYPE = defined types. Flattened supertype-then-subtype attribute order matches Part 21 record order, enabling positional binding. SELECT slots require runtime type dispatch (e.g. `trimming_select` is `cartesian_point` OR `parameter_value`).
- **Limitations**: full EXPRESS (rules, WHERE constraints, derived/inverse attributes, functions) is large; import only needs the read subset, can ignore WHERE-rule evaluation.
- **Kernel relevance**: tells Keel how to bind file records to entity fields positionally and how to handle SELECT-typed slots (notably trim endpoints and pcurve_or_surface).

### S3. AP242 standard and edition history: ap242.org / ISO / prostep
- **Citation**: ap242.org "AP242 editions"; ISO 10303-242:2020; prostep ivip fact sheet; Capvidia "AP203 vs AP214 vs AP242". https://www.ap242.org/ap242-standard.html ; https://www.iso.org/standard/66654.html ; https://www.capvidia.com/blog/best-step-file-to-use-ap203-vs-ap214-vs-ap242
- **Content**: AP242 merges/supersedes AP203 ed2 + AP214 ed3. ed1 (2014, ~533 new entities) covers all 203/214 functionality plus semantic PMI, tessellation, kinematics; ed2 (2020) adds electrical; ed3 (2022) corrective; ed4 in dev. Schema name `AP242_MANAGED_MODEL_BASED_3D_ENGINEERING`. The geometry/topology Part 42 core is shared with AP203/214.
- **Limitations**: marketing/overview level; does not give entity attribute lists.
- **Kernel relevance**: justifies one shared geometry mapping across AP203/214/242, with AP242-only branches for tessellation and semantic PMI; tells Keel to detect AP via FILE_SCHEMA but map by entity name.

### S4. STEP Tools: AP242 AIM notes
- **Citation**: STEP Tools "AP242 First Edition Notes". http://www.steptools.com/docs/stp_aim/notes_ap242.html
- **Content**: AP242 AIM B-rep entities (`manifold_solid_brep` etc.) plus added tessellation entities (`tessellated_solid`, `tessellated_surface_set`, `tessellated_face`, `tessellated_shape_representation`), `b_spline_surface_patch`, `b_spline_curve_segment`, and error/erroneous entities (`erroneous_manifold_solid_brep`). Confirms AP242 intended to replace AP203/214.
- **Limitations**: high-level highlight, not full attribute definitions.
- **Kernel relevance**: enumerates the AP242 tessellation entity names Keel's tessellation importer targets, and confirms the brep core is unchanged.

### S5. ISO 10303-42 schema (geometric and topological representation): STEP Tools SMRL
- **Citation**: STEP Tools, ISO 10303-42:2021(E) resource schema. https://www.steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/4_schema.htm
- **Content**: EXPRESS attribute lists: `line(pnt: cartesian_point, dir: vector)`; `axis2_placement_3d(location, axis: OPTIONAL direction, ref_direction: OPTIONAL direction)` with derived `p: LIST[3:3] OF direction`; `cartesian_point(coordinates: LIST[1:3] OF length_measure)`; `direction(direction_ratios: LIST[2:3] OF REAL)`; `vector(orientation: direction, magnitude: length_measure)`; `geometric_representation_context(coordinate_space_dimension)`.
- **Limitations**: the WebFetch excerpt rendered only some entities; full per-entity pages give the rest.
- **Kernel relevance**: authoritative field order and types for placement, point, direction, vector, line, the substrate every analytic surface and conic builds on.

### S6. b_spline_geometry module (ISO/TS 10303-1801): STEP Tools SMRL
- **Citation**: STEP Tools ISO/TS 10303-1801 "B spline geometry" ARM. https://www.steptools.com/stds/smrl/data/modules/b_spline_geometry/sys/e_exp_arm.htm
- **Content**: `B_spline_surface(u_degree, v_degree, control_points: LIST OF LIST OF cartesian_point, u_closed, v_closed)`; explicit-knot subtype adds `u_knot_multiplicities`, `u_knot_values`, `v_knot_multiplicities`, `v_knot_values`; `Rational_b_spline_surface` adds `weight_values: LIST OF LIST OF REAL`; curve analogues with `degree`, `control_points`, `knot_multiplicities`, `knot_values`, `weight_values`; `knot_specification` enum (bezier, quasi_uniform, uniform). Control points are nested LIST = row-by-row (U outer, V inner).
- **Limitations**: ARM names differ slightly from the AIM tokens used in actual files (`.QUASI_UNIFORM_KNOTS.` etc.); structure is identical.
- **Kernel relevance**: the exact NURBS field set and the row/column ordering rule for converting STEP control grids + weights into Keel's homogeneous-4D grid.

### S7. STEP Tools: surface_curve entity
- **Citation**: STEP Merged AP Library, ENTITY surface_curve. https://www.steptools.com/stds/stp_aim/html/t_surface_curve.html
- **Content**: `surface_curve(curve_3d: curve, associated_geometry: LIST[1:2] OF pcurve_or_surface, master_representation: preferred_surface_curve_representation)`; derived `basis_surface: SET[1:2] OF surface`. Subtypes `intersection_curve`, `seam_curve`, `bounded_surface_curve`. Enum includes `curve_3d`, `pcurve_s1`, `pcurve_s2`.
- **Limitations**: subtype-specific attributes and pcurve fields are on separate pages.
- **Kernel relevance**: defines how STEP delivers an edge's 3D curve plus up to two pcurves and which is authoritative (`master_representation`), the core of Keel's pcurve-supply-vs-reconstruct policy and seam detection.

### S8. STEP Tools: edge_curve entity
- **Citation**: STEP Merged AP Library, ENTITY edge_curve. https://www.steptools.com/stds/stp_aim/html/t_edge_curve.html
- **Content**: `edge_curve(edge_start: vertex, edge_end: vertex [inherited from edge], edge_geometry: curve, same_sense: BOOLEAN)`. Subtype of `edge` and `geometric_representation_item`. Referenced by `oriented_edge`.
- **Limitations**: oriented_edge/advanced_face/face_bound attributes on their own pages.
- **Kernel relevance**: the exact fields that become a Keel `Edge` (two vertices, one curve, the `same_sense` flag feeding the orientation algebra).

### S9. OpenCASCADE STEP translator user guide
- **Citation**: Open CASCADE Technology, "STEP Translator" user guide. https://dev.opencascade.org/doc/overview/html/occt_user_guides__step.html
- **Content**: Entity mapping (`manifold_solid_brep`/`brep_with_voids`/`faceted_brep` to Solid; `closed_shell`/`open_shell`/`oriented_closed_shell`/`connected_face_set` to Shell; `advanced_face`/`face_surface` to Face; `edge_curve`/`oriented_edge` to Edge; `face_bound`/`face_outer_bound`/`edge_loop`/`poly_loop` to Wire; `vertex_point` to Vertex; `vertex_loop` to a single degenerate edge). Precision: `read.precision.mode` (0=File uses `uncertainty_measure_with_unit` length_measure, 1=User uses `read.precision.val` default 0.0001 mm), `read.maxprecision.val` default 1.0, `read.surfacecurve.mode` (0=both curves, 3=3D preferred to rebuild 2D). Healing: `ShapeFix_Edge::FixSameParameter`, `ShapeFix_Wire::FixSelfIntersection/FixLacking/FixConnected`, controlled by resource files.
- **Limitations**: describes OCC internals (TopoDS, ShapeFix), not Keel; LGPL C++ not reusable directly.
- **Kernel relevance**: THE reference behavior spec, the precise mapping, precision-parameter semantics, and the heal-on-import sequence Keel must match. Confirms file-declared `uncertainty` drives tolerance and 3D-curve-to-pcurve rebuild is standard.

### S10. OpenCASCADE STEPControl_Reader / XSControl reference
- **Citation**: OCCT class refs STEPControl_Reader, XSControl_Reader, STEPCAFControl_Reader. https://dev.opencascade.org/doc/refman/html/class_s_t_e_p_control___reader.html
- **Content**: `TransferRoots()` / `TransferList()` / `TransferEntity()` API; root entities of type PRODUCT are transfer candidates; model returned as StepModel; results via `NbShapes()`/`Shape(num)`/`OneShape()`. STEPCAFControl_Reader adds colors/names/validation-props/assemblies.
- **Limitations**: API surface, not algorithm detail.
- **Kernel relevance**: shows the transfer-framework decomposition (parse model, select roots, transfer each to a shape) that Keel's importer pipeline mirrors at a high level.

### S11. CAx-IF Recommended Practices index (MBx-IF)
- **Citation**: MBx Interoperability Forum, CAx Recommended Practices. https://www.mbx-if.org/home/cax/recpractices/
- **Content**: Authoritative practice documents with versions/URLs: Geometric and Assembly Validation Properties v4.6 (2023); PMI Representation and Presentation (AP242) v4.1 (2024); PMI Polyline Presentation v2.3 (2014); Tessellated 3D Geometry v1.1 (2019); User Defined Attributes v1.8 (2021).
- **Limitations**: index page; the per-topic detail is in each PDF.
- **Kernel relevance**: the single best entity-level import-mapping source family for validation properties, tessellation, and PMI; pins the exact documents Keel implementers should follow for those layers.

### S12. CAx-IF Geometric and Assembly Validation Properties (v4.6)
- **Citation**: MBx-IF, "Recommended Practices for Geometric and Assembly Validation Properties" v4.6 (2023). https://www.mbx-if.org/home/wp-content/uploads/2024/05/rec_prac_gvp_v46.pdf (v4.2 mirror: rec_prac_gvp_v42.pdf)
- **Content**: For solids: volume, surface area, centroid (+ bounding box). For shells/surfaces: area + cloud of points. For curves: length + centroid. Encoded via `property_definition` + `property_definition_representation` + `measure_representation_item` (scalars) / `cartesian_point` (centroid), bound through `geometric_validation_property_assignment` and `shape_aspect`. "Basic" (per shape) vs "extended" (per assembly instance). Receiver recomputes and compares within an agreed tolerance to declare the exchange valid.
- **Limitations**: PDF rendered as binary in fetch; content reconstructed from CAx-IF summaries and OCC `STEPConstruct_ValidationProps`. Exact tolerance defaults are deployment-agreed.
- **Kernel relevance**: defines Keel's primary objective import-correctness oracle, the validation-property round-trip (sec. 9), and which properties exist per topology type.

### S13. OpenCASCADE STEPConstruct_ValidationProps
- **Citation**: OCCT class ref STEPConstruct_ValidationProps. https://dev.opencascade.org/doc/refman/html/class_s_t_e_p_construct___validation_props.html
- **Content**: Reads/writes area, volume, centroid via `PropertyDefinitionRepresentation`/`PropertyDefinition`/`RepresentationItem`. Writers `AddArea/AddVolume/AddCentroid` use `FindTarget` to bind to the mapped shape. Readers `GetPropReal` (area/volume), `GetPropPnt` (centroid), `GetPropShape` (associated shape) extract values.
- **Limitations**: extraction API; comparison logic is in the application layer.
- **Kernel relevance**: concrete proof of how the validation entities are read back and bound to shapes, the implementation template for Keel's round-trip reader.

### S14. CAx-IF Tessellated 3D Geometry (v1.1)
- **Citation**: MBx-IF, "Recommended Practices for Tessellated 3D Geometry" v1.1 (2019). https://www.mbx-if.org/home/wp-content/uploads/2024/05/rec_prac_3dtess_geo_v11.pdf
- **Content**: `tessellated_shape_representation` container; `triangulated_face_set` / `complex_triangulated_surface_set` carry facets; `coordinates_list` is a flat `cartesian_point` array; `pnindex` and per-triangle index triples reference into it; optional `normals`/`normal_index`. `tessellated_solid`/`tessellated_surface_set` group them.
- **Limitations**: PDF rendered as binary; entity names confirmed against STEP Tools AP242 notes (S4) and the practice index (S11).
- **Kernel relevance**: the facet-data-to-mesh mapping for Keel's convergent/mesh body import (sec. 11), and the link between coexisting exact-brep and tessellated representations.

### S15. CAx-IF PMI Representation and Presentation (v4.1)
- **Citation**: MBx-IF, "Recommended Practices for PMI Representation and Presentation (AP242)" v4.1 (2024). https://www.mbx-if.org/home/wp-content/uploads/2024/06/rec_pracs_pmi_v41.pdf
- **Content**: Semantic PMI (machine-readable GD&T) via datum/datum_feature/dimensional_size/geometric-tolerance entities linked to geometry through `shape_aspect`/`shape_aspect_relationship`; graphical presentation via polyline/tessellated annotations.
- **Limitations**: large topic; deferred as optional import layer for Keel.
- **Kernel relevance**: tells Keel how imported semantic PMI attaches to already-imported faces/edges (ties `kernel/17`), the optional later layer in sec. 11.

### S16. ruststep (and espr): Rust STEP toolkit
- **Citation**: ricosjp/ruststep GitHub + docs.rs. https://github.com/ricosjp/ruststep ; https://docs.rs/ruststep/latest/ruststep/ ; espr https://ricosjp.github.io/ruststep/espr/
- **Content**: Pure-Rust STEP toolkit aiming to replace stepcode. `espr` compiles EXPRESS (10303-11) to Rust structs. `parser` tokenizes the exchange structure to an `ast`; `primitive` module for STEP-specific types; `Holder` derive generates deferred-reference holders; `tables` module holds the exchange-structure graph for second-pass `#N` resolution; `Deserialize` derive binds records to entity structs. Bundles AP201/AP203 generated code behind feature flags.
- **Limitations**: explicitly experimental ("DO NOT USE FOR PRODUCT"); no AP214/AP242 generated by default; serialization and IR not finalized.
- **Kernel relevance**: the closest pure-Rust prior art; its `Holder`/`tables` two-pass design is the proven pattern Keel should borrow even if Keel hand-writes a curated entity dictionary instead of generating the full schema.

### S17. NIST STEPcode (SCL)
- **Citation**: stepcode/stepcode GitHub; NIST STEP Class Library. https://github.com/stepcode/stepcode ; https://www.nist.gov/services-resources/software/step-class-library-scl
- **Content**: Generates C++/Python from EXPRESS via `fedex_plus` (`libexpress` parses EXPRESS, emits schema-specific source); reads/writes Part 21; implements SDAI (Parts 22/23). 3-clause BSD. Originated at NIST, maintained by BRL-CAD developers. Proven on IFC and STEP schemas.
- **Limitations**: C++ (Python binding partial); not a Rust dependency; heavyweight schema-wide generation.
- **Kernel relevance**: the canonical EXPRESS-to-code generator and data-dictionary design; permissive BSD license makes its approach safe to study; informs the build-vs-generate decision.

### S18. STEP file structure / preprocessor reality: JSDAI, STEP File Analyzer
- **Citation**: JSDAI STEP-file guide; NIST STEP File Analyzer User's Guide v4. https://www.jsdai.net/support/about-step/step-file ; https://nvlpubs.nist.gov/nistpubs/ams/NIST.AMS.200-4.pdf
- **Content**: Concrete walk-through of HEADER/DATA, instance syntax, schema identification, and tooling that inspects/validates real exported STEP including PMI and validation properties, exposing per-vendor structural variety.
- **Limitations**: tool/overview oriented.
- **Kernel relevance**: grounds the "dirty STEP" reality (sec. 10) in what real exporters emit; the STEP File Analyzer is a useful external oracle for cross-checking Keel's parse.

### S19. STEP trimmed_curve / surface_curve trimming semantics
- **Citation**: STEP Merged AP Library trimmed_curve + surface_curve pages; IFC IfcTrimmedCurve parallel. https://www.steptools.com/stds/stp_aim/html/t_surface_curve.html ; https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifctrimmedcurve.htm
- **Content**: `trimmed_curve(basis_curve, trim_1, trim_2: SET[1:2] OF trimming_select, sense_agreement: BOOLEAN, master_representation: trimming_preference)`; `trimming_select = SELECT(cartesian_point, parameter_value)`; both forms must be consistent (point lies on curve at the parameter); `master_representation` (.PARAMETER./.CARTESIAN./.UNSPECIFIED.) selects the authoritative one.
- **Limitations**: IFC is a parallel schema, not identical.
- **Kernel relevance**: exact rules for converting STEP trims into Keel parameter ranges, including the point-vs-parameter dispatch and the sense flag.

### S20. CAD interoperability defects / gaps literature
- **Citation**: industry/patent literature on trimmed-NURBS gaps and B-rep healing; "gaps in trimmed NURBS models are one of the most serious impediments to interoperability between CAD, CAM and CAE"; sliver-face and non-watertight defect discussions. (USPTO patent corpus and CAD interoperability surveys surfaced via search.)
- **Content**: Translation across B-rep solid definitions breaks solid integrity; gaps from differing internal tolerances; sliver faces in thin shells; non-watertight models requiring heal/rebuild; a whole healing-tools industry exists.
- **Limitations**: dispersed across patents/blogs; not a single canonical paper.
- **Kernel relevance**: corroborates the characteristic-defect list (sec. 10) and why heal-on-import is mandatory for STEP, the first foreign-data test of Keel's tolerant machinery.

---

## Closing Synthesis: STEP AP242 Import Architecture for Keel

**Parser choice (reuse vs build)**: build a focused, hand-written pure-Rust Part 21 tokenizer plus a two-pass forward-reference resolver, with a hand-curated dictionary of the ~60 Part 42 + AP242-tessellation entities that B-rep import actually consumes. Do not FFI to OpenCASCADE or STEPcode (license/build/determinism cost), and do not generate the full 1000+-entity AP242 schema (most is unused). Borrow `ruststep`'s `Holder`/`tables` two-pass pattern; treat OpenCASCADE's reader (mapping rules, `read.precision`/`read.maxprecision`/`read.surfacecurve` params, the `ShapeFix` healing sequence) as the conformance reference behavior.

**Two-pass resolution**: pass one tokenizes and tables every `#N` record (reassembling complex/AND instances, notably the rational+b_spline NURBS combination, into one logical entity); pass two resolves references and builds the typed entity graph. Validate every cross-reference and aggregate length; remain panic-free and allocation-bounded against hostile files.

**Mapping pipeline**: `advanced_brep_shape_representation` to `Body`; `manifold_solid_brep`/`brep_with_voids` to solid `Region`s + outer/void `Shell`s; `closed_shell`/`open_shell` faces to `Face`s; each `advanced_face` to a `Face` with surface from the geometry enum; each `face_bound`/`edge_loop` to a `Loop` of `Coedge`s; each `oriented_edge` to a `Coedge` over a shared `edge_curve` `Edge`; `vertex_point` to `Vertex`. Resolve the orientation algebra (sec. 8) by XORing `advanced_face.same_sense`, `face_bound.orientation`, `oriented_edge.orientation`, and `edge_curve.same_sense` into a consistent half-edge structure, validated by the each-edge-used-twice-in-opposite-directions invariant.

**Units/tolerance**: read length and plane-angle units (handling `conversion_based_unit` for inch/degrees) from `global_unit_assigned_context`; convert all coordinates to mm and angles to radians. Take the file's declared accuracy from `uncertainty_measure_with_unit` as the session resolution, floored at Keel's minimum, seeding every per-entity tolerance; allow local relaxation up to a max-precision cap during stitching.

**Pcurve-reconstruction policy**: prefer the supplied `surface_curve`/`pcurve` when `master_representation` favors it and a consistency check passes; otherwise (missing, off-surface, or self-intersecting) reconstruct the pcurve by projecting the 3D edge curve onto the face surface (closed-form for analytics, Newton point-inversion for NURBS, ties `nurbs/04`). Every coedge ends with a valid pcurve, honoring Keel's mandatory-pcurve invariant.

**Heal-on-import integration**: run the stitch-then-heal pipeline (vertex merge to close gaps, pcurve repair/reconstruction, orientation repair, small-feature cleanup, shell-closure check) driven by the file's own tolerance plus Keel's floor, mirroring and exceeding OpenCASCADE's ShapeFix sequence (ties `kernel/13`, `kernel/29`, `kernel/30`).

**Validation-property round-trip**: after building the body, recompute volume/area/centroid (and spot-check the cloud of points) and compare against the stored `geometric_validation_property_assignment` values within an agreed tolerance; pass gates acceptance, failure localizes the import defect (ties `kernel/22`).

**Milestone-sized build plan for keel-io**:
1. Part 21 tokenizer + two-pass resolver + complex-instance reassembly (pure Rust, fuzz-tested against hostile files).
2. Entity dictionary + positional binding for the ~60 Part 42 entities; units/tolerance context extraction.
3. Geometry conversion: analytic surfaces/curves (placement to Keel frame), then NURBS surfaces/curves to homogeneous-4D (the high-stakes path), then trimmed/composite curves.
4. Topology assembly + orientation algebra into Keel arenas; seam and pole/degeneracy handling.
5. Pcurve supply-or-reconstruct (integrate `nurbs/04` projection).
6. Stitch-then-heal integration (reuse `kernel/13` machinery on imported topology).
7. Validation-property round-trip oracle + per-entity provenance reporting.
8. AP242 tessellation import to convergent/mesh bodies.
9. (Optional later) semantic PMI attachment via `shape_aspect`.

---

## References

1. ISO 10303-21 (clear-text encoding). https://en.wikipedia.org/wiki/ISO_10303-21 ; https://www.loc.gov/preservation/digital/formats/fdd/fdd000448.shtml ; https://www.iso.org/standard/63141.html
2. ISO 10303-11 EXPRESS / STEP-file structure. https://www.jsdai.net/support/about-step/step-file ; http://www.steptools.com/stds/step/
3. AP242 standard and editions. https://www.ap242.org/ap242-standard.html ; https://www.iso.org/standard/66654.html ; https://www.capvidia.com/blog/best-step-file-to-use-ap203-vs-ap214-vs-ap242
4. STEP Tools AP242 first-edition notes. http://www.steptools.com/docs/stp_aim/notes_ap242.html
5. ISO 10303-42 geometric/topological representation schema. https://www.steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/4_schema.htm
6. B-spline geometry module (ISO/TS 10303-1801). https://www.steptools.com/stds/smrl/data/modules/b_spline_geometry/sys/e_exp_arm.htm
7. STEP Tools entity: surface_curve. https://www.steptools.com/stds/stp_aim/html/t_surface_curve.html
8. STEP Tools entity: edge_curve. https://www.steptools.com/stds/stp_aim/html/t_edge_curve.html
9. OpenCASCADE STEP translator user guide. https://dev.opencascade.org/doc/overview/html/occt_user_guides__step.html
10. OpenCASCADE STEPControl_Reader / XSControl. https://dev.opencascade.org/doc/refman/html/class_s_t_e_p_control___reader.html
11. CAx Recommended Practices index (MBx-IF). https://www.mbx-if.org/home/cax/recpractices/
12. CAx-IF Geometric and Assembly Validation Properties v4.6. https://www.mbx-if.org/home/wp-content/uploads/2024/05/rec_prac_gvp_v46.pdf
13. OpenCASCADE STEPConstruct_ValidationProps. https://dev.opencascade.org/doc/refman/html/class_s_t_e_p_construct___validation_props.html
14. CAx-IF Tessellated 3D Geometry v1.1. https://www.mbx-if.org/home/wp-content/uploads/2024/05/rec_prac_3dtess_geo_v11.pdf
15. CAx-IF PMI Representation and Presentation v4.1. https://www.mbx-if.org/home/wp-content/uploads/2024/06/rec_pracs_pmi_v41.pdf
16. ruststep + espr. https://github.com/ricosjp/ruststep ; https://docs.rs/ruststep/latest/ruststep/
17. NIST STEPcode (SCL). https://github.com/stepcode/stepcode ; https://www.nist.gov/services-resources/software/step-class-library-scl
18. JSDAI STEP-file guide ; NIST STEP File Analyzer v4. https://www.jsdai.net/support/about-step/step-file ; https://nvlpubs.nist.gov/nistpubs/ams/NIST.AMS.200-4.pdf
19. STEP trimmed_curve / IFC IfcTrimmedCurve. https://www.steptools.com/stds/stp_aim/html/t_surface_curve.html ; https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifctrimmedcurve.htm
20. CAD interoperability defect / healing literature (trimmed-NURBS gaps, sliver faces, non-watertight models). USPTO patent corpus and CAD interoperability surveys.
