//! BUG EXTRACTOR: run realistic nested CAD sessions, and for every op that
//! declines/fails, record the whole chain that led to it, dedupe by error
//! signature, delta-debug each down to the MINIMAL prior-feature sequence that
//! still triggers it, and emit a ranked list of distinct gaps -- each a
//! paste-able minimal repro. Output is a FIX-LIST, not a score.
//! Run: cargo run --release -p keel-topo --example bug_extract -- [parts] [seed]
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use std::collections::HashMap;

const TOL: f64 = 1e-7;

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn r(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    fn pick(&mut self, n: u32) -> u32 {
        (self.f() * n as f64) as u32
    }
}

#[derive(Clone)]
enum Tool {
    Blk {
        o: Vec3,
        dx: f64,
        dy: f64,
        dz: f64,
    },
    Cyl {
        pos: Vec3,
        axis: Vec3,
        r: f64,
        h: f64,
    },
    Cone {
        pos: Vec3,
        axis: Vec3,
        r: f64,
        h: f64,
    },
    Sph {
        c: Vec3,
        r: f64,
    },
}
fn v(p: Vec3) -> String {
    format!("Vec3::new({:.3}, {:.3}, {:.3})", p.x, p.y, p.z)
}
impl Tool {
    fn build(&self) -> Body {
        let mut b = Body::new();
        match *self {
            Tool::Blk { o, dx, dy, dz } => b.block(o, dx, dy, dz).unwrap(),
            Tool::Cyl { pos, axis, r, h } => b
                .cylinder(Frame3::from_z(pos, axis).unwrap(), r, h)
                .unwrap(),
            Tool::Cone { pos, axis, r, h } => {
                b.cone(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap()
            }
            Tool::Sph { c, r } => b
                .sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r)
                .unwrap(),
        };
        b
    }
    fn code(&self) -> String {
        match *self {
            Tool::Blk { o, dx, dy, dz } => {
                format!("blk({}, {:.3}, {:.3}, {:.3})", v(o), dx, dy, dz)
            }
            Tool::Cyl { pos, axis, r, h } => {
                format!("cyl({}, {}, {:.3}, {:.3})", v(pos), v(axis), r, h)
            }
            Tool::Cone { pos, axis, r, h } => {
                format!("cone({}, {}, {:.3}, {:.3})", v(pos), v(axis), r, h)
            }
            Tool::Sph { c, r } => format!("sph({}, {:.3})", v(c), r),
        }
    }
}
#[derive(Clone)]
struct Feat {
    label: &'static str,
    tool: Tool,
    op: BoolOp,
}

fn opname(op: BoolOp) -> &'static str {
    match op {
        BoolOp::Union => "Union",
        BoolOp::Intersection => "Intersection",
        BoolOp::Difference => "Difference",
    }
}

/// Build the body from a stock + a sequence of feats; None if any feat fails to
/// land cleanly (so the prefix is a valid build).
fn replay(stock: &Tool, feats: &[Feat]) -> Option<Body> {
    let mut body = stock.build();
    for f in feats {
        match boolean(&body, &f.tool.build(), f.op, TOL) {
            Ok(r) if r.faults.is_empty() && r.body.validate().is_ok() => body = r.body,
            _ => return None,
        }
    }
    Some(body)
}

/// Does `fail` still NOT cleanly land on stock+prefix? (true = reproduces gap)
fn reproduces(stock: &Tool, prefix: &[Feat], fail: &Feat) -> bool {
    let Some(body) = replay(stock, prefix) else {
        return false; // prefix no longer builds -> not a clean repro
    };
    !matches!(boolean(&body, &fail.tool.build(), fail.op, TOL), Ok(r) if r.faults.is_empty() && r.body.validate().is_ok())
}

/// Greedily drop prior features that aren't needed to trigger the failure.
fn minimize(stock: &Tool, prefix: &[Feat], fail: &Feat) -> Vec<Feat> {
    let mut cur = prefix.to_vec();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..cur.len() {
            let mut cand = cur.clone();
            cand.remove(i);
            if reproduces(stock, &cand, fail) {
                cur = cand;
                changed = true;
                break;
            }
        }
    }
    cur
}

