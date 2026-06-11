# keel-ban v2: the validated Flux2-Klein prompt, re-composed as a wide
# banner (1536x512) with the title to the RIGHT of the ship. Template =
# the keel-rev graph from history; patches: positive text, size, seed,
# prefix. 10 seeds, synchronous, downloads at the end.
$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8188"
$outDir = $PSScriptRoot

$newPositive = @'
Cinematic dark hero render of a massive ship hull seen from the side in profile, low three-quarter camera angle, bow pointing right, the vessel occupying the left half of a wide banner frame, emerging from a deep midnight-navy studio void. A single dramatic amber-gold rim light traces the leading edge of the bow, from the deck down the curved stem to the rounded bulbous forefoot at the keel. The hull is near-black matte steel overlaid with faint glowing blueprint wireframe construction lines and panel seams. Small white draft-mark depth numerals run up the stem near the bow. A dark oxide-red anti-fouling band runs along the bottom of the hull at the waterline, tracing the keel line. Thin deck railings, masts and rigging lines silhouetted at the top of the hull, fading into darkness toward the left edge. Soft reflection on a dark glossy studio floor, gentle atmospheric haze. Bold clean white sans-serif title text "KEEL" set in the open dark space to the right of the ship, vertically centered, with generous margins. Wide cinematic banner composition, moody, precise, engineering-grade product aesthetic.
'@

# 1. Template: the keel-rev graph from history.
$template = $null
$h = Invoke-RestMethod "$base/history?max_items=100"
foreach ($p in $h.PSObject.Properties) {
    $graph = $p.Value.prompt[2]
    foreach ($n in $graph.PSObject.Properties) {
        if ($n.Value.class_type -eq "SaveImage" -and $n.Value.inputs.filename_prefix -eq "keel-rev") {
            $template = $graph | ConvertTo-Json -Depth 100
        }
    }
    if ($template) { break }
}
if (-not $template) { Write-Output "FATAL: keel-rev template not found in history"; exit 1 }
Write-Output "template located"

$waitDone = {
    param($pid2)
    $deadline = (Get-Date).AddMinutes(10)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 3
        try { $h = Invoke-RestMethod "$base/history/$pid2" } catch { continue }
        if ($h.PSObject.Properties.Name -contains $pid2) {
            $e = $h.$pid2
            if ($e.status.completed -eq $true -or $e.status.status_str -eq "success") { return $true }
            if ($e.status.status_str -eq "error") { Write-Output "ERROR on $pid2"; return $false }
        }
    }
    Write-Output "TIMEOUT on $pid2"; return $false
}

# 2. Ten seeds, one at a time.
$seeds = @(1112223, 2223334, 3334445, 4445556, 5556667, 6667778, 7778889, 8889990, 9991112, 1213141)
$ids = @()
foreach ($seed in $seeds) {
    $g = $template | ConvertFrom-Json -AsHashtable
    foreach ($nodeId in @($g.Keys)) {
        $inputs = $g[$nodeId].inputs
        if ($null -eq $inputs) { continue }
        # positive prompt (identified by its current content)
        if ($g[$nodeId].class_type -match "CLIPTextEncode" -and $inputs.ContainsKey("text") -and $inputs["text"] -like "Cinematic dark hero render*") {
            $inputs["text"] = $newPositive.Trim()
        }
        # banner resolution on the latent node
        if ($inputs.ContainsKey("width") -and $inputs.ContainsKey("height") -and $inputs["width"] -eq 1024 -and $inputs["height"] -eq 1024) {
            $inputs["width"] = 1536
            $inputs["height"] = 512
        }
        # fresh seed
        foreach ($k in @($inputs.Keys)) {
            if (($k -eq "seed" -or $k -eq "noise_seed") -and ($inputs[$k] -is [long] -or $inputs[$k] -is [int] -or $inputs[$k] -is [double])) {
                $inputs[$k] = $seed
            }
        }
        # output prefix
        if ($g[$nodeId].class_type -eq "SaveImage") { $inputs["filename_prefix"] = "keel-ban" }
    }
    $body = @{ prompt = $g } | ConvertTo-Json -Depth 100
    $resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json"
    Write-Output "queued seed $seed -> $($resp.prompt_id)"
    $ids += $resp.prompt_id
    & $waitDone $resp.prompt_id | Out-Null
}

# 3. Download in order.
$n = 0
foreach ($id in $ids) {
    try { $h = Invoke-RestMethod "$base/history/$id" } catch { continue }
    if (-not ($h.PSObject.Properties.Name -contains $id)) { continue }
    foreach ($node in $h.$id.outputs.PSObject.Properties) {
        foreach ($img in $node.Value.images) {
            $n++
            $name = "keel-ban-{0:d2}.png" -f $n
            $uri = "$base/view?filename=$([uri]::EscapeDataString($img.filename))&subfolder=$([uri]::EscapeDataString($img.subfolder))&type=$($img.type)"
            Invoke-WebRequest -Uri $uri -OutFile (Join-Path $outDir $name) | Out-Null
            Write-Output "saved $name"
        }
    }
}
"# keel-ban v2 (wide banner, title right of ship)`n1536x512, Flux2-Klein graph from keel-rev history.`nSeeds: $($seeds -join ', ')`nPositive prompt:`n$newPositive" | Set-Content (Join-Path $outDir "prompts-v2.md")
Write-Output "BATCH-DONE: $n renders saved"
