//! STEP import parser hostility soak (dossier 38 sec. 12): the Part 21
//! tokenizer + two-pass resolver must be panic-free and allocation-
//! bounded on arbitrary bytes (truncated instances, dangling refs,
//! deep nesting, garbage tokens). Any outcome is acceptable EXCEPT a
//! panic/abort; a returned body must additionally validate.

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = core::str::from_utf8(data) else {
        return;
    };
    // Wrap raw fuzz text in a DATA section half the time (first byte
    // odd) so the instance grammar gets coverage even when the corpus
    // lacks the framing.
    let framed;
    let input = if data.first().is_some_and(|b| b % 2 == 1) {
        framed = format!("DATA;\n{text}\nENDSEC;");
        &framed
    } else {
        text
    };
    if let Ok(body) = keel_topo::step_import::from_step_string(input, 1e-6) {
        let _ = body.validate();
    }
    // The geometry conversion layer (milestone 2) is its own attack
    // surface: knot expansion, control grids, placements.
    let _ = keel_topo::step_import::surfaces_from_step(input);
    let _ = keel_topo::step_import::curves_from_step(input);
});
