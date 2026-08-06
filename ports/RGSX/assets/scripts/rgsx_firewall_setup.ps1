# RGSX - Configuration automatique du pare-feu Windows (une seule fois)
# ---------------------------------------------------------------------
# Ajoute des regles de pare-feu entrantes pour qBittorrent integre et pour
# le port TCP 18572 utilise par l'interface WebUI de debug sur le reseau local.
#
# S'auto-eleve une seule fois (invite UAC unique) si necessaire. Si
# l'utilisateur refuse l'elevation ou n'a pas les droits admin, le script
# se termine silencieusement sans bloquer le lancement de RGSX : les
# telechargements fonctionnent toujours, mais l'acces LAN a la WebUI et les
# connexions entrantes qBittorrent peuvent etre moins efficaces.

param(
    [string]$QbittorrentPath,
    [int]$WebUiPort = 18572
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

function Add-RgsxPortRule {
    param([string]$Name, [int]$Port)
    if (-not $Port -or $Port -le 0) {
        return
    }
    try {
        $existing = Get-NetFirewallRule -DisplayName $Name -ErrorAction SilentlyContinue
        if (-not $existing) {
            New-NetFirewallRule -DisplayName $Name -Direction Inbound -Action Allow `
                -Profile Any -Protocol TCP -LocalPort $Port -ErrorAction Stop | Out-Null
            Write-Output "RGSX firewall: regle port ajoutee -> $Name (TCP/$Port)"
        } else {
            Write-Output "RGSX firewall: regle port deja presente -> $Name"
        }
    } catch {
        Write-Output "RGSX firewall: echec ajout regle port '$Name': $($_.Exception.Message)"
    }
}

if (-not (Test-IsAdmin)) {
    try {
        $selfArgs = @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-WindowStyle', 'Hidden',
            '-File', "`"$PSCommandPath`"",
            '-QbittorrentPath', "`"$QbittorrentPath`"",
            '-WebUiPort', "$WebUiPort"
        )
        Start-Process -FilePath 'powershell.exe' -ArgumentList $selfArgs -Verb RunAs -Wait -WindowStyle Hidden
    } catch {
        Write-Output "RGSX firewall: elevation refusee ou impossible ($($_.Exception.Message)); poursuite sans regle."
    }
    exit 0
}

Add-RgsxFirewallRule -Name 'RGSX qBittorrent Embedded' -ProgramPath $QbittorrentPath
Add-RgsxPortRule -Name 'RGSX qBittorrent WebUI (TCP 18572)' -Port $WebUiPort
