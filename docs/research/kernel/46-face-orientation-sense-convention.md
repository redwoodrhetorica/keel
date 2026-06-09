# 46. Face Orientation / Sense Convention: Unifying Keel's Two Construction Paths

## Title and Scope

This dossier resolves a load-bearing architectural inconsistency in Keel's face-orientation model. Keel is a radial-edge / partial-entity-structure (PES) non-manifold B-rep kernel edited through GWB Euler operators. A face references a surface, carries a `sense` boolean, owns loops of coedges ("fins"), and belongs to a region whose solidity says which side is material. The analytic `mass_properties` engine is a divergence-theorem surface integral that needs the solid-OUTWARD normal at every surface point. Today it derives that normal from REGION-SOLIDITY ALONE and ignores the face `sense` bool. Making it sense-aware fixes a genus-1 tube built by Euler operators (dossier 45) but REGRESSES the booleans, because the two producers populate orientation under two different, unreconciled conventions:

- **Euler-constructed faces** assign front/back by **FIN ORIENTATION**: the loop/coedge traversal direction is treated as authoritative, and the face normal is whatever the right-hand rule about the fin loop produces.
- **Boolean-constructed faces** assign front/back **NATURAL-NORMAL-BASED**: the surface's parametric (natural) normal is treated as authoritative, and the boolean keep/drop/reverse tables (dossier 39) operate on that outward normal directly.

Two inconsistent sources of truth for the same physical quantity. The mesh path (`mesh_volume`) is sense-based and is CORRECT on both construction paths, so Keel's geometry and topology are right: only the CONVENTION relating the four quantities {surface natural normal, sense bool, fin/loop orientation, region material side} is split between producers. This file states the single invariant production kernels use, documents how the four authoritative kernels encode it (OCCT, ACIS, Parasolid, STEP AP242), and gives an implementation-grade unification recipe for Keel: one canonicalization routine both producers call, one consumer helper every reader calls, the per-producer changes, and a debug-validator assertion that would have caught the split and prevents regression. It ties dossier 38 (the STEP XOR orientation algebra), dossier 39 (boolean sense bookkeeping), dossier 45 (the genus-1 Euler sequence, seams and poles), and dossier 01 (Euler operators / fins).

This is a DESIGN recommendation grounded in production-kernel practice, not a transcription of any kernel's source. The recommended convention must be validated against Keel's debug validator and its existing boolean and mass-property test suites before adoption.

---

## 1. THE CANONICAL INVARIANT

Production B-rep kernels all enforce ONE invariant. It binds four quantities so that any one of {sense bool, loop orientation, material side} can be derived from the other two plus the surface natural normal. State it precisely.

Let, at a regular (non-degenerate) point `p` of a face `F`:

- `n_nat(p)` = the surface's NATURAL parametric normal, the unit normal in the direction `dS/du x dS/dv` of the underlying surface's own parameterization. This is a property of the SURFACE alone, independent of the face.
- `s in {+1, -1}` = the face `sense`, written `+1` when `sense == FORWARD/same-sense/true` and `-1` when `sense == REVERSED/opposite-sense/false`.
- `n_out(p)` = the face's OUTWARD normal: the unit normal that points away from solid material on a single-sided face, i.e. into the void.
- The loop/fin orientation: the directed sequence of coedges bounding `F`.
- The material side: which side of `F` is solid (given by region-solidity in Keel).

**The invariant (three coupled clauses):**

1. **OUTWARD = SENSE times NATURAL.**
   `n_out(p) = s * n_nat(p)`.
   The face's outward normal is the surface's natural normal, flipped iff the sense bit is REVERSED. (OCCT: "If FACE orientation is FORWARD then its normal coincides with surface normal. If REVERSED then opposite to surface." ISO 10303-42 same_sense: TRUE = normals agree, FALSE = oppose.)

2. **MATERIAL IS OPPOSITE THE OUTWARD NORMAL.**
   Walking from `p` a small distance along `+n_out(p)` leaves the solid (enters void); walking along `-n_out(p)` enters material. (OCCT: "Shape material is always on a side opposite to FACE normal." Parasolid: "The normals of the faces in a solid body must point away from the solid region.") For an outer shell `n_out` points outward; for an inner-void shell `n_out` points INTO the void, which is still away from the solid. This is exactly what `mass_properties`' divergence integral needs.

3. **LOOPS RUN RIGHT-HANDED ABOUT THE OUTWARD NORMAL, MATERIAL ON THE LEFT.**
   Each loop traverses its coedges so that, viewed looking DOWN the outward normal (i.e. from the void toward the material, against `n_out`), the loop runs counterclockwise and keeps the face interior (hence material) on its LEFT. Equivalently, by the right-hand rule, the loop tangent `t` and the inward-to-interior direction `m` satisfy `t x m = n_out` along the boundary. (Parasolid: "The forward direction of a loop has the face on the left of the loop, when viewing the face down the face normal.") Outer loops are CCW about `n_out`; inner (hole) loops are CW about `n_out`.

**The single checkable relation** (this is what the validator asserts, section 3):

```
loop_implied_normal(F)  ==  s * n_nat(F)  ==  n_out(F)              [geometry/sense agreement]
dot( n_out(F, p) , (void_direction at p from region material side) ) > 0   [material agreement]
```

where `loop_implied_normal(F)` is the normal computed from the outer loop's winding by the right-hand rule (for an analytic or planar patch, the signed area normal of the projected loop; in general the surface normal at an interior sample, given consistently oriented coedges). The three expressions must be mutually consistent: the loop winding, the sense-times-natural product, and the region's void side all name the SAME outward normal. When they disagree, the model is malformed. This single triple-equality is the heart of every kernel's face-orientation discipline, and the heart of the fix below.

---

## 2. THE FOUR KERNELS, SIDE BY SIDE

