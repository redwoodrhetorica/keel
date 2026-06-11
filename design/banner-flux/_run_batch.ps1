# Finishes the keel-rev Flux2-Klein batch: takes the already-queued patched
# graph as the template, runs 9 more seeds, downloads all 10 renders.
$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8188"
$outDir = $PSScriptRoot

# 1. Locate the template graph + first prompt id (running now, or in history).
$template = $null; $ids = @(); $firstSeed = $null
$q = Invoke-RestMethod "$base/queue"
foreach ($item in @($q.queue_running) + @($q.queue_pending)) {
    $graph = $item[2]
    $isOurs = $false
    foreach ($p in $graph.PSObject.Properties) {
        if ($p.Value.class_type -eq "SaveImage" -and $p.Value.inputs.filename_prefix -eq "keel-rev") { $isOurs = $true }
    }
    if ($isOurs) { $template = $graph | ConvertTo-Json -Depth 100; $ids += $item[1] }
}
if (-not $template) {
    $h = Invoke-RestMethod "$base/history?max_items=50"
    foreach ($p in $h.PSObject.Properties) {
        $graph = $p.Value.prompt[2]
        foreach ($n in $graph.PSObject.Properties) {
            if ($n.Value.class_type -eq "SaveImage" -and $n.Value.inputs.filename_prefix -eq "keel-rev") {
                $template = $graph | ConvertTo-Json -Depth 100; $ids += $p.Name
            }
        }
        if ($template) { break }
    }
}
if (-not $template) { Write-Output "FATAL: no keel-rev template found in queue or history"; exit 1 }
Write-Output "template found; first prompt id(s): $($ids -join ', ')"

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

# 2. Wait for the in-flight first render.
foreach ($id in $ids) { & $waitDone $id | Out-Null }

# 3. Queue + wait the remaining 9 seeds, one at a time.
$seeds = @(1846213, 2951374, 3068425, 4172536, 5286647, 6390758, 7404869, 8518970, 9623081)
foreach ($seed in $seeds) {
    $g = $template | ConvertFrom-Json -AsHashtable
    foreach ($nodeId in @($g.Keys)) {
        $inputs = $g[$nodeId].inputs
        if ($null -ne $inputs) {
            foreach ($k in @($inputs.Keys)) {
                if (($k -eq "seed" -or $k -eq "noise_seed") -and ($inputs[$k] -is [long] -or $inputs[$k] -is [int] -or $inputs[$k] -is [double])) {
                    $inputs[$k] = $seed
                }
            }
        }
    }
    $body = @{ prompt = $g } | ConvertTo-Json -Depth 100
    $resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json"
    Write-Output "queued seed $seed -> $($resp.prompt_id)"
    $ids += $resp.prompt_id
    & $waitDone $resp.prompt_id | Out-Null
}

# 4. Download every render in order.
$n = 0
foreach ($id in $ids) {
    try { $h = Invoke-RestMethod "$base/history/$id" } catch { continue }
    if (-not ($h.PSObject.Properties.Name -contains $id)) { continue }
    foreach ($node in $h.$id.outputs.PSObject.Properties) {
        foreach ($img in $node.Value.images) {
            $n++
            $name = "keel-rev-{0:d2}.png" -f $n
            $uri = "$base/view?filename=$([uri]::EscapeDataString($img.filename))&subfolder=$([uri]::EscapeDataString($img.subfolder))&type=$($img.type)"
            Invoke-WebRequest -Uri $uri -OutFile (Join-Path $outDir $name) | Out-Null
            Write-Output "saved $name (from $($img.filename))"
        }
    }
}
"# keel-rev batch`nTemplate: the user's Flux2-Klein graph (history 672b0d67) with the reverse-engineered prompt.`nFirst seed 9585150 (agent), then seeds: $($seeds -join ', ')`nRenders: $n" | Set-Content (Join-Path $outDir "prompts.md")
Write-Output "BATCH-DONE: $n renders saved"
