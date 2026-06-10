//! STEP import (dossier 38: the entity-level schema-to-kernel mapping).
//! A pure-Rust ISO 10303-21 (Part 21) tokenizer + TWO-PASS resolver
//! (forward references are legal and pervasive, so pass one tables every
//! `#N = record(s)` including complex/AND instances; pass two resolves
//! references while mapping), then the planar AP203/AP214/AP242 B-rep
//! mapping: MANIFOLD_SOLID_BREP -> CLOSED_SHELL -> ADVANCED_FACE(PLANE)
//! -> FACE_(OUTER_)BOUND -> EDGE_LOOP -> ORIENTED_EDGE -> EDGE_CURVE ->
//! VERTEX_POINT. The mapping drives off entity names (shared across the
//! three APs) per the dossier, with units read from the
//! GLOBAL_UNIT_ASSIGNED_CONTEXT (SI prefixes and CONVERSION_BASED_UNIT
//! inches both convert to Keel's working millimetres).
//!
//! Assembly reuses `Body::from_polygon_faces` (item 10): each face's
//! outer loop becomes a polygon wound CCW about the outward normal (the
//! Part 21 orientation algebra: ORIENTED_EDGE.orientation picks the
//! traversal-start vertex, FACE_BOUND.orientation reverses the loop),
//! the knit machinery merges coincident vertices and glues shared edges
//! (heal-on-import step 2), and a closed set PROMOTES to a solid with
//! the mass == mesh gates intact.
//!
//! Scope (milestone 1 of the dossier's keel-io plan): planar solids,
//! single outer loop per face, straight edges. Analytic surfaces,
//! seams/pcurves, NURBS, voids, and validation-property round-trips are
//! the queued ladder; everything else DECLINES loudly, never imports a
//! wrong body. The parser itself accepts the full Part 21 grammar
//! (strings with '' escapes, enums, typed parameters, complex
//! instances, comments) and is panic-free with a bounded list depth
//! against hostile files.

use crate::Body;
use keel_geom::curve::{Circle3, Curve3, Ellipse3, Line3};
use keel_geom::knots::KnotVector;
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::nurbs_surface::NurbsSurface;
use keel_geom::surface::{Cone3, Cylinder3, Frame3, Plane3, Sphere3, Surface3, Torus3};
use keel_math::vec::{Vec3, Vec4};
use std::collections::BTreeMap;

/// Error from STEP import. `Parse` carries the byte offset where the
/// tokenizer gave up; `Unsupported` is the honest decline for geometry
/// outside the planar milestone; `Malformed` is a structurally invalid
/// entity graph; `Assemble` wraps a knit/validation failure.
#[derive(Debug)]
pub enum StepImportError {
    Parse(&'static str, usize),
    Unsupported(&'static str),
    Malformed(&'static str),
    Assemble(String),
}

/// One Part 21 attribute value.
#[derive(Debug, Clone, PartialEq)]
enum Val {
    Real(f64),
    Str(String),
    Enum(String),
    Ref(u64),
    List(Vec<Val>),
    /// Typed parameter, e.g. `LENGTH_MEASURE(25.4)`.
    Typed(String, Vec<Val>),
    /// `$` (unset OPTIONAL) and `*` (derived).
    Unset,
    Derived,
}

impl Val {
    fn as_ref_id(&self) -> Option<u64> {
        match self {
            Val::Ref(i) => Some(*i),
            _ => None,
        }
    }
    fn as_list(&self) -> Option<&[Val]> {
        match self {
            Val::List(v) => Some(v),
            _ => None,
        }
    }
    fn as_real(&self) -> Option<f64> {
        match self {
            Val::Real(x) => Some(*x),
            Val::Typed(_, args) => args.first().and_then(|a| a.as_real()),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            Val::Enum(e) if e == "T" => Some(true),
            Val::Enum(e) if e == "F" => Some(false),
            _ => None,
        }
    }
}

/// One entity record (`NAME(args)`); a complex/AND instance is a list
/// of these sharing one id.
#[derive(Debug, Clone)]
struct Record {
    name: String,
    args: Vec<Val>,
}

struct StepFile {
    ents: BTreeMap<u64, Vec<Record>>,
}

impl StepFile {
    /// The leaf record named `name` of instance `id` (a simple instance
    /// is a one-leaf complex).
    fn rec(&self, id: u64, name: &str) -> Option<&Record> {
        self.ents
            .get(&id)?
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
    }
    /// All instances carrying a leaf with this name.
    fn find_all(&self, name: &str) -> Vec<(u64, &Record)> {
        let mut out = Vec::new();
        for (id, recs) in &self.ents {
            if let Some(r) = recs.iter().find(|r| r.name.eq_ignore_ascii_case(name)) {
                out.push((*id, r));
            }
        }
        out
    }
}

const MAX_DEPTH: usize = 64;

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn err<T>(&self, m: &'static str) -> Result<T, StepImportError> {
        Err(StepImportError::Parse(m, self.i))
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    /// Skip whitespace and `/* ... */` comments.
    fn ws(&mut self) {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                self.i += 1;
            }
            if self.peek() == Some(b'/') && self.b.get(self.i + 1) == Some(&b'*') {
                self.i += 2;
                while self.i < self.b.len() {
                    if self.b[self.i] == b'*' && self.b.get(self.i + 1) == Some(&b'/') {
                        self.i += 2;
                        break;
                    }
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), StepImportError> {
        self.ws();
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            self.err("unexpected byte")
        }
    }
    fn ident(&mut self) -> Result<String, StepImportError> {
        self.ws();
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_') {
            self.i += 1;
        }
        if self.i == start {
            return self.err("expected identifier");
        }
        Ok(String::from_utf8_lossy(&self.b[start..self.i]).into_owned())
    }
    fn uint(&mut self) -> Result<u64, StepImportError> {
        self.ws();
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.i == start {
            return self.err("expected integer");
        }
        String::from_utf8_lossy(&self.b[start..self.i])
            .parse()
            .map_err(|_| StepImportError::Parse("integer overflow", start))
    }
    fn number(&mut self) -> Result<f64, StepImportError> {
        self.ws();
        let start = self.i;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == b'.') {
            self.i += 1;
        }
        if matches!(self.peek(), Some(b'E' | b'e')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        String::from_utf8_lossy(&self.b[start..self.i])
            .parse()
            .map_err(|_| StepImportError::Parse("bad number", start))
    }
    /// A quoted string; `''` escapes a quote. Extended `\X2\` escapes
    /// pass through raw (header strings are not load-bearing).
    fn string(&mut self) -> Result<String, StepImportError> {
        self.expect(b'\'')?;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == b'\'' {
                if self.peek() == Some(b'\'') {
                    out.push('\'');
                    self.i += 1;
                } else {
                    return Ok(out);
                }
            } else {
                out.push(c as char);
            }
        }
        self.err("unterminated string")
    }
    fn val(&mut self, depth: usize) -> Result<Val, StepImportError> {
        if depth > MAX_DEPTH {
            return self.err("list nesting too deep");
        }
        self.ws();
        match self.peek() {
            Some(b'#') => {
                self.i += 1;
                Ok(Val::Ref(self.uint()?))
            }
            Some(b'$') => {
                self.i += 1;
                Ok(Val::Unset)
            }
            Some(b'*') => {
                self.i += 1;
                Ok(Val::Derived)
            }
            Some(b'\'') => Ok(Val::Str(self.string()?)),
            Some(b'.') => {
                self.i += 1;
                let e = self.ident()?;
                self.expect(b'.')?;
                Ok(Val::Enum(e))
            }
            Some(b'(') => {
                self.i += 1;
                let items = self.args(depth + 1)?;
                Ok(Val::List(items))
            }
            Some(c) if c.is_ascii_digit() || c == b'-' || c == b'+' => {
                Ok(Val::Real(self.number()?))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let name = self.ident()?;
                self.expect(b'(')?;
                let args = self.args(depth + 1)?;
                Ok(Val::Typed(name, args))
            }
            _ => self.err("unexpected value"),
        }
    }
    /// Comma-separated values up to and including the closing `)`.
    fn args(&mut self, depth: usize) -> Result<Vec<Val>, StepImportError> {
        let mut out = Vec::new();
        self.ws();
        if self.peek() == Some(b')') {
            self.i += 1;
            return Ok(out);
        }
        loop {
            out.push(self.val(depth)?);
            self.ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b')') => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => return self.err("expected , or )"),
            }
        }
    }
    /// The record list for one instance: simple `NAME(args)` or complex
    /// `(NAME1(args) NAME2(args) ...)`.
    fn records(&mut self) -> Result<Vec<Record>, StepImportError> {
        self.ws();
        if self.peek() == Some(b'(') {
            self.i += 1;
            let mut recs = Vec::new();
            loop {
                self.ws();
                if self.peek() == Some(b')') {
                    self.i += 1;
                    if recs.is_empty() {
                        return self.err("empty complex instance");
                    }
                    return Ok(recs);
                }
                let name = self.ident()?;
                self.expect(b'(')?;
                let args = self.args(1)?;
                recs.push(Record { name, args });
            }
        }
        let name = self.ident()?;
        self.expect(b'(')?;
        let args = self.args(1)?;
        Ok(vec![Record { name, args }])
    }
}