Each production kernel enforces the section-1 invariant, but they differ in WHICH quantity is stored as the source of truth and which are derived. The decisive observation for Keel: the high-end commercial kernels make the SENSE-relative-to-surface the authoritative stored bit, and DERIVE the outward normal from it, while loop orientation is kept consistent as a co-equal invariant, not as the primary source.

### OpenCASCADE (OCCT): face Orientation is authoritative

- **Stored source of truth**: `TopoDS_Face::Orientation()`, a `TopAbs_Orientation` of `FORWARD` or `REVERSED`, interpreted RELATIVE TO the underlying `Geom_Surface`. The surface (`BRep_Tool::Surface`) supplies the natural normal; the face's Orientation flag is the only per-face datum that flips it.
- **Outward normal rule (derived)**: compute the surface natural normal `n_nat` from `Geom_Surface` (e.g. via `BRepLProp_SLProps`), then negate iff the face is REVERSED. The canonical OCCT idiom is literally `if (face.Orientation() == TopAbs_REVERSED) n *= -1;`. The OCCT user guide states the invariant directly: "Shape material is always on a side opposite to FACE normal. If FACE orientation is FORWARD then its normal coincides with a surface normal. If REVERSED then opposite to surface."
- **Wire/edge orientation (derived, kept consistent)**: edges carry their own FORWARD/REVERSED relative to their curve; a wire is oriented so the face material sits consistently. OCCT keeps loop orientation consistent with the face normal but the FACE Orientation bit, not the wire winding, is the stored primary. When BRepMesh tessellates, it emits triangle winding consistent with the face Orientation, and consumers must reverse triangle vertex order for REVERSED faces, confirming Orientation is the authority the mesh defers to.
- **Classification**: `BRepClass`/`BRepClass3d` solid classification relies on outward normals derived as above; "material opposite the normal" is the classifier's contract.
- **Authoritative = SENSE (face Orientation relative to surface).** Natural normal is geometry input; loop winding and material side are kept consistent but are derived/validated, not the stored primary.

### ACIS: FACE sense relative to SURFACE is authoritative

- **Stored source of truth**: each `FACE` carries a `sense` flag, FORWARD or REVERSED, relative to its `SURFACE`. "If the surface normal points away from the [solid], the sense of the face is forward"; a REVERSED face uses the surface with its normal flipped.
- **Outward normal rule (derived)**: face normal = surface natural normal if sense FORWARD, negated if sense REVERSED. Identical algebra to OCCT.
- **COEDGE sense relative to EDGE (separate, parallel flag)**: "A coedge records the occurrence of an edge in a loop of a face." Its sense flag says whether the coedge runs WITH the edge's curve direction (forward) or AGAINST it (reversed). This is the fin/half-edge direction, stored independently of the FACE sense.
- **LOOP orientation**: loops bound the face with material on a consistent side; coedge senses within a loop are arranged so the loop is consistent with the FACE normal. Loop direction is derived to match the face sense, not the primary store.
- **Double-sided faces (the non-manifold case Keel must handle)**: ACIS faces have a "sidedness". A SINGLE-SIDED face has material on one side (inside) and void on the other (outside): the section-1 invariant applies directly. A DOUBLE-SIDED face has material on both sides or neither (a lamina/sheet face, or a face embedded in solid such that "both sides are solid; the face normals point to the inside"). For double-sided faces there is NO unique outward normal: "away from material" is undefined because there is no single void side. ACIS marks such faces double-sided explicitly so consumers do not assume a material side. This is exactly Keel's PES lamina/sheet case.
- **Authoritative = SENSE (FACE sense relative to SURFACE)** for the face normal; COEDGE sense is a separate authoritative datum for fin direction. Material side is read from sidedness, not stored as the primary normal source.

### Parasolid: normal-away-from-material is the law, face/surface sense plus loop-on-left enforce it

- **The governing law (material side)**: "The normals of the faces in a solid body (face normals) must point away from the solid region. For faces of an outer shell, the normals point outwards. For an inner shell (a void inside a solid) the face normals point inwards, that is, still away from the solid." This is the section-1 clause 2, stated as Parasolid's defining face rule.
- **Face vs surface**: a face references a surface and carries a sense so the face normal (away from material) is the surface normal possibly negated, same algebra as OCCT/ACIS.
- **Loop/fin orientation rule (clause 3, stated explicitly)**: "The forward direction of a loop has the face on the left of the loop, when viewing the face down the face normal. A loop represents one boundary of a face as a closed set of fins, therefore the direction of the fin is the same as that of the loop that contains the fin." And edges take direction from their left fin: "an edge, which (on a manifold solid) has a left and right fin, takes its direction from the left fin (which takes its direction from its loop)."
- **Authoritative**: Parasolid presents the MATERIAL-AWAY normal as the defining invariant and keeps face sense and the face-on-left loop rule mutually consistent with it. In practice the face-normal/sense is the stored per-face orientation and the fin loop is constrained to agree (face on left down the normal). Parasolid's model is the cleanest statement that all three of {sense, loop-on-left, material-away} name one normal.

### STEP AP242 (ISO 10303-42): same_sense plus an XOR chain of flags

- **`advanced_face.same_sense : BOOLEAN`** (a `face_surface` from ISO 10303-42): "the sense of the surface normal agrees with (TRUE), or opposes (FALSE), the sense of the topological normal to the face." This is exactly `s = +1 if same_sense else -1`, with `n_face = s * n_nat`. The TOPOLOGICAL face normal (the one consistent with the loop winding) is the reference; same_sense relates the SURFACE normal to it.
- **The full XOR orientation algebra (dossier 38, section 8)**: the modeled outward normal and each coedge's traversal direction come from XOR-ing a chain of booleans:
  - `advanced_face.same_sense`: face normal vs surface natural normal.
  - `face_bound.orientation`: loop sense vs face (flip the loop if FALSE).
  - `oriented_edge.orientation`: this coedge's direction vs the edge.
  - `edge_curve.same_sense`: the 3D curve's natural direction vs the edge start-to-end direction.
  - Composite coedge-forward flag: `coedge_forward = oriented_edge.orientation XOR (NOT edge_curve.same_sense) XOR face_bound.orientation_flip`, and `n_out = (same_sense ? +1 : -1) * n_nat`, with the loop made CCW about `n_out`.
