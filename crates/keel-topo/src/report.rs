//! OpReport (swap task 22; dossier 29 Part 4, confidence reporting):
//! the uniform, consumer-facing account of HOW an answer was produced.
//! Designed against a real consumer: fieldforge's BuildReport
//! {shape, warnings, failedItemIds} channel, where warnings reach the
//! user's status bar and failures mark items rather than vanish.
//!
//! The mapping contract:
//! - A clean strict result: empty warnings, tier 1, achieved 0.0.
//! - Informational faults (e.g. a handled coincident contact) become
//!   human-readable WARNINGS, never silent.
//! - A tolerant salvage reports salvaged=true, the tier, and the
//!   achieved tolerance (the most any geometry moved): no silent
//!   salvage, the doctrine's hard rule.
//! - A declined op is an Err at the call site; it never produces a
//!   report (the consumer marks the item failed). Constructors are
//!   strict by contract (clean or Err), so they need no report.

use crate::boolean::{BoolFault, BoolResult, Confidence};

#[derive(Debug, Clone, Default)]
pub struct OpReport {
    /// Human-readable, user-facing notes; empty = pristine.
    pub warnings: Vec<String>,
    /// True when a tolerant tier modified geometry to produce the
    /// answer (within `achieved_tolerance`, reported, never silent).
    pub salvaged: bool,
    /// Escalation tier that produced the answer (dossier 29):
    /// 1 = strict exact pipeline, 2 = tolerant prepare.
    pub tier: u8,
    /// The most any input geometry was moved, 0.0 = exact input.
    pub achieved_tolerance: f64,
}

/// One fault, one user-readable sentence.
pub fn fault_warning(f: &BoolFault) -> String {
    match f {
        BoolFault::Coincident(a, b) => {
            format!("coincident contact handled (face pair {a}/{b})")
        }
        other => format!("{other:?}"),
    }
}

impl OpReport {
    /// The strict-pipeline report for a result: tier 1, exact, with
    /// any informational faults surfaced as warnings.
    pub fn strict(res: &BoolResult) -> Self {
        OpReport {
            warnings: res.faults.iter().map(fault_warning).collect(),
            salvaged: false,
            tier: 1,
            achieved_tolerance: 0.0,
        }
    }

    /// The tolerant-pipeline report: confidence + surfaced faults.
    pub fn tolerant(res: &BoolResult, conf: &Confidence) -> Self {
        let mut warnings: Vec<String> = res.faults.iter().map(fault_warning).collect();
        if conf.salvaged {
            warnings.push(format!(
                "geometry salvaged at tier {} (moved up to {:.3e})",
                conf.tier, conf.achieved_tolerance
            ));
        }
        OpReport {
            warnings,
            salvaged: conf.salvaged,
            tier: conf.tier,
            achieved_tolerance: conf.achieved_tolerance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Body;
    use crate::boolean::{BoolOp, boolean, boolean_tolerant};
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    #[test]
    fn strict_clean_report_is_pristine() {
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(0.5, 0.5, 0.5), 1.0, 1.0, 1.0).unwrap();
        let res = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        let rep = OpReport::strict(&res);
        assert!(rep.warnings.is_empty());
        assert!(!rep.salvaged);
        assert_eq!(rep.tier, 1);
        assert_eq!(rep.achieved_tolerance, 0.0);
    }

    #[test]
    fn tolerant_salvage_reports_loudly() {
        // The radial-gap clearance pin (M6): a real salvage must carry
        // salvaged=true, tier 2, the achieved tolerance, and a warning.
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut drill = Body::new();
        drill.cylinder(dframe, 1.0, 2.0).unwrap();
        let holed = boolean(&plate, &drill, BoolOp::Difference, 1e-7)
            .unwrap()
            .body;
        let pframe = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut pin = Body::new();
        pin.cylinder(pframe, 1.0 - 1e-5, 1.0).unwrap();
        let (res, conf) = boolean_tolerant(&holed, &pin, BoolOp::Union, 1e-7, 1e-4).unwrap();
        let rep = OpReport::tolerant(&res, &conf);
        assert!(rep.salvaged);
        assert_eq!(rep.tier, 2);
        assert!(rep.achieved_tolerance >= 1e-6 && rep.achieved_tolerance <= 1e-4);
        assert!(
            rep.warnings.iter().any(|w| w.contains("salvaged")),
            "salvage must be loud: {:?}",
            rep.warnings
        );
    }
}
