# keel-ban2 v2.1: wide banner (1536x512 via the PrimitiveInt width/height
# nodes), title right of ship, LIGHTING LIFTED (navy not black, ambient
# fill, wireframe clearly visible). Template = keel-rev/keel-ban graph
# from history. 10 seeds, synchronous, downloads at the end.
$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8188"
$outDir = $PSScriptRoot

$newPositive = @'
Cinematic dark hero render of a massive ship hull seen from the side in profile, low three-quarter camera angle, bow pointing right, the vessel occupying the left half of a wide banner frame, set against a deep navy blue studio background, rich dark blue, not black. A subtle cool ambient fill light gently separates the dark hull silhouette from the navy background. A single dramatic amber-gold rim light traces the leading edge of the bow, from the deck down the curved stem to the rounded bulbous forefoot at the keel. The hull is dark matte steel overlaid with clearly visible glowing blueprint wireframe construction lines and panel seams. Small white draft-mark depth numerals run up the stem near the bow. A dark oxide-red anti-fouling band runs along the bottom of the hull at the waterline, tracing the keel line. Thin deck railings, masts and rigging lines silhouetted along the top of the hull, softening toward the left edge. Soft reflection on a dark glossy studio floor, gentle atmospheric haze. Bold clean white sans-serif title text "KEEL" set in the open navy space to the right of the ship, vertically centered, with generous margins. Wide cinematic banner composition, moody, precise, engineering-grade product aesthetic.
'@

# 1. Template from history (keel-rev or keel-ban graph).
$template = $null
$h = Invoke-RestMethod "$base/history?max_items=100"
foreach ($p in $h.PSObject.Properties) {
    $graph = $p.Value.prompt[2]
    foreach ($n in $graph.PSObject.Properties) {
        if ($n.Value.class_type -eq "SaveImage" -and $n.Value.inputs.filename_prefix -in @("keel-rev", "keel-ban")) {
            $template = $graph | ConvertTo-Json -Depth 100
        }
    }
    if ($template) { break }
}
if (-not $template) { Write-Output "FATAL: template not found"; exit 1 }
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

$seeds = @(5151515, 6262626, 7373737, 8484848, 9595959, 1616161, 2727272, 3838383, 4949494, 6060606)
$ids = @()
foreach ($seed in $seeds) {
    $g = $template | ConvertFrom-Json -AsHashtable
    # Identify the width/height PrimitiveInt nodes by their CONSUMERS.
    $widthNodes = @(); $heightNodes = @()
    foreach ($nodeId in @($g.Keys)) {
        $inputs = $g[$nodeId].inputs
        if ($null -eq $inputs) { continue }
        foreach ($k in @($inputs.Keys)) {
            $v = $inputs[$k]
            if ($v -is [object[]] -and $v.Count -eq 2) {
                if ($k -eq "width") { $widthNodes += [string]$v[0] }
                if ($k -eq "height") { $heightNodes += [string]$v[0] }
            }
        }
    }
    foreach ($nid in ($widthNodes | Sort-Object -Unique)) {
        if ($g.ContainsKey($nid) -and $g[$nid].class_type -like "Primitive*") { $g[$nid].inputs["value"] = 1536 }
    }
    foreach ($nid in ($heightNodes | Sort-Object -Unique)) {
        if ($g.ContainsKey($nid) -and $g[$nid].class_type -like "Primitive*") { $g[$nid].inputs["value"] = 512 }
    }
    # Positive prompt, seed, prefix.
    foreach ($nodeId in @($g.Keys)) {
        $inputs = $g[$nodeId].inputs
        if ($null -eq $inputs) { continue }
        if ($g[$nodeId].class_type -match "CLIPTextEncode" -and $inputs.ContainsKey("text") -and $inputs["text"] -like "Cinematic dark hero render*") {
            $inputs["text"] = $newPositive.Trim()
        }
        foreach ($k in @($inputs.Keys)) {
            if (($k -eq "seed" -or $k -eq "noise_seed") -and ($inputs[$k] -is [long] -or $inputs[$k] -is [int] -or $inputs[$k] -is [double])) {
                $inputs[$k] = $seed
            }
        }
        if ($g[$nodeId].class_type -eq "SaveImage") { $inputs["filename_prefix"] = "keel-ban2" }
    }
    $body = @{ prompt = $g } | ConvertTo-Json -Depth 100
    $resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json"
    Write-Output "queued seed $seed -> $($resp.prompt_id)"
    $ids += $resp.prompt_id
    & $waitDone $resp.prompt_id | Out-Null
}

$n = 0
foreach ($id in $ids) {
    try { $h = Invoke-RestMethod "$base/history/$id" } catch { continue }
    if (-not ($h.PSObject.Properties.Name -contains $id)) { continue }
    foreach ($node in $h.$id.outputs.PSObject.Properties) {
        foreach ($img in $node.Value.images) {
            $n++
            $name = "keel-ban2-{0:d2}.png" -f $n
            $uri = "$base/view?filename=$([uri]::EscapeDataString($img.filename))&subfolder=$([uri]::EscapeDataString($img.subfolder))&type=$($img.type)"
            Invoke-WebRequest -Uri $uri -OutFile (Join-Path $outDir $name) | Out-Null
            Write-Output "saved $name"
        }
    }
}
"# keel-ban2 v2.1 (1536x512 via PrimitiveInt patch; lifted lighting; title right)`nSeeds: $($seeds -join ', ')`nPositive:`n$newPositive" | Set-Content (Join-Path $outDir "prompts-v2_1.md")
Write-Output "BATCH-DONE: $n renders saved"
