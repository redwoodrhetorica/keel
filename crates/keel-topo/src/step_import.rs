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
use keel_math::vec::Vec3;
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
    Body::from_polygon_faces(&polys, tol).map_err(|e| StepImportError::Assemble(format!("{e:?}")))
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
