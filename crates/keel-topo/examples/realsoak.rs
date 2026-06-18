//! Realistic-workflow kernel soak (decline-frontier): instead of the random
//! two-primitive booleans of `evolve.rs`, this samples the LEARNED GRAMMAR of
//! real Plasticity (CAD app) modeling sessions and runs LONG arc-following
//! op-chains (generated median ~14, mean ~24 ops) on a GROWING solid -- the
//! distribution that finds real-usage bugs. Each project is executed against
//! the kernel and classified by the SAME three-bucket oracle posture as
//! `evolve.rs` (PASS / DECLINE / WRONG), under the SACRED INVARIANT
//! decline-never-wrong: a body must satisfy mass==mesh (curved-aware band) or
//! be DECLINED; a wrong body is never silently accepted as a PASS.
//!
//!   The model (baked into the generator below), distilled from 113
//!   Plasticity tutorial transcripts:
//!     * ops/project: median 14, mean 24 (LONG chains -- the whole point;
//!       chain_len() samples a lognormal matching both).
//!     * start-op: form (sketch/extrude) ~28%, boolean 13%, fill 7%, loft 6%.
//!     * workflow ARC (0=start..1=end): form first (extrude ~0.47), then
//!       combine (boolean/loft/trim ~0.50), then detail/FINISH last
//!       (mirror/fillet ~0.53, fill/shell ~0.55, array ~0.57). Op selection
//!       is biased toward each op's mean arc position.
//!     * end-op: fillet 28%, fill 17%, boolean 14%, chamfer 13%.
//!     * grammar P(next|cur): fillet<->chamfer, extrude->boolean,
//!       boolean->fillet, fill<->loft, sketch->extrude, fillet->boolean,
//!       fillet->extrude, boolean->extrude, mirror->boolean, chamfer->fill
//!       (encoded in transition_table()).
//!     * geometry: feature 5..100 mm (median 20); fillet/chamfer radii SMALL
//!       vs the local feature (2..8%); booleans combine an overlapping
//!       primitive offset by a fraction of its size.
//!
//! Op -> kernel mapping: a primitive seed; Extrude/Sketch == union with an
//! overlapping tool (add form); Boolean == union/diff/intersect with a tool;
//! Fillet/Chamfer pick a real blendable edge; Mirror == mirror+union; Shell ==
//! hollow. Loft/Sweep/Fill/Array/Trim have no clean realization on an
//! arbitrary solid and DECLINE by design (recorded, not crashed). A declined
//! op is SKIPPED and the chain CONTINUES (a modeler adjusts past a feature
//! that will not build) -- this is what yields the LONG realistic chains.
//!
//! Oracle (per produced body): validate(); then the kernel's exact analytic
//! `mass_properties` is checked against (1) an INDEPENDENT volume interval the
//! generator tracks across the chain -- out-of-bound => WRONG; and (2) the
//! tessellated `mesh_volume` -- for an ALL-PLANAR body the mesh is exact so a
//! gap above 2% is WRONG, for a CURVED body a small gap is a tessellation note
//! (the mesh is the coarse party) but a gross gap above 25% is still WRONG.
//! WRONG is the decline-never-wrong violation class.
//!
//! Project dispositions: PASS (ran to the end, >=1 op realized, never WRONG);
//! STALL (abandoned after too many consecutive declines); WRONG (a violation);
//! TIMEOUT (a perf cliff, reported separately from correctness); SKIP (seed
//! build failed).
//!
//! Outputs (deterministic: a seed regenerates a project; only result ARRIVAL
//! order interleaves under the parallel pool -- every line carries its seed):
//!   passes.jsonl     -- one line per PASS project.
//!   declines.jsonl   -- every declined op (seed, prevOp, op, fault): the
//!                       minimize-declines worklist.
//!   failures.jsonl   -- the decline-never-wrong (WRONG) violations + repro.
//!   projects.jsonl   -- per-project summary (seed, seed-prim, realized,
//!                       attempted, declined, signature).
//!
//! Usage:
//!   cargo run --release -p keel-topo --example realsoak -- [projects] [seed] [outdir]
//!   KEEL_REPRO=<project-seed> cargo run ... --example realsoak   # verbose single-project repro
//! Defaults: 10000 projects, seed 1, realsoak-out/.
#![allow(clippy::unwrap_used)]
// `received % 1000 == 0` reads clearer than .is_multiple_of and keeps MSRV low.
#![allow(clippy::manual_is_multiple_of)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolFault, BoolOp, boolean};
use keel_topo::entity::AnyKey;
use std::io::Write as _;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Deterministic 64-bit LCG (no rand / no clock: a seed reproduces a run).
// Identical to evolve.rs's generator so seeds are comparable.
// ---------------------------------------------------------------------------
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    fn pick(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    /// Approx standard normal via the central-limit sum of 6 uniforms.
    fn gauss(&mut self) -> f64 {
        let s: f64 = (0..6).map(|_| self.f()).sum();
        s - 3.0
    }
    /// Weighted choice over `weights`; returns the chosen index.
    fn weighted(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().sum();
        let mut t = self.f() * total;
        for (i, &w) in weights.iter().enumerate() {
            if t < w {
                return i;
            }
            t -= w;
        }
        weights.len() - 1
    }
}

// ---------------------------------------------------------------------------
// The op vocabulary. A subset maps to real kernel calls; the rest are the
// CAD ops the kernel cannot do on an arbitrary solid yet (graceful decline).
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Extrude, // == "form": here a fresh primitive seed (block/cyl/cone/sphere)
    Boolean, // union / difference / intersection with an overlapping primitive
    Fillet,  // round a real edge
    Chamfer, // chamfer a real edge
    Mirror,  // mirror + union across a plane
    Shell,   // hollow to a wall thickness
    Sketch,  // grammar node; realizes as a re-seed (a new profile)
    Loft,    // free-form: declines on an arbitrary solid
    Sweep,   // free-form: declines on an arbitrary solid
    Fill,    // free-form surface patch: declines on an arbitrary solid
    Array,   // pattern: declines (no array op on a solid yet)
    Trim,    // declines (no standalone trim on a solid)
}

impl Op {
    const ALL: [Op; 12] = [
        Op::Extrude,
        Op::Boolean,
        Op::Fillet,
        Op::Chamfer,
        Op::Mirror,
        Op::Shell,
        Op::Sketch,
        Op::Loft,
        Op::Sweep,
        Op::Fill,
        Op::Array,
        Op::Trim,
    ];
    fn idx(self) -> usize {
        Op::ALL.iter().position(|&o| o == self).unwrap()
    }
    fn tag(self) -> &'static str {
        match self {
            Op::Extrude => "extrude",
            Op::Boolean => "boolean",
            Op::Fillet => "fillet",
            Op::Chamfer => "chamfer",
            Op::Mirror => "mirror",
            Op::Shell => "shell",
            Op::Sketch => "sketch",
            Op::Loft => "loft",
            Op::Sweep => "sweep",
            Op::Fill => "fill",
            Op::Array => "array",
            Op::Trim => "trim",
        }
    }
    /// Mean normalized arc position (0=start..1=end) of this op, from the
    /// transcripts. Used to bias selection toward the workflow arc.
    fn arc(self) -> f64 {
        match self {
            Op::Sketch => 0.40,
            Op::Extrude => 0.47,
            Op::Boolean => 0.50,
            Op::Loft => 0.50,
            Op::Trim => 0.50,
            Op::Sweep => 0.52,
            Op::Mirror => 0.53,
            Op::Fillet => 0.53,
            Op::Fill => 0.55,
            Op::Shell => 0.55,
            Op::Chamfer => 0.54,
            Op::Array => 0.57,
        }
    }
}

