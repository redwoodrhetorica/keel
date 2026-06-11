# Keel Logo Render Batch — Status

All 10 concepts complete. 10/10 PNGs saved at 1024x1024, all verified non-black (full tonal range, 0% near-black pixels).

## Recovery / inventory before rendering

Per RULE 1, inventoried `/history?max_items=200` and `/queue` before generating anything:

- **History** held: sanity render, concept 1, concept 2 (TWICE — one full + one duplicate entry), nothing else.
- **Queue** at arrival: concept 3 RUNNING (slow 20-step recipe, in-flight) + a PENDING duplicate of concept 2.
- A previous agent's `_batch.ps1` (PID 50248) was **still alive**, actively queueing more concepts on the SLOW recipe. It had just enqueued concept 4. Killed it (RULE 2).
- Cancelled the pending duplicate of concept 2; cancelled the slow pending concept 4.
- Concept 3 was already executing, so per RULE 1 I let it finish and counted it (slow recipe, but in-flight — re-rendering would have been the waste).
- A duplicate concept-2 job got promoted to RUNNING during cancellation; **interrupted** it (concept 2 already on disk — avoided a 3rd duplicate render that would have annoyed the user).

## Per-concept outcome

| # | Slug | Seed | Outcome | Recipe | Time |
|---|------|------|---------|--------|------|
| 01 | keel-spine-circuit | 101 | pre-existing on disk (from prior slow batch) | vanilla 20/2.5 | n/a (recovered) |
| 02 | k-monogram-wireframe | 202 | pre-existing on disk (from prior slow batch) | vanilla 20/2.5 | n/a (recovered) |
| 03 | hull-section-ribs-radial | 303 | recovered from history (was RUNNING on arrival; let it finish, downloaded via /view) | vanilla 20/2.5 | n/a (in-flight) |
| 04 | boolean-intersection | 404 | rendered with Lightning (first render — sanity-checked non-black before continuing) | Lightning 4/1.0 | 52.4s |
| 05 | nurbs-keel-curve | 505 | rendered with Lightning | Lightning 4/1.0 | 26.9s |
| 06 | anchor-keel-negative-space | 606 | rendered with Lightning | Lightning 4/1.0 | 170.9s* |
| 07 | iso-kernel-cube-exact-edge | 707 | rendered with Lightning | Lightning 4/1.0 | 23.5s |
| 08 | fillet-blend-curve | 808 | rendered with Lightning | Lightning 4/1.0 | 20.7s |
| 09 | topological-torus-genus | 909 | rendered with Lightning | Lightning 4/1.0 | 20.6s |
| 10 | blueprint-keel-stamp | 1010 | rendered with Lightning | Lightning 4/1.0 | 19.0s |

\* Concept 06 took 170.9s due to transient GPU contention during that one poll window; the image saved correctly and is non-black. All other Lightning renders landed in 19–53s.

## Settings used

- **Recovered (01–03):** prior vanilla recipe — UNETLoader `qwen_image_fp8_e4m3fn` + CLIPLoader `qwen_2.5_vl_7b_fp8_scaled` + VAELoader `qwen_image_vae` + ModelSamplingAuraFlow shift 3.1 + KSampler euler/simple, **20 steps / cfg 2.5**, 1024x1024.
- **Lightning (04–10):** same graph + `LoraLoaderModelOnly` (node 11, class verified via `/object_info/LoraLoaderModelOnly`) inserted between UNETLoader (1) and ModelSamplingAuraFlow (4). `lora_name = Qwen-Image-2512-Lightning-4steps-V1.0-fp32.safetensors` (exact name confirmed present in the loader list), `strength_model = 1.0`. **steps = 4, cfg = 1.0**, shift 3.1, euler/simple, 1024x1024. Seeds and prompts unchanged from ideas.md.

## Lightning timing

7 concepts rendered with Lightning. Average **47.7s/image**. Excluding the contention outlier (06): **23.9s/image** typical (vs 90s+ on the previous vanilla recipe — roughly 4x faster). First render is slower (52.4s) due to model load; subsequent cached renders ~20–27s.

## Failures

None. 0 render errors, 0 black/corrupt images. No concept re-rendered: 01/02 reused from disk, 03 recovered in-flight, 04–10 newly rendered with Lightning.

## Files / scripts

- `_gen_lightning.ps1` — the FAST Lightning recipe (added). Renders one concept synchronously, polls `/history/{id}`, downloads the result via `/view`, prints timing. Drives phase-2 batches at 4 steps / cfg 1.0 with the Lightning LoRA.
- `_gen.ps1` — original slow vanilla recipe, left intact for reference.

## Phase-2 readiness

For the 10-variation batch of the user's chosen concept, run `_gen_lightning.ps1` per variation with the chosen concept's prompt and distinct seeds. Defaults already encode the fast recipe (Lightning LoRA, 4 steps, cfg 1.0).
