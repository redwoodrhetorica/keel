$dir  = "C:\Users\mcdon\Documents\Repo\Claude\parasolid\design\logo-concepts"
$base = "http://127.0.0.1:8188"
$neg  = "text, words, letters, watermark, signature, photograph, photorealistic, 3d render, gradient mesh, busy, cluttered, low contrast, blurry, jpeg artifacts, ugly"

$concepts = @(
  @{ n=1;  slug="keel-spine-circuit";        seed=101;  p="flat vector logo, minimal geometric mark, the keel and spine of a ship hull rendered as one bold vertical backbone with thin precise rib lines branching off like circuit traces and wireframe edges, small node dots at the trace tips, monochrome deep navy on white, centered, strong silhouette, clean engineering linework, generous negative space, no text, high contrast, crisp" },
  @{ n=2;  slug="k-monogram-wireframe";       seed=202;  p="flat vector logo, a letter K monogram constructed entirely from straight B-rep wireframe edges and vertices, geometric line-segment construction with small joint dots, isometric precision, two-tone navy and white, centered on solid white background, bold readable silhouette, minimal, no extra text, sharp vector linework" },
  @{ n=3;  slug="hull-section-ribs-radial";   seed=303;  p="flat vector logo, a ship hull cross-section drawn as a radial mark, concentric curved ribs nested inside an outer ring like the frames of a wooden hull, perfectly symmetric, monochrome navy on white, centered, geometric, clean even line weights, minimal, no text, crisp engineering diagram style" },
  @{ n=4;  slug="boolean-intersection";       seed=404;  p="flat vector logo, two overlapping geometric solids a cube and a sphere shown mid boolean intersection, the lens-shaped intersection region filled solid as the focal accent while the rest is thin outline, two-tone navy and a single warm accent, centered, minimal constructive solid geometry mark, no text, clean vector, strong silhouette" },
  @{ n=5;  slug="nurbs-keel-curve";           seed=505;  p="flat vector logo, a smooth NURBS curve forming the elegant silhouette of a ship keel, with its straight control polygon and small square control-point handles drawn alongside, two-tone navy line on white with light accent handles, centered, minimal mathematical curve mark, clean precise linework, no text, crisp" },
  @{ n=6;  slug="anchor-keel-negative-space"; seed=606;  p="flat vector logo, a bold solid navy rounded square or shield containing a keel-and-anchor shape formed by negative space cut out of the solid, clever single-color silhouette, the white gap reads as both a ship keel and an anchor, centered, minimal, strong contrast, no text, crisp geometric mark" },
  @{ n=7;  slug="iso-kernel-cube-exact-edge"; seed=707;  p="flat vector logo, an isometric wireframe cube drawn with thin precise edges, one single edge highlighted bold and bright as the exact load-bearing edge, small vertex dots at the corners, two-tone navy with one accent edge, centered on white, minimal engineering wireframe mark, no text, crisp clean lines" },
  @{ n=8;  slug="fillet-blend-curve";         seed=808;  p="flat vector logo, a clean fillet blend mark, two straight edges meeting and joined by a smooth tangent quarter-circle radius, the blend arc emphasized, minimalist geometric corner-rounding symbol, monochrome navy on white, centered, lots of negative space, precise CAD linework, no text, crisp" },
  @{ n=9;  slug="topological-torus-genus";    seed=909;  p="flat vector logo, a topological torus shown as clean concentric ring outlines suggesting a single hole genus-one surface, minimal continuous-line geometry, smooth even curves, monochrome deep navy on white, centered, balanced symmetric mark, no text, crisp vector linework, generous negative space" },
  @{ n=10; slug="blueprint-keel-stamp";       seed=1010; p="flat vector logo, a blueprint-style keel section stamp, a ship keel profile inside a thin circular engineering seal with tick marks and a baseline, drafting blueprint aesthetic, two-tone navy on white, centered, precise technical linework, minimal, crisp, no legible text just abstract tick marks" }
)

function Build-Workflow($pos, $seed, $prefix) {
  @{
    "1"  = @{ class_type = "UNETLoader"; inputs = @{ unet_name = "qwen_image_fp8_e4m3fn.safetensors"; weight_dtype = "default" } }
    "2"  = @{ class_type = "CLIPLoader"; inputs = @{ clip_name = "qwen_2.5_vl_7b_fp8_scaled.safetensors"; type = "qwen_image"; device = "default" } }
    "3"  = @{ class_type = "VAELoader"; inputs = @{ vae_name = "qwen_image_vae.safetensors" } }
    "4"  = @{ class_type = "ModelSamplingAuraFlow"; inputs = @{ model = @("1",0); shift = 3.1 } }
    "5"  = @{ class_type = "CLIPTextEncode"; inputs = @{ clip = @("2",0); text = $pos } }
    "6"  = @{ class_type = "CLIPTextEncode"; inputs = @{ clip = @("2",0); text = $neg } }
    "7"  = @{ class_type = "EmptySD3LatentImage"; inputs = @{ width = 1024; height = 1024; batch_size = 1 } }
    "8"  = @{ class_type = "KSampler"; inputs = @{ model = @("4",0); positive = @("5",0); negative = @("6",0); latent_image = @("7",0); seed = $seed; steps = 20; cfg = 2.5; sampler_name = "euler"; scheduler = "simple"; denoise = 1.0 } }
    "9"  = @{ class_type = "VAEDecode"; inputs = @{ samples = @("8",0); vae = @("3",0) } }
    "10" = @{ class_type = "SaveImage"; inputs = @{ images = @("9",0); filename_prefix = $prefix } }
  }
}