/// Tokenize + table the DATA section (pass one).
fn parse(text: &str) -> Result<StepFile, StepImportError> {
    let data = text
        .find("DATA;")
        .ok_or(StepImportError::Parse("no DATA section", 0))?;
    let mut p = P {
        b: text.as_bytes(),
        i: data + 5,
    };
    let mut ents: BTreeMap<u64, Vec<Record>> = BTreeMap::new();
    loop {
        p.ws();
        match p.peek() {
            Some(b'#') => {
                p.i += 1;
                let id = p.uint()?;
                p.expect(b'=')?;
                let recs = p.records()?;
                p.expect(b';')?;
                // Last definition wins (tolerant of duplicate ids).
                ents.insert(id, recs);
            }
            Some(b'E') => {
                // ENDSEC terminates the data section.
                let save = p.i;
                let kw = p.ident()?;
                if kw == "ENDSEC" {
                    break;
                }
                return Err(StepImportError::Parse("unexpected keyword", save));
            }
            None => break,
            _ => return p.err("expected instance or ENDSEC"),
        }
    }
    Ok(StepFile { ents })
}

/// Millimetres per file length unit, from the unit context: an SI
/// length unit (prefix + .METRE.) or a CONVERSION_BASED_UNIT (inch and
/// friends) whose factor chains to an SI unit. Defaults to 1.0 (mm)
/// when no length unit is declared.
fn length_scale(f: &StepFile) -> Result<f64, StepImportError> {
    let si_mm = |args: &[Val]| -> Option<f64> {
        // SI_UNIT(prefix or $, name): metres with a prefix.
        let name = args.get(1)?;
        if !matches!(name, Val::Enum(n) if n == "METRE") {
            return None;
        }
        Some(match args.first() {
            Some(Val::Enum(p)) => match p.as_str() {
                "MILLI" => 1.0,
                "CENTI" => 10.0,
                "DECI" => 100.0,
                "KILO" => 1.0e6,
                "MICRO" => 1.0e-3,
                "NANO" => 1.0e-6,
                _ => return None,
            },
            _ => 1000.0, // unprefixed metre
        })
    };
    for (id, _) in f.find_all("LENGTH_UNIT") {
        if let Some(si) = f.rec(id, "SI_UNIT") {
            if let Some(s) = si_mm(&si.args) {
                return Ok(s);
            }
            return Err(StepImportError::Unsupported("non-metre SI length unit"));
        }
        if let Some(cb) = f.rec(id, "CONVERSION_BASED_UNIT") {
            // CONVERSION_BASED_UNIT(name, conversion_factor) where the
            // factor is a (LENGTH_)MEASURE_WITH_UNIT(value, #si_unit).
            let mid = cb
                .args
                .get(1)
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("conversion unit factor"))?;
            let mwu = f
                .rec(mid, "LENGTH_MEASURE_WITH_UNIT")
                .or_else(|| f.rec(mid, "MEASURE_WITH_UNIT"))
                .ok_or(StepImportError::Malformed("conversion measure"))?;
            let value = mwu
                .args
                .first()
                .and_then(Val::as_real)
                .ok_or(StepImportError::Malformed("conversion value"))?;
            let base = mwu
                .args
                .get(1)
                .and_then(Val::as_ref_id)
                .and_then(|bid| f.rec(bid, "SI_UNIT"))
                .and_then(|si| si_mm(&si.args))
                .ok_or(StepImportError::Unsupported("conversion base unit"))?;
            return Ok(value * base);
        }
    }
    Ok(1.0)
}

/// Radians per file plane-angle unit (degrees arrive as a
/// CONVERSION_BASED_UNIT chaining to radians). Defaults to 1.0.
fn angle_scale(f: &StepFile) -> Result<f64, StepImportError> {
    for (id, _) in f.find_all("PLANE_ANGLE_UNIT") {
        if let Some(si) = f.rec(id, "SI_UNIT") {
            if matches!(si.args.last(), Some(Val::Enum(n)) if n == "RADIAN") {
                return Ok(1.0);
            }
            return Err(StepImportError::Unsupported("non-radian SI angle unit"));
        }
        if let Some(cb) = f.rec(id, "CONVERSION_BASED_UNIT") {
            let mid = cb
                .args
                .get(1)
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("angle conversion factor"))?;
            let mwu = f
                .rec(mid, "PLANE_ANGLE_MEASURE_WITH_UNIT")
                .or_else(|| f.rec(mid, "MEASURE_WITH_UNIT"))
                .ok_or(StepImportError::Malformed("angle conversion measure"))?;
            let value = mwu
                .args
                .first()
                .and_then(Val::as_real)
                .ok_or(StepImportError::Malformed("angle conversion value"))?;
            return Ok(value);
        }
    }
    Ok(1.0)
}

/// Skip the leading name string when present: a SIMPLE instance's
/// record starts with the inherited name, a COMPLEX leaf usually does
/// not (the name lives on the REPRESENTATION_ITEM leaf).
fn unnamed(args: &[Val]) -> &[Val] {
    match args.first() {
        Some(Val::Str(_)) => &args[1..],
        _ => args,
    }
}

/// A converted STEP surface: exact analytic or NURBS.
#[derive(Clone, Debug)]
pub enum ImportedSurface {
    Analytic(Surface3),
    Nurbs(NurbsSurface),
}

