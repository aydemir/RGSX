' RGSX Web UI Desktop Shortcut Creator
' This script creates a desktop shortcut for RGSX Web UI
' Usage: cscript //nologo create_shortcut.vbs

Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

' Get the directory of this script
scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)

' Create shortcut on desktop
Set shortcut = WshShell.CreateShortcut(WshShell.SpecialFolders("Desktop") & "\RGSX Web UI.lnk")
shortcut.TargetPath = WComSpec
shortcut.Arguments = "/c """ & scriptDir & "\RGSX Retrobat.bat"" --webui"
shortcut.WorkingDirectory = scriptDir
shortcut.IconLocation = scriptDir & "\..\..\roms\ports\RGSX\assets\images\favicon_rgsx.ico"
shortcut.Description = "RGSX Web UI - Web browser interface"
shortcut.Save

WScript.Echo "Desktop shortcut created successfully!"
WScript.Echo "Location: " & WshShell.SpecialFolders("Desktop") & "\RGSX Web UI.lnk"