fn err_sig(stock: &Tool, prefix: &[Feat], fail: &Feat) -> String {
    let Some(body) = replay(stock, prefix) else {
        return "prefix-unbuildable".into();
    };
    match boolean(&body, &fail.tool.build(), fail.op, TOL) {
        Err(e) => format!("{e:?}"),
        Ok(r) if !r.faults.is_empty() => format!("faults {:?}", r.faults),
        Ok(r) => {
            if r.body.validate().is_err() {
                "INVALID-shell".into()
            } else {
                "valid (no repro)".into()
            }
        }
    }
}
// Coarse signature key: collapse instance ids so duplicates merge.
fn sig_key(raw: &str, label: &str, op: BoolOp) -> String {
    let head: String = raw
        .split(['(', '['])
        .next()
        .unwrap_or(raw)
        .trim()
        .to_string();
    format!("{head} | {label} {}", opname(op))
}

fn feature(rng: &mut Rng, cx: f64, cy: f64, s: f64, h: f64) -> Feat {
    let z = Vec3::new(0., 0., 1.);
    let jit = s * 0.25;
    let (x, y) = (cx + rng.r(-jit, jit), cy + rng.r(-jit, jit));
    let rmax = (s * 0.6).max(0.3);
    let table: &[(u32, u8)] = &[
        (22, 0),
        (12, 1),
        (14, 2),
        (14, 3),
        (8, 4),
        (8, 5),
        (8, 6),
        (7, 7),
        (7, 8),
    ];
    let tot: u32 = table.iter().map(|x| x.0).sum();
    let mut pk = rng.pick(tot);
    let mut t = 0u8;
    for &(w, id) in table {
        if pk < w {
            t = id;
            break;
        }
        pk -= w;
    }
    match t {
        0 => Feat {
            label: "through-hole",
            tool: Tool::Cyl {
                pos: Vec3::new(x, y, -0.5),
                axis: z,
                r: rng.r(0.3, rmax),
                h: h + 1.0,
            },
            op: BoolOp::Difference,
        },
        1 => {
            let dep = rng.r(0.3, h * 0.7);
            Feat {
                label: "blind-hole",
                tool: Tool::Cyl {
                    pos: Vec3::new(x, y, h - dep),
                    axis: z,
                    r: rng.r(0.3, rmax),
                    h: dep + 0.6,
                },
                op: BoolOp::Difference,
            }
        }
        2 => {
            let (pw, pd) = (rng.r(0.6, s), rng.r(0.6, s));
            let dep = rng.r(0.3, h * 0.7);
            Feat {
                label: "pocket",
                tool: Tool::Blk {
                    o: Vec3::new(x - pw / 2.0, y - pd / 2.0, h - dep),
                    dx: pw,
                    dy: pd,
                    dz: dep + 0.6,
                },
                op: BoolOp::Difference,
            }
        }
        3 => Feat {
            label: "boss-cyl",
            tool: Tool::Cyl {
                pos: Vec3::new(x, y, h - 0.02),
                axis: z,
                r: rng.r(0.3, rmax),
                h: rng.r(0.6, 2.5),
            },
            op: BoolOp::Union,
        },
        4 => {
            let (bw, bd) = (rng.r(0.6, s), rng.r(0.6, s));
            Feat {
                label: "boss-block",
                tool: Tool::Blk {
                    o: Vec3::new(x - bw / 2.0, y - bd / 2.0, h - 0.02),
                    dx: bw,
                    dy: bd,
                    dz: rng.r(0.6, 2.5),
                },
                op: BoolOp::Union,
            }
        }
        5 => {
            let dep = rng.r(0.4, h * 0.6);
            Feat {
                label: "countersink",
                tool: Tool::Cone {
                    pos: Vec3::new(x, y, h + 0.05),
                    axis: Vec3::new(0., 0., -1.),
                    r: rng.r(0.5, rmax),
                    h: dep + 0.05,
                },
                op: BoolOp::Difference,
            }
        }
        6 => Feat {
            label: "dome",
            tool: Tool::Sph {
                c: Vec3::new(x, y, h),
                r: rng.r(0.5, rmax),
            },
            op: BoolOp::Union,
        },
        7 => {
            let r = rng.r(0.5, rmax.min(h));
            Feat {
                label: "ball-cavity",
                tool: Tool::Sph {
                    c: Vec3::new(x, y, h - r * 0.4),
                    r,
                },
                op: BoolOp::Difference,
            }
        }
        _ => {
            let r = rng.r(0.25, rmax.min(h / 2.5));
            let zc = rng.r(r + 0.2, h - r - 0.2).max(r + 0.2);
            Feat {
                label: "cross-hole",
                tool: Tool::Cyl {
                    pos: Vec3::new(x - s - 0.5, y, zc),
                    axis: Vec3::new(1., 0., 0.),
                    r,
                    h: 2.0 * s + 1.0,
                },
                op: BoolOp::Difference,
            }
        }
    }
}

