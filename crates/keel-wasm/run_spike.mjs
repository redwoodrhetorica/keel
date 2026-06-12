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