- **Authoritative**: STEP is a NEUTRAL EXCHANGE format, so it carries BOTH the same_sense bit AND the explicit loop/coedge orientation flags and REQUIRES them to be mutually consistent (the half-edge consistency invariant: adjacent coedges across one edge traverse it in opposite directions). STEP does not pick a single source of truth; it transmits a redundant, internally-consistent set. On IMPORT, dossier 38 reconstructs Keel's per-face orientation from this XOR chain. The lesson for Keel: STEP proves the invariant is the same one OCCT/ACIS/Parasolid use (sense-times-natural for the normal, loop CCW about it), expressed as flags that must agree.

### Verdict across the four

| Kernel | Stored primary for face normal | Outward normal derivation | Loop/fin role | Material side |
|---|---|---|---|---|
| OCCT | FACE Orientation (FORWARD/REVERSED) rel. surface | `n_nat` negated iff REVERSED | kept consistent (derived) | opposite the normal (contract) |
| ACIS | FACE sense rel. SURFACE | `n_nat` negated iff REVERSED | COEDGE sense separate; loops derived | sidedness flag; double-sided = no unique side |
| Parasolid | face sense (normal away from material) | `n_nat` negated to point away from material | face-on-left, fin = loop dir, edge = left fin | the defining law (away from material) |
| STEP | `same_sense` (+ full XOR chain, redundant) | `(same_sense?+1:-1) * n_nat` | explicit flags, must be consistent | implied by shell orientation |

**The clear answer to the key question:** production commercial kernels (OCCT, ACIS, Parasolid) make a per-face SENSE bit RELATIVE TO THE SURFACE the authoritative stored quantity for the face normal, and DERIVE the outward normal as `sense * natural`. They do NOT make raw loop/fin winding the primary store, nor region/material side: instead they keep loop orientation consistent with that sense as a co-invariant, and they treat material side as the physical CONSEQUENCE (normal points away from material), checked, not as the primary source. STEP, being neutral, carries both redundantly and demands consistency.

---

## 3. THE UNIFICATION RECIPE FOR KEEL (centerpiece)

### 3.1 Which quantity should be Keel's source of truth

Two honest candidates:

**(a) SENSE-PRIMARY (OCCT/ACIS/Parasolid precedent).** The per-face `sense` bool, relative to the surface natural normal, is authoritative. `outward = sense * natural`. Loops are kept consistent with that. Producer changes: the BOOLEAN path already thinks in natural/outward terms and barely changes; only the EULER path must be taught to SET `sense` explicitly (today it implies orientation from fin winding alone). Mass_properties becomes `sense * natural`, dotted against region-solidity for the void side.

**(b) FIN/LOOP-PRIMARY.** The loop/coedge winding plus region material side are authoritative; `sense` becomes a DERIVED CACHE: `sense = sign(dot(material_outward_normal, natural_normal))`. Producer changes: the EULER path is already fin-authoritative and barely changes; the BOOLEAN path must learn to ORIENT fins from its material classification (it currently does not maintain fin winding as primary).

**The trade-off is robustness versus code churn, and the two pull the same way here.** The surface NATURAL normal is fragile exactly where Keel hurts: it FLIPS sign under reparameterization, and it is UNDEFINED or zero at poles and degeneracies (sphere/cone apex, the genus-1 seam, the cases dossier 45 flags). Making `sense` (a bit tied to that fragile natural normal) the irreducible source of truth means a reparameterization or a regenerated NURBS surface can silently invert the stored authority. The LOOP winding and the REGION material side, by contrast, are TOPOLOGICAL and survive reparameterization and remain meaningful at poles (the loop still closes in UV even when the 3D loop pinches to a point, dossier 38/45). For a PES / Euler-operator kernel, whose native construction language IS fins and whose mesh path (already correct on both producers) IS material-side based, the durable primary is the fin/material side.

**Recommendation: FIN/MATERIAL-PRIMARY, with `sense` as a derived, canonicalized cache.** Concretely:

> The authoritative, stored orientation data are (1) the coedge/fin loop winding and (2) the region-solidity material side. The face `sense` bool is a DERIVED CACHE, recomputed by a single canonicalization routine as `sense = sign(dot(n_out_material, n_nat))`, and it exists only so that consumers (mass_properties, mesh, classification) have an O(1) way to get the outward normal as `sense * natural` WITHOUT re-running material classification at every query.

This matches the section-1 invariant (the three clauses are mutually consistent, so deriving `sense` from the loop/material pair reconstructs exactly the bit OCCT/ACIS/Parasolid would store), it minimizes churn (the Euler path, the harder of the two, is left fin-authoritative as it already is; the Boolean path must set fin winding, but it already computes the outward material normal it needs, dossier 39), and it makes the cache derive from the robust quantities and degrade gracefully where the natural normal does not exist (section 4). The one cost is that the Boolean path must now ESTABLISH fin winding consistent with its classification rather than only tracking natural normals; that is a localized, well-specified change, and dossier 39's keep/drop/reverse tables already produce the outward normal that determines the winding.

Note the practical equivalence: because the invariant ties all three together, SENSE-PRIMARY and FIN-PRIMARY produce the SAME stored `sense` value on a well-formed face. The choice is about which quantity the CANONICALIZER trusts when they disagree (i.e. which one is recomputed from the others). FIN/material-primary is recommended because it trusts the quantities that remain well-defined under reparameterization and at degeneracies.

