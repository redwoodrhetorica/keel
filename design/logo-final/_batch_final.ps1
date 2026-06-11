# Phase-2 final batch driver. Renders 10 keel-on-drafting-table variations ONE AT A TIME,
# synchronously, via the proven _gen_lightning.ps1 Lightning recipe (4 steps / cfg 1.0).
# Skips any keel-final-NN.png that already exists on disk (never re-render a completed one).

$gen = "C:\Users\mcdon\Documents\Repo\Claude\parasolid\design\logo-concepts\_gen_lightning.ps1"
$out = "C:\Users\mcdon\Documents\Repo\Claude\parasolid\design\logo-final"

# Shared background/subject vocabulary fused from concept #08 (drafting-table look) + #10 (ship keel, simplified).
$draft = "presented on a drafting-table blueprint background with faint technical grid lines, light measurement tick marks and a thin baseline rule, engineering drafting aesthetic"

$variants = @(
  # --- V1: ultra-minimal 3-stroke mark (monochrome) ---
  @{ name="keel-final-01.png"; seed=2001; variant="V1 ultra-minimal 3-stroke keel, monochrome";
     prompt="flat vector logo, an ultra-minimal iconic ship keel mark reduced to just three bold confident strokes, a single long curved keel backbone line from bow to stern with two short rib stubs, strong silhouette, monochrome deep navy on white, $draft, centered, generous negative space, readable at favicon size, no text, crisp clean linework" }
  @{ name="keel-final-02.png"; seed=2002; variant="V1 ultra-minimal 3-stroke keel, monochrome";
     prompt="flat vector logo, an ultra-minimal iconic ship keel mark reduced to just three bold confident strokes, a single long curved keel backbone line from bow to stern with two short rib stubs, strong silhouette, monochrome deep navy on white, $draft, centered, generous negative space, readable at favicon size, no text, crisp clean linework" }
  @{ name="keel-final-03.png"; seed=2003; variant="V1 ultra-minimal 3-stroke keel, monochrome";
     prompt="flat vector logo, an ultra-minimal iconic ship keel mark reduced to just three bold confident strokes, a single long curved keel backbone line from bow to stern with two short rib stubs, strong silhouette, monochrome deep navy on white, $draft, centered, generous negative space, readable at favicon size, no text, crisp clean linework" }

  # --- V2: medium detail, a few rib stubs (monochrome) ---
  @{ name="keel-final-04.png"; seed=2101; variant="V2 medium detail, several rib stubs, monochrome";
     prompt="flat vector logo, a simplified iconic ship keel mark, one bold keel backbone line running bow to stern with a few evenly spaced short rib stubs branching off, clean minimal geometric construction, strong silhouette, monochrome deep navy on white, $draft, centered, balanced, no text, precise CAD linework, crisp" }
  @{ name="keel-final-05.png"; seed=2102; variant="V2 medium detail, several rib stubs, monochrome";
     prompt="flat vector logo, a simplified iconic ship keel mark, one bold keel backbone line running bow to stern with a few evenly spaced short rib stubs branching off, clean minimal geometric construction, strong silhouette, monochrome deep navy on white, $draft, centered, balanced, no text, precise CAD linework, crisp" }
  @{ name="keel-final-06.png"; seed=2103; variant="V2 medium detail, several rib stubs, monochrome";
     prompt="flat vector logo, a simplified iconic ship keel mark, one bold keel backbone line running bow to stern with a few evenly spaced short rib stubs branching off, clean minimal geometric construction, strong silhouette, monochrome deep navy on white, $draft, centered, balanced, no text, precise CAD linework, crisp" }

  # --- V3: badge / stamp enclosure (drafting seal) ---
  @{ name="keel-final-07.png"; seed=2201; variant="V3 badge/stamp enclosure, monochrome";
     prompt="flat vector logo, a simplified ship keel mark enclosed in a clean circular badge or drafting stamp, the bold keel backbone with two rib stubs inside a thin ring with small evenly spaced tick marks and a baseline, iconic seal, monochrome deep navy on white, $draft, centered, minimal, strong silhouette, no legible text just abstract ticks, crisp" }
  @{ name="keel-final-08.png"; seed=2202; variant="V3 badge/stamp enclosure, monochrome";
     prompt="flat vector logo, a simplified ship keel mark enclosed in a clean circular badge or drafting stamp, the bold keel backbone with two rib stubs inside a thin ring with small evenly spaced tick marks and a baseline, iconic seal, monochrome deep navy on white, $draft, centered, minimal, strong silhouette, no legible text just abstract ticks, crisp" }

  # --- V4: two-tone with single warm accent ---
  @{ name="keel-final-09.png"; seed=2301; variant="V4 two-tone, single warm accent keel";
     prompt="flat vector logo, a simplified iconic ship keel mark, one bold keel backbone line bow to stern with a few short rib stubs, the backbone in a single warm accent color and the ribs in deep navy, clean two-tone minimal mark, strong silhouette, $draft, centered, generous negative space, readable at favicon size, no text, crisp clean linework" }
  @{ name="keel-final-10.png"; seed=2302; variant="V4 two-tone, single warm accent keel";
     prompt="flat vector logo, a simplified iconic ship keel mark, one bold keel backbone line bow to stern with a few short rib stubs, the backbone in a single warm accent color and the ribs in deep navy, clean two-tone minimal mark, strong silhouette, $draft, centered, generous negative space, readable at favicon size, no text, crisp clean linework" }
)

foreach ($v in $variants) {
  $target = Join-Path $out $v.name
  if (Test-Path $target) {
    Write-Output "SKIP $($v.name) (already on disk)"
    continue
  }
  $prefix = "kf_" + ($v.name -replace '\.png$','')
  Write-Output "=== RENDER $($v.name) seed=$($v.seed) [$($v.variant)] ==="
  & $gen -Positive $v.prompt -Seed $v.seed -OutPrefix $prefix -FinalName $v.name
  # _gen_lightning.ps1 saves into ITS OWN dir (logo-concepts); move into logo-final.
  $srcInGenDir = Join-Path "C:\Users\mcdon\Documents\Repo\Claude\parasolid\design\logo-concepts" $v.name
  if ((Test-Path $srcInGenDir) -and -not (Test-Path $target)) {
    Move-Item -Path $srcInGenDir -Destination $target -Force
    Write-Output "MOVED $($v.name) -> logo-final"
  }
}
Write-Output "BATCH_COMPLETE"
