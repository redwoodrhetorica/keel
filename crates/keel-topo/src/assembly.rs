//! Assembly layer (parity items 82-85): a directed acyclic graph of
//! part / subassembly DEFINITIONS and INSTANCES. Research: dossier 16.
//!
//! The recurring industry model (Parasolid partitions + app instancing,
//! OCCT XCAF shape-refs + TopLoc_Location, STEP NAUO + mapped_item): an
//! assembly is a DAG of DEFINITIONS (geometry stored once) and INSTANCES (a
//! definition reference + a placement transform). The same definition can be
//! instanced many times, so structure (who contains whom) is orthogonal to
//! placement (the transform). An OCCURRENCE is a fully-expanded usage path;
//! its WORLD placement is the composition of instance transforms along that
//! path, and its IDENTITY is the path of stable instance ids -- which
//! survives edits to the underlying geometry (the kernel-boundary property
//! persistent naming / mating attach to). The kernel provides this thin
//! tier; mating, constraints, and PLM product structure stay in the host.
//!
//! - item 82 instances: [`Instance`] (a placed definition reference).
//! - item 83 assembly DAG: a `Def` can be referenced by many instances.
//! - item 84 per-instance transforms: world = path composition (`flatten`).
//! - item 85 stable edit-surviving ids: [`InstanceId`] / the occurrence path.

use crate::Body;
use crate::body::TopoError;
use keel_math::transform::Transform3;

/// Index of a definition within an [`Assembly`].
pub type DefId = usize;

/// A stable per-instance identifier, assigned when the instance is created.
/// It is independent of geometry, so it survives edits to the body a
/// definition holds (the basis for edit-surviving occurrence identity, 85).
pub type InstanceId = u64;

/// One placed reference to a definition inside a (sub)assembly.
#[derive(Clone, Debug)]
pub struct Instance {
    pub def: DefId,
    pub transform: Transform3,
    pub id: InstanceId,
}

#[derive(Clone, Debug)]
enum Def {
    /// A leaf: a body whose geometry is in the definition's local frame.
    /// Boxed because a `Body` is much larger than the `Sub` variant.
    Part(Box<Body>),
    /// A subassembly: instances of other (already-defined) definitions.
    Sub(Vec<Instance>),
}

/// A fully expanded leaf occurrence: the path of instance ids identifying it,
/// its world placement, and the body placed into world coordinates.
pub struct Occurrence {
    pub path: Vec<InstanceId>,
    pub world: Transform3,
    pub body: Body,
}

/// An assembly: a registry of definitions plus a root. Definitions can only
/// reference EARLIER-added definitions, so the graph is acyclic by
/// construction (no cycle check needed).
#[derive(Clone, Debug, Default)]
pub struct Assembly {
    defs: Vec<Def>,
    next_id: u64,
    root: Option<DefId>,
}

impl Assembly {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a part (leaf) definition; geometry is stored once and may be
    /// instanced any number of times. Returns its id.
    pub fn add_part(&mut self, body: Body) -> DefId {
        self.defs.push(Def::Part(Box::new(body)));
        self.defs.len() - 1
    }

    /// Add a subassembly definition from `(def, transform)` children. Each
    /// becomes an [`Instance`] with a fresh stable id. The `def`s must already
    /// exist (keeping the graph acyclic).
    pub fn add_subassembly(&mut self, children: &[(DefId, Transform3)]) -> DefId {
        let instances = children
            .iter()
            .map(|&(def, transform)| {
                let id = self.next_id;
                self.next_id += 1;
                Instance { def, transform, id }
            })
            .collect();
        self.defs.push(Def::Sub(instances));
        self.defs.len() - 1
    }

    /// Set the root definition the assembly expands from.
    pub fn set_root(&mut self, def: DefId) {
        self.root = Some(def);
    }

    /// Replace the body of a part definition in place (an EDIT). Instance ids
    /// and occurrence paths are unchanged, so downstream identity survives.
    /// Returns false if `def` is not a part.
    pub fn edit_part(&mut self, def: DefId, body: Body) -> bool {
        match self.defs.get_mut(def) {
            Some(d @ Def::Part(_)) => {
                *d = Def::Part(Box::new(body));
                true
            }
            _ => false,
        }
    }