### 3.2 The single CANONICALIZATION routine (both producers call this)

Every producer, after building or modifying a face's topology, calls ONE routine that establishes the invariant and writes the derived `sense` cache. Neither producer hand-sets `sense` anywhere else.

```rust
/// Establish the section-1 invariant on `face` and refresh its derived `sense` cache.
/// `material_outward_hint`: a unit vector known (from region-solidity or the
/// producer's material classification) to point AWAY from solid material at the
/// sample point `p`. For the Euler path the hint comes from the region's solid side;
/// for the Boolean path it comes from the two-sided winding-number test (dossier 39).
fn canonicalize_face_orientation(face: &mut Face, p: Point3, material_outward_hint: Vec3) {
    // 1. Natural normal of the underlying surface at p (may be ill-defined at poles; see sec.4).
    let n_nat = face.surface.natural_normal_at(p);          // unit, surface parameterization

    // 2. Robust outward normal: prefer the loop-implied normal; fall back to the hint.
    //    loop_implied_normal uses the outer-loop winding by the right-hand rule and is
    //    well-defined even where n_nat is not (it is topological, not parametric).
    let n_loop = face.outer_loop_implied_normal(p);          // from fin winding, RH rule
    let n_out  = orient_toward(n_loop, material_outward_hint); // flip n_loop if it disagrees
                                                              // with the known void side

    // 3. Make the fin loops consistent with n_out: outer loop CCW about n_out
    //    (material on the left down n_out), inner loops CW. Flip coedge order/senses
    //    of any loop whose winding disagrees. This is the authoritative store.
    face.orient_loops_about(n_out);

    // 4. Derive and cache the sense bit (the ONLY place sense is written):
    //    sense = +1 (FORWARD/same) iff n_out agrees with the surface natural normal.
    face.sense = if n_nat.is_well_defined() {
        Sense::from_sign(dot(n_out, n_nat))                 // sign(dot(n_out, n_nat))
    } else {
        // pole/degenerate: derive sense from an adjacent regular sample of the same face
        // (sec.4), never leave it stale.
        face.sense = derive_sense_from_regular_sample(face, n_out);
        face.sense
    };
}
```

`orient_toward(v, hint)` returns `v` if `dot(v, hint) > 0` else `-v`. `Sense::from_sign` maps `+ -> FORWARD/same`, `- -> REVERSED/opposite`.

### 3.3 The single CONSUMER helper (every reader calls this, never re-derives)

```rust
impl Face {
    /// The solid-OUTWARD unit normal at p: sense times the surface natural normal.
    /// This is the ONLY way any consumer obtains face orientation. mass_properties,
    /// mesh, and point classification all call this; none re-derive from region,
    /// loop, or surface independently.
    fn outward_normal(&self, p: Point3) -> Vec3 {
        let n_nat = self.surface.natural_normal_at(p);
        match self.sense {
            Sense::Forward  =>  n_nat,     // +1 * natural
            Sense::Reversed => -n_nat,     // -1 * natural
        }
        // at poles where n_nat is ill-defined, natural_normal_at returns the
        // limiting iso-normal per sec.4 so this stays continuous.
    }
}
```

`mass_properties` integrates `dot(F(x), outward_normal(p))` over each face; `mesh_volume` already does the equivalent with sense; classification rays test against `outward_normal`. With this helper, mass_properties STOPS deriving orientation from region-solidity alone (the original bug): it now uses `sense * natural`, where `sense` was canonicalized FROM region-solidity plus the loop, so region-solidity still feeds in, but through the single canonical bit, consistently with the booleans.

### 3.4 Exact change to each producer

- **Euler path** (today: fin-authoritative, sense ignored). KEEP fin winding as the authoritative store. ADD a call to `canonicalize_face_orientation(face, p, region_void_dir)` after each Euler op that creates or reorients a face (`mvfs`, `mev`/`mef` that close a face, `kemr`, etc., per dossiers 01/44/45). The hint is the region's void direction from region-solidity. Net effect: every Euler-built face now has a CORRECT derived `sense` cache instead of an unset/stale one. The genus-1 tube's inner-band faces get `sense = REVERSED` where their natural normal points into the solid, which is precisely what makes mass_properties correct.

- **Boolean path** (today: natural-normal-authoritative, fins not maintained as primary). After the keep/drop/reverse tables (dossier 39) decide each kept fragment's outward normal `n_out`, CALL `canonicalize_face_orientation(face, p, n_out)` with the classification's outward normal as the hint. This (a) ORIENTS the fins to agree with `n_out` (the new work the boolean path must do, but `n_out` is already in hand from the two-sided winding test), and (b) writes the SAME derived `sense` cache by the SAME formula. For the difference operation, where B's inner wall is kept with REVERSED normals (dossier 39), the hint is the reversed normal, so canonicalization flips both the fins and the sense cache coherently. Net effect: boolean faces now carry a consistent fin winding AND the same `sense` cache as Euler faces, computed identically.

Both producers now establish the IDENTICAL invariant through the IDENTICAL routine. There is exactly one place `sense` is written and exactly one place outward normal is read.

### 3.5 THE VALIDATOR INVARIANT (the assertion that prevents regression)

Add to Keel's debug validator, run after every Euler op and every boolean (the same validator that already checks Euler-Poincare). For EVERY face `F`, at a regular sample point `p`:

```rust
// (A) loop winding, sense*natural, and the cached outward normal all name ONE normal:
let n_nat  = F.surface.natural_normal_at(p);
let n_out  = F.outward_normal(p);                  // = sense * natural (the cache)
let n_loop = F.outer_loop_implied_normal(p);       // from fin winding, RH rule
debug_assert!(dot(n_out, n_nat) * F.sense.sign() > 0.0,
    "sense cache disagrees with sense*natural");   // tautology check on the cache
debug_assert!(dot(n_out, n_loop) > 0.0,
    "loop winding disagrees with outward normal (fin/normal split)");

// (B) outward normal agrees with region material side (points away from solid):
let void_dir = F.region().void_direction_at(p);    // from region-solidity
debug_assert!(dot(n_out, void_dir) > 0.0,
    "outward normal points into material (region/orientation split)");

// (C) half-edge consistency: each manifold edge's two coedges traverse it oppositely
for e in F.edges() { debug_assert!(e.coedges_traverse_oppositely()); }
```

Assertion (A second clause) is the one that would have caught the original bug: on the Euler path the loop winding said one thing while a natural-normal-derived sense (had the booleans' convention leaked in) said another. Assertion (B) is the one that catches the mass_properties / region-solidity mismatch. Together they pin all three clauses of the section-1 invariant on every face after every operation. For DOUBLE-SIDED / lamina faces (section 4), skip clause (B) (no unique material side) and assert only (A) and (C) against the face's chosen reference side.

### 3.6 Why this dissolves BOTH bugs (one shared helper)

The genus-1 mass_properties bug and the tilted-cut stitch-sense bug share a root: a face whose stored/derived orientation did not match its true solid-outward normal, consumed by an integrator that trusted the wrong quantity.

- **Genus-1 tube**: the inner-band faces are reversed-sense (the cavity wall, the canonical reversed case). Mass_properties derived orientation from region-solidity alone and got the inner-band normal's SIGN wrong; making it sense-aware via `outward_normal()` fixes it, BUT only once `sense` is canonicalized consistently (so it does not regress booleans). The fix is: Euler path canonicalizes -> inner band gets `sense = REVERSED` -> `outward_normal()` returns the true inward-pointing (away-from-solid) normal -> divergence integral is correct.

- **Tilted-cut boolean (OPEN bug)**: a non-45-degree planar difference cut gives `mass_properties != mesh_volume` from a "stitch sense" issue on the tilted cut face. Under the difference operation, the kept B-inner-wall face must be REVERSED (dossier 39). If the boolean path established that fragment's orientation from the natural normal but the stitch wrote a fin winding inconsistent with it (the two-convention split), the cut face's stored orientation and its loop winding disagree, and mass_properties (natural-normal side) and mesh_volume (sense/fin side) diverge. Canonicalizing the boolean output via the same routine forces the cut face's fins, sense cache, and outward normal into agreement, so both integrators read the same normal.

**Yes, a single shared "given a face, return its solid-outward normal" helper collapses both fixes into one.** Both bugs are the SAME divergence-integrand-sign error read through two conventions. Once `outward_normal()` is the sole orientation source for mass_properties AND the mesh AND classification, and once `canonicalize_face_orientation` is the sole writer of the `sense` cache for BOTH producers, there is no second convention left to disagree. The genus-1 fix and the tilted-cut fix become the same one-line change at the consumer (use `outward_normal()`) plus the same one-routine change at each producer (call `canonicalize_face_orientation`). The validator invariant (3.5) then guarantees no third producer or consumer can reintroduce a split.

---

## 4. DEGENERACY AND EDGE CASES

- **Poles / degenerate faces (sphere or cone apex; the degeneracies dossier 45 flags).** The natural normal `n_nat` is undefined or zero where the surface pinches to a point. The fix's robustness comes precisely from NOT making `sense`-times-`n_nat` the irreducible authority: the LOOP winding remains well-defined (the parametric loop closes along an iso-edge at the pole even though the 3D loop collapses, dossier 38/45), so `loop_implied_normal` and the region void side still give `n_out`. `natural_normal_at(p)` must return the LIMITING iso-normal (the one-sided limit along the surface toward the pole) so `outward_normal()` stays continuous; `canonicalize_face_orientation` derives `sense` from a nearby REGULAR sample of the same face when `n_nat` is ill-defined (the `derive_sense_from_regular_sample` branch). Never leave `sense` stale at a pole.

- **Periodic / seam faces (the genus-1 tube seam, dossier 45).** A single periodic face wraps and its seam edge appears TWICE in one loop (dossier 38 section 8, dossier 45 Q3 per-band seam). The natural normal is continuous across the seam, so `sense` is single-valued for the whole face; the two coedges on the seam carry opposite traversal (half-edge consistency, validator C). Canonicalization treats the periodic face as one face with one `sense`; the seam does not split orientation. The per-band seam structure (dossier 45: each band a periodic patch with its own seam, adjacent bands share a latitude edge) means each band face is canonicalized independently, all consistent.

- **Double-sided / non-manifold sheet faces (PES lamina).** A lamina face has material on NEITHER side (a free sheet) or, embedded in solid, on BOTH sides (ACIS "double-sided, both-inside"). There is no unique "away from material" direction, so clause 2 of the invariant (material opposite the normal) does not apply and the section-3.5 validator skips clause (B) for such faces. Keel must FLAG these faces double-sided (as ACIS does) so `outward_normal()` and mass_properties do not assume a void side. For a sheet body, mass_properties (volume) is not meaningful; surface area is, and uses the natural normal magnitude, not its sign. `sense` on a double-sided face is still well-defined relative to the surface (it orients the chosen reference side for the fin winding) but carries no material-side meaning. This is the one case where the FIN/material-primary recommendation needs the explicit double-sided flag to avoid asserting a nonexistent material side.

- **Reversed-sense faces generally (the canonical exposing case).** A cavity wall, an inner-void shell, the genus-1 inner band: these are the faces where `sense = REVERSED`, `n_out = -n_nat`, and the outward normal points INTO the void (away from the surrounding solid). They are the entire reason the bug exists: a consumer that ignores `sense` (the original mass_properties) gets every reversed face's normal backwards. The unification makes the reversed case ordinary: `outward_normal()` returns `-n_nat` for them automatically, the validator confirms it points away from material, and both producers reach it through one routine.

---

## Per-Source Entries

### S1. OpenCASCADE user guide + forum: face Orientation as the outward-normal authority
- **Citation**: Open CASCADE Technology, "Modeling Data" user guide and developer forum threads "TopoDS_Face with REVERSED orientation," "face normal direction," "Consistency of TopoDS_Face normals." https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html ; https://dev.opencascade.org/content/face-normal-direction ; https://dev.opencascade.org/content/topodsface-reversed-orientation
- **Content**: "Shape material is always on a side opposite to FACE normal. If FACE orientation is FORWARD then its normal coincides with a surface normal. If REVERSED then opposite to surface." Canonical idiom: `if (face.Orientation() == TopAbs_REVERSED) n *= -1;`. Orientation is the generalized sense-of-direction (TopAbs_Orientation = FORWARD/REVERSED), a relation between a shape and its underlying geometry; the face's Orientation flag is the per-face source of truth for the outward normal, derived as natural-normal-negated-iff-REVERSED. BRepMesh emits triangle winding consistent with face Orientation; consumers reverse triangle vertices for REVERSED faces.
- **Kernel relevance**: the cleanest single-sentence statement of Keel's section-1 invariant, and the precedent that a per-face sense-relative-to-surface bit is authoritative for the outward normal. Directly models Keel's `outward_normal()` helper.

### S2. ACIS Model Topology: FACE sense, COEDGE sense, single/double-sided faces
- **Citation**: Spatial ACIS documentation (R17 / FCG), "Model Topology > Faces," "COEDGE," "Faces" (Chapter 6 Model Topology). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mtface.htm ; http://www-isl.ece.arizona.edu/ACIS-docs/PDF/FCG/06TOPO.PDF ; http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/29CLC/0002.HTM
- **Content**: A FACE has a `sense` (FORWARD/REVERSED) relative to its SURFACE; "if the surface normal points away from the [solid], the sense of the face is forward." A COEDGE "records the occurrence of an edge in a loop of a face"; its sense says whether it runs with (forward) or against (reversed) the edge curve, enabling an edge to occur in one, two, or more faces (sheets and non-manifold). Faces have SIDEDNESS: single-sided (material inside, void outside) vs double-sided (material on both sides, e.g. a face embedded in a solid sphere "both-inside, the face normals point to the inside," or a lamina with material on neither side). Loops bound the face with coedge senses consistent with the face normal.
- **Kernel relevance**: ACIS's separation of FACE sense (face normal) from COEDGE sense (fin direction) maps exactly onto Keel's `sense` cache vs fin winding; the double-sided face notion is Keel's PES lamina case and dictates the validator's clause-(B) skip in section 4.

### S3. Parasolid Functional Description: normals away from material, loop-on-left, fin and edge direction
- **Citation**: Siemens Parasolid v12.0 / v35 Functional Description, "Model Structure" (chapter 4). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.04.html ; http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.04.html ; XT format reference http://www.13thmonkey.org/documentation/CAD/Parasolid-XT-format-reference.pdf
- **Content**: "The normals of the faces in a solid body (face normals) must point away from the solid region. For faces of an outer shell, the normals point outwards. For an inner shell (a void inside a solid) the face normals point inwards, that is, still away from the solid." "The forward direction of a loop has the face on the left of the loop, when viewing the face down the face normal." "A loop represents one boundary of a face as a closed set of fins, therefore the direction of the fin is the same as that of the loop that contains the fin." "An edge, which (on a manifold solid) has a left and right fin, takes its direction from the left fin (which takes its direction from its loop)."
- **Kernel relevance**: the q-solid mirror is the de-facto public Parasolid spec and Keel's closest design target. It states the section-1 invariant's clause 2 (material-away) and clause 3 (face-on-left, fin=loop, edge=left-fin) verbatim, grounding Keel's fin/material-primary recommendation in the closest peer kernel.

### S4. ISO 10303-42 same_sense via IfcFaceSurface restatement
- **Citation**: buildingSMART IFC4 schema, IfcFaceSurface (SameSense), adapted from ISO 10303-42 face_surface; ISO 10303-42:2021 geometric and topological representation. https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD1/HTML/schema/ifctopologyresource/lexical/ifcfacesurface.htm ; https://www.steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/4_schema.htm
- **Content**: SameSense "indicates whether the sense of the surface normal agrees with (TRUE), or opposes (FALSE), the sense of the topological normal to the face." The topological face normal (consistent with loop winding) is the reference; same_sense relates the surface natural normal to it. The face geometry surface must contain the domains of all bounding vertices and edge curves.
- **Kernel relevance**: the neutral-standard statement of `n_out = (same_sense ? +1 : -1) * n_nat`, identical algebra to OCCT/ACIS/Parasolid, confirming the invariant is universal. It is the bit Keel's STEP importer (dossier 38) reconstructs.

### S5. STEP AP242 XOR orientation algebra (Keel dossier 38, section 8)
- **Citation**: Keel dossier `kernel/38-step-ap242-import.md`, section 8 "Orientation, Seams, and Degeneracies," verified against ISO 10303-42 (advanced_face, face_bound, oriented_edge, edge_curve). https://www.steptools.com/stds/step/
- **Content**: Four booleans combine: `advanced_face.same_sense` (face vs surface natural normal), `face_bound.orientation` (loop vs face), `oriented_edge.orientation` (coedge vs edge), `edge_curve.same_sense` (curve direction vs edge start-to-end). Composite: `coedge_forward = oriented_edge.orientation XOR (NOT edge_curve.same_sense) XOR face_bound.orientation_flip`; `n_out = (same_sense?+1:-1) * n_nat` with loops CCW about `n_out`. Half-edge consistency invariant: adjacent coedges across an edge traverse it oppositely.
- **Kernel relevance**: shows STEP carries a REDUNDANT, internally-consistent flag set rather than one source of truth, and that import must reconstruct Keel's single orientation. It defines the validator's clause (C) and the seam handling for the genus-1 case.

### S6. Keel boolean sense bookkeeping (dossier 39)
- **Citation**: Keel dossier `kernel/39-coincident-tangent-face-booleans.md`, sections 1-2 (keep/drop/orient tables; two-sided neighborhood evaluation).
- **Content**: "Face normals point outward (away from solid material); a kept face's normal must point out of the result solid." Same-sense vs opposite-sense of coincident faces is `sign(dot(n_A, n_B))` on the overlap. Difference keeps B's inner wall WITH REVERSED normals (the cavity's new outward boundary); same-sense coincident overlap survives union/intersection, opposite-sense survives difference. Outward normal at a fragment comes from the two-sided winding-number test.
- **Kernel relevance**: proves the BOOLEAN producer already computes the outward material normal it needs to feed `canonicalize_face_orientation` as the hint, so the only new work is orienting fins to match. The reversed-normal difference case is exactly the tilted-cut bug's locus.