// Start-op distribution (index by Op::idx). A primitive seed is always the
// physical first body; "start op" here biases the FIRST grammar transition.
// Weights reflect: form (sketch/extrude) ~28% split, boolean 13%, fill 7%,
// loft 6%, with the rest seeded lower.
fn start_weights() -> [f64; 12] {
    let mut w = [0.0f64; 12];
    w[Op::Extrude.idx()] = 0.20;
    w[Op::Sketch.idx()] = 0.10;
    w[Op::Boolean.idx()] = 0.13;
    w[Op::Fill.idx()] = 0.07;
    w[Op::Loft.idx()] = 0.06;
    w[Op::Mirror.idx()] = 0.03;
    w[Op::Fillet.idx()] = 0.03;
    w[Op::Chamfer.idx()] = 0.02;
    w[Op::Shell.idx()] = 0.02;
    w[Op::Sweep.idx()] = 0.02;
    w[Op::Array.idx()] = 0.01;
    w[Op::Trim.idx()] = 0.01;
    w
}

// End-op distribution: fillet 28%, fill 17%, boolean 14%, chamfer 13%.
fn end_weights() -> [f64; 12] {
    let mut w = [0.0f64; 12];
    w[Op::Fillet.idx()] = 0.28;
    w[Op::Fill.idx()] = 0.17;
    w[Op::Boolean.idx()] = 0.14;
    w[Op::Chamfer.idx()] = 0.13;
    w[Op::Mirror.idx()] = 0.08;
    w[Op::Shell.idx()] = 0.06;
    w[Op::Extrude.idx()] = 0.06;
    w[Op::Loft.idx()] = 0.03;
    w[Op::Array.idx()] = 0.02;
    w[Op::Sweep.idx()] = 0.01;
    w[Op::Trim.idx()] = 0.01;
    w[Op::Sketch.idx()] = 0.01;
    w
}

// Transition table P(next | cur): a 12x12 weight matrix. We seed a small
// uniform floor so any op CAN follow any op (exploration), then add the
// documented strong transitions on top.
fn transition_table() -> [[f64; 12]; 12] {
    let mut t = [[0.05f64; 12]; 12];
    let mut bump = |a: Op, b: Op, w: f64| {
        t[a.idx()][b.idx()] += w;
    };
    // The documented top transitions (the grammar's backbone).
    bump(Op::Fillet, Op::Chamfer, 0.9);
    bump(Op::Chamfer, Op::Fillet, 0.9);
    bump(Op::Extrude, Op::Boolean, 1.0);
    bump(Op::Boolean, Op::Fillet, 0.9);
    bump(Op::Fill, Op::Loft, 0.8);
    bump(Op::Loft, Op::Fill, 0.8);
    bump(Op::Sketch, Op::Extrude, 1.2);
    bump(Op::Fillet, Op::Boolean, 0.7);
    bump(Op::Fillet, Op::Extrude, 0.6);
    bump(Op::Boolean, Op::Extrude, 0.7);
    bump(Op::Mirror, Op::Boolean, 0.8);
    bump(Op::Chamfer, Op::Fill, 0.6);
    // Secondary plausible flows that keep long chains moving on a solid.
    bump(Op::Boolean, Op::Boolean, 0.4);
    bump(Op::Extrude, Op::Extrude, 0.3);
    bump(Op::Boolean, Op::Chamfer, 0.4);
    bump(Op::Boolean, Op::Mirror, 0.4);
    bump(Op::Boolean, Op::Shell, 0.3);
    bump(Op::Fillet, Op::Fillet, 0.3);
    bump(Op::Chamfer, Op::Chamfer, 0.3);
    bump(Op::Extrude, Op::Fillet, 0.3);
    bump(Op::Mirror, Op::Fillet, 0.3);
    bump(Op::Shell, Op::Fillet, 0.3);
    t
}

// ---------------------------------------------------------------------------
// Primitive seed (block/cyl/cone/sphere), sized in the realistic 5..100 mm
// band (median ~20), positioned near the origin.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
enum PrimKind {
    Block,
    Cyl,
    Cone,
    Sphere,
}

/// Sample a feature size in [5,100] with median ~20 (lognormal-ish): exp of
/// a normal centred at ln(20).
fn feature_size(rng: &mut Lcg) -> f64 {
    let s = (20.0f64.ln() + 0.55 * rng.gauss()).exp();
    s.clamp(5.0, 100.0)
}

fn seed_primitive(rng: &mut Lcg) -> Option<(Body, PrimKind)> {
    // Real Plasticity workflows are overwhelmingly PRISMATIC (sketch ->
    // extrude -> box-like solid), with curvature added by fillet/revolve
    // rather than by booleaning spheres. So the seed is mostly a block, with
    // a cylinder minority and rare cone/sphere. (Uniform 25%-each made ~75%
    // of projects start with a curved body, which legitimately declines on
    // the first curved boolean -- the kernel's documented frontier -- and so
    // never grew the LONG chains that are this soak's whole point. The curved
    // boolean decline class is still well-represented via curved TOOLS and
    // the curved-seed minority.)
    let kind = match rng.weighted(&[0.70, 0.20, 0.04, 0.06]) {
        0 => PrimKind::Block,
        1 => PrimKind::Cyl,
        2 => PrimKind::Cone,
        _ => PrimKind::Sphere,
    };
    let mut b = Body::new();
    let s = feature_size(rng);
    let ok = match kind {
        PrimKind::Block => {
            // Roughly cubic with up to 2x anisotropy, centred at origin.
            let dx = s * rng.range(0.6, 1.6);
            let dy = s * rng.range(0.6, 1.6);
            let dz = s * rng.range(0.6, 1.6);
            b.block(Vec3::new(-dx * 0.5, -dy * 0.5, -dz * 0.5), dx, dy, dz)
                .is_ok()
        }
        PrimKind::Cyl => {
            let r = s * 0.5;
            let h = s * rng.range(0.8, 2.0);
            Frame3::from_z(Vec3::new(0.0, 0.0, -h * 0.5), Vec3::new(0.0, 0.0, 1.0))
                .ok()
                .map(|f| b.cylinder(f, r, h).is_ok())
                .unwrap_or(false)
        }
        PrimKind::Cone => {
            let r = s * 0.5;
            let h = s * rng.range(0.8, 2.0);
            Frame3::from_z(Vec3::new(0.0, 0.0, -h * 0.5), Vec3::new(0.0, 0.0, 1.0))
                .ok()
                .map(|f| b.cone(f, r, h).is_ok())
                .unwrap_or(false)
        }
        PrimKind::Sphere => {
            let r = s * 0.5;
            Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))
                .ok()
                .map(|f| b.sphere(f, r).is_ok())
                .unwrap_or(false)
        }
    };
    if ok { Some((b, kind)) } else { None }
}