struct Hit {
    count: u64,
    stock: Tool,
    prefix: Vec<Feat>,
    fail: Feat,
    raw: String,
}

fn main() {
    let parts: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let seed: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut rng = Rng(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1);
    let mut hits: HashMap<String, Hit> = HashMap::new();
    let mut total_fail = 0u64;
    for _ in 0..parts {
        let cell = 4.5;
        let s = cell / 2.0;
        let (nx, ny) = (3 + rng.pick(3) as usize, 3 + rng.pick(3) as usize);
        let (w, d, h) = (nx as f64 * cell, ny as f64 * cell, rng.r(2.0, 4.0));
        let stock = Tool::Blk {
            o: Vec3::ZERO,
            dx: w,
            dy: d,
            dz: h,
        };
        let mut body = stock.build();
        let mut landed: Vec<Feat> = Vec::new();
        for ix in 0..nx {
            for iy in 0..ny {
                let f = feature(
                    &mut rng,
                    (ix as f64 + 0.5) * cell,
                    (iy as f64 + 0.5) * cell,
                    s,
                    h,
                );
                let clean = match boolean(&body, &f.tool.build(), f.op, TOL) {
                    Ok(r) if r.faults.is_empty() && r.body.validate().is_ok() => Some(r.body),
                    _ => None,
                };
                match clean {
                    Some(nb) => {
                        body = nb;
                        landed.push(f);
                    }
                    None => {
                        total_fail += 1;
                        let raw = err_sig(&stock, &landed, &f);
                        if raw == "valid (no repro)" {
                            continue;
                        }
                        let key = sig_key(&raw, f.label, f.op);
                        let e = hits.entry(key).or_insert_with(|| Hit {
                            count: 0,
                            stock: stock.clone(),
                            prefix: landed.clone(),
                            fail: f.clone(),
                            raw: raw.clone(),
                        });
                        e.count += 1;
                        // keep the shortest-prefix instance
                        if landed.len() < e.prefix.len() {
                            e.stock = stock.clone();
                            e.prefix = landed.clone();
                            e.fail = f.clone();
                            e.raw = raw.clone();
                        }
                    }
                }
            }
        }
    }
    let mut v: Vec<(&String, &Hit)> = hits.iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1.count));
    println!(
        "=== BUG EXTRACT ({parts} parts, seed {seed}): {total_fail} failing ops, {} distinct gaps ===\n",
        v.len()
    );
    for (i, (key, hit)) in v.iter().enumerate().take(14) {
        let minp = minimize(&hit.stock, &hit.prefix, &hit.fail);
        println!("#{} [{} hits]  {key}", i + 1, hit.count);
        println!("   error: {}", hit.raw);
        println!("   MINIMAL REPRO ({} prior feature(s)):", minp.len());
        println!("     let mut b = {};", hit.stock.code());
        for f in &minp {
            println!(
                "     b = boolean(&b, &{}, BoolOp::{}, 1e-7).unwrap().body; // {}",
                f.tool.code(),
                opname(f.op),
                f.label
            );
        }
        println!(
            "     // FAILS: boolean(&b, &{}, BoolOp::{}, 1e-7)  [{}]",
            hit.fail.tool.code(),
            opname(hit.fail.op),
            hit.fail.label
        );
        println!();
    }
}