    /// Expand the DAG into all leaf occurrences (world-placed bodies). Each
    /// occurrence carries its instance-id path (identity) and the composition
    /// of instance transforms from the root (world placement). Errors only if
    /// placing a body fails (e.g. a non-rigid instance transform on a body
    /// kind `Body::transformed` does not accept).
    pub fn flatten(&self) -> Result<Vec<Occurrence>, TopoError> {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            self.walk_iter(root, &mut out)?;
        }
        Ok(out)
    }

    /// Expand the DAG by an EXPLICIT-STACK depth-first walk (was recursive).
    /// The recursion depth scaled with the assembly NESTING depth, so a deeply
    /// nested DAG could overflow the thread stack -- an abort, which bypasses
    /// the graceful-decline contract. The explicit stack runs in O(1) native
    /// stack at any nesting depth and reproduces the recursive version's output
    /// EXACTLY: leaves are emitted in pre-order (children left-to-right) and
    /// each carries the instance-id path + composed world transform from root.
    fn walk_iter(&self, root: DefId, out: &mut Vec<Occurrence>) -> Result<(), TopoError> {
        // Each frame is a node to visit with its accumulated world transform
        // and the instance-id path from the root to it. Children are pushed in
        // REVERSE so they pop (and emit) in forward order -- matching the
        // recursive `for inst in instances` traversal.
        struct Frame {
            def: DefId,
            acc: Transform3,
            path: Vec<InstanceId>,
        }
        let mut stack = vec![Frame {
            def: root,
            acc: Transform3::IDENTITY,
            path: Vec::new(),
        }];
        while let Some(Frame { def, acc, path }) = stack.pop() {
            match self.defs.get(def) {
                Some(Def::Part(body)) => {
                    out.push(Occurrence {
                        path,
                        world: acc,
                        body: body.transformed(&acc)?,
                    });
                }
                Some(Def::Sub(instances)) => {
                    for inst in instances.iter().rev() {
                        // Descend: child world = child-local placed by its
                        // transform, then by the accumulated parent transform.
                        let child_acc = inst.transform.then(acc);
                        let mut child_path = path.clone();
                        child_path.push(inst.id);
                        stack.push(Frame {
                            def: inst.def,
                            acc: child_acc,
                            path: child_path,
                        });
                    }
                }
                None => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_math::vec::Vec3;

    fn unit_box() -> Body {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        b
    }

    #[test]
    fn nested_dag_expands_to_world_occurrences() {
        // One box definition, instanced into a 2x2 grid via a SHARED
        // subassembly (the DAG: sub1 referenced twice by sub2).
        let mut a = Assembly::new();
        let part = a.add_part(unit_box()); // vol 8 at the origin
        // sub1: the box at x=0 and x=10.
        let sub1 = a.add_subassembly(&[
            (part, Transform3::IDENTITY),
            (
                part,
                Transform3::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            ),
        ]);
        // sub2: sub1 at y=0 and y=10 -> four boxes (sub1 is instanced twice).
        let sub2 = a.add_subassembly(&[
            (sub1, Transform3::IDENTITY),
            (
                sub1,
                Transform3::from_translation(Vec3::new(0.0, 10.0, 0.0)),
            ),
        ]);
        a.set_root(sub2);

        let occ = a.flatten().unwrap();
        assert_eq!(occ.len(), 4, "2x2 grid -> four leaf occurrences");
        // Every occurrence is the unit box (vol 8), validates, and its path
        // has length 2 (sub2 -> sub1 -> part). Identities are all distinct.
        let mut centers = Vec::new();
        for o in &occ {
            assert!(o.body.validate().is_ok(), "occurrence body invalid");
            assert!((o.body.mass_properties().unwrap().volume - 8.0).abs() < 1e-9);
            assert_eq!(o.path.len(), 2, "occurrence path = two instances deep");
            let bb = o.body.bounding_box();
            centers.push((bb.min + bb.max) * 0.5);
        }
        // The four box centers are the 2x2 grid corners (origin box center
        // is (1,1,1)); just check all four are distinct and total volume 32.
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert!(
                    (centers[i] - centers[j]).norm() > 1e-6,
                    "occurrences must occupy distinct positions"
                );
                assert!(
                    occ[i].path != occ[j].path,
                    "occurrence paths must be unique"
                );
            }
        }
    }

    #[test]
    fn instance_ids_survive_a_part_edit() {
        // Editing a definition's body must not change occurrence identity
        // (item 85: stable edit-surviving ids).
        let mut a = Assembly::new();
        let part = a.add_part(unit_box());
        let sub = a.add_subassembly(&[(part, Transform3::IDENTITY)]);
        a.set_root(sub);
        let paths_before: Vec<_> = a.flatten().unwrap().into_iter().map(|o| o.path).collect();
        // Edit: replace the box with a bigger box.
        let mut bigger = Body::new();
        bigger.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        assert!(a.edit_part(part, bigger));
        let after = a.flatten().unwrap();
        let paths_after: Vec<_> = after.iter().map(|o| o.path.clone()).collect();
        assert_eq!(
            paths_before, paths_after,
            "occurrence ids must survive the edit"
        );
        assert!(
            (after[0].body.mass_properties().unwrap().volume - 64.0).abs() < 1e-9,
            "edited geometry must flow through (4^3 = 64)"
        );
    }
}
