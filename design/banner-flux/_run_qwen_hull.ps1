# keel-qwen: the painted-hull banner prompt on Qwen Image (Lightning,
# 4 steps, cfg 1.0) at 1536x512. Three seeds: text rendering is the
# point of the comparison, so one unlucky seed shouldn't decide it.
$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8188"
$outDir = $PSScriptRoot

$positive = @'
Cinematic dark hero render of a massive ship hull seen from the side in profile, low three-quarter camera angle, bow pointing right, the vessel occupying the left half of a wide banner frame, set against a deep navy blue studio background, rich dark blue, not black. A subtle cool ambient fill light gently separates the dark hull silhouette from the navy background. A single dramatic amber-gold rim light traces the leading edge of the bow, from the deck down the curved stem to the rounded bulbous forefoot at the keel. The hull is dark matte steel overlaid with clearly visible glowing blueprint wireframe construction lines and panel seams. Small white draft-mark depth numerals run up the stem near the bow. A dark oxide-red anti-fouling band runs along the bottom of the hull at the waterline, tracing the keel line. Thin deck railings, masts and rigging lines silhouetted along the top of the hull, softening toward the left edge. Soft reflection on a dark glossy studio floor, gentle atmospheric haze. The ship's name "KEEL" is painted in large white block capital letters on the side of the hull near the bow, the paint weathered, worn and faded with age, subtle rust streaks bleeding down from the letters, the lettering following the gentle curvature of the hull plating. No floating text; generous empty deep-navy negative space fills the right side of the frame. Wide cinematic banner composition, moody, precise, engineering-grade product aesthetic.
'@
$negative = "front view, bow-on view, head-on symmetric composition, ship facing the camera, daylight, bright sky, ocean waves, water spray, cartoon, flat vector illustration, low quality, blurry, watermark, floating title text, photograph border"

$seeds = @(7373737, 8181818, 9292929)
$ids = @()
foreach ($seed in $seeds) {
    $wf = @{
        "1"  = @{ class_type = "UNETLoader"; inputs = @{ unet_name = "qwen_image_fp8_e4m3fn.safetensors"; weight_dtype = "default" } }
        "2"  = @{ class_type = "CLIPLoader"; inputs = @{ clip_name = "qwen_2.5_vl_7b_fp8_scaled.safetensors"; type = "qwen_image"; device = "default" } }
        "3"  = @{ class_type = "VAELoader"; inputs = @{ vae_name = "qwen_image_vae.safetensors" } }
        "11" = @{ class_type = "LoraLoaderModelOnly"; inputs = @{ model = @("1", 0); lora_name = "Qwen-Image-2512-Lightning-4steps-V1.0-fp32.safetensors"; strength_model = 1.0 } }
        "4"  = @{ class_type = "ModelSamplingAuraFlow"; inputs = @{ model = @("11", 0); shift = 3.1 } }
        "5"  = @{ class_type = "CLIPTextEncode"; inputs = @{ clip = @("2", 0); text = $positive.Trim() } }
        "6"  = @{ class_type = "CLIPTextEncode"; inputs = @{ clip = @("2", 0); text = $negative } }
        "7"  = @{ class_type = "EmptySD3LatentImage"; inputs = @{ width = 1536; height = 512; batch_size = 1 } }
        "8"  = @{ class_type = "KSampler"; inputs = @{ model = @("4", 0); positive = @("5", 0); negative = @("6", 0); latent_image = @("7", 0); seed = $seed; steps = 4; cfg = 1.0; sampler_name = "euler"; scheduler = "simple"; denoise = 1.0 } }
        "9"  = @{ class_type = "VAEDecode"; inputs = @{ samples = @("8", 0); vae = @("3", 0) } }
        "10" = @{ class_type = "SaveImage"; inputs = @{ images = @("9", 0); filename_prefix = "keel-qwen" } }
    }
    $body = @{ prompt = $wf } | ConvertTo-Json -Depth 12
    $resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json"
    Write-Output "queued seed $seed -> $($resp.prompt_id)"
    $ids += $resp.prompt_id
    # synchronous wait
    $deadline = (Get-Date).AddMinutes(8)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 3
        try { $h = Invoke-RestMethod "$base/history/$($resp.prompt_id)" } catch { continue }
        if ($h.PSObject.Properties.Name -contains $resp.prompt_id) {
            $e = $h.($resp.prompt_id)
            if ($e.status.completed -eq $true -or $e.status.status_str -eq "success") { break }
            if ($e.status.status_str -eq "error") { Write-Output "ERROR on seed $seed"; break }
        }
    }
}
$n = 0
foreach ($id in $ids) {
    try { $h = Invoke-RestMethod "$base/history/$id" } catch { continue }
    if (-not ($h.PSObject.Properties.Name -contains $id)) { continue }
    foreach ($node in $h.$id.outputs.PSObject.Properties) {
        foreach ($img in $node.Value.images) {
            $n++
            $name = "keel-qwen-{0:d2}.png" -f $n
            $uri = "$base/view?filename=$([uri]::EscapeDataString($img.filename))&subfolder=$([uri]::EscapeDataString($img.subfolder))&type=$($img.type)"
            Invoke-WebRequest -Uri $uri -OutFile (Join-Path $outDir $name) | Out-Null
            Write-Output "saved $name"
        }
    }
}
Write-Output "QWEN-BATCH-DONE: $n renders"