impl StepFile {
    fn point3(&self, id: u64, scale: f64) -> Result<Vec3, StepImportError> {
        let cp = self
            .rec(id, "CARTESIAN_POINT")
            .ok_or(StepImportError::Malformed("cartesian_point"))?;
        let coords = unnamed(&cp.args)
            .first()
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("point coordinates"))?;
        if coords.len() < 3 {
            return Err(StepImportError::Malformed("point dimensionality"));
        }
        let g = |i: usize| coords.get(i).and_then(Val::as_real).unwrap_or(0.0);
        Ok(Vec3::new(g(0) * scale, g(1) * scale, g(2) * scale))
    }

    fn dir3(&self, id: u64) -> Result<Vec3, StepImportError> {
        let d = self
            .rec(id, "DIRECTION")
            .ok_or(StepImportError::Malformed("direction"))?;
        let ratios = unnamed(&d.args)
            .first()
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("direction ratios"))?;
        let g = |i: usize| ratios.get(i).and_then(Val::as_real).unwrap_or(0.0);
        // Files are NOT guaranteed to normalize (classic import bug).
        Vec3::new(g(0), g(1), g(2))
            .try_normalize()
            .ok_or(StepImportError::Malformed("zero direction"))
    }

    /// AXIS2_PLACEMENT_3D -> orthonormal Keel frame: Z = axis (default
    /// global Z when `$`), X = ref_direction GRAM-SCHMIDT projected
    /// orthogonal to Z (files may supply a non-orthogonal seed; default
    /// global X), Y = Z x X.
    fn frame_at(&self, id: u64, scale: f64) -> Result<Frame3, StepImportError> {
        let ax = self
            .rec(id, "AXIS2_PLACEMENT_3D")
            .ok_or(StepImportError::Malformed("axis2_placement_3d"))?;
        let a = unnamed(&ax.args);
        let origin = a
            .first()
            .and_then(Val::as_ref_id)
            .map(|pid| self.point3(pid, scale))
            .transpose()?
            .ok_or(StepImportError::Malformed("placement location"))?;
        let z = match a.get(1) {
            Some(Val::Ref(did)) => self.dir3(*did)?,
            _ => Vec3::new(0.0, 0.0, 1.0),
        };
        let x_seed = match a.get(2) {
            Some(Val::Ref(did)) => self.dir3(*did)?,
            _ => Vec3::new(1.0, 0.0, 0.0),
        };
        let x = (x_seed - z * x_seed.dot(z))
            .try_normalize()
            .or_else(|| {
                // Seed parallel to the axis: any perpendicular works.
                let alt = if z.x.abs() < 0.9 {
                    Vec3::new(1.0, 0.0, 0.0)
                } else {
                    Vec3::new(0.0, 1.0, 0.0)
                };
                (alt - z * alt.dot(z)).try_normalize()
            })
            .ok_or(StepImportError::Malformed("degenerate placement"))?;
        Ok(Frame3 {
            origin,
            x,
            y: z.cross(x),
            z,
        })
    }

    /// Expand a (multiplicities, distinct-knots) pair into the full
    /// clamped knot array.
    fn expand_knots(mults: &[Val], knots: &[Val]) -> Result<Vec<f64>, StepImportError> {
        if mults.len() != knots.len() {
            return Err(StepImportError::Malformed("knot/multiplicity mismatch"));
        }
        let mut out = Vec::new();
        for (m, k) in mults.iter().zip(knots) {
            let m = m.as_real().ok_or(StepImportError::Malformed("knot mult"))? as usize;
            let k = k
                .as_real()
                .ok_or(StepImportError::Malformed("knot value"))?;
            out.extend(core::iter::repeat_n(k, m));
        }
        Ok(out)
    }

    /// Convert a B-spline SURFACE instance (simple flattened form or
    /// complex/AND form, rational via the RATIONAL leaf or simple
    /// 13-arg layout) to a homogeneous-4D Keel NURBS surface.
    fn convert_bspline_surface(
        &self,
        id: u64,
        scale: f64,
    ) -> Result<NurbsSurface, StepImportError> {
        let base = self
            .rec(id, "B_SPLINE_SURFACE")
            .or_else(|| self.rec(id, "B_SPLINE_SURFACE_WITH_KNOTS"))
            .ok_or(StepImportError::Malformed("b_spline_surface"))?;
        let b = unnamed(&base.args);
        let u_deg = b.first().and_then(Val::as_real).map(|d| d as usize);
        let v_deg = b.get(1).and_then(Val::as_real).map(|d| d as usize);
        let (u_deg, v_deg) = match (u_deg, v_deg) {
            (Some(u), Some(v)) => (u, v),
            _ => return Err(StepImportError::Malformed("b-spline degrees")),
        };
        let grid = b
            .get(2)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("control grid"))?;
        let wk = self
            .rec(id, "B_SPLINE_SURFACE_WITH_KNOTS")
            .ok_or(StepImportError::Unsupported("b-spline without knots"))?;
        let w = unnamed(&wk.args);
        // Complex leaf carries 5 args (mults x2, knots x2, spec); the
        // simple flattened form embeds them after the 7 base args.
        let off = if w.len() >= 12 { 7 } else { 0 };
        let u_mults = w
            .get(off)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("u multiplicities"))?;
        let v_mults = w
            .get(off + 1)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("v multiplicities"))?;
        let u_knots = w
            .get(off + 2)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("u knots"))?;
        let v_knots = w
            .get(off + 3)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("v knots"))?;
        let weights = self.rec(id, "RATIONAL_B_SPLINE_SURFACE").map(|r| {
            unnamed(&r.args)
                .first()
                .and_then(Val::as_list)
                .ok_or(StepImportError::Malformed("weight grid"))
        });
        let weights = match weights {
            Some(w) => Some(w?),
            None => None,
        };
        let kv_u = KnotVector::new(u_deg, Self::expand_knots(u_mults, u_knots)?)
            .map_err(|_| StepImportError::Unsupported("non-clamped u knots (periodic?)"))?;
        let kv_v = KnotVector::new(v_deg, Self::expand_knots(v_mults, v_knots)?)
            .map_err(|_| StepImportError::Unsupported("non-clamped v knots (periodic?)"))?;
        let (nu, nv) = (kv_u.control_count(), kv_v.control_count());
        if grid.len() != nu {
            return Err(StepImportError::Malformed("control grid U count"));
        }
        // STEP grid is ROW-MAJOR BY U (outer list U, inner V), the same
        // u-outer / v-inner layout Keel stores. Pre-multiply weights:
        // homogeneous (w x, w y, w z, w), never point-plus-weight.
        let mut ctrl = Vec::with_capacity(nu * nv);
        for (iu, row) in grid.iter().enumerate() {
            let row = row
                .as_list()
                .ok_or(StepImportError::Malformed("control row"))?;
            if row.len() != nv {
                return Err(StepImportError::Malformed("control grid V count"));
            }
            for (iv, pref) in row.iter().enumerate() {
                let pid = pref
                    .as_ref_id()
                    .ok_or(StepImportError::Malformed("control point ref"))?;
                let p = self.point3(pid, scale)?;
                let w = match weights {
                    Some(wg) => wg
                        .get(iu)
                        .and_then(Val::as_list)
                        .and_then(|r| r.get(iv))
                        .and_then(Val::as_real)
                        .ok_or(StepImportError::Malformed("weight grid shape"))?,
                    None => 1.0,
                };
                ctrl.push(Vec4::new(p.x * w, p.y * w, p.z * w, w));
            }
        }
        NurbsSurface::from_homogeneous(kv_u, kv_v, ctrl)
            .map_err(|_| StepImportError::Malformed("invalid b-spline surface"))
    }

    /// Convert a B-spline CURVE instance (simple or complex form).
    fn convert_bspline_curve(&self, id: u64, scale: f64) -> Result<NurbsCurve, StepImportError> {
        let base = self
            .rec(id, "B_SPLINE_CURVE")
            .or_else(|| self.rec(id, "B_SPLINE_CURVE_WITH_KNOTS"))
            .ok_or(StepImportError::Malformed("b_spline_curve"))?;
        let b = unnamed(&base.args);
        let degree = b
            .first()
            .and_then(Val::as_real)
            .ok_or(StepImportError::Malformed("curve degree"))? as usize;
        let pts = b
            .get(1)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("curve control list"))?;
        let wk = self
            .rec(id, "B_SPLINE_CURVE_WITH_KNOTS")
            .ok_or(StepImportError::Unsupported("b-spline curve without knots"))?;
        let w = unnamed(&wk.args);
        let off = if w.len() >= 8 { 5 } else { 0 };
        let mults = w
            .get(off)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("curve multiplicities"))?;
        let knots = w
            .get(off + 1)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("curve knots"))?;
        let weights = match self.rec(id, "RATIONAL_B_SPLINE_CURVE") {
            Some(r) => Some(
                unnamed(&r.args)
                    .first()
                    .and_then(Val::as_list)
                    .ok_or(StepImportError::Malformed("curve weights"))?,
            ),
            None => None,
        };
        let kv = KnotVector::new(degree, Self::expand_knots(mults, knots)?)
            .map_err(|_| StepImportError::Unsupported("non-clamped curve knots (periodic?)"))?;
        if pts.len() != kv.control_count() {
            return Err(StepImportError::Malformed("curve control count"));
        }
        let mut ctrl = Vec::with_capacity(pts.len());
        for (i, pref) in pts.iter().enumerate() {
            let pid = pref
                .as_ref_id()
                .ok_or(StepImportError::Malformed("curve control ref"))?;
            let p = self.point3(pid, scale)?;
            let w = match weights {
                Some(wl) => wl
                    .get(i)
                    .and_then(Val::as_real)
                    .ok_or(StepImportError::Malformed("curve weight count"))?,
                None => 1.0,
            };
            ctrl.push(Vec4::new(p.x * w, p.y * w, p.z * w, w));
        }
        NurbsCurve::from_homogeneous(kv, ctrl)
            .map_err(|_| StepImportError::Malformed("invalid b-spline curve"))
    }

    /// Convert any recognized surface instance (dossier 38 table).
    fn convert_surface(
        &self,
        id: u64,
        scale: f64,
        ang: f64,
    ) -> Result<Option<ImportedSurface>, StepImportError> {
        let analytic = |s: Surface3| Ok(Some(ImportedSurface::Analytic(s)));
        let bad = |_| StepImportError::Malformed("degenerate analytic surface");
        if let Some(r) = self.rec(id, "PLANE") {
            let f = self.placement_of(r, scale)?;
            return analytic(Surface3::Plane(Plane3::new(f)));
        }
        if let Some(r) = self.rec(id, "CYLINDRICAL_SURFACE") {
            let a = unnamed(&r.args);
            let f = self.placement_of(r, scale)?;
            let radius = a.get(1).and_then(Val::as_real).unwrap_or(0.0) * scale;
            return analytic(Surface3::Cylinder(Cylinder3::new(f, radius).map_err(bad)?));
        }
        if let Some(r) = self.rec(id, "CONICAL_SURFACE") {
            let a = unnamed(&r.args);
            let f = self.placement_of(r, scale)?;
            let radius = a.get(1).and_then(Val::as_real).unwrap_or(0.0) * scale;
            let semi = a.get(2).and_then(Val::as_real).unwrap_or(0.0) * ang;
            return analytic(Surface3::Cone(Cone3::new(f, radius, semi).map_err(bad)?));
        }
        if let Some(r) = self.rec(id, "SPHERICAL_SURFACE") {
            let a = unnamed(&r.args);
            let f = self.placement_of(r, scale)?;
            let radius = a.get(1).and_then(Val::as_real).unwrap_or(0.0) * scale;
            return analytic(Surface3::Sphere(Sphere3::new(f, radius).map_err(bad)?));
        }
        if let Some(r) = self.rec(id, "TOROIDAL_SURFACE") {
            let a = unnamed(&r.args);
            let f = self.placement_of(r, scale)?;
            let major = a.get(1).and_then(Val::as_real).unwrap_or(0.0) * scale;
            let minor = a.get(2).and_then(Val::as_real).unwrap_or(0.0) * scale;
            return analytic(Surface3::Torus(Torus3::new(f, major, minor).map_err(bad)?));
        }
        if self.rec(id, "B_SPLINE_SURFACE").is_some()
            || self.rec(id, "B_SPLINE_SURFACE_WITH_KNOTS").is_some()
        {
            return Ok(Some(ImportedSurface::Nurbs(
                self.convert_bspline_surface(id, scale)?,
            )));
        }
        Ok(None)
    }

    /// The placement frame referenced by an analytic surface/conic
    /// record (first non-name argument).
    fn placement_of(&self, r: &Record, scale: f64) -> Result<Frame3, StepImportError> {
        let pid = unnamed(&r.args)
            .first()
            .and_then(Val::as_ref_id)
            .ok_or(StepImportError::Malformed("surface placement"))?;
        self.frame_at(pid, scale)
    }

    /// Convert any recognized curve instance.
    fn convert_curve(&self, id: u64, scale: f64) -> Result<Option<Curve3>, StepImportError> {
        if let Some(r) = self.rec(id, "LINE") {
            let a = unnamed(&r.args);
            let p = a
                .first()
                .and_then(Val::as_ref_id)
                .map(|pid| self.point3(pid, scale))
                .transpose()?
                .ok_or(StepImportError::Malformed("line point"))?;
            let vid = a
                .get(1)
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("line vector"))?;
            let v = self
                .rec(vid, "VECTOR")
                .ok_or(StepImportError::Malformed("vector"))?;
            let did = unnamed(&v.args)
                .first()
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("vector direction"))?;
            let dir = self.dir3(did)?;
            let line =
                Line3::new(p, dir).map_err(|_| StepImportError::Malformed("degenerate line"))?;
            return Ok(Some(Curve3::Line(line)));
        }
        if let Some(r) = self.rec(id, "CIRCLE") {
            let a = unnamed(&r.args);
            let f = self.placement_of(r, scale)?;
            let radius = a.get(1).and_then(Val::as_real).unwrap_or(0.0) * scale;
            let c = Circle3::new(f.origin, f.x, f.y, radius)
                .map_err(|_| StepImportError::Malformed("degenerate circle"))?;
            return Ok(Some(Curve3::Circle(c)));
        }
        if let Some(r) = self.rec(id, "ELLIPSE") {
            let a = unnamed(&r.args);
            let f = self.placement_of(r, scale)?;
            let sa = a.get(1).and_then(Val::as_real).unwrap_or(0.0) * scale;
            let sb = a.get(2).and_then(Val::as_real).unwrap_or(0.0) * scale;
            let e = Ellipse3::new(f.origin, f.x, f.y, sa, sb)
                .map_err(|_| StepImportError::Malformed("degenerate ellipse"))?;
            return Ok(Some(Curve3::Ellipse(e)));
        }
        if self.rec(id, "B_SPLINE_CURVE").is_some()
            || self.rec(id, "B_SPLINE_CURVE_WITH_KNOTS").is_some()
        {
            return Ok(Some(Curve3::Nurbs(self.convert_bspline_curve(id, scale)?)));
        }
        Ok(None)
    }
}

