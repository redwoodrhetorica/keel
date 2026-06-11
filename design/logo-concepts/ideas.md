# Keel — Logo Concept Renders

Ten distinct logo directions for **Keel**, an open-source B-rep solid-modeling geometry kernel in Rust. Each concept is a different visual idea (not a reworded variant), chosen to read at favicon size and on a README banner, monochrome-friendly, with a strong silhouette.

## Generation setup (shared by all concepts)

- **Model family:** Qwen Image (text-to-image)
- **Diffusion model:** `qwen_image_fp8_e4m3fn.safetensors` (via `UNETLoader`, weight_dtype `default`)
- **Text encoder:** `qwen_2.5_vl_7b_fp8_scaled.safetensors` (via `CLIPLoader`, type `qwen_image`)
- **VAE:** `qwen_image_vae.safetensors`
- **Model sampling:** `ModelSamplingAuraFlow`, shift `3.1`
- **Sampler / scheduler:** `euler` / `simple`
- **Steps:** 20 | **CFG:** 2.5 | **Denoise:** 1.0
- **Latent:** `EmptySD3LatentImage`, 1024x1024, batch 1
- **Hardware:** NVIDIA RTX 3080 (10 GB), ComfyUI 0.22.3

**Shared negative prompt:**
`text, words, letters, watermark, signature, photograph, photorealistic, 3d render, gradient mesh, busy, cluttered, low contrast, blurry, jpeg artifacts, ugly`

Workflow shape (API format): `UNETLoader -> ModelSamplingAuraFlow` and `CLIPLoader -> CLIPTextEncode (pos) + CLIPTextEncode (neg)` and `EmptySD3LatentImage` feed a `KSampler`, then `VAEDecode -> SaveImage`. The reusable workflow lives in `_gen.ps1`; the full batch driver is `_batch.ps1` (re-runnable).

---

## Concepts

### 1. Keel Spine + Circuit Trace — `01-keel-spine-circuit.png`
**Seed 101.** The literal pitch: a ship's keel is the structural backbone, and so is a geometry kernel. A bold vertical spine carries thin rib lines that terminate in circuit-trace node dots, fusing the maritime metaphor with the software-foundation metaphor. Says: Keel is the load-bearing layer a CAD app is built on.

*Prompt:* `flat vector logo, minimal geometric mark, the keel and spine of a ship hull rendered as one bold vertical backbone with thin precise rib lines branching off like circuit traces and wireframe edges, small node dots at the trace tips, monochrome deep navy on white, centered, strong silhouette, clean engineering linework, generous negative space, no text, high contrast, crisp`

### 2. K Monogram from Wireframe Edges — `02-k-monogram-wireframe.png`
**Seed 202.** A letter K built only from straight B-rep edges and vertices, with small joint dots. The most "brandable" mark: a recognizable initial that is literally constructed the way the kernel constructs solids (edges + vertices = topology). Says: exact topology is the brand.

*Prompt:* `flat vector logo, a letter K monogram constructed entirely from straight B-rep wireframe edges and vertices, geometric line-segment construction with small joint dots, isometric precision, two-tone navy and white, centered on solid white background, bold readable silhouette, minimal, no extra text, sharp vector linework`

### 3. Hull Section Ribs, Radial — `03-hull-section-ribs-radial.png`
**Seed 303.** A ship hull cross-section reimagined as a radial, symmetric mark: concentric curved frames nested in a ring. Reads as both shipbuilding ribs and a precision gauge/seal. Says: structural, foundational, engineered.

*Prompt:* `flat vector logo, a ship hull cross-section drawn as a radial mark, concentric curved ribs nested inside an outer ring like the frames of a wooden hull, perfectly symmetric, monochrome navy on white, centered, geometric, clean even line weights, minimal, no text, crisp engineering diagram style`

### 4. Boolean Intersection — `04-boolean-intersection.png`
**Seed 404.** Two solids (cube + sphere) mid-boolean, with the intersection lens filled as the focal accent. The single most kernel-specific operation rendered as a mark. Says: robust booleans on solids are what Keel does.