# Pull history once; return a hashtable prefix -> first image record (filename/subfolder/type) for successful entries
function Get-HistoryByPrefix {
  $h = Invoke-RestMethod -Uri "$base/history?max_items=200"
  $map = @{}
  foreach ($pid in $h.PSObject.Properties.Name) {
    $entry = $h.$pid
    if ($entry.status.completed -ne $true -and $entry.status.status_str -ne "success") { continue }
    $node10 = $entry.prompt[2]."10"
    if (-not $node10) { continue }
    $prefix = $node10.inputs.filename_prefix
    $imgs = $entry.outputs."10".images
    if ($prefix -and $imgs) { $map[$prefix] = $imgs[0] }
  }
  return $map
}

function Save-Image($img, $finalName) {
  $fn = $img.filename; $sub = $img.subfolder; $typ = $img.type
  $url = "$base/view?filename=$([uri]::EscapeDataString($fn))&subfolder=$([uri]::EscapeDataString($sub))&type=$typ"
  Invoke-WebRequest -Uri $url -OutFile "$dir\$finalName" -ErrorAction Stop | Out-Null
  return ((Test-Path "$dir\$finalName") -and (Get-Item "$dir\$finalName").Length -gt 1000)
}

$results = @()
foreach ($c in $concepts) {
  $prefix    = "keel_{0:D2}_{1}" -f $c.n, $c.slug
  $finalName = "{0:D2}-{1}.png" -f $c.n, $c.slug
  $sw = [System.Diagnostics.Stopwatch]::StartNew()

  # Skip if already saved on disk
  if ((Test-Path "$dir\$finalName") -and (Get-Item "$dir\$finalName").Length -gt 1000) {
    $sw.Stop()
    Write-Output ("SKIP n={0} already-on-disk file={1}" -f $c.n, $finalName)
    $results += [pscustomobject]@{ n=$c.n; slug=$c.slug; seed=$c.seed; status="OK"; file=$finalName; sec=0; note="pre-existing" }
    continue
  }

  Write-Output ("---- CONCEPT {0}: {1} (seed {2}) ----" -f $c.n, $c.slug, $c.seed)

  $status = "FAIL"; $note = ""
  for ($attempt = 1; $attempt -le 2 -and $status -ne "OK"; $attempt++) {

    # 1) Is there already a completed history entry for this prefix?
    $hist = Get-HistoryByPrefix
    if ($hist.ContainsKey($prefix)) {
      Write-Output ("  found completed history entry for {0} (attempt {1})" -f $prefix, $attempt)
      if (Save-Image $hist[$prefix] $finalName) { $status = "OK"; $note = "from-history"; break }
    }

    # 2) Otherwise submit and poll. Use deterministic-but-distinct seed per retry.
    $useSeed = if ($attempt -eq 1) { $c.seed } else { $c.seed + 1 }
    $wf = Build-Workflow $c.p $useSeed $prefix
    $body = @{ prompt = $wf } | ConvertTo-Json -Depth 12
    try {
      $resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json" -ErrorAction Stop
    } catch {
      Write-Output ("  SUBMIT FAILED attempt {0}: {1}" -f $attempt, $_.Exception.Message)
      $note = "submit-failed"
      continue
    }
    $promptId = $resp.prompt_id
    Write-Output ("  QUEUED attempt={0} seed={1} prompt_id={2}" -f $attempt, $useSeed, $promptId)

    $deadline = (Get-Date).AddSeconds(420)
    $done = $false
    while ((Get-Date) -lt $deadline -and -not $done) {
      Start-Sleep -Seconds 3
      try { $h = Invoke-RestMethod -Uri "$base/history/$promptId" -ErrorAction Stop } catch { continue }
      if ($h.PSObject.Properties.Name -contains $promptId) {
        $entry = $h.$promptId
        if ($entry.status.completed -eq $true -or $entry.status.status_str -eq "success") {
          $imgs = $entry.outputs."10".images
          if ($imgs) {
            if (Save-Image $imgs[0] $finalName) { $status = "OK"; $note = "rendered" }
          }
          $done = $true
        } elseif ($entry.status.status_str -eq "error") {
          Write-Output ("  RENDER ERROR attempt {0}" -f $attempt)
          ($entry.status | ConvertTo-Json -Depth 8) | Write-Output
          $note = "render-error"
          $done = $true
        }
      }
    }
    if (-not $done) { Write-Output ("  TIMEOUT attempt {0}" -f $attempt); $note = "timeout" }
  }

  $sw.Stop()
  $secs = [math]::Round($sw.Elapsed.TotalSeconds,1)
  Write-Output ("RESULT n={0} status={1} file={2} sec={3} note={4}" -f $c.n, $status, $finalName, $secs, $note)
  $results += [pscustomobject]@{ n=$c.n; slug=$c.slug; seed=$c.seed; status=$status; file=$finalName; sec=$secs; note=$note }
}

Write-Output "==== SUMMARY ===="
$results | ForEach-Object { Write-Output ("{0}`t{1}`t{2}`t{3}s`t{4}" -f $_.n, $_.status, $_.file, $_.sec, $_.note) }
$results | Export-Csv -Path "$dir\_results.csv" -NoTypeInformation
$okCount = ($results | Where-Object { $_.status -eq "OK" }).Count
Write-Output ("==== OK {0}/10 ====" -f $okCount)
