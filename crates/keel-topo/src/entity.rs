//! The entity tower (gate design section 1): Body -> Region -> Shell
//! -> Face -> Loop -> Fin -> Edge -> Vertex, Parasolid names.
//!
//! Conventions (binding, from the M3 plan):
//! - A fin traverses its edge from start to end; `forward = true`
//!   means the edge's (bounds.0 -> bounds.1) direction.
//! - The face surface normal points out of the Front side;
//!   `front_region` is the region whose shell uses (face, Front).
//! - A shell is owned by exactly one region; a closed manifold face
//!   set appears as TWO shells (one per side). A sheet face has both
//!   sides in the same shell.
//! - Closed edges have a seam vertex (bounds.0 == bounds.1);
//!   vertex-free ring edges are not supported.

use crate::arena::Key;
use keel_math::vec::Vec3;

/// Stable identity: monotonic per body, never reused, persisted.
/// Arena keys are transient addresses; this is the name.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EntityId(pub u64);

pub type VertexKey = Key<Vertex>;
pub type EdgeKey = Key<Edge>;
pub type FinKey = Key<Fin>;
pub type LoopKey = Key<Loop>;
pub type FaceKey = Key<Face>;
pub type ShellKey = Key<Shell>;
pub type RegionKey = Key<Region>;
pub type CurveKey = Key<CurveGeom>;
pub type SurfaceKey = Key<SurfaceGeom>;

/// Untyped reference for the id map and lineage.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum AnyKey {
    Vertex(VertexKey),
    Edge(EdgeKey),
    Fin(FinKey),
    Loop(LoopKey),
    Face(FaceKey),
    Shell(ShellKey),
    Region(RegionKey),
}

/// Curve geometry attached to edges (canonical storage, shared by
/// reference; analytics first-class per spec D4). Curve3 already
/// carries analytic and NURBS variants.
pub type CurveGeom = keel_geom::curve::Curve3;

/// Surface geometry attached to faces.
#[derive(Clone, Debug)]
pub enum SurfaceGeom {
    Analytic(keel_geom::surface::Surface3),
    Nurbs(keel_geom::nurbs_surface::NurbsSurface),
}

#[derive(Clone, Debug)]
pub struct Vertex {
    pub id: EntityId,
    pub point: Vec3,
    pub tolerance: f64,
    /// One incident fin for the manifold umbrella; None for acorn or
    /// wire-only vertices.
    pub fin: Option<FinKey>,
    /// PES partial-entity slot: one representative fin per additional
    /// umbrella at a non-manifold vertex. Empty in the manifold case;
    /// populated by merge_vertices. Completed by the M5 imprint work.
    pub groups: Vec<FinKey>,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub id: EntityId,
    /// Geometry reference + sense (curve direction vs edge direction).
    pub curve: Option<(CurveKey, bool)>,
    /// (start, end); equal keys = closed edge with a seam vertex.
    pub bounds: (VertexKey, VertexKey),
    /// Radial cycle: ALL fins using this edge, in angular order around
    /// the edge (manifold = exactly 2). Wire edges have none.
    pub radial: Vec<FinKey>,
    pub tolerance: f64,
}

impl Edge {
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.bounds.0 == self.bounds.1
    }
}

#[derive(Clone, Debug)]
pub struct Fin {
    pub id: EntityId,
    pub edge: EdgeKey,
    /// True = traverses the edge from bounds.0 to bounds.1.
    pub forward: bool,
    pub owner: LoopKey,
    pub next: FinKey,
    pub prev: FinKey,
    /// Reserved: pcurve of the edge in the owning face's surface
    /// parameter space (filled by M4/M5 trimming).
    pub pcurve: Option<(CurveKey, bool)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoopKind {
    Outer,
    Inner,
}

#[derive(Clone, Debug)]
pub struct Loop {
    pub id: EntityId,
    pub face: FaceKey,
    /// Entry fin; None = vertex loop (isolated vertex in the face).
    pub fin: Option<FinKey>,
    /// Some iff vertex loop.
    pub vertex: Option<VertexKey>,
    pub kind: LoopKind,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Side {
    Front,
    Back,
}

impl Side {
    #[inline]
    pub fn flipped(self) -> Side {
        match self {
            Side::Front => Side::Back,
            Side::Back => Side::Front,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Face {
    pub id: EntityId,
    pub surface: Option<(SurfaceKey, bool)>,
    /// loops[0] is the outer loop by convention.
    pub loops: Vec<LoopKey>,
    pub front_region: RegionKey,
    pub back_region: RegionKey,
}

#[derive(Clone, Debug)]
pub struct Shell {
    pub id: EntityId,
    pub region: RegionKey,
    pub faces: Vec<(FaceKey, Side)>,
    pub wires: Vec<EdgeKey>,
    pub acorn: Option<VertexKey>,
    /// Genus contribution of this shell's closed surface; maintained
    /// only by kfmrh/mfkrh.
    pub genus: u32,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub id: EntityId,
    pub solid: bool,
    pub infinite: bool,
    pub shells: Vec<ShellKey>,
}

/// Minimal typed attribute value (gate design 1.4).
#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    F64(f64),
    I64(i64),
    Bool(bool),
    Str(String),
}
