# RGSX - TASK-012-gap-01 agir makine dogrulama listesi (Windows)
# ---------------------------------------------------------------
# Faz A+B+C degisiklikleri (0f01262 / c46d003 / b8a25c5) icin tam crate
# derleme + Windows-gnu cross-check. 2026-08-24'te gecen konfigurasyonla
# guncellendi (bkz. tasks/gap/TASK-012-gap-01-tvui-shell-review.md).
#
# Kullanim: powershell -ExecutionPolicy Bypass -File windows\scripts\verify_gap01.ps1
# Beklenen ozet: "test: 0 | cross: 0" (0 = basarili). Hata cikarsa
# tasks/gap/TASK-012-gap-01-tvui-shell-review.md dosyasina not dusulur.
#
# Zorunlu env notlari:
# - RUSTUP_HOME/CARGO_HOME acikca set edilmeli; oturumda eski env varsa cargo
#   yanlis rustup home'a (~/.rustup) bakip gnu std bulamayabilir (E0463).
# - CMAKE_GENERATOR=Ninja: MinGW make, PATH'teki busybox sh.exe ile Error 127,
#   sh'siz Error 2 veriyor; Ninja sh'e ihtiyac duymaz.
# - CFLAGS=-std=gnu11: GCC 16 varsayilani C23 (true/false keyword) SDL2'yi patlatir.
# - CMAKE_POLICY_VERSION_MINIMUM=3.5: cmake 4.x + eski CMakeLists uyumu.

$ErrorActionPreference = "Continue"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location (Join-Path $repoRoot "manager-rs")

$env:RUSTUP_HOME = "C:\Users\lv\scoop\persist\rustup\.rustup"
$env:CARGO_HOME  = "C:\Users\lv\scoop\persist\rustup\.cargo"
$env:CMAKE_POLICY_VERSION_MINIMUM = "3.5"
$env:CMAKE_GENERATOR = "Ninja"
$env:CFLAGS = "-std=gnu11"

$sys  = [Environment]::GetFolderPath('System')
$wind = [Environment]::GetFolderPath('Windows')
$env:PATH = @(
    "C:\Users\lv\scoop\apps\rustup\current\.cargo\bin",
    "C:\Users\lv\scoop\apps\mingw\current\bin",
    "C:\Users\lv\scoop\apps\ninja\current",
    "C:\Users\lv\scoop\apps\cmake\current\bin",
    "C:\Users\lv\scoop\apps\nasm\current",
    $sys, $wind, "$wind\System32\Wbem"
) -join ";"

Write-Host ""
Write-Host "== 1/2) cargo test -p manager-tvui (beklenen: yesil) ==" -ForegroundColor Cyan
cargo test -p manager-tvui 2>&1 | Tee-Object -Variable testOut | Select-Object -Last 15
$testCode = $LASTEXITCODE

Write-Host ""
Write-Host "== 2/2) cargo check --target x86_64-pc-windows-gnu -p manager-bin ==" -ForegroundColor Cyan
Write-Host "(sdl2 bundled -> mingw + cmake + nasm; AGENTS.md kural 5 cross-check)"
cargo check --target x86_64-pc-windows-gnu -p manager-bin 2>&1 | Tee-Object -Variable crossOut | Select-Object -Last 10
$crossCode = $LASTEXITCODE

Write-Host ""
Write-Host "== OZET ==" -ForegroundColor Yellow
Write-Host ("test: {0} | cross: {1}   (0 = basarili)" -f $testCode, $crossCode)

if ($testCode -eq 0 -and $crossCode -eq 0) {
    Write-Host ""
    Write-Host "Derleme temiz. Canli smoke (manuel):" -ForegroundColor Green
    Write-Host "  A) Reconnect : set RGSX_TVUI=1 && C:\Users\lv\RGSX\rust-target\debug\manager-bin.exe"
    Write-Host "     -> TVUI acikken manager'i restart et; loading bar DONMAMALI,"
    Write-Host "        <=3 sn'de 'TVUI SSE bagli' logu geri gelmeli."
    Write-Host "  B) Pencere+gamepad: set RGSX_TVUI_WINDOWED=1 && set RGSX_NATIVE_INPUT=1 && manager-bin.exe"
    Write-Host "     -> resizable pencere; gamepad confirm = Enter davranisi, back = cikis."
} else {
    Write-Host ""
    Write-Host "HATA VAR — ilgili fazin basligina donulmeli." -ForegroundColor Red
}

exit ($testCode -bor $crossCode)
