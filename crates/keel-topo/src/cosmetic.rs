//! Cosmetic thread features (parity item 139; dossier 25 sec 22:
//! "cosmetic thread representation is generally carried as attributes
//! ... Parasolid stores such data via its attribute system while the
//! host defines the feature semantics"). Keel does exactly that: a
//! typed thread record stored under reserved attribute keys on the
//! threaded entity (typically a cylindrical face or its edge), riding
//! the existing attribute system (items 117-121) -- so it serializes
//! with the body (126) and follows the attribute propagation rules
//! through operations (121). No thread GEOMETRY is modeled (that is the
//! Parasolid behavior being mirrored, not a shortcut).

use crate::body::Body;
use crate::entity::{AttrValue, EntityId};

/// Reserved attribute keys for the thread record.
const K_DESIGNATION: &str = "keel.thread.designation";
const K_PITCH: &str = "keel.thread.pitch";
const K_DEPTH: &str = "keel.thread.depth";
const K_RIGHT_HANDED: &str = "keel.thread.right_handed";

/// A cosmetic thread record (host semantics; kernel storage).
#[derive(Clone, Debug, PartialEq)]
pub struct CosmeticThread {
    /// E.g. "M8x1.25".
    pub designation: String,
    /// Advance per turn.
    pub pitch: f64,
    /// Thread depth along the axis.
    pub depth: f64,
    pub right_handed: bool,
}

impl Body {
    /// Attach a cosmetic thread to an entity (face/edge), item 139.
    pub fn set_cosmetic_thread(&mut self, id: EntityId, t: &CosmeticThread) {
        self.set_attr(id, K_DESIGNATION, AttrValue::Str(t.designation.clone()));
        self.set_attr(id, K_PITCH, AttrValue::F64(t.pitch));
        self.set_attr(id, K_DEPTH, AttrValue::F64(t.depth));
        self.set_attr(id, K_RIGHT_HANDED, AttrValue::Bool(t.right_handed));
    }

    /// Read back a cosmetic thread; `None` if absent or malformed.
    pub fn cosmetic_thread(&self, id: EntityId) -> Option<CosmeticThread> {
        let AttrValue::Str(designation) = self.attr(id, K_DESIGNATION)? else {
            return None;
        };
        let AttrValue::F64(pitch) = self.attr(id, K_PITCH)? else {
            return None;
        };
        let AttrValue::F64(depth) = self.attr(id, K_DEPTH)? else {
            return None;
        };
        let AttrValue::Bool(right_handed) = self.attr(id, K_RIGHT_HANDED)? else {
            return None;
        };
        Some(CosmeticThread {
            designation: designation.clone(),
            pitch: *pitch,
            depth: *depth,
            right_handed: *right_handed,
        })
    }

    /// Remove a cosmetic thread record.
    pub fn clear_cosmetic_thread(&mut self, id: EntityId) {
        for k in [K_DESIGNATION, K_PITCH, K_DEPTH, K_RIGHT_HANDED] {
            let _ = self.remove_attr(id, k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    #[test]
    fn thread_attaches_reads_back_and_persists() {
        let mut b = Body::new();
        b.cylinder(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            4.0,
            10.0,
        )
        .unwrap();
        let face = b.face_keys()[0];
        let id = b.face(face).unwrap().id;
        let t = CosmeticThread {
            designation: "M8x1.25".into(),
            pitch: 1.25,
            depth: 8.0,
            right_handed: true,
        };
        b.set_cosmetic_thread(id, &t);
        assert_eq!(b.cosmetic_thread(id).as_ref(), Some(&t));

        // Rides the body serde (item 126): survives a save round-trip.
        let json = b.to_json().unwrap();
        let b2 = Body::from_json(&json).unwrap();
        assert_eq!(b2.cosmetic_thread(id).as_ref(), Some(&t));

        let mut b3 = b;
        b3.clear_cosmetic_thread(id);
        assert_eq!(b3.cosmetic_thread(id), None);
    }
}