### S7. Keel genus-1 Euler sequence, seams, poles (dossier 45)
- **Citation**: Keel dossier `kernel/45-genus1-solid-of-revolution-euler-sequence.md`, Q3 (seam/periodicity), Q5 (degeneracies).
- **Content**: per-band seam is the correct structure (each band a periodic patch with its own seam edge; adjacent bands share a latitude edge and a seam vertex); the parametric loop closes at poles even where the 3D loop pinches. Mixed pole/ring profiles branch on `radius == 0`. The inner-band (cavity wall) faces are the reversed-sense faces that expose the mass_properties bug.
- **Kernel relevance**: supplies the degeneracy and seam cases section 4 must handle and identifies the reversed inner-band faces as the canonical bug-exposing case the unification fixes.

### S8. Mantyla, An Introduction to Solid Modeling (half-edge, Euler operators, face/loop orientation)
- **Citation**: Martti Mantyla, "An Introduction to Solid Modeling," Computer Science Press, 1988. Companion course notes: https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler-op.html ; full-text scan http://www.cad.zju.edu.cn/home/zhx/GM/015/00-ism.pdf
- **Content**: the half-edge boundary model and the proof (Mantyla 1984) that Euler operators are a complete set for manifold solids. A face's outer loop is added with the face (L and F increment together, canceling in L - F of Euler-Poincare). Loop orientation (CCW outer, CW inner about the face normal) encodes which side is interior; the half-edge (fin) direction is the primary topological store in the half-edge model.
- **Kernel relevance**: the academic foundation for Keel's Euler/fin-authoritative path and for treating fin winding as the durable primary, with `sense` derived. Grounds the fin/material-primary recommendation in the standard manifold-modeling theory.