/// Convert every recognized SURFACE entity in a Part 21 file (dossier
/// 38 build-plan step 3, the geometry layer of import milestone 2):
/// analytic placements orthonormalized, coordinates in millimetres,
/// angles in radians, NURBS in homogeneous 4D. Curved TOPOLOGY assembly
/// (seams, pcurves) is the next milestone; this layer is what it
/// consumes.
pub fn surfaces_from_step(text: &str) -> Result<Vec<ImportedSurface>, StepImportError> {
    let f = parse(text)?;
    let scale = length_scale(&f)?;
    let ang = angle_scale(&f)?;
    let ids: Vec<u64> = f.ents.keys().copied().collect();
    let mut out = Vec::new();
    for id in ids {
        if let Some(s) = f.convert_surface(id, scale, ang)? {
            out.push(s);
        }
    }
    Ok(out)
}

/// Validation properties stored in the file (CAx-IF GVP, dossier 38
/// sec 9), converted to Keel units. Tolerant scan: any
/// VOLUME_MEASURE / AREA_MEASURE typed value anywhere in a
/// measure item (simple or complex form), and a CARTESIAN_POINT item
/// of a representation named "...centroid...".
struct ValidationProps {
    volume: Option<f64>,
    area: Option<f64>,
    centroid: Option<Vec3>,
}

