//! STEP AP203 export (parity interchange, roadmap 0d; the crusst peer
//! borrowable). Maps the B-rep topology directly to STEP entities with
//! NO tessellation: VERTEX_POINT / EDGE_CURVE / ADVANCED_FACE /
//! CLOSED_SHELL / MANIFOLD_SOLID_BREP under AUTOMOTIVE_DESIGN. Covers the
//! full analytic set: PLANE / CYLINDRICAL_SURFACE / CONICAL_SURFACE /
//! SPHERICAL_SURFACE / TOROIDAL_SURFACE with LINE / CIRCLE / ELLIPSE edge
//! curves and FACE_BOUND inner loops (rings). NURBS
//! (B_SPLINE_SURFACE_WITH_KNOTS) and vertex loops are the next slice.

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
        // {:?} prints the SHORTEST ROUND-TRIP decimal for f64 and always
        // keeps a decimal point / exponent (a bare integer would be the
        // wrong Part 21 token kind), so save -> load preserves every bit
        // (the corpus-audit serialization finding).
        self.add(&format!(
            "CARTESIAN_POINT('',({:?},{:?},{:?}))",
            p.x, p.y, p.z
        ))
    }
    fn dir(&mut self, d: keel_math::vec::Vec3) -> usize {
        self.add(&format!("DIRECTION('',({:?},{:?},{:?}))", d.x, d.y, d.z))
    }
    /// AXIS2_PLACEMENT_3D from an origin, axis (z) and ref direction (x).
    fn axis2(
        &mut self,
        origin: keel_math::vec::Vec3,
        z: keel_math::vec::Vec3,
        x: keel_math::vec::Vec3,
    ) -> usize {
        let o = self.point(origin);
        let zd = self.dir(z);
        let xd = self.dir(x);
        self.add(&format!("AXIS2_PLACEMENT_3D('',#{o},#{zd},#{xd})"))
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
            // The curved slice: rim circles and cap ellipses (the
            // analytic primitives' and booleans' edge vocabulary).
            Some(Curve3::Circle(ci)) => {
                let n = ci.x_axis.cross(ci.y_axis);
                let ax = s.axis2(ci.center, n, ci.x_axis);
                s.add(&format!("CIRCLE('',#{ax},{:?})", ci.radius))
            }
            Some(Curve3::Ellipse(el)) => {
                let n = el.x_axis.cross(el.y_axis);
                let ax = s.axis2(el.center, n, el.x_axis);
                s.add(&format!("ELLIPSE('',#{ax},{:?},{:?})", el.a, el.b))
            }
            Some(Curve3::Nurbs(_)) => {
                return Err(StepError::Unsupported("NURBS edge curve (next slice)"));
            }
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
            Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => {
                let ax = s.axis2(c.frame.origin, c.frame.z, c.frame.x);
                let sense = body
                    .face(fk)
                    .and_then(|f| f.surface)
                    .map(|(_, sn)| sn)
                    .unwrap_or(true);
                (
                    s.add(&format!("CYLINDRICAL_SURFACE('',#{ax},{:?})", c.radius)),
                    sense,
                )
            }
            Some(SurfaceGeom::Analytic(Surface3::Cone(c))) => {
                let ax = s.axis2(c.frame.origin, c.frame.z, c.frame.x);
                let sense = body
                    .face(fk)
                    .and_then(|f| f.surface)
                    .map(|(_, sn)| sn)
                    .unwrap_or(true);
                (
                    s.add(&format!(
                        "CONICAL_SURFACE('',#{ax},{:?},{:?})",
                        c.radius,
                        c.half_angle.abs()
                    )),
                    sense,
                )
            }
            Some(SurfaceGeom::Analytic(Surface3::Sphere(c))) => {
                let ax = s.axis2(c.frame.origin, c.frame.z, c.frame.x);
                let sense = body
                    .face(fk)
                    .and_then(|f| f.surface)
                    .map(|(_, sn)| sn)
                    .unwrap_or(true);
                (
                    s.add(&format!("SPHERICAL_SURFACE('',#{ax},{:?})", c.radius)),
                    sense,
                )
            }
            Some(SurfaceGeom::Analytic(Surface3::Torus(c))) => {
                let ax = s.axis2(c.frame.origin, c.frame.z, c.frame.x);
                let sense = body
                    .face(fk)
                    .and_then(|f| f.surface)
                    .map(|(_, sn)| sn)
                    .unwrap_or(true);
                (
                    s.add(&format!(
                        "TOROIDAL_SURFACE('',#{ax},{:?},{:?})",
                        c.major, c.minor
                    )),
                    sense,
                )
            }
            Some(SurfaceGeom::Nurbs(_)) => {
                return Err(StepError::Unsupported("NURBS surface"));
            }
            None => return Err(StepError::BadTopology),
        };

        // ALL loops: the first is the FACE_OUTER_BOUND, the rest are
        // FACE_BOUNDs (rings: a drilled plate's hole must export, the
        // planar slice only walked the outer loop).
        let loops = body
            .faces
            .get(fk)
            .map(|f| f.loops.clone())
            .ok_or(StepError::BadTopology)?;
        if loops.is_empty() {
            return Err(StepError::BadTopology);
        }
        let mut bounds = Vec::new();
        for (li, lp) in loops.iter().enumerate() {
            let first = body
                .loops
                .get(*lp)
                .and_then(|l| l.fin)
                .ok_or(StepError::Unsupported("vertex loop (next slice)"))?;
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
            let kind = if li == 0 {
                "FACE_OUTER_BOUND"
            } else {
                "FACE_BOUND"
            };
            bounds.push(s.add(&format!("{kind}('',#{loop_id},.T.)")));
        }
        let bound_refs: Vec<String> = bounds.iter().map(|i| format!("#{i}")).collect();
        let ss = if same_sense { ".T." } else { ".F." };
        face_ids.push(s.add(&format!(
            "ADVANCED_FACE('',({}),#{surf_id},{ss})",
            bound_refs.join(",")
        )));
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

    // Geometric validation properties (CAx-IF GVP practice, dossier 38
    // sec 9): volume, surface area, and centroid, so a receiver can
    // recompute and confirm the geometry survived translation. Keel's
    // importer treats a mismatch as a hard decline.
    if let Ok(mp) = body.mass_properties() {
        let area = body.surface_area();
        let vol_item = s.add(&format!(
            "MEASURE_REPRESENTATION_ITEM('volume measure',VOLUME_MEASURE({:?}),$)",
            mp.volume
        ));
        let vol_rep = s.add(&format!(
            "REPRESENTATION('geometric validation property volume',(#{vol_item}),#{ctx})"
        ));
        let vol_def = s.add("PROPERTY_DEFINITION('geometric validation property','volume',$)");
        s.add(&format!(
            "PROPERTY_DEFINITION_REPRESENTATION(#{vol_def},#{vol_rep})"
        ));
        let area_item = s.add(&format!(
            "MEASURE_REPRESENTATION_ITEM('surface area measure',AREA_MEASURE({:?}),$)",
            area
        ));
        let area_rep = s.add(&format!(
            "REPRESENTATION('geometric validation property surface area',(#{area_item}),#{ctx})"
        ));
        let area_def =
            s.add("PROPERTY_DEFINITION('geometric validation property','surface area',$)");
        s.add(&format!(
            "PROPERTY_DEFINITION_REPRESENTATION(#{area_def},#{area_rep})"
        ));
        let c = mp.centroid;
        let c_pt = s.add(&format!(
            "CARTESIAN_POINT('centre point',({:?},{:?},{:?}))",
            c.x, c.y, c.z
        ));
        let c_rep = s.add(&format!(
            "REPRESENTATION('geometric validation property centroid',(#{c_pt}),#{ctx})"
        ));
        let c_def = s.add("PROPERTY_DEFINITION('geometric validation property','centroid',$)");
        s.add(&format!(
            "PROPERTY_DEFINITION_REPRESENTATION(#{c_def},#{c_rep})"
        ));
    }

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
    fn cylinder_exports_curved_entities_that_reimport_exactly() {
        use crate::step_import::{ImportedSurface, curves_from_step, surfaces_from_step};
        use keel_geom::curve::Curve3;
        use keel_geom::surface::Surface3;

        let mut b = Body::new();
        let f = keel_geom::surface::Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        b.cylinder(f, 1.0, 2.0).unwrap();
        let step = to_step_string(&b).unwrap();
        // Geometry round trip through the importer's parsing layer: the
        // lateral cylinder and both rim circles must come back exactly.
        // (Full curved BODY reassembly is the importer's next milestone.)
        let surfs = surfaces_from_step(&step).unwrap();
        let cyl = surfs
            .iter()
            .find_map(|s| match s {
                ImportedSurface::Analytic(Surface3::Cylinder(c)) => Some(c),
                _ => None,
            })
            .expect("cylindrical surface survives the round trip");
        assert!((cyl.radius - 1.0).abs() < 1e-12);
        assert!((cyl.frame.z.z.abs() - 1.0).abs() < 1e-12);
        let circles: Vec<_> = curves_from_step(&step)
            .unwrap()
            .into_iter()
            .filter_map(|c| match c {
                Curve3::Circle(ci) => Some(ci),
                _ => None,
            })
            .collect();
        assert_eq!(circles.len(), 2, "two rim circles");
        for ci in &circles {
            assert!((ci.radius - 1.0).abs() < 1e-12);
        }
        // The embedded validation volume is the exact analytic mass.
        let v = b.mass_properties().unwrap().volume;
        assert!((v - std::f64::consts::PI * 2.0).abs() < 1e-12);
        assert!(step.contains(&format!("VOLUME_MEASURE({v:?})")));
    }

    #[test]
    fn drilled_plate_exports_hole_rings_as_inner_bounds() {
        use crate::step_import::{ImportedSurface, surfaces_from_step};
        use keel_geom::surface::Surface3;

        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let mut drill = Body::new();
        let f =
            keel_geom::surface::Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0))
                .unwrap();
        drill.cylinder(f, 1.0, 2.0).unwrap();
        let holed =
            crate::boolean::boolean(&plate, &drill, crate::boolean::BoolOp::Difference, 1e-9)
                .unwrap()
                .body;
        let step = to_step_string(&holed).unwrap();
        // The hole's rings on the top and bottom plate faces must export
        // as FACE_BOUND inner loops, not be silently dropped (the planar
        // slice walked only the outer loop).
        assert_eq!(step.matches("FACE_BOUND(").count(), 2);
        let surfs = surfaces_from_step(&step).unwrap();
        let bore = surfs
            .iter()
            .find_map(|s| match s {
                ImportedSurface::Analytic(Surface3::Cylinder(c)) => Some(c),
                _ => None,
            })
            .expect("bore wall survives the round trip");
        assert!((bore.radius - 1.0).abs() < 1e-12);
    }
}
