//! The OP GYM (task 46): the demo corpus's op families exercised with
//! RANDOM parameters and NO images: the numeric analogue of the visual
//! bug hunt. Per trial one op family runs with LCG-drawn parameters
//! inside its documented contract; an Err is an honest DECLINE
//! (bucketed, never penalized); an Ok body must satisfy:
//!
//!   1. validate() (topological soundness),
//!   2. finite, non-negative mesh volume,
//!   3. mass == mesh within the curved CHORDAL band when mass computes,
//!   4. surface_area (analytic-preferred) == summed tessellation area
//!      within the same band: the NUMERIC VISUAL ORACLE: a face that
//!      tessellates as a flat triangle, a fin, or a missing band moves
//!      the tessellated area even when volumes cancel,
//!   5. DIFFERENTIAL invariants where the family has one (slice
//!      partitions the mesh volume; unblend restores the parent block;
//!      uniform scale cubes the volume).
//!
//! BLIND SPOT (stated plainly): invariant 4 only bites on faces whose
//! area reads ANALYTICALLY; a face whose analytic area declines falls
//! back to its own tessellation and compares trivially equal, so a
//! mis-tessellated face without an analytic carrier (the task-42
//! variable-cone band, the task-45 setback fin) passes here and stays
//! the visual corpus's catch. The two instruments are complementary.
//!
//! N scales by KEEL_OPGYM_N (default keeps CI fast); the instrument run
//! is the same binary at thousands of trials:
//!   KEEL_OPGYM_N=5000 cargo test --release -p keel-topo --test op_gym
//!     -- --ignored --nocapture
//! Every violation prints its full trial context; the gate is
//! violations == 0.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::{Frame3, Plane3};
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::EdgeKey;

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
}

fn edge_at(b: &Body, mid: Vec3) -> Option<EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(keel_topo::entity::AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .find(|&k| {
            let Some(edge) = b.edge(k) else { return false };
            let (Some(p0), Some(p1)) = (
                b.vertex(edge.bounds.0).map(|v| v.point),
                b.vertex(edge.bounds.1).map(|v| v.point),
            ) else {
                return false;
            };
            ((p0 + p1) * 0.5 - mid).norm() < 1e-9
        })
}

/// Summed area of the kernel's own tessellation (worker_mesh).
fn tess_area(b: &Body) -> f64 {
    let m = b.worker_mesh();
    let mut area = 0.0;
    let p = |i: u32| {
        let k = i as usize * 3;
        Vec3::new(
            m.positions[k] as f64,
            m.positions[k + 1] as f64,
            m.positions[k + 2] as f64,
        )
    };
    for t in m.indices.chunks(3) {
        let (a, b2, c) = (p(t[0]), p(t[1]), p(t[2]));
        area += 0.5 * (b2 - a).cross(c - a).norm();
    }
    area
}

#[derive(Default)]
struct Buckets {
    ok: usize,
    decline: usize,
    violations: usize,
    first: String,
}

impl Buckets {
    /// The shared invariant battery; `extra` is the family's
    /// differential check result (None = no differential).
    fn judge(&mut self, trial: usize, family: &str, body: &Body, extra: Option<(f64, f64, f64)>) {
        let mut fail = |what: &str, detail: String| {
            self.violations += 1;
            eprintln!("VIOLATION trial {trial} [{family}] {what}: {detail}");
            if self.first.is_empty() {
                self.first = format!("trial {trial} [{family}] {what}");
            }
        };
        if let Err(e) = body.validate() {
            fail("validate", format!("{e:?}"));
            return;
        }
        let mesh = body.mesh_volume();
        if !mesh.is_finite() || mesh < -1e-6 {
            fail("mesh volume", format!("{mesh}"));
            return;
        }
        if let Ok(m) = body.mass_properties() {
            let v = m.volume;
            if (v - mesh).abs() > 2e-2 * (1.0 + v.abs()) {
                fail("mass vs mesh", format!("mass {v} mesh {mesh}"));
            }
        }
        // The numeric visual oracle: analytic-preferred total area vs
        // the tessellation's own area.
        let sa = body.surface_area();
        let ta = tess_area(body);
        if sa.is_finite() && (sa - ta).abs() > 2e-2 * (1.0 + sa.abs()) {
            fail("area vs tess-area", format!("analytic {sa} tess {ta}"));
        }
        if let Some((got, want, tol)) = extra
            && (got - want).abs() > tol
        {
            fail("differential", format!("got {got} want {want} tol {tol}"));
        }
        self.ok += 1;
    }
}