### S9. Weiler, The Radial Edge Structure (non-manifold, oriented uses)
- **Citation**: Kevin Weiler, "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling," in Geometric Modeling for CAD Applications, North-Holland, 1988, pp. 3-36. Survey context: ACM DL 10.1145/304012.304042.
- **Content**: generalizes Baumgart's winged-edge to non-manifold geometry by recording the radial ordering of face-uses around an edge. Key distinction: an abstract unoriented entity (edge, face) versus an ORIENTED USE of it (edge-use/coedge, face-use). Each side of a face has a face-use; every edge carries pairs of uses equal to the pairs of face-uses meeting it.
- **Kernel relevance**: the theoretical basis of Keel's PES radial-edge model; the face-use / edge-use (fin) abstraction is exactly where Keel stores fin orientation. Justifies double-sided faces having two face-uses (section 4 lamina case) and the fin as the authoritative oriented use.

### S10. Lee and Lee, Partial Entity Structure (PES, Keel's actual topology)
- **Citation**: Sang Hun Lee and Kunwoo Lee, "Partial Entity Structure: A Compact Boundary Representation for Non-Manifold Geometric Modeling," J. Comput. Inf. Sci. Eng., 1(4):356-365, 2001. https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
- **Content**: a compact non-manifold B-rep storing partial topological entities (partial-face, partial-edge, partial-vertex) with orientation, reducing the storage of the full radial-edge structure while preserving non-manifold adjacency and oriented uses.
- **Kernel relevance**: Keel's named data structure. Confirms orientation lives on the partial (use) entities, i.e. fins, consistent with fin/material-primary; the `sense` cache sits on the face entity as a derived convenience.

### S11. Stroud, Boundary Representation Modelling Techniques (orientation by normal or edge order)
- **Citation**: Ian Stroud, "Boundary Representation Modelling Techniques," Springer, 2006 (ISBN 1-84628-312-4). https://link.springer.com/content/pdf/10.1007/978-1-84628-616-2.pdf
- **Content**: "the orientation of a face can be represented by associating a normal with it or by associating an order in the list of edges that define the face." A face is defined by loops; each loop by one or more edges. The two representations (explicit normal vs edge ordering) are interchangeable and must be kept consistent, the exact duality between Keel's `sense` cache and its fin winding.
- **Kernel relevance**: states the design choice Keel faces (normal-primary vs edge-order/fin-primary) as the textbook duality, supporting the recommendation that one is the store and the other a consistent derivation, with a canonicalizer enforcing agreement.

### S12. CAx-IF / OCCT classification and shell orientation context
- **Citation**: OCCT BRepClass3d solid classification and BOPTools_AlgoTools::OrientFacesOnShell; "Normals to Outside" forum. https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_algos.html ; https://dev.opencascade.org/content/normals-outside
- **Content**: solid point classification depends on consistently outward face normals; OrientFacesOnShell flips faces so a shell's normals are globally consistent (outward for the outer shell). The classifier's contract is "material opposite the normal," identical to section 1.
- **Kernel relevance**: confirms the THIRD consumer (classification), beyond mass_properties and mesh, must also read the single `outward_normal()` helper, closing the loop that one helper serves all consumers and one canonicalizer serves all producers.

