# keel-ban3: single render. Same composition/negative space as v2.1 #3
# (seed 7373737), but "KEEL" painted on the hull as weathered ship
# lettering instead of floating title text.
$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8188"
$outDir = $PSScriptRoot

$newPositive = @'
Cinematic dark hero render of a massive ship hull seen from the side in profile, low three-quarter camera angle, bow pointing right, the vessel occupying the left half of a wide banner frame, set against a deep navy blue studio background, rich dark blue, not black. A subtle cool ambient fill light gently separates the dark hull silhouette from the navy background. A single dramatic amber-gold rim light traces the leading edge of the bow, from the deck down the curved stem to the rounded bulbous forefoot at the keel. The hull is dark matte steel overlaid with clearly visible glowing blueprint wireframe construction lines and panel seams. Small white draft-mark depth numerals run up the stem near the bow. A dark oxide-red anti-fouling band runs along the bottom of the hull at the waterline, tracing the keel line. Thin deck railings, masts and rigging lines silhouetted along the top of the hull, softening toward the left edge. Soft reflection on a dark glossy studio floor, gentle atmospheric haze. The ship's name "KEEL" is painted in large white block capital letters on the side of the hull near the bow, the paint weathered, worn and faded with age, subtle rust streaks bleeding down from the letters, the lettering following the gentle curvature of the hull plating. No floating text; generous empty deep-navy negative space fills the right side of the frame. Wide cinematic banner composition, moody, precise, engineering-grade product aesthetic.
'@

$template = $null
$h = Invoke-RestMethod "$base/history?max_items=100"
foreach ($p in $h.PSObject.Properties) {
    $graph = $p.Value.prompt[2]
    foreach ($n in $graph.PSObject.Properties) {
        if ($n.Value.class_type -eq "SaveImage" -and $n.Value.inputs.filename_prefix -eq "keel-ban2") {
            $template = $graph | ConvertTo-Json -Depth 100
        }
    }
    if ($template) { break }
}
if (-not $template) { Write-Output "FATAL: keel-ban2 template not found"; exit 1 }

$g = $template | ConvertFrom-Json -AsHashtable
foreach ($nodeId in @($g.Keys)) {
    $inputs = $g[$nodeId].inputs
    if ($null -eq $inputs) { continue }
    if ($g[$nodeId].class_type -match "CLIPTextEncode" -and $inputs.ContainsKey("text") -and $inputs["text"] -like "Cinematic dark hero render*") {
        $inputs["text"] = $newPositive.Trim()
    }
    foreach ($k in @($inputs.Keys)) {
        if (($k -eq "seed" -or $k -eq "noise_seed") -and ($inputs[$k] -is [long] -or $inputs[$k] -is [int] -or $inputs[$k] -is [double])) {
            $inputs[$k] = 7373737
        }
    }
    if ($g[$nodeId].class_type -eq "SaveImage") { $inputs["filename_prefix"] = "keel-ban3" }
}
$body = @{ prompt = $g } | ConvertTo-Json -Depth 100
$resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json"
$id = $resp.prompt_id
Write-Output "queued -> $id (seed 7373737, painted-hull lettering)"

$deadline = (Get-Date).AddMinutes(10)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 3
    try { $hh = Invoke-RestMethod "$base/history/$id" } catch { continue }
    if ($hh.PSObject.Properties.Name -contains $id) {
        $e = $hh.$id
        if ($e.status.completed -eq $true -or $e.status.status_str -eq "success") {
            foreach ($node in $e.outputs.PSObject.Properties) {
                foreach ($img in $node.Value.images) {
                    $uri = "$base/view?filename=$([uri]::EscapeDataString($img.filename))&subfolder=$([uri]::EscapeDataString($img.subfolder))&type=$($img.type)"
                    Invoke-WebRequest -Uri $uri -OutFile (Join-Path $outDir "keel-ban3-01.png") | Out-Null
                    Write-Output "saved keel-ban3-01.png"
                }
            }
            Write-Output "DONE"
            exit 0
        }
        if ($e.status.status_str -eq "error") { Write-Output "ERROR"; exit 2 }
    }
}
Write-Output "TIMEOUT"
