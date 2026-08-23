# RGSX - TASK-012-gap-01 agir makine dogrulama listesi (Windows)
# ---------------------------------------------------------------
# Sandbox (ARM proot) SDL2 bundled C derlemesini kaldıramadığı için
# Faz A+B+C degisiklikleri (commit 0f01262 / c46d003 / b8a25c5) burada
# ilk kez tam crate olarak derlenir. Amac: hatalari SIMDI gormek,
# TASK-012h ustune biriktirmeden once temizlemek.
#
# Kullanim: powershell -ExecutionPolicy Bypass -File windows\scripts\verify_gap01.ps1
# Beklenen ozet: "test: 0 | cross: 0" (0 = basarili). Hata cikarsa
# tasks/gap/TASK-012-gap-01-tvui-shell-review.md dosyasina not dusulur.

$ErrorActionPreference = "Continue"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location (Join-Path $repoRoot "manager-rs")

Write-Host ""
Write-Host "== 1/2) cargo test -p manager-tvui (beklenen: 20/20 yesil) ==" -ForegroundColor Cyan
cargo test -p manager-tvui 2>&1 | Tee-Object -Variable testOut | Select-Object -Last 15
$testCode = $LASTEXITCODE

Write-Host ""
Write-Host "== 2/2) cargo check --target x86_64-pc-windows-gnu -p manager-bin ==" -ForegroundColor Cyan
Write-Host "(sdl2 bundled -> CMake + nasm gerektirir; AGENTS.md kural 5 cross-check)"
cargo check --target x86_64-pc-windows-gnu -p manager-bin 2>&1 | Tee-Object -Variable crossOut | Select-Object -Last 10
$crossCode = $LASTEXITCODE

Write-Host ""
Write-Host "== OZET ==" -ForegroundColor Yellow
Write-Host ("test: {0} | cross: {1}   (0 = basarili)" -f $testCode, $crossCode)

if ($testCode -eq 0 -and $crossCode -eq 0) {
    Write-Host ""
    Write-Host "Derleme temiz. Canli smoke (manuel):" -ForegroundColor Green
    Write-Host "  A) Reconnect : set RGSX_TVUI=1 && target\x86_64-pc-windows-msvc\debug\manager-bin.exe"
    Write-Host "     -> TVUI acikken manager'i restart et; loading bar DONMAMALI,"
    Write-Host "        <=3 sn'de 'TVUI SSE bagli' logu geri gelmeli."
    Write-Host "  B) Pencere+gamepad: set RGSX_TVUI_WINDOWED=1 && set RGSX_NATIVE_INPUT=1 && manager-bin.exe"
    Write-Host "     -> resizable pencere; gamepad confirm = Enter davranisi, back = cikis."
} else {
    Write-Host ""
    Write-Host "HATA VAR — TASK-012h baslamadan once duzeltilmeli." -ForegroundColor Red
}

exit ($testCode -bor $crossCode)
