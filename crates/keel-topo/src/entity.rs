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
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
pub struct EntityId(pub u64);

/// Transient handle to a [`Vertex`] in a body's arena.
pub type VertexKey = Key<Vertex>;
/// Transient handle to an [`Edge`] in a body's arena.
pub type EdgeKey = Key<Edge>;
/// Transient handle to a [`Fin`] in a body's arena.
pub type FinKey = Key<Fin>;
/// Transient handle to a [`Loop`] in a body's arena.
pub type LoopKey = Key<Loop>;
/// Transient handle to a [`Face`] in a body's arena.
pub type FaceKey = Key<Face>;
/// Transient handle to a [`Shell`] in a body's arena.
pub type ShellKey = Key<Shell>;
/// Transient handle to a [`Region`] in a body's arena.
pub type RegionKey = Key<Region>;
/// Transient handle to attached curve geometry.
pub type CurveKey = Key<CurveGeom>;
/// Transient handle to attached surface geometry.
pub type SurfaceKey = Key<SurfaceGeom>;

/// Untyped reference to any topological entity, used by the id map and
/// lineage. Match on it to recover the typed key (e.g. for
/// [`Body::lookup`](crate::Body::lookup)).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize,
)]
pub enum AnyKey {
    /// A vertex.
    Vertex(VertexKey),
    /// An edge.
    Edge(EdgeKey),
    /// A fin (directed edge use).
    Fin(FinKey),
    /// A loop.
    Loop(LoopKey),
    /// A face.
    Face(FaceKey),
    /// A shell.
    Shell(ShellKey),
    /// A region.
    Region(RegionKey),
}

/// Curve geometry attached to edges (canonical storage, shared by
/// reference; analytics first-class per spec D4). Curve3 already
/// carries analytic and NURBS variants.
pub type CurveGeom = keel_geom::curve::Curve3;

/// Surface geometry attached to faces: either a first-class analytic
/// surface or a NURBS surface.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SurfaceGeom {
    /// An exact analytic surface (plane, cylinder, cone, sphere, torus).
    Analytic(keel_geom::surface::Surface3),
    /// A NURBS surface.
    Nurbs(keel_geom::nurbs_surface::NurbsSurface),
}

/// A point in space (a 0-cell). Carries its position, modeling
/// tolerance, and the incident fins that meet at it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    /// Stable identity of this vertex.
    pub id: EntityId,
    /// Position in model coordinates.
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

/// A curve segment between two vertices (a 1-cell). Carries its curve
/// geometry, end vertices, and the fins that wind around it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    /// Stable identity of this edge.
    pub id: EntityId,
    /// Geometry reference + sense (curve direction vs edge direction).
    pub curve: Option<(CurveKey, bool)>,
    /// (start, end); equal keys = closed edge with a seam vertex.
    pub bounds: (VertexKey, VertexKey),
    /// For a CIRCULAR arc edge, the signed angular sweep (radians, in
    /// (-2pi, 2pi)) from `bounds.0` to `bounds.1` in the circle's own
    /// frame, disambiguating the minor vs major arc that endpoints alone
    /// cannot. `None` (the default) means "use the short span" -- the
    /// behaviour every arc relied on before partial revolve, so existing
    /// arcs are unaffected. Set only where an arc may exceed pi (e.g. a
    /// wide-angle `revolve_partial`).
    pub arc_sweep: Option<f64>,
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

/// A directed use of an edge by one loop (a half-edge). The fin ring of
/// a loop and the radial cycle of an edge are both built from fins.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Fin {
    /// Stable identity of this fin.
    pub id: EntityId,
    /// The edge this fin uses.
    pub edge: EdgeKey,
    /// True = traverses the edge from bounds.0 to bounds.1.
    pub forward: bool,
    /// The loop that owns this fin.
    pub owner: LoopKey,
    /// Next fin in the owning loop's ring.
    pub next: FinKey,
    /// Previous fin in the owning loop's ring.
    pub prev: FinKey,
    /// Reserved: pcurve of the edge in the owning face's surface
    /// parameter space (filled by M4/M5 trimming).
    pub pcurve: Option<(CurveKey, bool)>,
}

/// Whether a loop is a face's outer boundary or an inner (hole) boundary.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum LoopKind {
    /// The face's outer boundary.
    Outer,
    /// An inner boundary (a hole in the face).
    Inner,
}

/// A closed boundary of a face (a ring of fins). A face has one outer
/// loop and zero or more inner loops; an isolated vertex in a face is a
/// degenerate vertex loop.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Loop {
    /// Stable identity of this loop.
    pub id: EntityId,
    /// The face this loop bounds.
    pub face: FaceKey,
    /// Entry fin; None = vertex loop (isolated vertex in the face).
    pub fin: Option<FinKey>,
    /// Some iff vertex loop.
    pub vertex: Option<VertexKey>,
    pub kind: LoopKind,
}

/// One of a face's two sides. The surface normal points out of the
/// `Front` side toward `front_region`.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize,
)]
pub enum Side {
    /// The side the surface normal points out of.
    Front,
    /// The opposite side.
    Back,
}

impl Side {
    /// The opposite side.
    #[inline]
    pub fn flipped(self) -> Side {
        match self {
            Side::Front => Side::Back,
            Side::Back => Side::Front,
        }
    }
}

/// A bounded piece of surface (a 2-cell): its surface geometry, its
/// boundary loops, and the regions on either side.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Face {
    /// Stable identity of this face.
    pub id: EntityId,
    /// Surface geometry + sense (surface normal vs face Front side);
    /// `None` until geometry is attached.
    pub surface: Option<(SurfaceKey, bool)>,
    /// `loops[0]` is the outer loop by convention.
    pub loops: Vec<LoopKey>,
    /// Region on the face's Front side (the side the normal points out of).
    pub front_region: RegionKey,
    /// Region on the face's Back side.
    pub back_region: RegionKey,
}

/// A connected boundary of a region: a set of faces (each with the side
/// facing the region), plus any wire edges or a lone acorn vertex.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Shell {
    /// Stable identity of this shell.
    pub id: EntityId,
    /// The region this shell bounds.
    pub region: RegionKey,
    /// Boundary faces, each tagged with the side facing into `region`.
    pub faces: Vec<(FaceKey, Side)>,
    /// Wire (free) edges carried by this shell.
    pub wires: Vec<EdgeKey>,
    /// A lone isolated vertex, if this is an acorn shell.
    pub acorn: Option<VertexKey>,
    /// Genus contribution of this shell's closed surface; maintained
    /// only by kfmrh/mfkrh.
    pub genus: u32,
}

/// A volume of space (a 3-cell): solid material or void, bounded by one
/// or more shells. The unbounded `infinite` region is the outside world.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Region {
    /// Stable identity of this region.
    pub id: EntityId,
    /// True if the region is filled material; false if void.
    pub solid: bool,
    /// True for the single unbounded region (the exterior).
    pub infinite: bool,
    /// Boundary shells of this region.
    pub shells: Vec<ShellKey>,
}

/// Typed attribute value (gate design 1.4; extended for parity items
/// 117-120: `Vec3` carries colors/directions, `Bytes` carries raw
/// per-entity user fields).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AttrValue {
    /// A floating-point value.
    F64(f64),
    /// A signed integer value.
    I64(i64),
    /// A boolean value.
    Bool(bool),
    /// A string value.
    Str(String),
    /// Three floats (e.g. an RGB color in `[0,1]` or a direction).
    Vec3([f64; 3]),
    /// Raw user bytes.
    Bytes(Vec<u8>),
}
