# Builds ZenLink.exe for Start ZenLink.bat and shows the elapsed time and an
# estimate in the window title. The last cargo step ("ZenLink(bin)", the
# whole-program optimisation of the release profile) is one rustc process
# that reports no progress, so the estimate comes from the previous build.
# -Workspace: the repository root (the Cargo workspace).
param([Parameter(Mandatory = $true)][string]$Workspace)

$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Workspace "target" }
$stamp = Join-Path $target "last-build-seconds.txt"
$last = 0
if (Test-Path $stamp) { [int]::TryParse((Get-Content $stamp -Raw).Trim(), [ref]$last) | Out-Null }

function Format-Time([int]$s) { "{0}:{1:00}" -f [math]::Floor($s / 60), ($s % 60) }

$est = if ($last -gt 0) { "~" + (Format-Time $last) } else { "a few minutes" }
Write-Host "Expected time: $est. The last step (ZenLink(bin)) optimises the whole app"
Write-Host "and shows no progress; it takes the longest. Keep this window open."

$oldTitle = $Host.UI.RawUI.WindowTitle
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process cargo -ArgumentList "build", "--release", "-p", "zenlink-app" `
    -WorkingDirectory $Workspace -NoNewWindow -PassThru
$null = $p.Handle  # keeps the exit code readable after the process ends
while (-not $p.WaitForExit(1000)) {
    $s = [int]$sw.Elapsed.TotalSeconds
    $pct = if ($last -gt 0) { " (~{0}%)" -f [math]::Min(99, [int](100 * $s / $last)) } else { "" }
    $Host.UI.RawUI.WindowTitle = "ZenLink - building {0} / {1}{2}" -f (Format-Time $s), $est, $pct
}
$p.WaitForExit()
$Host.UI.RawUI.WindowTitle = $oldTitle
$took = [int]$sw.Elapsed.TotalSeconds
if ($p.ExitCode -eq 0) {
    Write-Host ("Built in " + (Format-Time $took) + ".")
    # Only real builds count; a no-change build takes about a second.
    if ($took -ge 20) { Set-Content -Path $stamp -Value $took -Encoding ascii }
}
exit $p.ExitCode
