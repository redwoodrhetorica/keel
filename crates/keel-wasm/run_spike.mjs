// WASM spike harness (task 21): loads the kernel wasm, verifies the
// canonical results exactly, and measures wasm timing for comparison
// against native (pin union expected exactly 16.0; box union 1.875).
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(
  here,
  "../../target/wasm32-unknown-unknown/release/keel_wasm.wasm",
);
const bytes = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, {});
const { pin_in_hole, box_union } = instance.exports;

const time = (label, f, reps, expected) => {
  let v = f(); // warm-up + correctness
  const t0 = performance.now();
  for (let i = 0; i < reps; i++) v = f();
  const ms = (performance.now() - t0) / reps;
  const ok = Math.abs(v - expected) < 1e-9 ? "EXACT" : `value ${v} (expected ${expected})`;
  console.log(`${label}: ${ms.toFixed(2)} ms/op (${reps} reps) -> ${ok}`);
  return v;
};

console.log(`module: ${bytes.length} bytes (${(bytes.length / 1048576).toFixed(2)} MiB)`);
time("box_union   ", box_union, 50, 1.875);
time("pin_in_hole ", pin_in_hole, 20, 16.0);

// Worker-protocol buffers (task 28): build the demo mesh, view the
// buffers directly in wasm memory, verify the contract.
const e = instance.exports;
const t0 = performance.now();
const status = e.demo_mesh_build();
const buildMs = performance.now() - t0;
if (status !== 0) throw new Error(`demo_mesh_build failed: ${status}`);
const mem = e.memory.buffer;
const positions = new Float32Array(mem, e.mesh_positions_ptr(), e.mesh_positions_len());
const normals = new Float32Array(mem, e.mesh_normals_ptr(), e.mesh_normals_len());
const indices = new Uint32Array(mem, e.mesh_indices_ptr(), e.mesh_indices_len());
const groups = new Uint32Array(mem, e.mesh_groups_ptr(), e.mesh_groups_len());
const lines = new Float32Array(mem, e.mesh_lines_ptr(), e.mesh_lines_len());

let covered = 0;
for (let i = 0; i < groups.length; i += 3) covered += groups[i + 2];
const ok =
  positions.length === normals.length &&
  covered === indices.length &&
  lines.length % 6 === 0 &&
  positions.length > 0 &&
  lines.length > 0;
console.log(
  `worker mesh: built in ${buildMs.toFixed(1)} ms: ${indices.length / 3} tris, ` +
    `${groups.length / 3} face groups (cover ${covered === indices.length ? "EXACT" : "BROKEN"}), ` +
    `${lines.length / 6} line segments -> ${ok ? "CONTRACT OK" : "CONTRACT VIOLATION"}`,
);
if (!ok) process.exit(1);

// OpReport across the boundary (task 22): a tolerant salvage must
// arrive loud: salvaged flag, tier, achieved tolerance, warnings text.
const vol = e.demo_tolerant_build();
const warn = new TextDecoder().decode(
  new Uint8Array(e.memory.buffer, e.report_warnings_ptr(), e.report_warnings_len()),
);
const exact = Math.abs(vol - 16.0) < 1e-9;
const salvaged = e.report_salvaged() === 1;
console.log(
  `op report: volume ${exact ? "EXACT" : vol}, salvaged ${salvaged}, tier ${e.report_tier()}, ` +
    `achieved ${e.report_achieved_tolerance().toExponential(2)}`,
);
console.log(`  warnings: ${warn.split("\n").length} -> "${warn.split("\n").pop()}"`);
if (!exact || !salvaged) process.exit(1);
