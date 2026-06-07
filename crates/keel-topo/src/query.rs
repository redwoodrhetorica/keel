//! Adjacency and interrogation queries (gate design scope item 8) and
//! the deterministic debug text dump (the seed of the future text
//! format grammar, not the format itself).

use crate::body::Body;
use crate::entity::{
    AnyKey, EdgeKey, FaceKey, FinKey, LoopKey, RegionKey, ShellKey, Side, VertexKey,
};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Parasolid-style body classification lattice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyClass {
    /// Only the infinite region.
    Empty,
    /// Acorn vertices only.
    Acorn,
    /// Wire edges (with or without acorns), no faces.
    Wire,
    /// Faces exist but no second region (open sheets).
    Sheet,
    /// Closed solid regions, all edges manifold.
    Solid,
    /// Anything mixed-dimension or non-manifold.
    General,
}

impl Body {
    /// Fins of an edge in radial order.
    pub fn fins_of_edge(&self, e: EdgeKey) -> Vec<FinKey> {
        self.edges
            .get(e)
            .map(|x| x.radial.clone())
            .unwrap_or_default()
    }

    /// Loops of a face (outer first).
    pub fn loops_of_face(&self, f: FaceKey) -> Vec<LoopKey> {
        self.faces
            .get(f)
            .map(|x| x.loops.clone())
            .unwrap_or_default()
    }

    /// Distinct faces around an edge, in radial fin order.
    pub fn faces_around_edge(&self, e: EdgeKey) -> Vec<FaceKey> {
        let mut out = Vec::new();
        for fk in self.fins_of_edge(e) {
            if let Some(face) = self
                .fins
                .get(fk)
                .and_then(|f| self.loops.get(f.owner))
                .map(|l| l.face)
                && !out.contains(&face)
            {
                out.push(face);
            }
        }
        out
    }

    /// Edges incident to a vertex (by bounds scan; the umbrella-walk
    /// optimization lands when profiling demands it). EntityId order.
    pub fn edges_of_vertex(&self, v: VertexKey) -> Vec<EdgeKey> {
        let mut pairs: Vec<(crate::entity::EntityId, EdgeKey)> = self
            .edges
            .iter()
            .filter(|(_, e)| e.bounds.0 == v || e.bounds.1 == v)
            .map(|(k, e)| (e.id, k))
            .collect();
        pairs.sort_unstable_by_key(|(id, _)| *id);
        pairs.into_iter().map(|(_, k)| k).collect()
    }

    /// Shells of a region.
    pub fn shells_of_region(&self, r: RegionKey) -> Vec<ShellKey> {
        self.regions
            .get(r)
            .map(|x| x.shells.clone())
            .unwrap_or_default()
    }

