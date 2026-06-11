//! OPT-M1 instrumentation (task 20): env-gated stage timers and call
//! counters for the boolean pipeline. Enabled by setting KEEL_PROFILE;
//! when disabled the only cost is one cached bool load per scope, and
//! the kernel's behavior is bit-identical either way (timers never
//! branch on results). This stays in-tree so every optimization
//! milestone can re-measure the same breakdown.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub(crate) static FRONT_DOOR_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static PREIMPRINT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static PREIMPRINT_DETECT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static PREIMPRINT_CUT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static INTERIOR_PT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static INTERIOR_PT_CALLS: AtomicU64 = AtomicU64::new(0);
pub(crate) static SEAM_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static SHORTCUT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_FILTER_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_PRESPLIT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_DISPATCH_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_OPS_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_OPS_CALLS: AtomicU64 = AtomicU64::new(0);
pub(crate) static CLOSED_IMPRINT_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static RING_SUBDIV_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_MEV_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_SPLITF_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMPRINT_SEAMGEO_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static CLASSIFY_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static STITCH_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static VALIDATE_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static MASS_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static MASS_CALLS: AtomicU64 = AtomicU64::new(0);
pub(crate) static MESHVOL_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static MESHVOL_CALLS: AtomicU64 = AtomicU64::new(0);
pub(crate) static GWN_NS: AtomicU64 = AtomicU64::new(0);
pub(crate) static GWN_CALLS: AtomicU64 = AtomicU64::new(0);
pub(crate) static TESS_FACE_CALLS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("KEEL_PROFILE").is_ok())
}

/// RAII stage timer; accumulates into its slot on drop when enabled.
pub(crate) struct Scope {
    slot: &'static AtomicU64,
    start: Option<Instant>,
}
impl Scope {
    pub(crate) fn new(slot: &'static AtomicU64) -> Self {
        Self {
            slot,
            start: if enabled() {
                Some(Instant::now())
            } else {
                None
            },
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(s) = self.start {
            self.slot
                .fetch_add(s.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }
}

pub(crate) fn count(slot: &'static AtomicU64) {
    if enabled() {
        slot.fetch_add(1, Ordering::Relaxed);
    }
}

#[doc(hidden)]
pub fn report() -> String {
    let ms = |a: &AtomicU64| a.load(Ordering::Relaxed) as f64 / 1e6;
    let n = |a: &AtomicU64| a.load(Ordering::Relaxed);
    format!(
        "KEEL_PROFILE breakdown (ms total):\n\
         front-door mesh checks {:.1}\n\
         preimprint             {:.1}\n\
           detect pairs         {:.1}\n\
           imprint cuts         {:.1}\n\
         face_interior_point    {:.1} ({} calls)\n\
         seam_curves            {:.1}\n\
         no-interaction shortcut{:.1}\n\
         imprint_operand x2     {:.1}\n\
           boundary filter      {:.1}\n\
           corner pre-split     {:.1}\n\
           dispatch+imprints    {:.1}\n\
           imprint ops          {:.1} ({} calls)\n\
             closed imprint     {:.1}\n\
             ring subdivide     {:.1}\n\
             mev chain          {:.1}\n\
             split_face         {:.1}\n\
             seam geometry      {:.1}\n\
         classify_faces x2      {:.1}\n\
         stitch+select+finalize {:.1}\n\
         validate               {:.1}\n\
         mass_properties        {:.1} ({} calls)\n\
         mesh_volume            {:.1} ({} calls)\n\
         winding number         {:.1} ({} calls)\n\
         tessellate_face calls  {}",
        ms(&FRONT_DOOR_NS),
        ms(&PREIMPRINT_NS),
        ms(&PREIMPRINT_DETECT_NS),
        ms(&PREIMPRINT_CUT_NS),
        ms(&INTERIOR_PT_NS),
        n(&INTERIOR_PT_CALLS),
        ms(&SEAM_NS),
        ms(&SHORTCUT_NS),
        ms(&IMPRINT_NS),
        ms(&IMPRINT_FILTER_NS),
        ms(&IMPRINT_PRESPLIT_NS),
        ms(&IMPRINT_DISPATCH_NS),
        ms(&IMPRINT_OPS_NS),
        n(&IMPRINT_OPS_CALLS),
        ms(&CLOSED_IMPRINT_NS),
        ms(&RING_SUBDIV_NS),
        ms(&IMPRINT_MEV_NS),
        ms(&IMPRINT_SPLITF_NS),
        ms(&IMPRINT_SEAMGEO_NS),
        ms(&CLASSIFY_NS),
        ms(&STITCH_NS),
        ms(&VALIDATE_NS),
        ms(&MASS_NS),
        n(&MASS_CALLS),
        ms(&MESHVOL_NS),
        n(&MESHVOL_CALLS),
        ms(&GWN_NS),
        n(&GWN_CALLS),
        n(&TESS_FACE_CALLS),
    )
}

#[doc(hidden)]
pub fn reset() {
    for a in [
        &FRONT_DOOR_NS,
        &PREIMPRINT_NS,
        &PREIMPRINT_DETECT_NS,
        &PREIMPRINT_CUT_NS,
        &INTERIOR_PT_NS,
        &INTERIOR_PT_CALLS,
        &SEAM_NS,
        &SHORTCUT_NS,
        &IMPRINT_NS,
        &IMPRINT_FILTER_NS,
        &IMPRINT_PRESPLIT_NS,
        &IMPRINT_DISPATCH_NS,
        &IMPRINT_OPS_NS,
        &IMPRINT_OPS_CALLS,
        &CLOSED_IMPRINT_NS,
        &RING_SUBDIV_NS,
        &IMPRINT_MEV_NS,
        &IMPRINT_SPLITF_NS,
        &IMPRINT_SEAMGEO_NS,
        &CLASSIFY_NS,
        &STITCH_NS,
        &VALIDATE_NS,
        &MASS_NS,
        &MASS_CALLS,
        &MESHVOL_NS,
        &MESHVOL_CALLS,
        &GWN_NS,
        &GWN_CALLS,
        &TESS_FACE_CALLS,
    ] {
        a.store(0, Ordering::Relaxed);
    }
}