#[test]
#[ignore = "bug-hunt instrument; run: KEEL_OPGYM_N=5000 cargo test --release -p keel-topo --test op_gym -- --ignored --nocapture"]
fn op_gym() {
    let n: usize = std::env::var("KEEL_OPGYM_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let start: usize = std::env::var("KEEL_OPGYM_START")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut rng = Lcg(0x0961_5EED_C0FF_EE00 ^ (start as u64).wrapping_mul(0x9E37_79B9));
    for _ in 0..start * 16 {
        rng.f();
    }
    let mut bk = Buckets::default();
    let mut family_counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();

    // (family name, Ok(body + optional differential) or decline).
    type Outcome = Option<(Body, Option<(f64, f64, f64)>)>;
    for trial in start..start + n {
        let fam = trial % 13;
        let (name, result): (&'static str, Outcome) = match fam {
            // 0: constant fillet (cliff fallback allowed), random block.
            0 => {
                let (lx, ly, lz) = (
                    rng.range(2.0, 6.0),
                    rng.range(0.8, 3.0),
                    rng.range(1.0, 3.0),
                );
                let r = rng.range(0.1, ly.min(lz) * 0.9);
                let mut b = Body::new();
                b.block(Vec3::ZERO, lx, ly, lz).unwrap();
                let e = edge_at(&b, Vec3::new(lx * 0.5, 0.0, lz)).unwrap();
                let out = b.fillet_edge(e, r).or_else(|_| b.fillet_edge_cliff(e, r));
                ("fillet", out.ok().map(|b| (b, None)))
            }
            // 1: variable fillet (the task-42 class).
            1 => {
                let (lx, ly, lz) = (
                    rng.range(2.0, 6.0),
                    rng.range(1.2, 3.0),
                    rng.range(1.2, 3.0),
                );
                let r0 = rng.range(0.08, 0.4);
                let r1 = rng.range(0.08, ly.min(lz) * 0.6);
                let mut b = Body::new();
                b.block(Vec3::ZERO, lx, ly, lz).unwrap();
                let e = edge_at(&b, Vec3::new(lx * 0.5, 0.0, lz)).unwrap();
                (
                    "vfillet",
                    b.fillet_edge_variable(e, r0, r1).ok().map(|b| (b, None)),
                )
            }
            // 2: partial-extent fillet (the task-44 class).
            2 => {
                let (lx, ly, lz) = (
                    rng.range(2.5, 6.0),
                    rng.range(1.2, 3.0),
                    rng.range(1.2, 3.0),
                );
                let r = rng.range(0.1, ly.min(lz) * 0.5);
                let mid = rng.range(0.35, 0.65);
                let half = rng.range(0.08, 0.22);
                let mut b = Body::new();
                b.block(Vec3::ZERO, lx, ly, lz).unwrap();
                let e = edge_at(&b, Vec3::new(lx * 0.5, 0.0, lz)).unwrap();
                (
                    "pfillet",
                    b.fillet_edge_partial(e, mid - half, mid + half, r)
                        .ok()
                        .map(|b| (b, None)),
                )
            }
            // 3: corner blends (octant / setback, the task-45 class).
            3 => {
                let s = rng.range(1.5, 3.0);
                let r = rng.range(0.12, s * 0.3);
                let mut b = Body::new();
                b.block(Vec3::ZERO, s, s, s).unwrap();
                if trial % 2 == 0 {
                    let corner = b
                        .entity_ids()
                        .filter_map(|id| match b.lookup(id) {
                            Some(keel_topo::entity::AnyKey::Vertex(k)) => Some(k),
                            _ => None,
                        })
                        .find(|&k| {
                            b.vertex(k)
                                .map(|v| (v.point - Vec3::new(s, s, s)).norm() < 1e-9)
                                .unwrap_or(false)
                        })
                        .unwrap();
                    (
                        "octant",
                        b.fillet_corner_octant(corner, r).ok().map(|b| (b, None)),
                    )
                } else {
                    let d = rng.range(r * 1.2, s * 0.45);
                    let e1 = edge_at(&b, Vec3::new(s * 0.5, s, s)).unwrap();
                    let e2 = edge_at(&b, Vec3::new(s, s * 0.5, s)).unwrap();
                    let e3 = edge_at(&b, Vec3::new(s, s, s * 0.5)).unwrap();
                    (
                        "setback",
                        b.fillet_corner_setback(&[(e1, r, d), (e2, r, d), (e3, r, d)])
                            .ok()
                            .map(|out| {
                                // The op's own rigorous volume bounds
                                // (dossier 53 / the in-tree oracle):
                                // removal at least the edge prisms up to
                                // the cross planes, at most the prisms
                                // full-length plus the corner box.
                                let pi = core::f64::consts::PI;
                                let cut = 3.0 * r * r * (1.0 - pi / 4.0);
                                let lo = s * s * s - cut * s - (d + r).powi(3);
                                let hi = s * s * s - cut * (s - d);
                                let v = out.mesh_volume();
                                let mid = 0.5 * (lo + hi);
                                let tol = 0.5 * (hi - lo);
                                (out, Some((v, mid, tol)))
                            }),
                    )
                }
            }
            // 4: draft / taper a side wall.
            4 => {
                let s = rng.range(1.5, 3.5);
                let ang = rng.range(-0.4, 0.4);
                let mut b = Body::new();
                b.block(Vec3::ZERO, s, s, s).unwrap();
                let f = b.pick_face(Vec3::new(s, s * 0.5, s * 0.5), 1e-6).unwrap();
                let out = if trial % 2 == 0 {
                    let neutral = Plane3 {
                        frame: Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
                    };
                    b.draft_face(f, &neutral, ang).map(|_| b)
                } else {
                    b.taper_face(f, Vec3::new(s, s * 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0), ang)
                        .map(|_| b)
                };
                ("draft", out.ok().map(|b| (b, None)))
            }
            // 5: move_face (exact prismatic differential).
            5 => {
                let s = rng.range(1.5, 3.5);
                let dz = rng.range(0.1, 2.0);
                let mut b = Body::new();
                b.block(Vec3::ZERO, s, s, s).unwrap();
                let top = b.pick_face(Vec3::new(s * 0.5, s * 0.5, s), 1e-6).unwrap();
                let out = b.move_face(top, Vec3::new(0.0, 0.0, dz)).map(|_| b);
                let want = s * s * (s + dz);
                (
                    "moveface",
                    out.ok().map(|b| {
                        let got = b.mesh_volume();
                        (b, Some((got, want, 1e-6 * (1.0 + want))))
                    }),
                )
            }
            // 6: hollow / pierce.
            6 => {
                let (lx, ly, lz) = (
                    rng.range(2.0, 4.0),
                    rng.range(2.0, 4.0),
                    rng.range(1.2, 3.0),
                );
                let t = rng.range(0.08, lz.min(lx).min(ly) * 0.3);
                let mut b = Body::new();
                b.block(Vec3::ZERO, lx, ly, lz).unwrap();
                let out = if trial % 2 == 0 {
                    b.hollow(t)
                } else {
                    let top = b
                        .pick_face(Vec3::new(lx * 0.5, ly * 0.5, lz), 1e-6)
                        .unwrap();
                    b.hollow_pierce(t, |f| f == top)
                };
                ("hollow", out.ok().map(|b| (b, None)))
            }
            // 7: partition by a tilted knife sheet.
            7 => {
                let s = rng.range(2.0, 4.0);
                let ang = rng.range(-0.5, 0.5);
                let nrm = Vec3::new(ang.sin(), 0.0, ang.cos());
                let u = Vec3::new(ang.cos(), 0.0, -ang.sin());
                let v = nrm.cross(u);
                let c = Vec3::new(s * 0.5, s * 0.5, s * rng.range(0.3, 0.7));
                let corner = c - u * (s * 2.0) - v * (s * 2.0);
                let knife = Body::rectangular_sheet(corner, nrm, u, s * 4.0, s * 4.0).unwrap();
                let mut block = Body::new();
                block.block(Vec3::ZERO, s, s, s).unwrap();
                (
                    "partition",
                    keel_topo::boolean::partition_by_sheet(&block, &knife, 1e-7)
                        .ok()
                        .map(|b| (b, None)),
                )
            }
            // 8: curved booleans: drill / countersink / socket / crossing
            // cylinders (the task-41 class), op cycling.
            8 => {
                let op = match (trial / 13) % 3 {
                    0 => BoolOp::Difference,
                    1 => BoolOp::Intersection,
                    _ => BoolOp::Union,
                };
                let kind = (trial / 39) % 4;
                let out = match kind {
                    0 => {
                        let (lx, ly, lz) = (
                            rng.range(3.0, 6.0),
                            rng.range(3.0, 6.0),
                            rng.range(1.0, 3.0),
                        );
                        let r = rng.range(0.3, lx.min(ly) * 0.5 - 0.4);
                        let cx = rng.range(r + 0.2, lx - r - 0.2);
                        let cy = rng.range(r + 0.2, ly - r - 0.2);
                        let mut a = Body::new();
                        a.block(Vec3::ZERO, lx, ly, lz).unwrap();
                        let f = Frame3::from_z(Vec3::new(cx, cy, -0.5), Vec3::new(0.0, 0.0, 1.0))
                            .unwrap();
                        let mut t = Body::new();
                        t.cylinder(f, r, lz + 1.0).unwrap();
                        boolean(&a, &t, op, 1e-7)
                    }
                    1 => {
                        let (lx, ly, lz) = (
                            rng.range(3.0, 6.0),
                            rng.range(3.0, 6.0),
                            rng.range(1.5, 3.0),
                        );
                        let base = rng.range(0.3, lz - 0.3);
                        let h = lz - base + rng.range(0.3, 2.0);
                        let rmax = lx.min(ly) * 0.5 - 0.4;
                        let r0 = rng.range(0.2, rmax * 0.9);
                        let r1 = rng.range(0.2, rmax * 0.9);
                        let mut a = Body::new();
                        a.block(Vec3::ZERO, lx, ly, lz).unwrap();
                        let f = Frame3::from_z(
                            Vec3::new(lx * 0.5, ly * 0.5, base),
                            Vec3::new(0.0, 0.0, 1.0),
                        )
                        .unwrap();
                        let mut t = Body::new();
                        if t.loft_circles(f, r0, r1, h).is_err() {
                            Err(keel_topo::boolean::BoolFault::AssemblyFailed("loft"))
                        } else {
                            boolean(&a, &t, op, 1e-7)
                        }
                    }
                    2 => {
                        let s = rng.range(3.0, 5.0);
                        let r = rng.range(0.6, 1.4);
                        let zc = rng.range(s * 0.3, s * 0.7);
                        let mut a = Body::new();
                        a.block(Vec3::ZERO, s, s, zc + r * rng.range(0.1, 0.9))
                            .unwrap();
                        let f = Frame3::from_z(
                            Vec3::new(s * 0.5, s * 0.5, zc),
                            Vec3::new(0.0, 0.0, 1.0),
                        )
                        .unwrap();
                        let mut t = Body::new();
                        t.sphere(f, r).unwrap();
                        boolean(&a, &t, op, 1e-7)
                    }
                    _ => {
                        let h = rng.range(3.0, 8.0);
                        let mut a = Body::new();
                        a.cylinder(
                            Frame3::from_z(Vec3::new(0.0, 0.0, -h * 0.5), Vec3::new(0.0, 0.0, 1.0))
                                .unwrap(),
                            1.0,
                            h,
                        )
                        .unwrap();
                        let mut b2 = Body::new();
                        b2.cylinder(
                            Frame3::from_z(Vec3::new(-h * 0.5, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
                                .unwrap(),
                            1.0,
                            h,
                        )
                        .unwrap();
                        boolean(&a, &b2, op, 1e-7)
                    }
                };
                ("boolean", out.ok().map(|r| (r.body, None)))
            }
            // 9: slice partitions the mesh volume (differential).
            9 => {
                let s = rng.range(1.5, 3.0);
                let mut b = Body::new();
                b.block(Vec3::new(-s, -s, 0.0), 2.0 * s, 2.0 * s, 2.0 * s)
                    .unwrap();
                let cuts: Vec<f64> = (1..3).map(|k| 2.0 * s * k as f64 / 3.0).collect();
                let whole = b.mesh_volume();
                (
                    "slice",
                    b.slice(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &cuts)
                        .ok()
                        .map(|ps| {
                            let total: f64 = ps.iter().map(|p| p.mesh_volume()).sum();
                            // Judge the FIRST wafer's invariants; the
                            // differential covers the set.
                            let first = ps.into_iter().next().unwrap();
                            (first, Some((total, whole, 1e-6 * (1.0 + whole))))
                        }),
                )
            }
            // 10: constructors: swept helix / multi-section loft /
            // partial revolve.
            10 => {
                let kind = (trial / 13) % 3;
                let out = match kind {
                    0 => {
                        let turns = rng.range(0.5, 2.5);
                        let m = 40usize;
                        let path: Vec<Vec3> = (0..=m)
                            .map(|k| {
                                let a = turns * core::f64::consts::TAU * k as f64 / m as f64;
                                Vec3::new(1.2 * a.cos(), 1.2 * a.sin(), 0.35 * a)
                            })
                            .collect();
                        let w = rng.range(0.1, 0.25);
                        let profile = [
                            Vec3::new(-w, -w, 0.0),
                            Vec3::new(w, -w, 0.0),
                            Vec3::new(w, w, 0.0),
                            Vec3::new(-w, w, 0.0),
                        ];
                        let mut b = Body::new();
                        b.sweep_along_path(&profile, &path).map(|_| b)
                    }
                    1 => {
                        let m = 10usize;
                        let ring = |r: f64, z: f64| -> Vec<Vec3> {
                            (0..m)
                                .map(|k| {
                                    let a = core::f64::consts::TAU * k as f64 / m as f64;
                                    Vec3::new(r * a.cos(), r * a.sin(), z)
                                })
                                .collect()
                        };
                        let s0 = ring(rng.range(0.6, 1.4), 0.0);
                        let s1 = ring(rng.range(0.4, 1.8), rng.range(0.8, 1.5));
                        let s2 = ring(rng.range(0.5, 1.2), rng.range(2.0, 3.0));
                        let mut b = Body::new();
                        b.loft_sections(&[&s0, &s1, &s2]).map(|_| b)
                    }
                    _ => {
                        let theta = rng.range(0.5, core::f64::consts::TAU - 0.3);
                        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
                        let mut b = Body::new();
                        b.revolve_partial(
                            f,
                            &[
                                (0.4, 0.0),
                                (rng.range(0.9, 1.4), 0.0),
                                (rng.range(0.6, 1.1), rng.range(0.6, 1.0)),
                                (0.4, rng.range(1.2, 1.8)),
                            ],
                            theta,
                        )
                        .map(|_| b)
                    }
                };
                ("construct", out.ok().map(|b| (b, None)))
            }
            // 11: unblend restores the parent block EXACTLY
            // (differential to 1e-9).
            11 => {
                let (lx, ly, lz) = (
                    rng.range(2.5, 6.0),
                    rng.range(1.2, 3.0),
                    rng.range(1.2, 3.0),
                );
                let r = rng.range(0.1, ly.min(lz) * 0.45);
                let mut b = Body::new();
                b.block(Vec3::ZERO, lx, ly, lz).unwrap();
                let e = edge_at(&b, Vec3::new(lx * 0.5, 0.0, lz)).unwrap();
                (
                    "unblend",
                    b.fillet_edge(e, r).ok().map(|mut blended| {
                        let (_removed, _remaining) = blended.unblend_all(1e-6);
                        let got = blended.mesh_volume();
                        let want = lx * ly * lz;
                        (blended, Some((got, want, 1e-9 * (1.0 + want))))
                    }),
                )
            }
            // 12: scale. Even trials: uniform block (cubes the volume,
            // exact). Odd trials: NON-UNIFORM scale of a cylinder to an
            // elliptic cylinder (task 43), mesh vs pi*a*b*h in the
            // chordal band.
            _ if trial % 2 == 0 => {
                let s = rng.range(1.5, 3.0);
                let k = rng.range(0.5, 2.0);
                let mut b = Body::new();
                b.block(Vec3::ZERO, s, s, s).unwrap();
                (
                    "scale",
                    b.scaled(Vec3::ZERO, k).ok().map(|sc| {
                        let got = sc.mesh_volume();
                        let want = s * s * s * k * k * k;
                        (sc, Some((got, want, 1e-6 * (1.0 + want))))
                    }),
                )
            }
            _ => {
                let r = rng.range(0.6, 1.6);
                let h = rng.range(1.5, 4.0);
                let (sx, sy) = (rng.range(1.2, 2.5), rng.range(0.5, 0.9));
                let mut b = Body::new();
                b.cylinder(
                    Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
                    r,
                    h,
                )
                .unwrap();
                (
                    "scale_curved",
                    b.scaled_nonuniform(Vec3::new(0.0, 0.0, h * 0.5), Vec3::new(sx, sy, 1.0))
                        .ok()
                        .map(|sc| {
                            let got = sc.mesh_volume();
                            let want = core::f64::consts::PI * (r * sx) * (r * sy) * h;
                            (sc, Some((got, want, 2e-2 * (1.0 + want))))
                        }),
                )
            }
        };
        *family_counts.entry(name).or_insert(0) += 1;
        match result {
            None => bk.decline += 1,
            Some((body, extra)) => bk.judge(trial, name, &body, extra),
        }
    }
    eprintln!(
        "op gym: N {n}: OK {} / DECLINE {} / VIOLATIONS {}",
        bk.ok, bk.decline, bk.violations
    );
    for (k, v) in &family_counts {
        eprintln!("  family {k}: {v}");
    }
    assert_eq!(bk.violations, 0, "op gym violations: {}", bk.first);
}
