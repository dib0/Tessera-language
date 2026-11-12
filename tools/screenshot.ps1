# Saves a picture of a window to a PNG file, the Windows counterpart of
# `import -window <id> shot.png`. Pairs with tools/xsend/xsend.exe, which
# prints window handles and sends clicks and keys.
#
#   powershell -ExecutionPolicy Bypass -File tools/screenshot.ps1 Tesseradoom shot.png
#   powershell -ExecutionPolicy Bypass -File tools/screenshot.ps1 0x3306fc shot.png
#
# The window is named by a piece of its title or by a handle as `xsend list`
# prints it. The whole window is captured, title bar included, so the picture
# shows what a person sees.
param(
    [Parameter(Mandatory = $true)][string]$Window,
    [Parameter(Mandatory = $true)][string]$Out
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class TesseraShot {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

function Resolve-Window([string]$name) {
    if ($name -match '^(0x)?[0-9a-fA-F]+$') {
        $value = if ($name.StartsWith('0x')) { [Convert]::ToInt64($name.Substring(2), 16) } else { [Convert]::ToInt64($name) }
        $handle = [IntPtr]$value
        if ([TesseraShot]::IsWindow($handle)) { return $handle }
    }
    $found = @()
    $callback = [TesseraShot+EnumProc] {
        param($h, $l)
        if ([TesseraShot]::IsWindowVisible($h)) {
            $title = New-Object System.Text.StringBuilder 512
            if ([TesseraShot]::GetWindowTextW($h, $title, $title.Capacity) -gt 0 -and $title.ToString().Contains($name)) {
                $script:found += @{ Handle = $h; Title = $title.ToString() }
            }
        }
        return $true
    }
    [void][TesseraShot]::EnumWindows($callback, [IntPtr]::Zero)
    $found = $script:found
    if ($found.Count -eq 0) { throw "no window has `"$name`" in its title; try: tools/xsend/xsend.exe list" }
    if ($found.Count -gt 1) { throw "`"$name`" matches $($found.Count) windows: $(($found | ForEach-Object { $_.Title }) -join ', ')" }
    return $found[0].Handle
}

$script:found = @()
$handle = Resolve-Window $Window
[void][TesseraShot]::SetForegroundWindow($handle)
Start-Sleep -Milliseconds 200

$rect = New-Object TesseraShot+RECT
if (-not [TesseraShot]::GetWindowRect($handle, [ref]$rect)) { throw 'the window has no position' }
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) { throw 'the window is not on screen' }

$bitmap = New-Object System.Drawing.Bitmap($width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
$bitmap.Save((Resolve-Path -LiteralPath (Split-Path -Parent (Join-Path (Get-Location) $Out))).Path + '\' + (Split-Path -Leaf $Out), [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
Write-Output "$Out ($width x $height)"
