#!/usr/bin/env pwsh
# ============================================================================
# build-packages.ps1 — empacota o Mustard (SEM dashboard) para distribuição.
#
# Gera pacotes auto-contidos de binários pré-compilados para um testador rodar
# sem precisar do toolchain Rust:
#   dist/mustard-windows-x64.zip       (binários .exe MSVC, compilados aqui)
#   dist/mustard-linux-x64.tar.gz      (binários glibc, compilados num Docker
#                                        rust:1-bullseye -> glibc 2.31+)
#
# Cada pacote contém: bin/ (scan, mustard-rt, mustard-mcp, mustard, rtk),
# templates/ (a carga do `mustard init`), o instalador (install.ps1/.sh) e o
# README.txt (o pacote Linux leva também o TUTORIAL-LINUX.md). O dashboard
# (apps/dashboard) NÃO é incluído de propósito.
#
# Uso:
#   .\packaging\build-packages.ps1                 # windows + linux
#   .\packaging\build-packages.ps1 -Targets windows
#   .\packaging\build-packages.ps1 -Targets linux
#   .\packaging\build-packages.ps1 -Image rust:1-bookworm   # base Linux alt.
# ============================================================================
[CmdletBinding()]
param(
    [ValidateSet('windows', 'linux', 'both')][string]$Targets = 'both',
    [string]$Image = 'rust:1-bullseye'
)
$ErrorActionPreference = 'Stop'

$PkgDir       = $PSScriptRoot
$Root         = Split-Path -Parent $PkgDir
$Installer    = Join-Path $PkgDir 'installer'
$Dist         = Join-Path $Root 'dist'
$Stage        = Join-Path $Dist '_stage'
$TemplatesSrc = Join-Path $Root 'apps\cli\templates'
$Bins         = @('scan', 'mustard-rt', 'mustard-mcp', 'mustard')

function New-CleanDir([string]$p) {
    if (Test-Path $p) { Remove-Item -Recurse -Force $p }
    New-Item -ItemType Directory -Force -Path $p | Out-Null
}

if (-not (Test-Path $TemplatesSrc)) { throw "templates payload não encontrado em $TemplatesSrc — rode da raiz do repo." }
if (-not (Test-Path $Installer))    { throw "instaladores não encontrados em $Installer." }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

# ---------------------------------------------------------------- Windows ----
if ($Targets -in 'windows', 'both') {
    Write-Host "==> [windows] cargo build --release (4 binários)"
    Push-Location $Root
    try {
        cargo build --release --bin scan --bin mustard-rt --bin mustard-mcp --bin mustard
        if ($LASTEXITCODE -ne 0) { throw "cargo build (windows) falhou (exit $LASTEXITCODE)." }
    } finally { Pop-Location }

    $pkg = Join-Path $Stage 'mustard-windows-x64'
    New-CleanDir (Join-Path $pkg 'bin')
    foreach ($b in $Bins) {
        $src = Join-Path $Root "target\release\$b.exe"
        if (-not (Test-Path $src)) { throw "binário Windows ausente: $src" }
        Copy-Item $src (Join-Path $pkg 'bin') -Force
    }
    # rtk empacotado (best-effort, a partir do que estiver no PATH desta máquina)
    $rtk = (Get-Command rtk -ErrorAction SilentlyContinue).Source
    if ($rtk) {
        Copy-Item $rtk (Join-Path $pkg 'bin\rtk.exe') -Force
        Write-Host "  rtk empacotado: $rtk"
    } else {
        Write-Warning "  rtk não está no PATH — pacote Windows vai sem rtk (o instalador instrui)."
    }
    Copy-Item $TemplatesSrc (Join-Path $pkg 'templates') -Recurse -Force
    Copy-Item (Join-Path $Installer 'install.ps1') $pkg -Force
    Copy-Item (Join-Path $Installer 'README.txt')  $pkg -Force

    $zip = Join-Path $Dist 'mustard-windows-x64.zip'
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path $pkg -DestinationPath $zip
    Write-Host "==> gravado $zip"
}

# ------------------------------------------------------------------ Linux ----
if ($Targets -in 'linux', 'both') {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        throw "docker não encontrado — necessário para o build Linux."
    }
    $tar = Join-Path $Dist 'mustard-linux-x64.tar.gz'
    if (Test-Path $tar) { Remove-Item -Force $tar }

    # Tudo roda DENTRO da imagem rust: compila, baixa o rtk, monta o pacote,
    # aplica chmod +x e gera o tar.gz já no Linux — assim o bit de execução é
    # preservado (o tar.exe do Windows não grava permissões Unix).
    #   CARGO_TARGET_DIR=/tmp/t  -> não colide com o target/ do host em /work
    #   --locked                 -> não mexe no Cargo.lock do host
    # Volumes nomeados cacheiam o registry (CARGO_HOME) e o target entre execuções
    # para re-empacotar rápido; remova-os com `docker volume rm mustard-pkg-cargo
    # mustard-pkg-target` se quiser um build do zero.
    $sh = @'
set -e
export CARGO_TARGET_DIR=/tmp/t
PKG=/tmp/pkg/mustard-linux-x64
rm -rf /tmp/pkg && mkdir -p "$PKG/bin"
echo "[linux] cargo build --release --locked (4 binários)"
cargo build --release --locked --bin scan --bin mustard-rt --bin mustard-mcp --bin mustard
cp /tmp/t/release/scan /tmp/t/release/mustard-rt /tmp/t/release/mustard-mcp /tmp/t/release/mustard "$PKG/bin/"
echo "[linux] obtendo rtk (binário pré-compilado oficial)"
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/master/install.sh | sh || true
for p in "$HOME/.local/bin/rtk" "$HOME/.cargo/bin/rtk" /usr/local/bin/rtk /usr/bin/rtk; do
  if [ -x "$p" ]; then cp "$p" "$PKG/bin/rtk"; echo "[linux] rtk empacotado de $p"; break; fi
done
echo "[linux] montando o pacote (templates + instalador)"
cp -R /work/apps/cli/templates "$PKG/templates"
cp /work/packaging/installer/install.sh /work/packaging/installer/README.txt /work/packaging/installer/TUTORIAL-LINUX.md "$PKG/"
sed -i 's/\r$//' "$PKG/install.sh" 2>/dev/null || true
chmod +x "$PKG"/bin/*
echo "[linux] gerando tar.gz"
cd /tmp/pkg && tar -czf /dist/mustard-linux-x64.tar.gz mustard-linux-x64
echo "[linux] conteúdo do pacote:"; ls -la "$PKG" "$PKG/bin"
'@
    $sh = $sh -replace "`r`n", "`n"   # garante LF para o bash

    Write-Host "==> [linux] docker run $Image  (build pode levar alguns minutos)"
    docker run --rm `
        -e CARGO_HOME=/cache/cargo `
        -v "mustard-pkg-cargo:/cache/cargo" `
        -v "mustard-pkg-target:/tmp/t" `
        -v "${Root}:/work" `
        -v "${Dist}:/dist" `
        -w /work `
        $Image `
        bash -c $sh
    if ($LASTEXITCODE -ne 0) { throw "build Linux no Docker falhou (exit $LASTEXITCODE)." }
    if (-not (Test-Path $tar)) { throw "build Linux não gerou $tar." }
    Write-Host "==> gravado $tar"
}

Write-Host ""
Write-Host "==> Pacotes em $Dist :"
Get-ChildItem $Dist -Filter 'mustard-*' -File | ForEach-Object {
    Write-Host ("    {0}  ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB))
}
