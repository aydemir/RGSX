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
    [int]$WebUiPort = 18572,
    [string]$LogFile = "",
    [string]$MarkerFile = ""
)

function Write-Log {
    param([string]$Message)

    $line = "RGSX firewall: $Message"
    if (-not ($LogFile -and $LogFile.Trim())) {
        Write-Output $line
    }

    if ($LogFile -and $LogFile.Trim()) {
        try {
            $logDir = Split-Path -Parent $LogFile
            if ($logDir -and -not (Test-Path -LiteralPath $logDir)) {
                New-Item -ItemType Directory -Path $logDir -Force | Out-Null
            }

            $logLine = "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] $line"
            $written = $false
            for ($attempt = 0; $attempt -lt 3 -and -not $written; $attempt++) {
                try {
                    $fileStream = [System.IO.File]::Open(
                        $LogFile,
                        [System.IO.FileMode]::Append,
                        [System.IO.FileAccess]::Write,
                        [System.IO.FileShare]::ReadWrite
                    )
                    try {
                        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
                        $writer = New-Object System.IO.StreamWriter($fileStream, $utf8NoBom)
                        try {
                            $writer.WriteLine($logLine)
                            $writer.Flush()
                            $written = $true
                        } finally {
                            $writer.Dispose()
                        }
                    } finally {
                        $fileStream.Dispose()
                    }
                } catch {
                    # Retry silently; this log is best effort and must never block startup.
                }
            }
        } catch {
            # Ne pas bloquer le script si le log echoue
        }
    }
}

function Write-FirewallMarker {
    param([string]$MarkerFile)
    if (-not $MarkerFile -or -not $MarkerFile.Trim()) {
        return $false
    }
    try {
        $markerDir = Split-Path -Parent $MarkerFile
        if ($markerDir -and -not (Test-Path -LiteralPath $markerDir)) {
            New-Item -ItemType Directory -Path $markerDir -Force | Out-Null
        }
        $line = "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] RGSX firewall: regles verifiees"
        [System.IO.File]::AppendAllText(
            $MarkerFile,
            $line + [Environment]::NewLine,
            [System.Text.UTF8Encoding]::new($false)
        )
        return $true
    } catch {
        Write-Log "echec ecriture marqueur '$MarkerFile': $($_.Exception.Message)"
        return $false
    }
}

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Add-RgsxFirewallRule {
    param([string]$Name, [string]$ProgramPath)
    if (-not $ProgramPath -or -not (Test-Path -LiteralPath $ProgramPath)) {
        Write-Log "programme introuvable, regle ignoree -> $Name ($ProgramPath)"
        return $false
    }
    try {
        $existing = Get-NetFirewallRule -DisplayName $Name -ErrorAction SilentlyContinue
        if (-not $existing) {
            New-NetFirewallRule -DisplayName $Name -Direction Inbound -Action Allow `
                -Program $ProgramPath -Profile Any -Protocol Any -ErrorAction Stop | Out-Null
            Write-Log "regle ajoutee -> $Name ($ProgramPath)"
        } else {
            Write-Log "regle deja presente -> $Name"
        }
        return $true
    } catch {
        Write-Log "echec ajout regle '$Name': $($_.Exception.Message)"
        return $false
    }
}

function Add-RgsxPortRule {
    param([string]$Name, [int]$Port)
    if (-not $Port -or $Port -le 0) {
        Write-Log "port invalide, regle ignoree -> $Name ($Port)"
        return $false
    }
    try {
        $existing = Get-NetFirewallRule -DisplayName $Name -ErrorAction SilentlyContinue
        if (-not $existing) {
            New-NetFirewallRule -DisplayName $Name -Direction Inbound -Action Allow `
                -Profile Any -Protocol TCP -LocalPort $Port -ErrorAction Stop | Out-Null
            Write-Log "regle port ajoutee -> $Name (TCP/$Port)"
        } else {
            Write-Log "regle port deja presente -> $Name"
        }
        return $true
    } catch {
        Write-Log "echec ajout regle port '$Name': $($_.Exception.Message)"
        return $false
    }
}

if (-not (Test-IsAdmin)) {
    try {
        Write-Log "elevation admin demandee via UAC"
        $selfArgs = @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-WindowStyle', 'Hidden',
            '-File', "`"$PSCommandPath`"",
            '-QbittorrentPath', "`"$QbittorrentPath`"",
            '-WebUiPort', "$WebUiPort"
        )
        if ($LogFile -and $LogFile.Trim()) {
            $selfArgs += @('-LogFile', "`"$LogFile`"")
        }
        if ($MarkerFile -and $MarkerFile.Trim()) {
            $selfArgs += @('-MarkerFile', "`"$MarkerFile`"")
        }
        Start-Process -FilePath 'powershell.exe' -ArgumentList $selfArgs -Verb RunAs -Wait -WindowStyle Hidden
        Write-Log "processus eleve termine"
    } catch {
        Write-Log "elevation refusee ou impossible ($($_.Exception.Message)); poursuite sans regle"
    }
    exit 1
}

Write-Log "debut configuration des regles pare-feu"
$programOk = Add-RgsxFirewallRule -Name 'RGSX qBittorrent Embedded' -ProgramPath $QbittorrentPath
$portOk = Add-RgsxPortRule -Name 'RGSX qBittorrent WebUI (TCP 18572)' -Port $WebUiPort

if ($programOk -and $portOk) {
    if (Write-FirewallMarker -MarkerFile $MarkerFile) {
        Write-Log "regles verifiees, marqueur ecrit"
        exit 0
    }
    Write-Log "regles verifiees mais ecriture marqueur echouee"
    exit 1
}

Write-Log "echec verification regles (program=$programOk, port=$portOk); marqueur non ecrit, nouvel essai au prochain lancement"
exit 1