---

## Recommended face-orientation unification for Keel (synthesis)

**The single canonical invariant.** At every regular face point: `n_out = sense * n_nat`; material lies opposite `n_out` (normal points away from solid, into void); the outer loop runs CCW about `n_out` with material on the left (inner loops CW). The three named quantities, loop winding, `sense * natural`, and the region void side, must all name the same `n_out`.

**Which quantity production kernels make authoritative.** OCCT, ACIS, and Parasolid all make a per-face SENSE bit RELATIVE TO THE SURFACE the stored authority for the face normal, deriving `n_out = sense * natural`, and keep loop/fin winding consistent as a co-invariant; material side is the physical consequence (normal away from material), enforced not stored. STEP carries both redundantly and demands consistency.

**Recommendation for Keel: FIN/MATERIAL-PRIMARY, sense as a derived cache.** For a PES / Euler-operator kernel whose native language is fins and whose correct mesh path is already material-side based, store the fin winding and region material side as authoritative and DERIVE `sense = sign(dot(n_out_material, n_nat))`. Reason: the surface natural normal flips under reparameterization and is undefined at poles/seams (dossier 45), so the irreducible authority must be the topological quantities that survive those cases; this also leaves the harder Euler path essentially unchanged (it is already fin-authoritative). The only new producer work is teaching the boolean path to orient fins from the outward normal it already computes (dossier 39). On a well-formed face this yields the SAME `sense` value the sense-primary kernels would store; the difference is only which quantity is recomputed when they disagree, and the robust quantities win.

**The two routines.** One PRODUCER routine, `canonicalize_face_orientation(face, p, material_outward_hint)`, the sole writer of `sense`: it orients the fins about the material-outward normal and caches `sense = sign(dot(n_out, n_nat))` (deriving from a regular sample at poles). One CONSUMER helper, `face.outward_normal(p) = sense * natural`, the sole orientation source for mass_properties, mesh, and classification.

**The validator invariant.** After every op, for every face: `sign(dot(n_out, n_nat)) == sense` AND `dot(n_out, loop_implied_normal) > 0` AND `dot(n_out, region_void_dir) > 0` AND each manifold edge's two coedges traverse it oppositely (skip the region-void clause for flagged double-sided/lamina faces). The second clause catches the fin/normal split; the third catches the region/orientation split.

**One helper collapses both fixes.** The genus-1 mass_properties bug (reversed inner-band faces) and the tilted-cut stitch-sense bug are the same divergence-integrand-sign error read through two conventions. Once `outward_normal()` is the only orientation source and `canonicalize_face_orientation` the only `sense` writer for both producers, no second convention remains to disagree, and both bugs close with the same one-line consumer change plus the same one-routine producer change. The validator then prevents any future producer or consumer from reintroducing the split.

**Caveat.** This is a design recommendation grounded in OCCT/ACIS/Parasolid/STEP practice, not a transcription of any kernel's source. Validate it against Keel's debug validator and the existing boolean and mass-property test suites (especially the genus-1 tube and the tilted-cut difference) before committing the convention.

---

## References

1. Open CASCADE Technology, "Modeling Data" and "Modeling Algorithms" user guides; forum threads on TopoDS_Face orientation and face normals. https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html ; https://dev.opencascade.org/content/face-normal-direction ; https://dev.opencascade.org/content/topodsface-reversed-orientation ; https://dev.opencascade.org/content/normals-outside
2. Spatial ACIS R17 / FCG documentation, "Model Topology > Faces," "COEDGE," Chapter 6 Model Topology. http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mtface.htm ; http://www-isl.ece.arizona.edu/ACIS-docs/PDF/FCG/06TOPO.PDF
3. Siemens Parasolid Functional Description / Overview, "Model Structure" (chapter 4), v12.0 and v35; Parasolid XT format reference. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.04.html ; http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.04.html ; http://www.13thmonkey.org/documentation/CAD/Parasolid-XT-format-reference.pdf
4. ISO 10303-42 geometric and topological representation (advanced_face same_sense), via buildingSMART IfcFaceSurface and STEP Tools SMRL. https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD1/HTML/schema/ifctopologyresource/lexical/ifcfacesurface.htm ; https://www.steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/4_schema.htm
5. Keel dossier 38, STEP AP242 import, section 8 (orientation XOR algebra). `docs/research/kernel/38-step-ap242-import.md`
6. Keel dossier 39, coincident/tangent face booleans (keep/drop/orient sense tables). `docs/research/kernel/39-coincident-tangent-face-booleans.md`
7. Keel dossier 45, genus-1 solid-of-revolution Euler sequence (seams, poles, reversed inner band). `docs/research/kernel/45-genus1-solid-of-revolution-euler-sequence.md`
8. Martti Mantyla, An Introduction to Solid Modeling, Computer Science Press, 1988. https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler-op.html
9. Kevin Weiler, The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling, in Geometric Modeling for CAD Applications, North-Holland, 1988, pp. 3-36.
10. Sang Hun Lee and Kunwoo Lee, Partial Entity Structure: A Compact Boundary Representation for Non-Manifold Geometric Modeling, J. Comput. Inf. Sci. Eng., 1(4):356-365, 2001. https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
11. Ian Stroud, Boundary Representation Modelling Techniques, Springer, 2006. https://link.springer.com/content/pdf/10.1007/978-1-84628-616-2.pdf
12. B. G. Baumgart, Winged-edge polyhedron representation, Stanford AI Report CS-320, 1972 (winged-edge antecedent to the half-edge / radial-edge model).
