# windows-support 5.3: install the NSIS build silently, start D.Duck, check that a main window appears, then kill it.
# No login, no network calls of its own; the app finds no Claude CLI on a fresh runner and shows its setup screen.
$ErrorActionPreference = 'Stop'
$installer = Get-ChildItem -Path 'app/target/release/bundle/nsis' -Filter '*-setup.exe' | Select-Object -First 1
if (-not $installer) { throw 'no NSIS installer found' }
Write-Host "installer: $($installer.Name) ($([math]::Round($installer.Length / 1MB, 1)) MB)"
$p = Start-Process -FilePath $installer.FullName -ArgumentList '/S' -PassThru -Wait
if ($p.ExitCode -ne 0) { throw "installer exit code $($p.ExitCode)" }
$dir = Join-Path $env:LOCALAPPDATA 'D.Duck'
$exe = Get-ChildItem -Path $dir -Filter '*.exe' -ErrorAction SilentlyContinue | Where-Object { $_.Name -notlike 'uninstall*' } | Select-Object -First 1
if (-not $exe) { throw "no app executable under $dir (currentUser install)" }
Write-Host "installed: $($exe.FullName)"
$app = Start-Process -FilePath $exe.FullName -PassThru
$shown = $false
for ($i = 0; $i -lt 60 -and -not $shown; $i++) {
  Start-Sleep -Seconds 1
  $proc = Get-Process -Id $app.Id -ErrorAction SilentlyContinue
  if (-not $proc) { throw 'D.Duck exited during start' }
  $proc.Refresh()
  if ($proc.MainWindowHandle -ne 0) { $shown = $true }
}
taskkill /F /T /PID $app.Id | Out-Null
if (-not $shown) { throw 'no main window within 60 s' }
Write-Host 'main window shown; killed'
