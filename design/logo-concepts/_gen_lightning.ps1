param(
  [Parameter(Mandatory=$true)][string]$Positive,
  [Parameter(Mandatory=$true)][int]$Seed,
  [Parameter(Mandatory=$true)][string]$OutPrefix,   # filename_prefix for SaveImage, e.g. keel_04_boolean-intersection
  [string]$FinalName = "",                          # if set, save the result image as this name in the script dir
  [string]$Negative = "text, words, letters, watermark, signature, photograph, photorealistic, 3d render, gradient mesh, busy, cluttered, low contrast, blurry, jpeg artifacts, ugly",
  [int]$Steps = 4,                                  # FAST Lightning recipe
  [double]$Cfg = 1.0,                               # FAST Lightning recipe
  [string]$Sampler = "euler",
  [string]$Scheduler = "simple",
  [double]$Shift = 3.1,
  [int]$Width = 1024,
  [int]$Height = 1024,
  [string]$LoraName = "Qwen-Image-2512-Lightning-4steps-V1.0-fp32.safetensors",
  [double]$LoraStrength = 1.0
)

$base = "http://127.0.0.1:8188"
$dir  = $PSScriptRoot

# Qwen Image text-to-image workflow (API format) — FAST Lightning recipe.
# LoraLoaderModelOnly (node 11) is inserted between UNETLoader (1) and ModelSamplingAuraFlow (4);
# node 4 takes its model from 11 instead of 1. Steps=4, cfg=1.0; everything else matches the vanilla shape.
$wf = @{
  "1"  = @{ class_type = "UNETLoader"; inputs = @{ unet_name = "qwen_image_fp8_e4m3fn.safetensors"; weight_dtype = "default" } }
  "11" = @{ class_type = "LoraLoaderModelOnly"; inputs = @{ model = @("1",0); lora_name = $LoraName; strength_model = $LoraStrength } }
  "2"  = @{ class_type = "CLIPLoader"; inputs = @{ clip_name = "qwen_2.5_vl_7b_fp8_scaled.safetensors"; type = "qwen_image"; device = "default" } }
  "3"  = @{ class_type = "VAELoader"; inputs = @{ vae_name = "qwen_image_vae.safetensors" } }
  "4"  = @{ class_type = "ModelSamplingAuraFlow"; inputs = @{ model = @("11",0); shift = $Shift } }
  "5"  = @{ class_type = "CLIPTextEncode"; inputs = @{ clip = @("2",0); text = $Positive } }
  "6"  = @{ class_type = "CLIPTextEncode"; inputs = @{ clip = @("2",0); text = $Negative } }
  "7"  = @{ class_type = "EmptySD3LatentImage"; inputs = @{ width = $Width; height = $Height; batch_size = 1 } }
  "8"  = @{ class_type = "KSampler"; inputs = @{ model = @("4",0); positive = @("5",0); negative = @("6",0); latent_image = @("7",0); seed = $Seed; steps = $Steps; cfg = $Cfg; sampler_name = $Sampler; scheduler = $Scheduler; denoise = 1.0 } }
  "9"  = @{ class_type = "VAEDecode"; inputs = @{ samples = @("8",0); vae = @("3",0) } }
  "10" = @{ class_type = "SaveImage"; inputs = @{ images = @("9",0); filename_prefix = $OutPrefix } }
}

$body = @{ prompt = $wf } | ConvertTo-Json -Depth 12
$resp = Invoke-RestMethod -Uri "$base/prompt" -Method Post -Body $body -ContentType "application/json"
$promptId = $resp.prompt_id
Write-Output "QUEUED prompt_id=$promptId"

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$deadline = (Get-Date).AddSeconds(180)
while ((Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 1000
  try { $h = Invoke-RestMethod -Uri "$base/history/$promptId" -ErrorAction Stop } catch { continue }
  if ($h.PSObject.Properties.Name -contains $promptId) {
    $entry = $h.$promptId
    $status = $entry.status
    if ($status.completed -eq $true -or $status.status_str -eq "success") {
      $sw.Stop()
      $secs = [math]::Round($sw.Elapsed.TotalSeconds,1)
      $imgs = $entry.outputs."10".images
      if ($imgs) {
        $img = $imgs[0]
        Write-Output ("IMAGE " + $img.filename + "|" + $img.subfolder + "|" + $img.type)
        if ($FinalName -ne "") {
          $url = "$base/view?filename=$([uri]::EscapeDataString($img.filename))&subfolder=$([uri]::EscapeDataString($img.subfolder))&type=$($img.type)"
          Invoke-WebRequest -Uri $url -OutFile "$dir\$FinalName" -ErrorAction Stop | Out-Null
          Write-Output ("SAVED $FinalName")
        }
      }
      Write-Output ("DONE sec=$secs")
      exit 0
    }
    if ($status.status_str -eq "error") {
      Write-Output "ERROR_STATUS"
      $entry.status | ConvertTo-Json -Depth 8
      exit 2
    }
  }
}
Write-Output "TIMEOUT"
exit 3
