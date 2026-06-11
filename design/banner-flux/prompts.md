# KEEL banner batch (keel-rev) - Flux2-Klein pipeline

## Graph settings (reused verbatim from history entry 672b0d67, the run that produced Flux2-Klein_00035_.png)

- Model (UNET): flux-2-klein-base-9b-fp8.safetensors
- Text encoder (CLIPLoader): qwen_3_8b_fp8mixed.safetensors, type flux2
- VAE: full_encoder_small_decoder.safetensors
- Sampler: euler (KSamplerSelect + SamplerCustomAdvanced)
- Scheduler: Flux2Scheduler, steps = 20
- CFG: 5.0 (CFGGuider)
- Resolution: 1024 x 1024, batch size 1

## Positive prompt

Cinematic dark hero render of a massive ship hull seen from the side in profile, low three-quarter camera angle, bow pointing right, the vessel emerging from a deep midnight-navy studio void. A single dramatic amber-gold rim light traces the leading edge of the bow, from the deck down the curved stem to the rounded bulbous forefoot at the keel. The hull is near-black matte steel overlaid with faint glowing blueprint wireframe construction lines and panel seams. Small white draft-mark depth numerals run up the stem near the bow. A dark oxide-red anti-fouling band runs along the bottom of the hull at the waterline, tracing the keel line. Thin deck railings, masts and rigging lines silhouetted at the top of the hull, fading into darkness toward the left edge. Soft reflection on a dark glossy studio floor, gentle atmospheric haze, premium minimalist composition with generous empty dark space at the top. Bold clean white sans-serif title text "KEEL" centered in the upper area. Moody, precise, engineering-grade product aesthetic.

## Negative prompt

front view, bow-on view, head-on symmetric composition, ship facing the camera, daylight, bright sky, ocean waves, water spray, cartoon, flat vector illustration, low quality, blurry, watermark

## File -> seed

- keel-rev-01.png  seed=9585150
- keel-rev-02.png  seed=4510615  (NOT RENDERED: interrupted by concurrent queue activity)
- keel-rev-03.png  seed=1790020  (NOT RENDERED: interrupted by concurrent queue activity)
- keel-rev-04.png  seed=3591506
- keel-rev-05.png  seed=3907711
- keel-rev-06.png  seed=3570793  (NOT RENDERED: interrupted by concurrent queue activity)
- keel-rev-07.png  seed=7396313  (NOT RENDERED: interrupted by concurrent queue activity)
- keel-rev-08.png  seed=7620525  (NOT RENDERED: interrupted by concurrent queue activity)
- keel-rev-09.png  seed=1258314  (NOT RENDERED: interrupted by concurrent queue activity)
- keel-rev-10.png  seed=5474141  (NOT RENDERED: interrupted by concurrent queue activity)