// ---------------------------------------------------------------------------
// A project: a seed primitive (chosen by the seed) and an ordered op chain.
// The chain is generated FROM the seed too, so the seed regenerates it.
// ---------------------------------------------------------------------------
struct Project {
    seed: u64,
    ops: Vec<Op>,
}

/// Sample the GENERATED chain length from the learned distribution: median
/// ~14, mean ~24 (the transcripts' long right tail). A lognormal with
/// median = exp(mu) = 14 and mean = exp(mu + sigma^2/2) = 24 needs
/// sigma = sqrt(2 ln(24/14)) ~= 1.04, so the heavy tail (the genuinely long
/// modeling sessions) is represented, not just the typical ~14-op project.
fn chain_len(rng: &mut Lcg) -> usize {
    let n = (14.0f64.ln() + 1.04 * rng.gauss()).exp();
    (n.round() as usize).clamp(4, 160)
}

/// Generate the op chain for a project by walking the grammar with the arc
/// bias, anchored by the start- and end-op distributions.
fn gen_project(seed: u64) -> Project {
    let mut rng = Lcg(0x2545_F491_4F6C_DD1D ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let len = chain_len(&mut rng);
    let table = transition_table();
    let sw = start_weights();
    let ew = end_weights();

    let mut ops = Vec::with_capacity(len);
    // First grammar op (after the implicit primitive seed) from start dist.
    let mut cur = Op::ALL[rng.weighted(&sw)];
    ops.push(cur);
    for i in 1..len {
        let pos = i as f64 / (len - 1).max(1) as f64;
        let row = &table[cur.idx()];
        // Arc bias: weight each candidate by closeness of its mean arc
        // position to the current normalized position; sharpen near the
        // ends so finishing ops (fillet/fill/chamfer) dominate the tail.
        let mut w = [0.0f64; 12];
        for j in 0..12 {
            let arc = Op::ALL[j].arc();
            let arc_bias = (-((pos - arc) * (pos - arc)) / (2.0 * 0.18 * 0.18)).exp();
            w[j] = row[j] * (0.25 + arc_bias);
        }
        // The very last op is drawn from the end-op distribution (blended
        // with the grammar so it still respects what can follow `cur`).
        if i == len - 1 {
            for j in 0..12 {
                w[j] = 0.4 * w[j] + 0.6 * ew[j] * (0.3 + row[j]);
            }
        }
        let nxt = Op::ALL[rng.weighted(&w)];
        ops.push(nxt);
        cur = nxt;
    }
    Project { seed, ops }
}

// ---------------------------------------------------------------------------
// Edge / face enumeration via the PUBLIC api (entity_ids + lookup + the
// edge/vertex/face accessors). face_outward_normal is crate-private, so we
// select edges purely geometrically (endpoints), which is all we need.
// ---------------------------------------------------------------------------
fn edge_keys(b: &Body) -> Vec<keel_topo::entity::EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .collect()
}

/// Endpoints of an edge (start, end) via the public accessors, if live.
fn edge_endpoints(b: &Body, e: keel_topo::entity::EdgeKey) -> Option<(Vec3, Vec3)> {
    let edge = b.edge(e)?;
    let (va, vb) = edge.bounds;
    let pa = b.vertex(va)?.point;
    let pb = b.vertex(vb)?.point;
    Some((pa, pb))
}

/// Straight-line length of an edge (0 for a closed/seam edge whose endpoints
/// coincide). Used to size the round/chamfer relative to the local feature.
fn edge_len(b: &Body, e: keel_topo::entity::EdgeKey) -> f64 {
    edge_endpoints(b, e)
        .map(|(a, c)| (c - a).norm())
        .unwrap_or(0.0)
}

/// Pick a STRAIGHT manifold edge bounded by exactly two faces (the kind
/// fillet/chamfer accept), deterministically from the RNG. Returns the key
/// and its length, or None if the body has no such edge.
/// True iff `f`'s surface is an analytic PLANE (the support the blend/chamfer
/// code handles most reliably). Public-API only.
fn face_is_planar(b: &Body, f: keel_topo::entity::FaceKey) -> bool {
    use keel_geom::surface::Surface3;
    use keel_topo::entity::SurfaceGeom;
    match b.face(f).and_then(|x| x.surface) {
        Some((sk, _)) => matches!(
            b.surface(sk),
            Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
        ),
        None => false,
    }
}

/// Ordered list of blendable-edge CANDIDATES (best first), so a caller can
/// try several before declining: the kernel's fillet/chamfer Preconditions on
/// many edges of a complex body (support geometry, convexity, radius
/// overflow), and trying only one edge wasted most attempts. A straight edge
/// bounded by exactly two faces is required; (plane,plane) edges rank highest
/// (the cylinder rung the kernel handles best), then (plane,cyl), then the
/// rest. Within a rank, RNG shuffles so runs stay varied yet deterministic.
fn blend_edge_candidates(b: &Body, rng: &mut Lcg) -> Vec<(keel_topo::entity::EdgeKey, f64)> {
    let mut scored: Vec<(u8, f64, keel_topo::entity::EdgeKey, f64)> = Vec::new();
    for e in edge_keys(b) {
        let faces = b.faces_around_edge(e);
        if faces.len() != 2 {
            continue;
        }
        let len = edge_len(b, e);
        if len <= 1e-6 {
            continue;
        }
        let planar = faces.iter().filter(|&&f| face_is_planar(b, f)).count();
        // rank 0 best: two planar supports; 1: one planar; 2: neither.
        let rank = match planar {
            2 => 0u8,
            1 => 1u8,
            _ => 2u8,
        };
        scored.push((rank, rng.f(), e, len));
    }
    // Sort by rank, then by the random key (deterministic shuffle within rank).
    scored.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    scored.into_iter().map(|(_, _, e, len)| (e, len)).collect()
}

// ---------------------------------------------------------------------------
// The three-bucket oracle, generalized from evolve.rs to operate on an
// arbitrary solid carrying an INDEPENDENT expected-volume interval that the
// generator maintains across the chain.  WRONG = mass==mesh agree but lie
// outside the interval, OR mass != mesh.  Both are decline-never-wrong
// violations.  The interval is generous (curved chordal slack) so only gross
// violations flag -- exactly evolve.rs's posture.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
struct VolBound {
    lo: f64,
    hi: f64,
}
impl VolBound {
    fn slack(&self) -> f64 {
        // 5% of the upper bound, like evolve.rs's vslack, plus an absolute
        // floor so tiny bodies are not flagged on rounding noise.
        0.05 * (1.0 + self.hi)
    }
    fn out(&self, x: f64) -> bool {
        let s = self.slack();
        x > self.hi + s || x < self.lo - s
    }
}

/// Outcome of executing one op against the current body.
#[allow(clippy::large_enum_variant)] // Ok carries a whole Body by design.
enum Step {
    /// Op produced a valid body; carry it forward with its new bound.
    Ok(Body, VolBound),
    /// Op declined gracefully (kernel said no, or not applicable). The op is
    /// recorded and SKIPPED; the chain continues. `fault` is the signature.
    Decline(String),
    /// decline-never-wrong VIOLATION (panic / invalid / mass!=mesh / wrong).
    /// `reason` is the failure class; the chain stops and it is recorded.
    Fail(String),
}

