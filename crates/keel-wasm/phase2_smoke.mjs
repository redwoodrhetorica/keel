// Smoke test for the COMPLETE keel-wasm binding (task 28).
// Loads the wasm, checks every contract export is present, then runs a real
// fieldforge-style chain end-to-end: box - cylinder bore -> fillet the rim
// (the P0), mesh, edge-count, STEP export.
import { readFileSync } from 'node:fs';

const wasmPath = new URL(
  '../../target/wasm32-unknown-unknown/release/keel_wasm.wasm',
  import.meta.url,
);
const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const e = instance.exports;

const required = [
  'kw_alloc', 'kw_free',
  'kw_block', 'kw_cylinder', 'kw_cone', 'kw_sphere', 'kw_torus',
  'kw_place', 'kw_mirror',
  'kw_prism', 'kw_revolve', 'kw_loft', 'kw_loft_circles',
  'kw_boolean',
  'kw_fillet_nearest', 'kw_chamfer_nearest',
  'kw_push_face', 'kw_draft_face', 'kw_hollow', 'kw_hollow_pierce',
  'kw_mesh',
  'mesh_positions_ptr', 'mesh_positions_len', 'mesh_normals_ptr', 'mesh_normals_len',
  'mesh_indices_ptr', 'mesh_indices_len', 'mesh_lines_ptr', 'mesh_lines_len',
  'mesh_groups_ptr', 'mesh_groups_len', 'mesh_edge_groups_ptr', 'mesh_edge_groups_len',
  'kw_volume', 'kw_edge_count',
  'kw_release', 'kw_clear',
  'kw_step_export', 'step_buffer_ptr', 'step_buffer_len',
  'kw_offset_body',
  'memory',
];
const missing = required.filter((n) => !(n in e));
const total = Object.keys(e).length;
console.log(`exports: ${total}; required present: ${required.length - missing.length}/${required.length}`);
if (missing.length) {
  console.log('MISSING:', missing.join(', '));
  process.exit(1);
}

let fail = 0;
const near = (got, want, tol, what) => {
  const ok = Number.isFinite(got) && Math.abs(got - want) <= tol;
  console.log(`  ${ok ? 'OK ' : 'XX '} ${what}: ${got.toFixed ? got.toFixed(3) : got} (want ~${want})`);
  if (!ok) fail++;
};

// --- the fieldforge first-op chain ---
e.kw_clear();

const box = e.kw_block(-5, -5, -5, 10, 10, 10);
near(e.kw_volume(box), 1000.0, 1e-6, 'block 10^3 volume');

const cyl = e.kw_cylinder(0, 0, -6, 0, 0, 1, 3, 12); // R3 through bore along Z
near(e.kw_volume(cyl), Math.PI * 9 * 12, 1e-3, 'cylinder R3 H12 volume');

const holed = e.kw_boolean(box, cyl, 1); // 1 = Difference
console.log(`  boolean difference -> handle ${holed}`);
// the R3xH12 cyl (z=-6..6) overshoots the box (z=-5..5): bore height inside = 10
const bored = 1000 - Math.PI * 9 * 10;
near(e.kw_volume(holed), bored, 1e-2, 'box - bore volume');

// P0: fillet the circular top-rim edge (pick a point on the rim at z=5, x=3)
const fil = e.kw_fillet_nearest(holed, 3, 0, 5, 1.0, 0.05);
if (fil >= 0) {
  const v = e.kw_volume(fil);
  // full-360 toroidal wedge ~ r^2*(1-pi/4)*2*pi*R = 1*0.2146*2pi*3 ~= 4.04
  near(v, bored - 4.04, 2.5, 'P0 fillet bore rim (full loop, valid solid)');
} else {
  console.log(`  XX P0 fillet DECLINED (code ${fil})`);
  fail++;
}

// chamfer the same rim on the un-filleted body
const cha = e.kw_chamfer_nearest(holed, 3, 0, 5, 1.0, 0.05);
console.log(`  ${cha >= 0 ? 'OK ' : 'XX '} P0 chamfer bore rim -> ${cha >= 0 ? 'handle ' + cha : 'DECLINE ' + cha}`);
if (cha < 0) fail++;

// mesh + interrogation on the holed body
const meshRc = e.kw_mesh(holed, 0.1);
const tris = e.mesh_indices_len() / 3;
const edgeGroups = e.mesh_edge_groups_len() / 3;
console.log(`  ${meshRc === 0 ? 'OK ' : 'XX '} kw_mesh rc=${meshRc}: ${e.mesh_positions_len() / 3} verts, ${tris} tris, ${e.mesh_lines_len() / 6} segs, ${edgeGroups} edge-groups`);
if (meshRc !== 0 || tris < 4) fail++;

const ec = e.kw_edge_count(holed);
console.log(`  ${ec > 0 ? 'OK ' : 'XX '} kw_edge_count = ${ec}`);
if (ec <= 0) fail++;

// STEP export
const stepRc = e.kw_step_export(holed);
const stepLen = e.step_buffer_len();
console.log(`  ${stepRc === 0 && stepLen > 0 ? 'OK ' : 'XX '} kw_step_export rc=${stepRc}, ${stepLen} bytes`);
if (stepRc !== 0 || stepLen <= 0) fail++;

// a couple more primitives to exercise the surface
const sph = e.kw_sphere(0, 0, 0, 2);
near(e.kw_volume(sph), (4 / 3) * Math.PI * 8, 0.5, 'sphere R2 volume');
const con = e.kw_cone(0, 0, 0, 0, 0, 1, 3, 6);
near(e.kw_volume(con), (1 / 3) * Math.PI * 9 * 6, 0.5, 'cone R3 H6 volume');

console.log(fail === 0 ? '\nALL SMOKE CHECKS PASSED' : `\n${fail} CHECK(S) FAILED`);
process.exit(fail === 0 ? 0 : 1);
