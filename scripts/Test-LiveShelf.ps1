param(
    [Parameter(Mandatory=$true)][string]$Target,
    [string]$Binary = 'target/linux-wsl/debug/ssh_clipboard'
)
$ErrorActionPreference = 'Stop'
if ($Target.StartsWith('-') -or $Target -match '\s') { throw 'Use an SSH alias or user@host.' }
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
& cargo test --no-default-features --lib live_shelf_over_ssh --no-run
if ($LASTEXITCODE -ne 0) { throw 'Could not build the live SSH test.' }
$fixtureSource = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot 'live-shelf-fixture.py'))
$encodedFixture = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($fixtureSource))
$remoteCommand = "python3 -u -c 'import base64;exec(base64.b64decode(`"$encodedFixture`"))'"
$startInfo = [Diagnostics.ProcessStartInfo]::new('ssh')
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.RedirectStandardInput = $true
$startInfo.RedirectStandardOutput = $true
foreach ($argument in @('-o','BatchMode=yes','-o','ConnectTimeout=8',$Target,$remoteCommand)) { $startInfo.ArgumentList.Add($argument) }
$fixtureProcess = [Diagnostics.Process]::Start($startInfo)
$variables = @('SSH_CLIPBOARD_TEST_TARGET','SSH_CLIPBOARD_TEST_BINARY','SSH_CLIPBOARD_TEST_SOCKET')
$oldValues = @{}
foreach ($name in $variables) { $oldValues[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
function Read-FixtureLine {
    $lineTask = $fixtureProcess.StandardOutput.ReadLineAsync()
    if (-not $lineTask.Wait(15000)) { throw 'SSH test fixture did not respond in time.' }
    if (-not $lineTask.Result) { throw 'SSH test fixture ended unexpectedly.' }
    return ($lineTask.Result | ConvertFrom-Json)
}
try {
    $remoteRoot = (Read-FixtureLine).root
    if ($remoteRoot -notmatch '^/tmp/ssh-clipboard-integration-[a-zA-Z0-9_]+$') { throw 'Unexpected temporary fixture path.' }
    & scp -q -o BatchMode=yes -o ConnectTimeout=8 $binaryPath "${Target}:$remoteRoot/ssh_clipboard"
    if ($LASTEXITCODE -ne 0) { throw 'Could not copy the test binary.' }
    $fixtureProcess.StandardInput.WriteLine('{"command":"start"}')
    $fixtureProcess.StandardInput.Flush()
    $ready = Read-FixtureLine
    if (-not $ready.ready) { throw "Disposable shelf server failed to start (exit $($ready.exit_code))." }
    [Environment]::SetEnvironmentVariable($variables[0],$Target,'Process')
    [Environment]::SetEnvironmentVariable($variables[1],"$remoteRoot/ssh_clipboard",'Process')
    [Environment]::SetEnvironmentVariable($variables[2],"$remoteRoot/runtime/shelf.sock",'Process')
    & cargo test --no-default-features --lib live_shelf_over_ssh -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Real SSH shelf integration failed.' }
} finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name,$oldValues[$name],'Process') }
    if (-not $fixtureProcess.HasExited) {
        $fixtureProcess.StandardInput.WriteLine('{"command":"stop"}')
        $fixtureProcess.StandardInput.Close()
        if (-not $fixtureProcess.WaitForExit(10000)) { $fixtureProcess.Kill($true) }
    }
    $fixtureProcess.Dispose()
}
& ssh -o BatchMode=yes -o ConnectTimeout=8 $Target "test ! -e $remoteRoot"
if ($LASTEXITCODE -ne 0) { throw 'The disposable fixture directory was not cleaned up.' }
Write-Output 'Verified real SSH shelf exchanges and disposable fixture cleanup.'
