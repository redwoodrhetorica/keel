# The completion gate (task 18 / Addendum 177-178 instruments), run from
# a FRESH clone on D: so the ~10h of build + corpus + log churn stays off
# C:. Two halves, launched together and safe to run concurrently:
#   1. WSL all-sectors fuzz soak: bash fuzz/soak_sectors.sh (15 targets
#      x 2400 s, ~10 h).
#   2. Windows release three-bucket oracle at KEEL_ORACLE_N=1000000.
# The gate passes when the soak reports zero crashes and the oracle
# reports WRONG == 0 in both lanes.
#
# Usage (from the repo on C:, on the commit to certify):
#   powershell -File scripts\run_completion_gate.ps1
param(
    [string]$GateDir = "D:\keel-gate",
    [string]$SourceRepo = "C:\Users\mcdon\Documents\Repo\Claude\parasolid",
    [long]$OracleN = 1000000
)
$ErrorActionPreference = "Stop"

# Fresh clone of the CURRENT commit (stale gate dirs certify the wrong code).
if (Test-Path $GateDir) {
    Remove-Item -Recurse -Force $GateDir
}
git clone --local $SourceRepo $GateDir
$head = git -C $SourceRepo rev-parse HEAD
git -C $GateDir checkout $head 2>$null
Write-Host "Gate clone at $GateDir on $head"

# Half 1: the WSL soak, in the background. drvfs (/mnt/d) build perf
# matches the /mnt/c soaks we run today.
$wslPath = "/mnt/" + $GateDir.Substring(0,1).ToLower() + ($GateDir.Substring(2) -replace '\\','/')
$soak = Start-Job -ScriptBlock {
    param($p)
    wsl -e bash -lc "cd $p && source ~/.cargo/env 2>/dev/null; bash fuzz/soak_sectors.sh 2>&1 | tee soak_gate.log; echo GATE-SOAK-EXIT-`$?"
} -ArgumentList $wslPath
Write-Host "Soak launched (WSL, $wslPath, ~10h). Log: $GateDir\soak_gate.log"

# Half 2: the million-trial oracle, foreground (release build on D:).
$env:KEEL_ORACLE_N = "$OracleN"
Push-Location $GateDir
try {
    cargo test --release -p keel-topo --test three_bucket -- --ignored --nocapture 2>&1 |
        Tee-Object -FilePath oracle_gate.log
} finally {
    Pop-Location
    Remove-Item Env:KEEL_ORACLE_N -ErrorAction SilentlyContinue
}

Write-Host "Oracle done. Soak job state: $((Get-Job $soak.Id).State). Wait with: Receive-Job $($soak.Id) -Wait"