*Prompt:* `flat vector logo, two overlapping geometric solids a cube and a sphere shown mid boolean intersection, the lens-shaped intersection region filled solid as the focal accent while the rest is thin outline, two-tone navy and a single warm accent, centered, minimal constructive solid geometry mark, no text, clean vector, strong silhouette`

### 5. NURBS Keel Curve — `05-nurbs-keel-curve.png`
**Seed 505.** A smooth NURBS curve traces a keel silhouette, shown with its straight control polygon and square control-point handles. The exact mathematical object (control polygon -> curve) drawn honestly. Says: precise curves, exact geometry.

*Prompt:* `flat vector logo, a smooth NURBS curve forming the elegant silhouette of a ship keel, with its straight control polygon and small square control-point handles drawn alongside, two-tone navy line on white with light accent handles, centered, minimal mathematical curve mark, clean precise linework, no text, crisp`

### 6. Anchor-Keel Negative Space — `06-anchor-keel-negative-space.png`
**Seed 606.** A solid shield with a keel/anchor shape cut out as negative space, so the white gap reads as both keel and anchor. The cleverest one-color mark; the trick is exactly the dual-read every great logo wants. Says: solid, anchored, foundational.

*Prompt:* `flat vector logo, a bold solid navy rounded square or shield containing a keel-and-anchor shape formed by negative space cut out of the solid, clever single-color silhouette, the white gap reads as both a ship keel and an anchor, centered, minimal, strong contrast, no text, crisp geometric mark`

### 7. Isometric Kernel Cube, Exact Edge — `07-iso-kernel-cube-exact-edge.png`
**Seed 707.** An isometric wireframe cube with one edge highlighted bold/bright: the "one exact edge." Directly visualizes the kernel's promise of exact topology and "decline, never wrong." Says: exactness is the differentiator.

*Prompt:* `flat vector logo, an isometric wireframe cube drawn with thin precise edges, one single edge highlighted bold and bright as the exact load-bearing edge, small vertex dots at the corners, two-tone navy with one accent edge, centered on white, minimal engineering wireframe mark, no text, crisp clean lines`

### 8. Fillet / Blend Curve — `08-fillet-blend-curve.png`
**Seed 808.** Two straight edges joined by a tangent radius arc: the fillet, the most everyday CAD operation. Minimal and quiet, reads instantly at 16px. Says: clean geometry, smooth robust operations.

*Prompt:* `flat vector logo, a clean fillet blend mark, two straight edges meeting and joined by a smooth tangent quarter-circle radius, the blend arc emphasized, minimalist geometric corner-rounding symbol, monochrome navy on white, centered, lots of negative space, precise CAD linework, no text, crisp`

### 9. Topological Torus / Genus — `09-topological-torus-genus.png`
**Seed 909.** A torus drawn as concentric continuous-line rings, signaling genus-one topology. A nod to the topological correctness the kernel guarantees (the project tracks genus explicitly). Says: topology-aware, mathematically rigorous.

*Prompt:* `flat vector logo, a topological torus shown as clean concentric ring outlines suggesting a single hole genus-one surface, minimal continuous-line geometry, smooth even curves, monochrome deep navy on white, centered, balanced symmetric mark, no text, crisp vector linework, generous negative space`

### 10. Blueprint Keel Stamp — `10-blueprint-keel-stamp.png`
**Seed 1010.** A keel profile inside a circular engineering seal with tick marks and a baseline: a drafting-stamp aesthetic. Conveys provenance, precision, and the "certified-exact" feel. Says: engineering-grade, trustworthy foundation.

*Prompt:* `flat vector logo, a blueprint-style keel section stamp, a ship keel profile inside a thin circular engineering seal with tick marks and a baseline, drafting blueprint aesthetic, two-tone navy on white, centered, precise technical linework, minimal, crisp, no legible text just abstract tick marks`