fn validation_props(f: &StepFile, scale: f64) -> ValidationProps {
    let mut out = ValidationProps {
        volume: None,
        area: None,
        centroid: None,
    };
    for recs in f.ents.values() {
        for r in recs {
            for a in &r.args {
                if let Val::Typed(t, args) = a {
                    let v = args.first().and_then(Val::as_real);
                    if t.eq_ignore_ascii_case("VOLUME_MEASURE") {
                        out.volume = v.map(|x| x * scale.powi(3)).or(out.volume);
                    } else if t.eq_ignore_ascii_case("AREA_MEASURE") {
                        out.area = v.map(|x| x * scale.powi(2)).or(out.area);
                    }
                }
            }
        }
    }
    for (_, r) in f.find_all("REPRESENTATION") {
        let Some(Val::Str(name)) = r.args.first() else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("centroid") {
            continue;
        }
        if let Some(pid) = r
            .args
            .get(1)
            .and_then(Val::as_list)
            .and_then(|l| l.first())
            .and_then(Val::as_ref_id)
            && let Ok(p) = f.point3(pid, scale)
        {
            out.centroid = Some(p);
        }
    }
    out
}

/// Convert every recognized CURVE entity in a Part 21 file.
pub fn curves_from_step(text: &str) -> Result<Vec<Curve3>, StepImportError> {
    let f = parse(text)?;
    let scale = length_scale(&f)?;
    let ids: Vec<u64> = f.ents.keys().copied().collect();
    let mut out = Vec::new();
    for id in ids {
        if let Some(c) = f.convert_curve(id, scale)? {
            out.push(c);
        }
    }
    Ok(out)
}

