param([switch]$Launch)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd([IO.Path]::DirectorySeparatorChar)
$source = Join-Path $root 'target/release/cum-dl.exe'
$destination = Join-Path $root 'cum-dl.exe'
if (Get-Process -Name 'cum-dl' -ErrorAction SilentlyContinue) {
    throw 'Cierra Cum.st antes de actualizar; no se detendra ninguna descarga.'
}
if (-not (Test-Path -LiteralPath $source)) { throw 'Compila primero con cargo build --release --locked.' }
foreach ($path in @($source,$destination)) {
    if (-not ([IO.Path]::GetFullPath($path)).StartsWith($root + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Ruta fuera del repositorio.' }
}
$temporary = Join-Path $root ('target/cum-dl-install-' + [Guid]::NewGuid().ToString('N') + '.exe')
[IO.File]::Copy($source,$temporary)
$expected = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
if ((Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash -ne $expected) { throw 'No coincide el hash de la copia.' }
if (Test-Path -LiteralPath $destination) {
    $backupDirectory = Join-Path $root 'target/previous-builds'
    [IO.Directory]::CreateDirectory($backupDirectory) | Out-Null
    $backup = Join-Path $backupDirectory ('cum-dl-' + [Guid]::NewGuid().ToString('N') + '.exe')
    [IO.File]::Replace($temporary,$destination,$backup)
} else { [IO.File]::Move($temporary,$destination) }
if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $expected) { throw 'No coincide el hash del ejecutable instalado.' }
Write-Output 'Cum.st actualizado. Las descargas y SQLite se conservan.'
if ($Launch) { Start-Process -FilePath $destination -WorkingDirectory $root -WindowStyle Normal }
