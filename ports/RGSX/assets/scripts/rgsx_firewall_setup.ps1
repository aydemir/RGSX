# RGSX - Configuration automatique du pare-feu Windows (une seule fois)
# ---------------------------------------------------------------------
# Ajoute des regles de pare-feu entrantes pour aria2c.exe (telechargements
# BitTorrent) et python.exe (decouverte UPnP), afin que les utilisateurs
# n'aient jamais besoin de configurer le pare-feu manuellement.
#
# S'auto-eleve une seule fois (invite UAC unique) si necessaire. Si
# l'utilisateur refuse l'elevation ou n'a pas les droits admin, le script
# se termine silencieusement sans bloquer le lancement de RGSX : les
# telechargements fonctionnent toujours, seul le port-forwarding
# automatique (UPnP) et les connexions entrantes BitTorrent peuvent etre
# moins efficaces.

param(
    [string]$Aria2cPath,
    [string]$PythonPath
)

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Add-RgsxFirewallRule {
    param([string]$Name, [string]$ProgramPath)
    if (-not $ProgramPath -or -not (Test-Path -LiteralPath $ProgramPath)) {
        return
    }
    try {
        $existing = Get-NetFirewallRule -DisplayName $Name -ErrorAction SilentlyContinue
        if (-not $existing) {
            New-NetFirewallRule -DisplayName $Name -Direction Inbound -Action Allow `
                -Program $ProgramPath -Profile Any -Protocol Any -ErrorAction Stop | Out-Null
            Write-Output "RGSX firewall: regle ajoutee -> $Name ($ProgramPath)"
        } else {
            Write-Output "RGSX firewall: regle deja presente -> $Name"
        }
    } catch {
        Write-Output "RGSX firewall: echec ajout regle '$Name': $($_.Exception.Message)"
    }
}

if (-not (Test-IsAdmin)) {
    try {
        $selfArgs = @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-WindowStyle', 'Hidden',
            '-File', "`"$PSCommandPath`"",
            '-Aria2cPath', "`"$Aria2cPath`"",
            '-PythonPath', "`"$PythonPath`""
        )
        Start-Process -FilePath 'powershell.exe' -ArgumentList $selfArgs -Verb RunAs -Wait -WindowStyle Hidden
    } catch {
        Write-Output "RGSX firewall: elevation refusee ou impossible ($($_.Exception.Message)); poursuite sans regle."
    }
    exit 0
}

Add-RgsxFirewallRule -Name 'RGSX aria2c (BitTorrent/UPnP)' -ProgramPath $Aria2cPath
Add-RgsxFirewallRule -Name 'RGSX Python (UPnP discovery)' -ProgramPath $PythonPath
