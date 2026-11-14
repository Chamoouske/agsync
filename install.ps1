param(
    [string]$Version = ''
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$base = 'https://github.com/Chamoouske/agsync/releases'
if ($env:AGSYNC_INSTALL_BASE_URL) {
    $base = $env:AGSYNC_INSTALL_BASE_URL.TrimEnd('/')
}
$asset = 'agsync-x86_64-pc-windows-msvc.exe'
if ($Version) {
    $tag = if ($Version.StartsWith('v')) { $Version } else { "v$Version" }
    $url = "$base/download/$tag"
} else {
    $url = "$base/latest/download"
}

$bin = Join-Path $env:LOCALAPPDATA 'agsync\bin'
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("agsync-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $download = Join-Path $tmp $asset
    $sums = Join-Path $tmp 'SHA256SUMS'
    Invoke-WebRequest -UseBasicParsing -Uri "$url/$asset" -OutFile $download
    Invoke-WebRequest -UseBasicParsing -Uri "$url/SHA256SUMS" -OutFile $sums

    $expected = $null
    foreach ($line in Get-Content $sums) {
        $fields = $line.Trim() -split '\s+'
        if ($fields.Count -eq 2 -and $fields[1].TrimStart('*') -eq $asset) {
            $expected = $fields[0].ToLower()
        }
    }
    if (-not $expected) {
        throw "SHA256SUMS nao tem $asset"
    }
    $actual = (Get-FileHash -Algorithm SHA256 -Path $download).Hash.ToLower()
    if ($actual -ne $expected) {
        throw "SHA256 de $asset nao confere: esperado $expected, veio $actual"
    }

    New-Item -ItemType Directory -Force -Path $bin | Out-Null
    Copy-Item -Force -Path $download -Destination (Join-Path $bin 'agsync.exe')
} finally {
    Remove-Item -Recurse -Force -Path $tmp
}

$pathVar = 'Path'
if ($env:AGSYNC_TEST_PATH_VAR -like 'AGSYNC_TEST_*') {
    $pathVar = $env:AGSYNC_TEST_PATH_VAR
}
$current = [Environment]::GetEnvironmentVariable($pathVar, 'User')
$parts = @(($current -split ';') | Where-Object { $_ })
if ($parts -notcontains $bin) {
    [Environment]::SetEnvironmentVariable($pathVar, (($parts + $bin) -join ';'), 'User')
    Write-Host "PATH do usuario: $bin adicionado; abra um terminal novo"
}

Write-Host "agsync instalado em $(Join-Path $bin 'agsync.exe')"
Write-Host 'proximo passo: agsync init <url> --profile <perfil>'
