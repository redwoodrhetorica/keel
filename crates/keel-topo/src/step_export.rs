//! STEP AP203 export (parity interchange, roadmap 0d; the crusst peer
//! borrowable). Maps the B-rep topology directly to STEP entities with
//! NO tessellation: VERTEX_POINT / EDGE_CURVE / ADVANCED_FACE /
//! CLOSED_SHELL / MANIFOLD_SOLID_BREP under AUTOMOTIVE_DESIGN. This first
//! cut covers PLANAR solids (PLANE + LINE); analytic-curved and NURBS
//! surfaces (CYLINDRICAL_SURFACE / B_SPLINE_SURFACE_WITH_KNOTS) are the
//! next slice.

use crate::Body;
use crate::entity::{EdgeKey, SurfaceGeom, VertexKey};
use keel_geom::curve::Curve3;
use keel_geom::surface::Surface3;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Error from STEP export.
#[derive(Debug)]
pub enum StepError {
    /// A surface/curve type not yet supported by the exporter.
    Unsupported(&'static str),
    /// The topology could not be walked (stale reference).
    BadTopology,
}

struct Step {
    out: String,
    next: usize,
}

impl Step {
    fn new() -> Self {
        Step {
            out: String::new(),
            next: 1,
        }
    }
    /// Emit an entity body, return its `#id`.
    fn add(&mut self, body: &str) -> usize {
        let id = self.next;
        self.next += 1;
        let _ = writeln!(self.out, "#{id}={body};");
        id
    }
    fn point(&mut self, p: keel_math::vec::Vec3) -> usize {
        self.add(&format!(
            "CARTESIAN_POINT('',({:.9},{:.9},{:.9}))",
            p.x, p.y, p.z
        ))
    }
    fn dir(&mut self, d: keel_math::vec::Vec3) -> usize {
        self.add(&format!("DIRECTION('',({:.9},{:.9},{:.9}))", d.x, d.y, d.z))
    }
}

/// Export `body` as a STEP AP203 part. Returns the STEP text, or an
/// error for unsupported geometry.
pub fn to_step_string(body: &Body) -> Result<String, StepError> {
    let mut s = Step::new();

    // Vertices -> VERTEX_POINT.
    let mut vmap: HashMap<VertexKey, usize> = HashMap::new();
    let vkeys: Vec<VertexKey> = body.vertices.iter().map(|(k, _)| k).collect();
    for vk in vkeys {
        let p = body.vertices.get(vk).ok_or(StepError::BadTopology)?.point;
        let pid = s.point(p);
        let vid = s.add(&format!("VERTEX_POINT('',#{pid})"));
        vmap.insert(vk, vid);
    }

    // Edges -> curve + EDGE_CURVE.
    let mut emap: HashMap<EdgeKey, usize> = HashMap::new();
    let ekeys: Vec<EdgeKey> = body.edges.iter().map(|(k, _)| k).collect();
    for ek in ekeys {
        let e = body.edges.get(ek).ok_or(StepError::BadTopology)?;
        let (v0, v1) = e.bounds;
        let ecurve = e.curve;
        let p0 = body.vertices.get(v0).ok_or(StepError::BadTopology)?.point;
        let p1 = body.vertices.get(v1).ok_or(StepError::BadTopology)?.point;
        let curve3 = ecurve.and_then(|(ck, _)| body.curves.get(ck).cloned());
        let curve_id = match curve3 {
            Some(Curve3::Line(_)) | None => {
                // LINE through p0 with direction p0->p1.
                let d = (p1 - p0)
                    .try_normalize()
                    .ok_or(StepError::Unsupported("degenerate edge"))?;
                let op = s.point(p0);
                let dd = s.dir(d);
                let vec = s.add(&format!("VECTOR('',#{dd},1.0)"));
                s.add(&format!("LINE('',#{op},#{vec})"))
            }
            Some(_) => return Err(StepError::Unsupported("non-line edge curve")),
        };
        let sv = *vmap.get(&v0).ok_or(StepError::BadTopology)?;
        let ev = *vmap.get(&v1).ok_or(StepError::BadTopology)?;
        let ec = s.add(&format!("EDGE_CURVE('',#{sv},#{ev},#{curve_id},.T.)"));
        emap.insert(ek, ec);
    }

    // Faces -> surface + ADVANCED_FACE.
    let mut face_ids = Vec::new();
    for fk in body.face_keys() {
        let (surf_id, same_sense) = match body.face_surface_geom(fk) {
            Some(SurfaceGeom::Analytic(Surface3::Plane(p))) => {
                let o = s.point(p.frame.origin);
                let z = s.dir(p.frame.z);
                let x = s.dir(p.frame.x);
                let ax = s.add(&format!("AXIS2_PLACEMENT_3D('',#{o},#{z},#{x})"));
                let sense = body
                    .face(fk)
                    .and_then(|f| f.surface)
                    .map(|(_, sn)| sn)
                    .unwrap_or(true);
                (s.add(&format!("PLANE('',#{ax})")), sense)
            }
            Some(SurfaceGeom::Analytic(_)) => {
                return Err(StepError::Unsupported("non-planar analytic surface"));
            }
            Some(SurfaceGeom::Nurbs(_)) => {
                return Err(StepError::Unsupported("NURBS surface"));
            }
            None => return Err(StepError::BadTopology),
        };

        // Outer loop -> EDGE_LOOP of ORIENTED_EDGEs.
        let lp = body
            .faces
            .get(fk)
            .and_then(|f| f.loops.first().copied())
            .ok_or(StepError::BadTopology)?;
        let first = body
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(StepError::BadTopology)?;
        let mut fins: Vec<(EdgeKey, bool)> = Vec::new();
        let mut cur = first;
        loop {
            let fin = body.fins.get(cur).ok_or(StepError::BadTopology)?;
            fins.push((fin.edge, fin.forward));
            cur = fin.next;
            if cur == first {
                break;
            }
        }
        let mut oriented = Vec::new();
        for (ek, forward) in fins {
            let ec = *emap.get(&ek).ok_or(StepError::BadTopology)?;
            let f = if forward { ".T." } else { ".F." };
            oriented.push(s.add(&format!("ORIENTED_EDGE('',*,*,#{ec},{f})")));
        }
        let refs: Vec<String> = oriented.iter().map(|i| format!("#{i}")).collect();
        let loop_id = s.add(&format!("EDGE_LOOP('',({}))", refs.join(",")));
        let bound = s.add(&format!("FACE_OUTER_BOUND('',#{loop_id},.T.)"));
        let ss = if same_sense { ".T." } else { ".F." };
        face_ids.push(s.add(&format!("ADVANCED_FACE('',(#{bound}),#{surf_id},{ss})")));
    }

    let face_refs: Vec<String> = face_ids.iter().map(|i| format!("#{i}")).collect();
    let shell = s.add(&format!("CLOSED_SHELL('',({}))", face_refs.join(",")));
    let brep = s.add(&format!("MANIFOLD_SOLID_BREP('Keel',#{shell})"));

    // Minimal product/representation context so readers find the solid.
    let app = s.add("APPLICATION_CONTEXT('automotive design')");
    let _ = app;
    let dim = s.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))");
    let ang = s.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))");
    let sang = s.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())");
    let unc = s.add(&format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.0E-6),#{dim},'','')"
    ));
    let ctx = s.add(&format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{unc}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{dim},#{ang},#{sang}))REPRESENTATION_CONTEXT('',''))"
    ));
    s.add(&format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('Keel',(#{brep}),#{ctx})"
    ));

    // Assemble the file.
    let mut file = String::new();
    file.push_str("ISO-10303-21;\nHEADER;\n");
    file.push_str("FILE_DESCRIPTION(('Keel B-rep'),'2;1');\n");
    file.push_str("FILE_NAME('part.step','',(''),(''),'Keel','','');\n");
    file.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n");
    file.push_str(&s.out);
    file.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_math::vec::Vec3;

    #[test]
    fn block_exports_valid_step() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let step = to_step_string(&b).unwrap();
        assert!(step.starts_with("ISO-10303-21;"));
        assert!(step.contains("MANIFOLD_SOLID_BREP"));
        assert!(step.contains("CLOSED_SHELL"));
        assert!(step.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'))"));
        assert!(step.trim_end().ends_with("END-ISO-10303-21;"));
        // A box: 6 faces, 8 vertices, 12 edges.
        assert_eq!(step.matches("ADVANCED_FACE").count(), 6);
        assert_eq!(step.matches("VERTEX_POINT").count(), 8);
        assert_eq!(step.matches("EDGE_CURVE").count(), 12);
        assert_eq!(step.matches("PLANE(").count(), 6);
    }

    #[test]
    fn curved_body_declines() {
        let mut b = Body::new();
        let f = keel_geom::surface::Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        b.cylinder(f, 1.0, 2.0).unwrap();
        assert!(matches!(to_step_string(&b), Err(StepError::Unsupported(_))));
    }
}
