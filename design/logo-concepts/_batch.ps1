$dir = "C:\Users\mcdon\Documents\Repo\Claude\parasolid\design\logo-concepts"
$gen = "$dir\_gen.ps1"
$base = "http://127.0.0.1:8188"

# slug, seed, prompt
$concepts = @(
  @{ n=1; slug="keel-spine-circuit"; seed=101;
     p="flat vector logo, minimal geometric mark, the keel and spine of a ship hull rendered as one bold vertical backbone with thin precise rib lines branching off like circuit traces and wireframe edges, small node dots at the trace tips, monochrome deep navy on white, centered, strong silhouette, clean engineering linework, generous negative space, no text, high contrast, crisp" },
  @{ n=2; slug="k-monogram-wireframe"; seed=202;
     p="flat vector logo, a letter K monogram constructed entirely from straight B-rep wireframe edges and vertices, geometric line-segment construction with small joint dots, isometric precision, two-tone navy and white, centered on solid white background, bold readable silhouette, minimal, no extra text, sharp vector linework" },
  @{ n=3; slug="hull-section-ribs-radial"; seed=303;
     p="flat vector logo, a ship hull cross-section drawn as a radial mark, concentric curved ribs nested inside an outer ring like the frames of a wooden hull, perfectly symmetric, monochrome navy on white, centered, geometric, clean even line weights, minimal, no text, crisp engineering diagram style" },
  @{ n=4; slug="boolean-intersection"; seed=404;
     p="flat vector logo, two overlapping geometric solids a cube and a sphere shown mid boolean intersection, the lens-shaped intersection region filled solid as the focal accent while the rest is thin outline, two-tone navy and a single warm accent, centered, minimal constructive solid geometry mark, no text, clean vector, strong silhouette" },
  @{ n=5; slug="nurbs-keel-curve"; seed=505;
     p="flat vector logo, a smooth NURBS curve forming the elegant silhouette of a ship keel, with its straight control polygon and small square control-point handles drawn alongside, two-tone navy line on white with light accent handles, centered, minimal mathematical curve mark, clean precise linework, no text, crisp" },
  @{ n=6; slug="anchor-keel-negative-space"; seed=606;
     p="flat vector logo, a bold solid navy rounded square or shield containing a keel-and-anchor shape formed by negative space cut out of the solid, clever single-color silhouette, the white gap reads as both a ship keel and an anchor, centered, minimal, strong contrast, no text, crisp geometric mark" },
  @{ n=7; slug="iso-kernel-cube-exact-edge"; seed=707;
     p="flat vector logo, an isometric wireframe cube drawn with thin precise edges, one single edge highlighted bold and bright as the exact load-bearing edge, small vertex dots at the corners, two-tone navy with one accent edge, centered on white, minimal engineering wireframe mark, no text, crisp clean lines" },
  @{ n=8; slug="fillet-blend-curve"; seed=808;
     p="flat vector logo, a clean fillet blend mark, two straight edges meeting and joined by a smooth tangent quarter-circle radius, the blend arc emphasized, minimalist geometric corner-rounding symbol, monochrome navy on white, centered, lots of negative space, precise CAD linework, no text, crisp" },
  @{ n=9; slug="topological-torus-genus"; seed=909;
     p="flat vector logo, a topological torus shown as clean concentric ring outlines suggesting a single hole genus-one surface, minimal continuous-line geometry, smooth even curves, monochrome deep navy on white, centered, balanced symmetric mark, no text, crisp vector linework, generous negative space" },
  @{ n=10; slug="blueprint-keel-stamp"; seed=1010;
     p="flat vector logo, a blueprint-style keel section stamp, a ship keel profile inside a thin circular engineering seal with tick marks and a baseline, drafting blueprint aesthetic, two-tone navy on white, centered, precise technical linework, minimal, crisp, no legible text just abstract tick marks" }
)

$results = @()
foreach ($c in $concepts) {
  $prefix = "keel_{0:D2}_{1}" -f $c.n, $c.slug
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  Write-Output ("---- CONCEPT {0}: {1} (seed {2}) ----" -f $c.n, $c.slug, $c.seed)
  $out = & $gen -Positive $c.p -Seed $c.seed -OutPrefix $prefix 2>&1
  $sw.Stop()
  $secs = [math]::Round($sw.Elapsed.TotalSeconds,1)
  $imgLine = $out | Where-Object { $_ -like "IMAGE *" } | Select-Object -First 1
  $status = "FAIL"
  $finalName = ""
  if ($imgLine) {
    $parts = ($imgLine -replace "^IMAGE ","").Split("|")
    $fn = $parts[0]; $sub = $parts[1]; $typ = $parts[2]
    $finalName = "{0:D2}-{1}.png" -f $c.n, $c.slug
    $url = "$base/view?filename=$fn&subfolder=$sub&type=$typ"
    curl.exe -s $url -o "$dir\$finalName"
    if ((Test-Path "$dir\$finalName") -and (Get-Item "$dir\$finalName").Length -gt 1000) { $status = "OK" }
  }
  Write-Output ("RESULT n={0} status={1} file={2} sec={3}" -f $c.n, $status, $finalName, $secs)
  $results += [pscustomobject]@{ n=$c.n; slug=$c.slug; seed=$c.seed; status=$status; file=$finalName; sec=$secs }
}

Write-Output "==== SUMMARY ===="
$results | ForEach-Object { Write-Output ("{0}`t{1}`t{2}`t{3}s" -f $_.n, $_.status, $_.file, $_.sec) }
$results | Export-Csv -Path "$dir\_results.csv" -NoTypeInformation