    /// Connected components of shells: shells sharing any edge (via
    /// their faces) or vertex are one component. EntityId-deterministic.
    pub fn connected_components(&self) -> Vec<Vec<ShellKey>> {
        // Union shells through shared edges.
        let shell_list: Vec<ShellKey> = {
            let mut v: Vec<(crate::entity::EntityId, ShellKey)> =
                self.shells.iter().map(|(k, s)| (s.id, k)).collect();
            v.sort_unstable_by_key(|(id, _)| *id);
            v.into_iter().map(|(_, k)| k).collect()
        };
        let mut comp: Vec<usize> = (0..shell_list.len()).collect();
        fn find(comp: &mut Vec<usize>, i: usize) -> usize {
            if comp[i] != i {
                let r = find(comp, comp[i]);
                comp[i] = r;
            }
            comp[i]
        }
        // Map each edge to the shells touching it.
        let mut edge_shells: std::collections::BTreeMap<crate::entity::EntityId, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (si, &sk) in shell_list.iter().enumerate() {
            let Some(shell) = self.shells.get(sk) else {
                continue;
            };
            let mut edge_ids: BTreeSet<crate::entity::EntityId> = BTreeSet::new();
            for &(fk, _) in &shell.faces {
                for lk in self.loops_of_face(fk) {
                    let Some(l) = self.loops.get(lk) else {
                        continue;
                    };
                    let Some(entry) = l.fin else { continue };
                    let mut cur = entry;
                    while let Some(f) = self.fins.get(cur) {
                        if let Some(e) = self.edges.get(f.edge) {
                            edge_ids.insert(e.id);
                        }
                        cur = f.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
            }
            for &ek in &shell.wires {
                if let Some(e) = self.edges.get(ek) {
                    edge_ids.insert(e.id);
                }
            }
            for eid in edge_ids {
                edge_shells.entry(eid).or_default().push(si);
            }
        }
        for (_, sis) in edge_shells {
            for w in sis.windows(2) {
                let (a, b) = (find(&mut comp, w[0]), find(&mut comp, w[1]));
                if a != b {
                    comp[a] = b;
                }
            }
        }
        let mut groups: std::collections::BTreeMap<usize, Vec<ShellKey>> =
            std::collections::BTreeMap::new();
        for (i, &sk) in shell_list.iter().enumerate() {
            let root = find(&mut comp, i);
            groups.entry(root).or_default().push(sk);
        }
        groups.into_values().collect()
    }

    /// Classify the body per the Parasolid lattice.
    pub fn body_class(&self) -> BodyClass {
        let has_faces = !self.faces.is_empty();
        let has_wires = self.shells.iter().any(|(_, s)| !s.wires.is_empty());
        let has_acorns = self.shells.iter().any(|(_, s)| s.acorn.is_some());
        let nonmanifold = self.edges.iter().any(|(_, e)| e.radial.len() > 2)
            || self.vertices.iter().any(|(_, v)| !v.groups.is_empty());
        let closed_regions = self.regions.len() > 1;
        if !has_faces && !has_wires && !has_acorns {
            return BodyClass::Empty;
        }
        if nonmanifold || (has_faces && has_wires) || (has_faces && has_acorns) {
            return BodyClass::General;
        }
        if has_faces {
            return if closed_regions {
                BodyClass::Solid
            } else {
                BodyClass::Sheet
            };
        }
        if has_wires {
            return BodyClass::Wire;
        }
        BodyClass::Acorn
    }

    /// Deterministic, EntityId-ordered text dump. Grammar (one entity
    /// per line, fields space-separated, ids are `#n`):
    ///   `V #id (x y z) tol fin=#id|-`
    ///   `E #id #v0 #v1 radial[#f ...] curve=…|-`
    ///   `N #id edge=#e fwd|rev loop=#l next=#f prev=#f`
    ///   `L #id face=#f outer|inner entry=#f|vertex=#v`
    ///   `F #id loops[#l ...] front=#r back=#r surface=yes|-`
    ///   `S #id region=#r faces[(#f F|B) ...] wires[#e ...] acorn=#v|- genus=g`
    ///   `R #id solid|void finite|infinite shells[#s ...]`
    pub fn dump(&self) -> String {
        let mut out = String::new();
        let id_of = |k: AnyKey| -> u64 {
            match k {
                AnyKey::Vertex(k) => self.vertices.get(k).map(|x| x.id.0),
                AnyKey::Edge(k) => self.edges.get(k).map(|x| x.id.0),
                AnyKey::Fin(k) => self.fins.get(k).map(|x| x.id.0),
                AnyKey::Loop(k) => self.loops.get(k).map(|x| x.id.0),
                AnyKey::Face(k) => self.faces.get(k).map(|x| x.id.0),
                AnyKey::Shell(k) => self.shells.get(k).map(|x| x.id.0),
                AnyKey::Region(k) => self.regions.get(k).map(|x| x.id.0),
            }
            .unwrap_or(u64::MAX)
        };
        for (id, key) in self.ids.iter() {
            match *key {
                AnyKey::Vertex(k) => {
                    let Some(v) = self.vertices.get(k) else {
                        continue;
                    };
                    let fin = v
                        .fin
                        .map(|f| format!("#{}", id_of(AnyKey::Fin(f))))
                        .unwrap_or_else(|| "-".into());
                    let _ = writeln!(
                        out,
                        "V #{} ({:?} {:?} {:?}) {:?} fin={}",
                        id.0, v.point.x, v.point.y, v.point.z, v.tolerance, fin
                    );
                }
                AnyKey::Edge(k) => {
                    let Some(e) = self.edges.get(k) else { continue };
                    let radial: Vec<String> = e
                        .radial
                        .iter()
                        .map(|&f| format!("#{}", id_of(AnyKey::Fin(f))))
                        .collect();
                    let _ = writeln!(
                        out,
                        "E #{} #{} #{} radial[{}] curve={}",
                        id.0,
                        id_of(AnyKey::Vertex(e.bounds.0)),
                        id_of(AnyKey::Vertex(e.bounds.1)),
                        radial.join(" "),
                        if e.curve.is_some() { "yes" } else { "-" }
                    );
                }
                AnyKey::Fin(k) => {
                    let Some(f) = self.fins.get(k) else { continue };
                    let _ = writeln!(
                        out,
                        "N #{} edge=#{} {} loop=#{} next=#{} prev=#{}",
                        id.0,
                        id_of(AnyKey::Edge(f.edge)),
                        if f.forward { "fwd" } else { "rev" },
                        id_of(AnyKey::Loop(f.owner)),
                        id_of(AnyKey::Fin(f.next)),
                        id_of(AnyKey::Fin(f.prev)),
                    );
                }
                AnyKey::Loop(k) => {
                    let Some(l) = self.loops.get(k) else { continue };
                    let body = match (l.fin, l.vertex) {
                        (Some(f), _) => format!("entry=#{}", id_of(AnyKey::Fin(f))),
                        (_, Some(v)) => format!("vertex=#{}", id_of(AnyKey::Vertex(v))),
                        _ => "-".into(),
                    };
                    let _ = writeln!(
                        out,
                        "L #{} face=#{} {} {}",
                        id.0,
                        id_of(AnyKey::Face(l.face)),
                        if l.kind == crate::entity::LoopKind::Outer {
                            "outer"
                        } else {
                            "inner"
                        },
                        body
                    );
                }
                AnyKey::Face(k) => {
                    let Some(f) = self.faces.get(k) else { continue };
                    let loops: Vec<String> = f
                        .loops
                        .iter()
                        .map(|&l| format!("#{}", id_of(AnyKey::Loop(l))))
                        .collect();
                    let _ = writeln!(
                        out,
                        "F #{} loops[{}] front=#{} back=#{} surface={}",
                        id.0,
                        loops.join(" "),
                        id_of(AnyKey::Region(f.front_region)),
                        id_of(AnyKey::Region(f.back_region)),
                        if f.surface.is_some() { "yes" } else { "-" }
                    );
                }
                AnyKey::Shell(k) => {
                    let Some(s) = self.shells.get(k) else {
                        continue;
                    };
                    let faces: Vec<String> = s
                        .faces
                        .iter()
                        .map(|&(f, side)| {
                            format!(
                                "(#{} {})",
                                id_of(AnyKey::Face(f)),
                                if side == Side::Front { "F" } else { "B" }
                            )
                        })
                        .collect();
                    let wires: Vec<String> = s
                        .wires
                        .iter()
                        .map(|&e| format!("#{}", id_of(AnyKey::Edge(e))))
                        .collect();
                    let acorn = s
                        .acorn
                        .map(|v| format!("#{}", id_of(AnyKey::Vertex(v))))
                        .unwrap_or_else(|| "-".into());
                    let _ = writeln!(
                        out,
                        "S #{} region=#{} faces[{}] wires[{}] acorn={} genus={}",
                        id.0,
                        id_of(AnyKey::Region(s.region)),
                        faces.join(" "),
                        wires.join(" "),
                        acorn,
                        s.genus
                    );
                }
                AnyKey::Region(k) => {
                    let Some(r) = self.regions.get(k) else {
                        continue;
                    };
                    let shells: Vec<String> = r
                        .shells
                        .iter()
                        .map(|&s| format!("#{}", id_of(AnyKey::Shell(s))))
                        .collect();
                    let _ = writeln!(
                        out,
                        "R #{} {} {} shells[{}]",
                        id.0,
                        if r.solid { "solid" } else { "void" },
                        if r.infinite { "infinite" } else { "finite" },
                        shells.join(" ")
                    );
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    #[test]
    fn classification_lattice() {
        let b = Body::new();
        assert_eq!(b.body_class(), BodyClass::Empty);

        let mut b = Body::new();
        let r = b.infinite_region();
        b.embed_vertex(r, Vec3::ZERO).unwrap();
        assert_eq!(b.body_class(), BodyClass::Acorn);

        let mut b = Body::new();
        let r = b.infinite_region();
        b.embed_wire(r, None, Vec3::ZERO, Vec3::new(1., 0., 0.))
            .unwrap();
        assert_eq!(b.body_class(), BodyClass::Wire);

        let mut b = Body::new();
        b.block(Vec3::ZERO, 1., 1., 1.).unwrap();
        assert_eq!(b.body_class(), BodyClass::Solid);

        // Mixed dimension = general.
        let r = b.infinite_region();
        b.embed_wire(r, None, Vec3::new(5., 0., 0.), Vec3::new(6., 0., 0.))
            .unwrap();
        assert_eq!(b.body_class(), BodyClass::General);
    }

    #[test]
    fn adjacency_on_block() {
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 1., 1., 1.).unwrap();
        // Every block edge has exactly two distinct adjacent faces.
        for &e in &out.edges {
            assert_eq!(b.fins_of_edge(e).len(), 2);
            assert_eq!(b.faces_around_edge(e).len(), 2);
        }
        // Every vertex touches exactly three edges.
        for &v in &out.vertices {
            assert_eq!(b.edges_of_vertex(v).len(), 3);
        }
        assert_eq!(b.connected_components().len(), 1);
        // Two blocks: two components.
        b.block(Vec3::new(5., 5., 5.), 1., 1., 1.).unwrap();
        assert_eq!(b.connected_components().len(), 2);
    }

    #[test]
    fn dump_is_deterministic_and_stable() {
        let build = || -> String {
            let mut b = Body::new();
            let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
            b.cylinder(f, 1.0, 2.0).unwrap();
            b.dump()
        };
        let d1 = build();
        assert_eq!(d1, build());
        // Grammar smoke checks.
        assert!(d1.lines().any(|l| l.starts_with("R #0 void infinite")));
        assert!(d1.lines().any(|l| l.starts_with("V #")));
        assert!(d1.lines().any(|l| l.contains("genus=0")));
    }
}