/// Import the FIRST manifold solid from a Part 21 STEP file (AP203 /
/// AP214 / AP242, shared Part 42 core). Planar milestone: every face a
/// PLANE with one outer bound of straight edges; anything else declines
/// with `Unsupported`. Coordinates convert to millimetres; assembly
/// goes through `from_polygon_faces` (knit: vertex merge, edge glue,
/// closed-shell promotion, mass == mesh gates).
pub fn from_step_string(text: &str, tol: f64) -> Result<Body, StepImportError> {
    let f = parse(text)?;
    let scale = length_scale(&f)?;
    let solids = f.find_all("MANIFOLD_SOLID_BREP");
    let (_, msb) = solids
        .first()
        .ok_or(StepImportError::Unsupported("no manifold_solid_brep"))?;
    let shell_id = msb
        .args
        .get(1)
        .and_then(Val::as_ref_id)
        .ok_or(StepImportError::Malformed("solid outer shell"))?;
    let shell = f
        .rec(shell_id, "CLOSED_SHELL")
        .ok_or(StepImportError::Malformed("outer shell kind"))?;
    let face_refs = shell
        .args
        .get(1)
        .and_then(Val::as_list)
        .ok_or(StepImportError::Malformed("shell face list"))?;

    let vertex_point = |vid: u64| -> Result<Vec3, StepImportError> {
        let vp = f
            .rec(vid, "VERTEX_POINT")
            .ok_or(StepImportError::Malformed("vertex_point"))?;
        let pid = vp
            .args
            .get(1)
            .and_then(Val::as_ref_id)
            .ok_or(StepImportError::Malformed("vertex geometry"))?;
        let cp = f
            .rec(pid, "CARTESIAN_POINT")
            .ok_or(StepImportError::Malformed("cartesian_point"))?;
        let coords = cp
            .args
            .get(1)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("point coordinates"))?;
        let g = |i: usize| coords.get(i).and_then(Val::as_real).unwrap_or(0.0);
        if coords.len() < 3 {
            return Err(StepImportError::Malformed("point dimensionality"));
        }
        Ok(Vec3::new(g(0) * scale, g(1) * scale, g(2) * scale))
    };

    let mut polys: Vec<Vec<Vec3>> = Vec::new();
    for fr in face_refs {
        let fid = fr
            .as_ref_id()
            .ok_or(StepImportError::Malformed("face reference"))?;
        let af = f
            .rec(fid, "ADVANCED_FACE")
            .or_else(|| f.rec(fid, "FACE_SURFACE"))
            .ok_or(StepImportError::Unsupported("non-advanced face kind"))?;
        let bounds = af
            .args
            .get(1)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("face bounds"))?;
        if bounds.len() != 1 {
            return Err(StepImportError::Unsupported(
                "inner loops / holed faces (follow-up)",
            ));
        }
        let sid = af
            .args
            .get(2)
            .and_then(Val::as_ref_id)
            .ok_or(StepImportError::Malformed("face surface"))?;
        if f.rec(sid, "PLANE").is_none() {
            return Err(StepImportError::Unsupported(
                "non-planar surface (follow-up)",
            ));
        }
        let bid = bounds[0]
            .as_ref_id()
            .ok_or(StepImportError::Malformed("bound reference"))?;
        let fb = f
            .rec(bid, "FACE_OUTER_BOUND")
            .or_else(|| f.rec(bid, "FACE_BOUND"))
            .ok_or(StepImportError::Malformed("face bound kind"))?;
        let lid = fb
            .args
            .get(1)
            .and_then(Val::as_ref_id)
            .ok_or(StepImportError::Malformed("bound loop"))?;
        let bound_fwd = fb.args.get(2).and_then(Val::as_bool).unwrap_or(true);
        let el = f
            .rec(lid, "EDGE_LOOP")
            .ok_or(StepImportError::Unsupported("non-edge loop (follow-up)"))?;
        let oes = el
            .args
            .get(1)
            .and_then(Val::as_list)
            .ok_or(StepImportError::Malformed("edge list"))?;
        let mut pts = Vec::with_capacity(oes.len());
        for oe_ref in oes {
            let oid = oe_ref
                .as_ref_id()
                .ok_or(StepImportError::Malformed("oriented edge ref"))?;
            let oe = f
                .rec(oid, "ORIENTED_EDGE")
                .ok_or(StepImportError::Malformed("oriented_edge"))?;
            let eid = oe
                .args
                .get(3)
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("edge element"))?;
            let fwd = oe.args.get(4).and_then(Val::as_bool).unwrap_or(true);
            let ec = f
                .rec(eid, "EDGE_CURVE")
                .ok_or(StepImportError::Malformed("edge_curve"))?;
            let cid = ec
                .args
                .get(3)
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("edge geometry"))?;
            if f.rec(cid, "LINE").is_none() && f.rec(cid, "POLYLINE").is_none() {
                return Err(StepImportError::Unsupported(
                    "curved edge geometry (follow-up)",
                ));
            }
            // Traversal-start vertex: edge start when the coedge runs
            // forward, edge end when reversed.
            let v = if fwd { ec.args.get(1) } else { ec.args.get(2) };
            let vid = v
                .and_then(Val::as_ref_id)
                .ok_or(StepImportError::Malformed("edge vertex"))?;
            pts.push(vertex_point(vid)?);
        }
        if pts.len() < 3 {
            return Err(StepImportError::Malformed("loop with fewer than 3 edges"));
        }
        // FACE_BOUND.orientation = .F. reverses the loop relative to
        // the face; STEP closed shells orient face normals OUTWARD, so
        // the corrected loop is CCW about the outward normal, exactly
        // what from_polygon_faces requires.
        if !bound_fwd {
            pts.reverse();
        }
        polys.push(pts);
    }
    let body = Body::from_polygon_faces(&polys, tol)
        .map_err(|e| StepImportError::Assemble(format!("{e:?}")))?;
    // Validation-property acceptance gate (dossier 38 sec 9): when the
    // file declares volume / area / centroid, recompute and compare; a
    // mismatch means the import built the WRONG geometry and DECLINES.
    let props = validation_props(&f, scale);
    if props.volume.is_some() || props.area.is_some() || props.centroid.is_some() {
        let mp = body
            .mass_properties()
            .map_err(|e| StepImportError::Assemble(format!("mass properties: {e:?}")))?;
        if let Some(v) = props.volume
            && (mp.volume - v).abs() > 1e-3 * v.abs().max(1.0)
        {
            return Err(StepImportError::Assemble(format!(
                "validation volume mismatch: file {v}, recomputed {}",
                mp.volume
            )));
        }
        if let Some(a) = props.area {
            let got = body.surface_area();
            if (got - a).abs() > 1e-3 * a.abs().max(1.0) {
                return Err(StepImportError::Assemble(format!(
                    "validation area mismatch: file {a}, recomputed {got}"
                )));
            }
        }
        if let Some(c) = props.centroid
            && (mp.centroid - c).norm() > (1e-6 * c.norm()).max(1e-3)
        {
            return Err(StepImportError::Assemble(format!(
                "validation centroid mismatch: file {c:?}, recomputed {:?}",
                mp.centroid
            )));
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boolean::{BoolOp, boolean};
    use crate::step_export::to_step_string;

    #[test]
    fn round_trip_block() {
        // Export a 2 x 3 x 4 block, import it back: same counts, exact
        // mass == mesh == 24, valid.
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 2.0, 3.0), 2.0, 3.0, 4.0).unwrap();
        let text = to_step_string(&b).unwrap();
        let r = from_step_string(&text, 1e-6).unwrap();
        assert!(r.validate().is_ok(), "round-trip block invalid");
        let c = r.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "block counts");
        let v = r.mass_properties().unwrap().volume;
        let mv = r.mesh_volume();
        assert!(
            (v - 24.0).abs() < 1e-9 && (mv - 24.0).abs() < 1e-9,
            "round-trip volume {v} / {mv} != 24"
        );
    }

    #[test]
    fn round_trip_boolean_result() {
        // A boolean result with oblique faces (the asymmetric-chamfer
        // class): box minus a tilted slab. Export -> import -> the same
        // exact volume, proving the orientation algebra on a non-axis-
        // aligned planar body.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let mut tool = Body::new();
        // A big tilted prism clipping the top edge: triangle profile
        // swept across y.
        let profile = [
            Vec3::new(1.0, -1.0, 2.5),
            Vec3::new(2.5, -1.0, 2.5),
            Vec3::new(2.5, -1.0, 1.0),
        ];
        tool.prism(&profile, Vec3::new(0.0, 4.0, 0.0)).unwrap();
        let res = boolean(&a, &tool, BoolOp::Difference, 1e-7).unwrap();
        let want = res.body.mass_properties().unwrap().volume;
        let text = to_step_string(&res.body).unwrap();
        let r = from_step_string(&text, 1e-6).unwrap();
        assert!(r.validate().is_ok(), "round-trip chamfer invalid");
        let v = r.mass_properties().unwrap().volume;
        let mv = r.mesh_volume();
        assert!(
            (v - want).abs() < 1e-9 && (v - mv).abs() < 1e-9,
            "round-trip chamfer volume {v} (mesh {mv}) != {want}"
        );
    }

    #[test]
    fn foreign_file_with_metre_units_and_flipped_senses() {
        // A hand-written foreign unit cube in METRES (no .MILLI.
        // prefix), exercising: forward references, a comment, a complex
        // unit context, FACE_BOUND orientation .F. (with the loop
        // stored CW so the flip restores CCW), and reversed
        // ORIENTED_EDGEs. Imports to a 1000 mm cube.
        let text = step_cube_text("", 1.0);
        let r = from_step_string(&text, 1e-3).unwrap();
        assert!(r.validate().is_ok(), "foreign cube invalid");
        let v = r.mass_properties().unwrap().volume;
        assert!((v - 1.0e9).abs() < 1.0, "metre cube volume {v} != 1e9 mm^3");
    }

    #[test]
    fn foreign_file_in_inches() {
        // CONVERSION_BASED_UNIT('INCH') = 25.4 mm: a unit cube imports
        // at 25.4^3 mm^3.
        let unit = "#200=(CONVERSION_BASED_UNIT('INCH',#201)LENGTH_UNIT()NAMED_UNIT(*));\n#201=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(25.4),#202);\n#202=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n";
        let text = step_cube_text(unit, 1.0);
        let r = from_step_string(&text, 1e-3).unwrap();
        let v = r.mass_properties().unwrap().volume;
        let want = 25.4f64.powi(3);
        assert!(
            (v - want).abs() < 1e-6 * want,
            "inch cube volume {v} != {want}"
        );
    }

    #[test]
    fn curved_and_holed_faces_decline() {
        // A cylinder face must DECLINE (planar milestone), never import
        // a wrong body.
        let text =
            step_cube_text("", 1.0).replace("PLANE('',#90)", "CYLINDRICAL_SURFACE('',#90,1.0)");
        assert!(matches!(
            from_step_string(&text, 1e-3),
            Err(StepImportError::Unsupported(_))
        ));
    }

    #[test]
    fn tampered_validation_volume_declines() {
        // The export now embeds CAx-IF validation properties; the
        // importer recomputes them as its acceptance oracle. A tampered
        // volume must DECLINE the import (the round-trip tests prove
        // the untampered path passes the same gate).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let text = to_step_string(&b).unwrap();
        let mp = b.mass_properties().unwrap();
        let needle = format!("VOLUME_MEASURE({:?})", mp.volume);
        assert!(text.contains(needle.as_str()), "props embedded");
        let bad = text.replace(needle.as_str(), "VOLUME_MEASURE(25.0)");
        assert!(matches!(
            from_step_string(&bad, 1e-6),
            Err(StepImportError::Assemble(_))
        ));
        // A tampered centroid also declines.
        let needle = format!(
            "({:?},{:?},{:?})",
            mp.centroid.x, mp.centroid.y, mp.centroid.z
        );
        let bad = text.replace(needle.as_str(), "(9.0,9.0,9.0)");
        assert!(matches!(
            from_step_string(&bad, 1e-6),
            Err(StepImportError::Assemble(_))
        ));
    }

    #[test]
    fn analytic_surfaces_convert_with_dirty_placements_and_units() {
        // METRE units, a NON-NORMALIZED axis, and a ref_direction NOT
        // orthogonal to the axis (both classic import bugs, dossier 38
        // sec 5): every analytic surface converts with an orthonormal
        // frame and millimetre radii.
        let text = "DATA;\n\
            #10=CARTESIAN_POINT('',(0.,0.,0.001));\n\
            #11=DIRECTION('',(0.,0.,2.));\n\
            #12=DIRECTION('',(1.,0.,0.5));\n\
            #13=AXIS2_PLACEMENT_3D('',#10,#11,#12);\n\
            #20=CYLINDRICAL_SURFACE('',#13,0.002);\n\
            #21=CONICAL_SURFACE('',#13,0.003,0.5);\n\
            #22=SPHERICAL_SURFACE('',#13,0.004);\n\
            #23=TOROIDAL_SURFACE('',#13,0.005,0.001);\n\
            #24=PLANE('',#13);\n\
            #200=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.));\n\
            #201=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));\n\
            ENDSEC;";
        let surfs = surfaces_from_step(text).unwrap();
        assert_eq!(surfs.len(), 5, "five analytic surfaces");
        let mut seen_cyl = false;
        for s in &surfs {
            let ImportedSurface::Analytic(a) = s else {
                panic!("unexpected NURBS")
            };
            let f = match a {
                keel_geom::surface::Surface3::Plane(p) => &p.frame,
                keel_geom::surface::Surface3::Cylinder(c) => {
                    assert!((c.radius - 2.0).abs() < 1e-12, "cyl radius {}", c.radius);
                    seen_cyl = true;
                    &c.frame
                }
                keel_geom::surface::Surface3::Cone(c) => {
                    assert!((c.radius - 3.0).abs() < 1e-12);
                    assert!((c.half_angle - 0.5).abs() < 1e-12);
                    &c.frame
                }
                keel_geom::surface::Surface3::Sphere(sp) => {
                    assert!((sp.radius - 4.0).abs() < 1e-12);
                    &sp.frame
                }
                keel_geom::surface::Surface3::Torus(t) => {
                    assert!((t.major - 5.0).abs() < 1e-12 && (t.minor - 1.0).abs() < 1e-12);
                    &t.frame
                }
            };
            // Orthonormal, Gram-Schmidt corrected, metre-scaled origin.
            assert!((f.z - Vec3::new(0., 0., 1.)).norm() < 1e-12);
            assert!((f.x - Vec3::new(1., 0., 0.)).norm() < 1e-12, "{:?}", f.x);
            assert!(f.x.dot(f.z).abs() < 1e-12 && (f.y - f.z.cross(f.x)).norm() < 1e-12);
            assert!((f.origin - Vec3::new(0., 0., 1.0)).norm() < 1e-12);
        }
        assert!(seen_cyl);
    }

    #[test]
    fn rational_bspline_surface_complex_instance_is_exact() {
        // THE high-stakes conversion (dossier 38 sec 5): a complex/AND
        // instance gluing B_SPLINE_SURFACE + ..._WITH_KNOTS +
        // RATIONAL_B_SPLINE_SURFACE. The patch is an exact rational
        // quarter cylinder (radius 2 about z, w = sqrt(2)/2 middle
        // row): every sampled point must satisfy x^2 + y^2 = 4 to
        // 1e-12, proving knot expansion, the row-major-by-U grid, and
        // the homogeneous weight pre-multiplication.
        let text = "DATA;\n\
            #30=CARTESIAN_POINT('',(2.,0.,0.));\n\
            #31=CARTESIAN_POINT('',(2.,0.,3.));\n\
            #32=CARTESIAN_POINT('',(2.,2.,0.));\n\
            #33=CARTESIAN_POINT('',(2.,2.,3.));\n\
            #34=CARTESIAN_POINT('',(0.,2.,0.));\n\
            #35=CARTESIAN_POINT('',(0.,2.,3.));\n\
            #40=(B_SPLINE_SURFACE(2,1,((#30,#31),(#32,#33),(#34,#35)),.UNSPECIFIED.,.F.,.F.,.F.)\n\
              B_SPLINE_SURFACE_WITH_KNOTS((3,3),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)\n\
              GEOMETRIC_REPRESENTATION_ITEM()\n\
              RATIONAL_B_SPLINE_SURFACE(((1.,1.),(0.7071067811865476,0.7071067811865476),(1.,1.)))\n\
              REPRESENTATION_ITEM('') SURFACE() BOUNDED_SURFACE());\n\
            ENDSEC;";
        let surfs = surfaces_from_step(text).unwrap();
        assert_eq!(surfs.len(), 1);
        let ImportedSurface::Nurbs(n) = &surfs[0] else {
            panic!("expected NURBS")
        };
        let ((u0, u1), (v0, v1)) = n.domain();
        for i in 0..=8 {
            for j in 0..=4 {
                let u = u0 + (u1 - u0) * i as f64 / 8.0;
                let v = v0 + (v1 - v0) * j as f64 / 4.0;
                let p = n.point(u, v);
                let r = (p.x * p.x + p.y * p.y).sqrt();
                assert!((r - 2.0).abs() < 1e-12, "off cylinder: {r} at ({u},{v})");
                assert!((-1e-9..=3.0 + 1e-9).contains(&p.z));
            }
        }
    }

    #[test]
    fn curves_convert_including_rational_arcs() {
        let text = "DATA;\n\
            #10=CARTESIAN_POINT('',(1.,1.,0.));\n\
            #11=DIRECTION('',(0.,0.,1.));\n\
            #12=DIRECTION('',(1.,0.,0.));\n\
            #13=AXIS2_PLACEMENT_3D('',#10,#11,#12);\n\
            #30=CARTESIAN_POINT('',(2.,0.,0.));\n\
            #32=CARTESIAN_POINT('',(2.,2.,0.));\n\
            #34=CARTESIAN_POINT('',(0.,2.,0.));\n\
            #50=CIRCLE('',#13,1.5);\n\
            #60=B_SPLINE_CURVE_WITH_KNOTS('',1,(#30,#34),.UNSPECIFIED.,.F.,.F.,(2,2),(0.,1.),.PIECEWISE_BEZIER_KNOTS.);\n\
            #61=(B_SPLINE_CURVE(2,(#30,#32,#34),.UNSPECIFIED.,.F.,.F.)\n\
              B_SPLINE_CURVE_WITH_KNOTS((3,3),(0.,1.),.UNSPECIFIED.)\n\
              RATIONAL_B_SPLINE_CURVE((1.,0.7071067811865476,1.))\n\
              BOUNDED_CURVE() CURVE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(''));\n\
            ENDSEC;";
        let curves = curves_from_step(text).unwrap();
        assert_eq!(curves.len(), 3);
        let mut found_circle = false;
        let mut found_arc = false;
        for c in &curves {
            match c {
                keel_geom::curve::Curve3::Circle(ci) => {
                    assert!((ci.radius - 1.5).abs() < 1e-12);
                    let p = ci.point(0.0);
                    assert!((p - Vec3::new(2.5, 1.0, 0.0)).norm() < 1e-12);
                    found_circle = true;
                }
                keel_geom::curve::Curve3::Nurbs(n) => {
                    let (t0, t1) = n.domain();
                    if n.degree() == 2 {
                        // The rational quarter arc: exactly on r = 2.
                        for i in 0..=8 {
                            let t = t0 + (t1 - t0) * i as f64 / 8.0;
                            let p = n.point(t);
                            let r = (p.x * p.x + p.y * p.y).sqrt();
                            assert!((r - 2.0).abs() < 1e-12, "arc off circle: {r}");
                        }
                        found_arc = true;
                    } else {
                        assert!((n.point(t0) - Vec3::new(2., 0., 0.)).norm() < 1e-12);
                    }
                }
                other => panic!("unexpected curve {other:?}"),
            }
        }
        assert!(found_circle && found_arc);
    }

    /// A hand-written AP203 cube [0,s]^3 with one face bound stored CW
    /// and flagged .F., several reversed oriented edges, forward
    /// references, and a comment. `unit_override` (Part 21 text)
    /// replaces the default metre unit context when non-empty.
    fn step_cube_text(unit_override: &str, s: f64) -> String {
        let mut t = String::new();
        t.push_str("ISO-10303-21;\nHEADER;\n");
        t.push_str("FILE_DESCRIPTION(('hand cube'),'2;1');\n");
        t.push_str("FILE_NAME('cube.step','',('keel ''test'''),(''),'','','');\n");
        t.push_str("FILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));\nENDSEC;\nDATA;\n");
        t.push_str("/* a comment the tokenizer must skip */\n");
        // 8 vertices of [0,s]^3: index bit pattern zyx.
        for k in 0..8u32 {
            let (x, y, z) = (
                if k & 1 != 0 { s } else { 0.0 },
                if k & 2 != 0 { s } else { 0.0 },
                if k & 4 != 0 { s } else { 0.0 },
            );
            t.push_str(&format!(
                "#{}=CARTESIAN_POINT('',({x:.1},{y:.1},{z:.1}));\n",
                10 + k
            ));
            t.push_str(&format!("#{}=VERTEX_POINT('',#{});\n", 20 + k, 10 + k));
        }
        // One LINE reused by every edge (geometry is unused for vertex
        // sequencing in the planar milestone; a real exporter writes one
        // per edge).
        t.push_str("#30=DIRECTION('',(1.,0.,0.));\n#31=VECTOR('',#30,1.0);\n");
        t.push_str("#32=LINE('',#10,#31);\n");
        // 12 edges (vertex index pairs).
        let edges: [(u32, u32); 12] = [
            (0, 1),
            (1, 3),
            (3, 2),
            (2, 0), // bottom ring (z=0)
            (4, 5),
            (5, 7),
            (7, 6),
            (6, 4), // top ring (z=s)
            (0, 4),
            (1, 5),
            (3, 7),
            (2, 6), // verticals
        ];
        for (i, (a, b)) in edges.iter().enumerate() {
            t.push_str(&format!(
                "#{}=EDGE_CURVE('',#{},#{},#32,.T.);\n",
                40 + i as u32,
                20 + a,
                20 + b
            ));
        }
        // Faces: each an ORIENTED_EDGE loop CCW about the OUTWARD
        // normal, except the bottom face which is stored CW with a
        // FACE_BOUND orientation .F. (the flip must restore it).
        // (edge index, forward) per face.
        let faces: [(&[(usize, bool)], bool); 6] = [
            // bottom z=0, outward -z; CCW about -z is 0->2->3->1; we
            // store the REVERSE (CW) and flag the bound .F..
            (&[(0, true), (1, true), (2, true), (3, true)], false),
            // top z=s, outward +z: 4->5->7->6.
            (&[(4, true), (5, true), (6, true), (7, true)], true),
            // y=0 side, outward -y: 0->1, 1->5, 5->4, 4->0.
            (&[(0, true), (9, true), (4, false), (8, false)], true),
            // x=s side, outward +x: 1->3, 3->7, 7->5, 5->1.
            (&[(1, true), (10, true), (5, false), (9, false)], true),
            // y=s side, outward +y: 3->2, 2->6, 6->7, 7->3.
            (&[(2, true), (11, true), (6, false), (10, false)], true),
            // x=0 side, outward -x: 2->0, 0->4, 4->6, 6->2.
            (&[(3, true), (8, true), (7, false), (11, false)], true),
        ];
        let mut oe_id = 300u32;
        let mut face_ids = Vec::new();
        for (fi, (loop_edges, bound_fwd)) in faces.iter().enumerate() {
            let mut oe_refs = Vec::new();
            for (ei, fwd) in loop_edges.iter() {
                let flag = if *fwd { ".T." } else { ".F." };
                t.push_str(&format!(
                    "#{}=ORIENTED_EDGE('',*,*,#{},{});\n",
                    oe_id,
                    40 + *ei as u32,
                    flag
                ));
                oe_refs.push(format!("#{oe_id}"));
                oe_id += 1;
            }
            let lid = oe_id;
            t.push_str(&format!(
                "#{}=EDGE_LOOP('',({}));\n",
                lid,
                oe_refs.join(",")
            ));
            let bid = oe_id + 1;
            let bf = if *bound_fwd { ".T." } else { ".F." };
            t.push_str(&format!("#{bid}=FACE_OUTER_BOUND('',#{lid},{bf});\n"));
            let fid = oe_id + 2;
            // Forward reference: the PLANEs (#91..#96) are defined
            // AFTER the faces that use them.
            t.push_str(&format!(
                "#{fid}=ADVANCED_FACE('',(#{bid}),#{},.T.);\n",
                91 + fi
            ));
            face_ids.push(format!("#{fid}"));
            oe_id += 3;
        }
        for fi in 0..6 {
            t.push_str(&format!("#{}=PLANE('',#90);\n", 91 + fi));
        }
        // The placement is shared and unused by the planar milestone.
        t.push_str("#90=AXIS2_PLACEMENT_3D('',#10,$,$);\n");
        t.push_str(&format!(
            "#150=CLOSED_SHELL('',({}));\n",
            face_ids.join(",")
        ));
        t.push_str("#151=MANIFOLD_SOLID_BREP('cube',#150);\n");
        if unit_override.is_empty() {
            // Unprefixed METRE length unit: coordinates are metres.
            t.push_str("#200=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.));\n");
        } else {
            t.push_str(unit_override);
        }
        t.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        t
    }
}
