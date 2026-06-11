# The completion gate (task 18 / Addendum 177-178 instruments), built and
# run from a clone on D: so the build + corpus + log churn stays off C:.
# Two halves:
#   1. WSL all-sectors fuzz soak: bash fuzz/soak_sectors.sh (15 targets
#      x 2400 s, ~10 h sequential).
#   2. The three-bucket oracle at 1M trials, SHARDED across processes
#      (the LCG trial window is seekable via KEEL_ORACLE_START): on the
#      5950X, 14 shards bring ~26 h single-threaded to ~2 h.
# The gate passes when the soak reports zero crashes and the summed
# oracle buckets report WRONG == 0 in both lanes.
#
# Usage (from the repo on C:, on the commit to certify):
#   prepare (clone + build only):  scripts\run_completion_gate.ps1 -PrepareOnly
#   run:                           scripts\run_completion_gate.ps1
param(
    [string]$GateDir = "D:\keel-gate",
    [string]$SourceRepo = "C:\Users\mcdon\Documents\Repo\Claude\parasolid",
    [long]$OracleN = 1000000,
    [int]$OracleShards = 14,
    [switch]$PrepareOnly,
    [switch]$SkipClone
)
$ErrorActionPreference = "Stop"
# Native tools (cargo, git) write progress to stderr; do not promote
# that to terminating errors. Requires pwsh 7 (run with pwsh, not
# Windows PowerShell 5.1).
$PSNativeCommandUseErrorActionPreference = $false

# Fresh clone of the CURRENT commit (a stale gate dir certifies the wrong
# code). -SkipClone reuses an existing prepared clone ONLY if its HEAD
# matches the source repo's.
$head = git -C $SourceRepo rev-parse HEAD
if ($SkipClone -and (Test-Path $GateDir)) {
    $gateHead = git -C $GateDir rev-parse HEAD
    if ($gateHead -ne $head) {
        throw "Gate clone at $GateDir is on $gateHead but source is on $head. Re-run without -SkipClone."
    }
} else {
    if (Test-Path $GateDir) {
        Remove-Item -Recurse -Force $GateDir
    }
    git clone --local --no-hardlinks $SourceRepo $GateDir
}
Write-Host "Gate clone at $GateDir on $head"
$wslPath = "/mnt/" + $GateDir.Substring(0,1).ToLower() + ($GateDir.Substring(2) -replace '\\','/')

# Build both halves up front (prepare phase): the Windows release oracle
# binary and the WSL fuzz targets.
Push-Location $GateDir
try {
    cargo test --release -p keel-topo --test three_bucket --no-run 2>&1 |
        Tee-Object -FilePath build_oracle.log | Out-Null
} finally {
    Pop-Location
}
$exe = Get-ChildItem "$GateDir\target\release\deps\three_bucket-*.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $exe) { throw "oracle test binary not found after build" }
Write-Host "Oracle binary: $($exe.FullName)"
wsl -e bash -lc "cd $wslPath && source ~/.cargo/env 2>/dev/null; cargo +nightly fuzz build 2>&1 | tail -3"
Write-Host "WSL fuzz targets built."
if ($PrepareOnly) {
    Write-Host "PREPARE complete. Run the gate later with: scripts\run_completion_gate.ps1 -SkipClone"
    exit 0
}

# Half 1: the WSL soak, in the background.
$soak = Start-Job -ScriptBlock {
    param($p)
    wsl -e bash -lc "cd $p && source ~/.cargo/env 2>/dev/null; bash fuzz/soak_sectors.sh 2>&1 | tee soak_gate.log; echo GATE-SOAK-DONE"
} -ArgumentList $wslPath
Write-Host "Soak launched (WSL, $wslPath, ~10h sequential). Log: $GateDir\soak_gate.log"

# Half 2: the sharded oracle. Shard i covers trials [i*chunk, (i+1)*chunk).
$chunk = [long][math]::Ceiling($OracleN / $OracleShards)
$shardJobs = @()
for ($i = 0; $i -lt $OracleShards; $i++) {
    $s = $i * $chunk
    $n = [math]::Min($chunk, $OracleN - $s)
    if ($n -le 0) { break }
    $log = Join-Path $GateDir "oracle_shard_$i.log"
    $shardJobs += Start-Job -ScriptBlock {
        param($exe, $s, $n, $log)
        $env:KEEL_ORACLE_START = "$s"
        $env:KEEL_ORACLE_N = "$n"
        & $exe --ignored --nocapture 2>&1 | Out-File $log -Encoding utf8
    } -ArgumentList $exe.FullName, $s, $n, $log
}
Write-Host "Oracle: $($shardJobs.Count) shards x $chunk trials. Waiting..."
$shardJobs | Wait-Job | Out-Null
# Aggregate the shard buckets.
$tot = @{ pass = 0L; dec = 0L; wrong = 0L; tpass = 0L; tdec = 0L; twrong = 0L }
Get-ChildItem "$GateDir\oracle_shard_*.log" | ForEach-Object {
    $m = Select-String -Path $_ -Pattern "strict PASS (\d+) / DECLINE (\d+) / WRONG (\d+); tolerant PASS (\d+) / DECLINE (\d+) / WRONG (\d+)"
    if ($m) {
        $g = $m.Matches[0].Groups
        $tot.pass += [long]$g[1].Value; $tot.dec += [long]$g[2].Value; $tot.wrong += [long]$g[3].Value
        $tot.tpass += [long]$g[4].Value; $tot.tdec += [long]$g[5].Value; $tot.twrong += [long]$g[6].Value
    }
}
"GATE ORACLE TOTAL (N=$OracleN): strict PASS $($tot.pass) / DECLINE $($tot.dec) / WRONG $($tot.wrong); tolerant PASS $($tot.tpass) / DECLINE $($tot.tdec) / WRONG $($tot.twrong)" |
    Tee-Object -FilePath "$GateDir\oracle_gate_total.log"
Select-String -Path "$GateDir\oracle_shard_*.log" -Pattern "WRONG (strict|tolerant)" | ForEach-Object { $_.Line }
# The oracle now finishes in minutes; the soak takes ~10 h. The script
# MUST outlive the soak job (a child job dies with its parent), so wait.
Write-Host "Oracle done. Waiting for the soak (~10h)..."
Wait-Job $soak | Out-Null
Get-Content "$GateDir\soak_gate.log" -Tail 8
Write-Host "GATE-COMPLETE"