/// Number of non-planar (curved / NURBS) faces of the body, via the PUBLIC
/// face surface accessor. 0 == an all-planar body whose tessellation is
/// EXACT (no chord error), so it earns the tight band; curved faces carry
/// chordal deficit that grows with their count/area.
fn curved_face_count(b: &Body) -> usize {
    use keel_geom::surface::Surface3;
    use keel_topo::entity::SurfaceGeom;
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Face(k)) => b.face(k),
            _ => None,
        })
        .filter(|f| match f.surface {
            Some((sk, _)) => !matches!(
                b.surface(sk),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ),
            None => false,
        })
        .count()
}

/// The PLANAR mass==mesh band. An all-planar body's tessellation is EXACT
/// (triangulating flat polygons introduces no chord error), so mass and mesh
/// must agree to a hair; 2% absorbs only floating-point/divergence-quadrature
/// noise. This makes mass!=mesh a HARD wrong detector for prismatic bodies.
fn planar_mass_mesh_ok(mass: f64, mesh: f64) -> bool {
    let band = 2e-2 * (1.0 + mass.abs());
    (mass - mesh).abs() <= band
}

/// Verbose-repro flag: when set, `audit` prints mass/mesh/bound for every op
/// (used by the KEEL_REPRO=<seed> single-project mode for minimal repros).
static REPRO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Count of curved-body tessellation divergences observed (mass inside the
/// independent bound but mesh disagreeing): a tessellation-quality metric,
/// NOT a correctness violation.
static TESS_NOTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Audit a freshly produced body against the decline-never-wrong contract.
///
/// The correctness net has two tiers, mirroring evolve.rs's stated posture
/// ("the vol-bound is the primary correctness net; mass==mesh is necessary,
/// not sufficient"):
///
///   1. INDEPENDENT volume bound on `mass` (the kernel's AUTHORITATIVE exact
///      analytic volume): the generator tracks a generous [lo,hi] interval
///      the op's result must lie in regardless of geometry. mass outside it
///      is a gross WRONG -- the hard net, applied to planar AND curved bodies
///      alike.
///   2. mass==mesh self-consistency. For an ALL-PLANAR body the mesh is EXACT,
///      so disagreement is a real WRONG (tight 2% band). For a body with ANY
///      curved face the MESH carries chordal / junction deficit (a single
///      fillet face beside a chamfer can lose >6% of the meshed volume while
///      the analytic mass stays correct -- the documented tessellation trap),
///      so a curved mass!=mesh where mass is IN BOUND is recorded as a
///      tessellation note, not a violation. (A curved wrong with mass OUT of
///      bound is still caught by tier 1.)
fn audit(body: Body, bound: VolBound, what: &str) -> Step {
    if body.validate().is_err() {
        return Step::Fail(format!("invalid:{what}"));
    }
    let mesh = body.mesh_volume();
    if !mesh.is_finite() || mesh < -1e-6 {
        return Step::Fail(format!("bad-mesh:{what}"));
    }
    let curved = curved_face_count(&body);
    let mass_dbg = body.mass_properties().map(|m| m.volume).ok();
    if REPRO.load(std::sync::atomic::Ordering::Relaxed) {
        let c = body.counts();
        eprintln!(
            "    [{what}] mass={:?} mesh={mesh:.4} bound=[{:.4},{:.4}] (+slack {:.4})  g{} f{} curved={curved}",
            mass_dbg,
            bound.lo,
            bound.hi,
            bound.slack(),
            c.genus,
            c.f
        );
    }
    match body.mass_properties().map(|m| m.volume) {
        Ok(mass) => {
            // Tier 1: the hard, geometry-independent wrong-net. mass is the
            // kernel's exact analytic volume; outside the op's [lo,hi] it is a
            // gross WRONG (planar and curved alike).
            if bound.out(mass) {
                return Step::Fail(format!("wrong:{what}"));
            }
            // Tier 2: mass==mesh self-consistency.
            if mass > 0.0 {
                let disagree = (mass - mesh).abs();
                let rel = disagree / mass.abs().max(mesh.abs()).max(1e-9);
                if curved == 0 {
                    // All-planar => tessellation is EXACT => any real
                    // divergence is a WRONG (2% absorbs only fp noise).
                    if !planar_mass_mesh_ok(mass, mesh) {
                        return Step::Fail(format!("mass!=mesh-planar:{what}"));
                    }
                } else if rel > 0.25 {
                    // Curved, but a GROSS disagreement (>25%) is far beyond any
                    // legitimate chordal deficit (the worst single curved
                    // primitive meshes ~4% under): the kernel emitted a body
                    // whose analytic mass and tessellation are inconsistent --
                    // a decline-never-wrong VIOLATION it should have declined.
                    return Step::Fail(format!("mass!=mesh-curved:{what}"));
                } else if disagree > 6e-2 * (1.0 + mass.abs()) {
                    // Curved, small (<=25%) disagreement: the mesh is merely
                    // coarse while mass stays exact and in-bound -- a
                    // tessellation note, NOT a wrong (the documented trap).
                    TESS_NOTES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
        // mass honestly declined but mesh is a confident out-of-bounds value:
        // a malformed body the op should have declined (evolve.rs's class).
        Err(_) if bound.out(mesh) => {
            return Step::Fail(format!("malformed:{what}"));
        }
        Err(_) => {}
    }
    Step::Ok(body, bound)
}

fn fault_kind(f: &BoolFault) -> String {
    let s = format!("{f:?}");
    s.split(['(', ' ']).next().unwrap_or("?").to_string()
}

/// An overlapping primitive for a boolean, sized/placed relative to the
/// current body's bbox: a primitive a fraction of the body offset by a
/// fraction of its size so the operands genuinely overlap.
fn boolean_tool(b: &Body, rng: &mut Lcg) -> Option<(Body, VolBound)> {
    let bb = b.bounding_box();
    if !bb.min.is_finite() || !bb.max.is_finite() {
        return None;
    }
    let size = bb.max - bb.min;
    let diag = size.norm();
    if diag < 1e-6 {
        return None;
    }
    let center = (bb.min + bb.max) * 0.5;
    // The tool is a TRANSVERSAL boss/peg, the clean-boolean workhorse of real
    // CAD: pick an axis; in the two PERPENDICULAR axes the tool is strictly
    // SMALLER than the body (30..70%) so its side faces cross the body's
    // faces transversally (general position, never coplanar); along the axis
    // it is tall and offset so it PROTRUDES past one face. This avoids the
    // coplanar-partial-overlap configuration (shared sliver faces) that makes
    // the assembler reject a union as AssemblyFailed. Box tools dominate
    // (clean prismatic boolean); cylinder = a round boss/drill; cone and
    // sphere tools are rarer (the curved boolean frontier, mostly declines).
    let ax = rng.pick(3); // protrusion axis
    let (e0, e1, e2) = (size.x.max(1.0), size.y.max(1.0), size.z.max(1.0));
    // Cross-section fractions in the two perpendicular axes (strictly < 1).
    let cf_a = rng.range(0.3, 0.7);
    let cf_b = rng.range(0.3, 0.7);
    // Small lateral jitter (a fraction of the slack) so the peg is off-centre
    // but still fully inside the cross-section -- avoids any accidental
    // face/edge alignment without poking a side out.
    let jit = |ext: f64, cf: f64, rng: &mut Lcg| (ext * (1.0 - cf) * 0.5) * rng.range(-0.6, 0.6);
    // Axial: length ~1.3..1.9x the body extent on `ax`, offset so it sticks
    // out one end (protrusion) by ~20..60% of the body extent.
    let axext = match ax {
        0 => e0,
        1 => e1,
        _ => e2,
    };
    let len_ax = axext * rng.range(1.3, 1.9);
    let protrude = axext * rng.range(0.2, 0.6);
    let mut offset = Vec3::ZERO;
    let mut lat = Vec3::ZERO;
    match ax {
        0 => {
            offset.x = protrude;
            lat.y = jit(e1, cf_a, rng);
            lat.z = jit(e2, cf_b, rng);
        }
        1 => {
            offset.y = protrude;
            lat.x = jit(e0, cf_a, rng);
            lat.z = jit(e2, cf_b, rng);
        }
        _ => {
            offset.z = protrude;
            lat.x = jit(e0, cf_a, rng);
            lat.y = jit(e1, cf_b, rng);
        }
    }
    let pcenter = center + offset + lat;
    let axis = match ax {
        0 => Vec3::new(1.0, 0.0, 0.0),
        1 => Vec3::new(0.0, 1.0, 0.0),
        _ => Vec3::new(0.0, 0.0, 1.0),
    };
    // Cross-section radius / half-extents (the two perpendicular axes).
    let (px, py) = match ax {
        0 => (e1 * cf_a, e2 * cf_b),
        1 => (e0 * cf_a, e2 * cf_b),
        _ => (e0 * cf_a, e1 * cf_b),
    };
    let rad = 0.5 * px.min(py);
    let mut tool = Body::new();
    // 64% block, 24% cylinder, 6% cone, 6% sphere.
    let kind = rng.weighted(&[0.64, 0.24, 0.06, 0.06]);
    let vol;
    let ok = match kind {
        0 => {
            // Box peg: full cross-section px x py, length len_ax along axis.
            let (dx, dy, dz) = match ax {
                0 => (len_ax, px, py),
                1 => (px, len_ax, py),
                _ => (px, py, len_ax),
            };
            vol = dx * dy * dz;
            let corner = pcenter - Vec3::new(dx, dy, dz) * 0.5;
            tool.block(corner, dx, dy, dz).is_ok()
        }
        1 => {
            // Round peg/drill along the axis, radius rad, length len_ax.
            vol = std::f64::consts::PI * rad * rad * len_ax;
            Frame3::from_z(pcenter - axis * (len_ax * 0.5), axis)
                .ok()
                .map(|f| tool.cylinder(f, rad, len_ax).is_ok())
                .unwrap_or(false)
        }
        2 => {
            vol = std::f64::consts::PI / 3.0 * rad * rad * len_ax;
            Frame3::from_z(pcenter - axis * (len_ax * 0.5), axis)
                .ok()
                .map(|f| tool.cone(f, rad, len_ax).is_ok())
                .unwrap_or(false)
        }
        _ => {
            // Sphere boss centred on a face so it protrudes (curved frontier).
            let r = rad.max(0.5 * diag * 0.3);
            vol = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
            Frame3::from_z(pcenter, Vec3::new(0.0, 0.0, 1.0))
                .ok()
                .map(|f| tool.sphere(f, r).is_ok())
                .unwrap_or(false)
        }
    };
    if ok && vol.is_finite() && vol > 0.0 {
        Some((tool, VolBound { lo: 0.0, hi: vol }))
    } else {
        None
    }
}

/// Execute one op against the current body+bound. Pure (no panic boundary):
/// the caller wraps the whole project in catch_unwind so a panic anywhere is
/// a single FAIL for the project.
fn apply_op(op: Op, body: &Body, bound: VolBound, rng: &mut Lcg) -> Step {
    match op {
        // "form" ops on an existing solid: the kernel has no profile-sketch
        // extrude that fuses with an arbitrary body, so on a non-empty body
        // these read as "add another overlapping solid then union" -- which
        // IS a boolean. We route Extrude/Sketch to a union-with-tool so the
        // long chain keeps adding form, exactly the realistic behaviour.
        Op::Extrude | Op::Sketch => do_boolean(body, bound, BoolOp::Union, "extrude", rng),
        Op::Boolean => {
            let (bop, otag) = match rng.pick(3) {
                0 => (BoolOp::Union, "boolean"),
                1 => (BoolOp::Difference, "boolean"),
                _ => (BoolOp::Intersection, "boolean"),
            };
            do_boolean(body, bound, bop, otag, rng)
        }
        Op::Fillet => {
            let cands = blend_edge_candidates(body, rng);
            if cands.is_empty() {
                return Step::Decline("fillet:no-edge".into());
            }
            // Radius SMALL vs the local feature (edge length): 2..8% (large
            // radii overflow the local geometry and the kernel declines them).
            // Try the best few candidate edges; accept the first that blends.
            let frac = rng.range(0.02, 0.08);
            let mut last = "Precondition".to_string();
            for (e, len) in cands.iter().take(4) {
                match body.fillet_edge(*e, len * frac) {
                    Ok(nb) => {
                        // Fillet rounds off a small wedge (volume drops a
                        // little); bound stays [0.6*lo, hi], safely covering it.
                        return audit(
                            nb,
                            VolBound {
                                lo: 0.6 * bound.lo,
                                hi: bound.hi,
                            },
                            "fillet",
                        );
                    }
                    Err(err) => last = topo_err(&err),
                }
            }
            Step::Decline(format!("fillet:{last}"))
        }
        Op::Chamfer => {
            let cands = blend_edge_candidates(body, rng);
            if cands.is_empty() {
                return Step::Decline("chamfer:no-edge".into());
            }
            let frac = rng.range(0.02, 0.08);
            let mut last = "Precondition".to_string();
            for (e, len) in cands.iter().take(4) {
                match body.chamfer_edge(*e, len * frac) {
                    Ok(nb) => {
                        return audit(
                            nb,
                            VolBound {
                                lo: 0.6 * bound.lo,
                                hi: bound.hi,
                            },
                            "chamfer",
                        );
                    }
                    Err(err) => last = topo_err(&err),
                }
            }
            Step::Decline(format!("chamfer:{last}"))
        }
        Op::Mirror => {
            // Mirror across a plane through the bbox max corner so the copy
            // sits beside the body, then union: volume roughly doubles, but
            // overlap can reduce it, so bound = [old_hi, 2*old_hi].
            let bb = body.bounding_box();
            if !bb.min.is_finite() {
                return Step::Decline("mirror:empty".into());
            }
            // Mirror plane: one of the three axis planes at the body's far
            // face, so the reflection is adjacent (touching) -> a clean union.
            let axis = rng.pick(3);
            let (pt, n) = match axis {
                0 => (Vec3::new(bb.max.x, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
                1 => (Vec3::new(0.0, bb.max.y, 0.0), Vec3::new(0.0, 1.0, 0.0)),
                _ => (Vec3::new(0.0, 0.0, bb.max.z), Vec3::new(0.0, 0.0, 1.0)),
            };
            let mirrored = match body.mirrored(pt, n) {
                Err(err) => return Step::Decline(format!("mirror:{}", topo_err(&err))),
                Ok(m) => m,
            };
            let nb = VolBound {
                lo: bound.lo,
                hi: 2.0 * bound.hi,
            };
            match boolean(body, &mirrored, BoolOp::Union, 1e-7) {
                Err(fault) => Step::Decline(format!("mirror-union:{}", fault_kind(&fault))),
                Ok(r) => {
                    let informational = r
                        .faults
                        .iter()
                        .all(|f| matches!(f, BoolFault::Coincident(..)));
                    if !informational {
                        let k = r.faults.first().map(fault_kind).unwrap_or_default();
                        return Step::Decline(format!("mirror-union:faulted:{k}"));
                    }
                    audit(r.body, nb, "mirror")
                }
            }
        }
        Op::Shell => {
            // Wall thickness a small fraction of the smallest bbox extent so
            // the inner shell does not collapse past the medial axis.
            let bb = body.bounding_box();
            if !bb.min.is_finite() {
                return Step::Decline("shell:empty".into());
            }
            let size = bb.max - bb.min;
            let tmin = size.x.min(size.y).min(size.z).max(1e-3);
            let t = tmin * rng.range(0.05, 0.2);
            match body.hollow(t) {
                Err(err) => Step::Decline(format!("shell:{}", topo_err(&err))),
                // Hollow removes the interior: new volume in (0, old_hi].
                Ok(nb) => audit(
                    nb,
                    VolBound {
                        lo: 0.0,
                        hi: bound.hi,
                    },
                    "shell",
                ),
            }
        }
        // Free-form ops with no clean realization on an arbitrary solid:
        // decline gracefully (the minimize-declines worklist) and end here.
        Op::Loft => Step::Decline("loft:unsupported-on-solid".into()),
        Op::Sweep => Step::Decline("sweep:unsupported-on-solid".into()),
        Op::Fill => Step::Decline("fill:unsupported-on-solid".into()),
        Op::Array => Step::Decline("array:unsupported-on-solid".into()),
        Op::Trim => Step::Decline("trim:unsupported-on-solid".into()),
    }
}

/// Run a boolean of `op` against a fresh overlapping tool, RETRYING up to a
/// few different tool placements before declining (one peg placement may clip
/// a concave feature of a complex body and AssemblyFail, while another lands
/// clean -- a real modeler likewise nudges a feature until it builds). The
/// op-letter (U/D/I) is appended to `tag` for the decline signature so the
/// worklist distinguishes which boolean failed.
fn do_boolean(body: &Body, bound: VolBound, op: BoolOp, tag: &str, rng: &mut Lcg) -> Step {
    let letter = match op {
        BoolOp::Union => "U",
        BoolOp::Difference => "D",
        BoolOp::Intersection => "I",
    };
    let mut last = "no-tool".to_string();
    for _ in 0..3 {
        let Some((tool, tbound)) = boolean_tool(body, rng) else {
            continue;
        };
        let nb = match op {
            BoolOp::Union => VolBound {
                lo: bound.lo.max(tbound.lo),
                hi: bound.hi + tbound.hi,
            },
            BoolOp::Intersection => VolBound {
                lo: 0.0,
                hi: bound.hi.min(tbound.hi),
            },
            BoolOp::Difference => VolBound {
                lo: 0.0f64.max(bound.lo - tbound.hi),
                hi: bound.hi,
            },
        };
        match boolean(body, &tool, op, 1e-7) {
            Err(fault) => last = fault_kind(&fault),
            Ok(r) => {
                let informational = r
                    .faults
                    .iter()
                    .all(|f| matches!(f, BoolFault::Coincident(..)));
                if !informational {
                    last = format!(
                        "faulted:{}",
                        r.faults.first().map(fault_kind).unwrap_or_default()
                    );
                    continue;
                }
                return audit(r.body, nb, tag);
            }
        }
    }
    Step::Decline(format!("{tag}{letter}:{last}"))
}

fn topo_err(e: &keel_topo::body::TopoError) -> String {
    let s = format!("{e:?}");
    // Keep the variant + the first word of any message, dropping addresses.
    s.split(['(', ' ']).next().unwrap_or("?").to_string()
}

// ---------------------------------------------------------------------------
// Project execution + classification.
//
// A project runs its WHOLE op chain: a declined op is SKIPPED (recorded, the
// last good body carried forward) and the chain CONTINUES, exactly as a CAD
// modeler adjusts past a feature that will not build. This is what produces
// the LONG realistic chains -- ending on the first decline capped chains at
// ~1 op, which is neither realistic nor the point. A project STOPS only on a
// decline-never-wrong VIOLATION (Fail), or after `MAX_CONSEC_DECLINES`
// consecutive declines (a body no op can touch -- abandon it). PASS = ran to
// the end with >=1 op realized and never WRONG.
// ---------------------------------------------------------------------------
const MAX_CONSEC_DECLINES: usize = 12;

#[derive(Clone)]
struct ProjResult {
    realized: usize,                 // ops that EXECUTED (produced a body)
    attempted: usize,                // ops attempted before stop (<= chain len)
    declines: Vec<(Op, Op, String)>, // every (prev-realized-op, declined-op, fault)
    failed: Option<(Op, String)>,    // decline-never-wrong violation, if any
    seed_tag: &'static str,
    final_sig: String,
}

/// Run one project, skipping declines and continuing. Runs entirely inside
/// the worker thread so a hang is caught by the timeout and a panic by the
/// catch_unwind below.
fn run_project(p: &Project) -> ProjResult {
    let mut rng =
        Lcg(0xDEAD_BEEF_CAFE_F00D ^ p.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17));
    let Some((mut body, kind)) = seed_primitive(&mut rng) else {
        return ProjResult {
            realized: 0,
            attempted: 0,
            declines: Vec::new(),
            failed: None,
            seed_tag: "?",
            final_sig: "skip:seed".into(),
        };
    };
    let kind_tag = match kind {
        PrimKind::Block => "block",
        PrimKind::Cyl => "cyl",
        PrimKind::Cone => "cone",
        PrimKind::Sphere => "sph",
    };
    // Exact seed volume -> a tight initial bound (small slack via VolBound).
    let v0 = body
        .mass_properties()
        .map(|m| m.volume)
        .unwrap_or_else(|_| body.mesh_volume());
    let mut bound = VolBound {
        lo: v0 * 0.97,
        hi: v0 * 1.03,
    };
    let mut realized = 0usize;
    let mut attempted = 0usize;
    let mut declines: Vec<(Op, Op, String)> = Vec::new();
    let mut prev = Op::Sketch; // notional pre-seed op for the first transition key
    let mut consec = 0usize;

    for &op in p.ops.iter() {
        attempted += 1;
        match apply_op(op, &body, bound, &mut rng) {
            Step::Ok(nb, nbound) => {
                body = nb;
                bound = nbound;
                realized += 1;
                prev = op;
                consec = 0;
            }
            Step::Decline(fault) => {
                declines.push((prev, op, fault));
                consec += 1;
                if consec >= MAX_CONSEC_DECLINES {
                    let c = body.counts();
                    return ProjResult {
                        realized,
                        attempted,
                        declines,
                        failed: None,
                        seed_tag: kind_tag,
                        final_sig: format!(
                            "STALL seed={kind_tag} realized={realized}/{} g{} f{}",
                            p.ops.len(),
                            c.genus,
                            c.f
                        ),
                    };
                }
            }
            Step::Fail(reason) => {
                return ProjResult {
                    realized,
                    attempted,
                    declines,
                    failed: Some((op, reason.clone())),
                    seed_tag: kind_tag,
                    final_sig: format!("FAIL {reason} seed={kind_tag} realized={realized}"),
                };
            }
        }
    }
    let c = body.counts();
    ProjResult {
        realized,
        attempted,
        declines,
        failed: None,
        seed_tag: kind_tag,
        final_sig: format!(
            "PASS seed={kind_tag} ops={realized}/{} g{} r{} s{} f{}",
            p.ops.len(),
            c.genus,
            c.regions,
            c.shells,
            c.f
        ),
    }
}

// ---------------------------------------------------------------------------
// Worker POOL (parallel). N threads pull projects from a shared queue and
// push (seed, chain_len, result) back on a shared channel. Each project is
// independent and its outcome is a deterministic function of its seed, so
// parallelism does not affect reproducibility -- only the result ARRIVAL
// order interleaves (and every line carries its seed). A project that HANGS
// the kernel wedges one worker; the others keep draining, and the dispatcher
// reconciles the missing result as a TIMEOUT at the end (a perf note, never a
// wrong). A panic is caught and surfaced as a wrong-class fault.
// ---------------------------------------------------------------------------
type PoolResult = (u64, usize, ProjResult); // (seed, chain_len, result)

fn spawn_pool(n: usize) -> (mpsc::Sender<Project>, mpsc::Receiver<PoolResult>) {
    let (job_tx, job_rx) = mpsc::channel::<Project>();
    let (res_tx, res_rx) = mpsc::channel::<PoolResult>();
    let job_rx = std::sync::Arc::new(std::sync::Mutex::new(job_rx));
    for _ in 0..n.max(1) {
        let job_rx = std::sync::Arc::clone(&job_rx);
        let res_tx = res_tx.clone();
        std::thread::spawn(move || {
            loop {
                // Pull one job (lock only for the recv, never across the work).
                let p = {
                    let guard = match job_rx.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    match guard.recv() {
                        Ok(p) => p,
                        Err(_) => break, // queue closed
                    }
                };
                let (seed, len) = (p.seed, p.ops.len());
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| run_project(&p)));
                let r = res.unwrap_or_else(|_| ProjResult {
                    realized: 0,
                    attempted: 0,
                    declines: Vec::new(),
                    failed: Some((Op::Boolean, "panic".into())),
                    seed_tag: "?",
                    final_sig: "FAIL panic".into(),
                });
                if res_tx.send((seed, len, r)).is_err() {
                    break;
                }
            }
        });
    }
    (job_tx, res_rx)
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    // Single-project verbose repro: KEEL_REPRO=<project-seed> replays exactly
    // that project (the seed printed in declines/failures.jsonl), logging the
    // op chain and each op's mass/mesh/bound -- the minimal-repro tool.
    if let Ok(Ok(pseed)) = std::env::var("KEEL_REPRO").map(|s| s.parse::<u64>()) {
        {
            REPRO.store(true, std::sync::atomic::Ordering::Relaxed);
            let proj = gen_project(pseed);
            eprintln!(
                "REPRO seed={pseed}: {} ops: {}",
                proj.ops.len(),
                proj.ops
                    .iter()
                    .map(|o| o.tag())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            let res = run_project(&proj);
            eprintln!("  => realized={} {}", res.realized, res.final_sig);
            return;
        }
    }

    let args: Vec<String> = std::env::args().collect();
    let nproj: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10000);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let outdir = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "realsoak-out".to_string());
    std::fs::create_dir_all(&outdir).unwrap();
    // Quiet the panic firehose; the project seed replays any panic on demand.
    std::panic::set_hook(Box::new(|_| {}));

    let mut pfile = std::fs::File::create(Path::new(&outdir).join("passes.jsonl")).unwrap();
    // Declines are the bulk and the worklist: buffered.
    let mut dfile = std::io::BufWriter::new(
        std::fs::File::create(Path::new(&outdir).join("declines.jsonl")).unwrap(),
    );
    let mut ffile = std::fs::File::create(Path::new(&outdir).join("failures.jsonl")).unwrap();
    let mut sfile = std::io::BufWriter::new(
        std::fs::File::create(Path::new(&outdir).join("projects.jsonl")).unwrap(),
    );

    // Project-level tally. A project either runs to the end (PASS, if it
    // realized >=1 op and never went WRONG), STALLs (abandoned after
    // MAX_CONSEC_DECLINES consecutive declines), FAILs (a decline-never-wrong
    // VIOLATION -- the wrong-body class, which MUST be 0), TIMES OUT (a perf
    // cliff: the op chain did not finish in the budget; a separate concern
    // from correctness), or is SKIPped (seed failed).
    let (mut n_pass, mut n_stall, mut n_fail, mut n_skip, mut n_timeout) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    // OP-level tally (the honest executed-vs-declined accounting the brief
    // asks for): every op in every chain is either realized or declined.
    let mut total_ops_generated = 0u64;
    let mut total_ops_realized = 0u64;
    let mut total_ops_declined = 0u64;
    let mut realized_lens: Vec<usize> = Vec::with_capacity(nproj);
    let mut gen_lens: Vec<usize> = Vec::with_capacity(nproj);
    // Decline-class histogram, keyed by "prevOp->declinedOp:fault" -- the
    // minimize-declines worklist.
    let mut decline_hist: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    // First minimal repro (project seed) per decline class.
    let mut decline_repro: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut leaked = 0usize;

    // Generous WHOLE-RUN stall budget: if no result arrives for this long, a
    // worker is presumed wedged on a hung project (a perf cliff, not a wrong).
    // A single chain finishes well under this even on complex bodies.
    let stall_budget = Duration::from_secs(90);
    let nworkers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 12);
    let (job_tx, res_rx) = spawn_pool(nworkers);
    eprintln!("  dispatching {nproj} projects across {nworkers} workers...");

    let t0 = Instant::now();
    // Dispatch every project up front (deterministic per-index seeds).
    for k in 0..nproj {
        let pseed = seed
            .wrapping_mul(0x100000001B3)
            .wrapping_add(k as u64)
            .wrapping_add(0x9E37_79B9_7F4A_7C15);
        let proj = gen_project(pseed);
        total_ops_generated += proj.ops.len() as u64;
        if job_tx.send(proj).is_err() {
            break;
        }
    }
    drop(job_tx); // close the queue so workers exit once drained

    // Collect results (arrival order interleaved; each carries its seed).
    let mut received = 0usize;
    while received + (n_timeout as usize) < nproj {
        let (pseed, chain_len, res) = match res_rx.recv_timeout(stall_budget) {
            Ok(t) => t,
            Err(_) => {
                // No result for the whole budget: a worker is wedged on a hung
                // project. Account one timeout and keep collecting; remaining
                // live workers continue. (Reconciles the shortfall at the end.)
                n_timeout += 1;
                leaked += 1;
                if leaked >= 16 {
                    eprintln!("  stopping collection: {leaked} presumed hangs (systemic)");
                    break;
                }
                continue;
            }
        };
        received += 1;

        total_ops_realized += res.realized as u64;
        total_ops_declined += res.declines.len() as u64;
        realized_lens.push(res.realized);
        gen_lens.push(chain_len);

        // Record every declined op (the worklist), regardless of project fate.
        for (prev, op, fault) in &res.declines {
            let key = format!("{}->{}:{}", prev.tag(), op.tag(), fault);
            *decline_hist.entry(key.clone()).or_insert(0) += 1;
            decline_repro.entry(key).or_insert(pseed);
            let _ = writeln!(
                dfile,
                "{{\"seed\":{},\"prev\":\"{}\",\"op\":\"{}\",\"fault\":\"{}\"}}",
                pseed,
                prev.tag(),
                op.tag(),
                json_escape(fault),
            );
        }

        if let Some((op, reason)) = &res.failed {
            // A hang is a PERF cliff (handled above via recv timeout); the
            // only `failed` reasons reaching here are real wrong-class faults
            // (panic / invalid / planar mass!=mesh / out-of-bound / malformed).
            n_fail += 1;
            let _ = writeln!(
                ffile,
                "{{\"seed\":{},\"op\":\"{}\",\"reason\":\"{}\",\"sig\":\"{}\"}}",
                pseed,
                op.tag(),
                json_escape(reason),
                json_escape(&res.final_sig)
            );
            let _ = ffile.flush();
        } else if res.final_sig.starts_with("skip") {
            n_skip += 1;
        } else if res.final_sig.starts_with("STALL") {
            n_stall += 1;
        } else {
            n_pass += 1;
            let _ = writeln!(
                pfile,
                "{{\"seed\":{},\"realized\":{},\"attempted\":{},\"declines\":{},\"sig\":\"{}\"}}",
                pseed,
                res.realized,
                res.attempted,
                res.declines.len(),
                json_escape(&res.final_sig)
            );
        }
        let _ = writeln!(
            sfile,
            "{{\"seed\":{},\"seed_prim\":\"{}\",\"realized\":{},\"attempted\":{},\"declined\":{},\"sig\":\"{}\"}}",
            pseed,
            res.seed_tag,
            res.realized,
            res.attempted,
            res.declines.len(),
            json_escape(&res.final_sig)
        );

        if received % 1000 == 0 {
            eprintln!(
                "  {received} / {nproj}  PASS {n_pass} STALL {n_stall} WRONG {n_fail} TIMEOUT {n_timeout}  ({:.0}/s)",
                received as f64 / t0.elapsed().as_secs_f64().max(1e-6)
            );
        }
    }

    let _ = pfile.flush();
    let _ = dfile.flush();
    let _ = ffile.flush();
    let _ = sfile.flush();
    let _ = std::panic::take_hook();

    // --- Summary ----------------------------------------------------------
    realized_lens.sort_unstable();
    let median_realized = realized_lens
        .get(realized_lens.len() / 2)
        .copied()
        .unwrap_or(0);
    let mean_realized = if realized_lens.is_empty() {
        0.0
    } else {
        total_ops_realized as f64 / realized_lens.len() as f64
    };
    let total_ops_attempted = total_ops_realized + total_ops_declined;
    let exec_frac = if total_ops_attempted == 0 {
        0.0
    } else {
        total_ops_realized as f64 / total_ops_attempted as f64
    };
    let tess_notes = TESS_NOTES.load(std::sync::atomic::Ordering::Relaxed);
    gen_lens.sort_unstable();
    let median_gen = gen_lens.get(gen_lens.len() / 2).copied().unwrap_or(0);
    let mean_gen = if gen_lens.is_empty() {
        0.0
    } else {
        total_ops_generated as f64 / gen_lens.len() as f64
    };

    println!(
        "\nrealsoak: {} projects  PASS {n_pass} / STALL {n_stall} / WRONG {n_fail} / TIMEOUT {n_timeout} / SKIP {n_skip}",
        n_pass + n_stall + n_fail + n_timeout + n_skip
    );
    println!(
        "  GENERATED chain length: median {median_gen}, mean {mean_gen:.1} (target from transcripts: median 14, mean 24)"
    );
    println!(
        "  REALIZED (executed) chain length: median {median_realized}, mean {mean_realized:.1} ops/project"
    );
    println!(
        "  ops: {total_ops_generated} generated; {total_ops_realized} executed + {total_ops_declined} declined = {total_ops_attempted} attempted; execution rate {:.1}%",
        exec_frac * 100.0
    );
    println!(
        "  WRONG (decline-never-wrong violations + crashes/hangs): {n_fail}  {}",
        if n_fail == 0 {
            "[invariant HELD]"
        } else {
            "[INVARIANT BROKEN]"
        }
    );
    println!(
        "  (curved-body tessellation divergences noted, NOT wrongs: {tess_notes} -- mass in-bound, mesh coarse)"
    );

    // Decline classes split into two groups:
    //   * "unsupported-on-solid": ops the generator's mapping cannot realize
    //     on an arbitrary solid (loft/sweep/fill/array/trim). These are SCOPE
    //     limits of the soak, declined by design -- not kernel faults.
    //   * KERNEL-FRONTIER: the kernel itself declined (AssemblyFailed,
    //     Precondition, faulted, ...). THIS is the minimize-declines worklist.
    let mut classes: Vec<(String, u64)> = decline_hist.into_iter().collect();
    classes.sort_by_key(|x| std::cmp::Reverse(x.1)); // descending by count
    let is_unsupported = |c: &str| c.contains("unsupported-on-solid");
    let total_declines: u64 = classes.iter().map(|(_, n)| *n).sum();
    let unsupported: u64 = classes
        .iter()
        .filter(|(c, _)| is_unsupported(c))
        .map(|(_, n)| *n)
        .sum();
    let frontier = total_declines - unsupported;
    println!(
        "\n  declines: {total_declines} total = {unsupported} unsupported-on-solid (by-design scope) + {frontier} KERNEL-FRONTIER (the worklist)"
    );

    println!("\n  top KERNEL-FRONTIER decline classes (prevOp->op:fault   count   repro-seed):");
    for (cls, cnt) in classes.iter().filter(|(c, _)| !is_unsupported(c)).take(25) {
        let repro = decline_repro.get(cls).copied().unwrap_or(0);
        println!("    {cnt:>7}  {cls}   (seed {repro})");
    }
    println!("\n  (top by-design unsupported-on-solid declines, for completeness):");
    for (cls, cnt) in classes.iter().filter(|(c, _)| is_unsupported(c)).take(6) {
        println!("    {cnt:>7}  {cls}");
    }
    println!(
        "\n  distinct decline classes: {} ({} kernel-frontier); wrote {outdir}/{{passes,declines,failures,projects}}.jsonl",
        classes.len(),
        classes.iter().filter(|(c, _)| !is_unsupported(c)).count()
    );
}
