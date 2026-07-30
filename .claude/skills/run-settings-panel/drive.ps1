# Win32 driver for window-selector's settings panel.
# Finds the settings HWND by class name, maps the renderer's CLIENT
# coordinates to screen coordinates, clicks, sends keys, and captures
# window-only screenshots.

Add-Type -AssemblyName System.Drawing

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern IntPtr FindWindowW(string cls, string win);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern short VkKeyScanW(char c);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X,Y; }
  public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004, KEYUP = 0x0002;
}
'@

$SETTINGS_CLASS = "WindowSelectorSettings"

function Get-SettingsHwnd {
  param([int]$TimeoutSec = 10)
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $h = [W]::FindWindowW($SETTINGS_CLASS, [NullString]::Value)
    if ($h -ne [IntPtr]::Zero -and [W]::IsWindowVisible($h)) { return $h }
    Start-Sleep -Milliseconds 250
  }
  return [IntPtr]::Zero
}

# Convert a renderer CLIENT coordinate to screen and click it.
function Click-Client {
  param([IntPtr]$Hwnd, [int]$X, [int]$Y)
  $p = New-Object W+POINT
  $p.X = $X; $p.Y = $Y
  [void][W]::ClientToScreen($Hwnd, [ref]$p)
  [void][W]::SetForegroundWindow($Hwnd)
  Start-Sleep -Milliseconds 250
  [void][W]::SetCursorPos($p.X, $p.Y)
  Start-Sleep -Milliseconds 150
  [W]::mouse_event([W]::LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  [W]::mouse_event([W]::LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 400
  Write-Host "  clicked client($X,$Y) -> screen($($p.X),$($p.Y))"
}

function Send-Key {
  param([byte]$Vk, [byte[]]$Modifiers = @())
  foreach ($m in $Modifiers) { [W]::keybd_event($m, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40 }
  [W]::keybd_event($Vk, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  [W]::keybd_event($Vk, 0, [W]::KEYUP, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 40
  foreach ($m in $Modifiers) { [W]::keybd_event($m, 0, [W]::KEYUP, [UIntPtr]::Zero) }
  Start-Sleep -Milliseconds 500
}

# Capture just the window's client area (not the whole desktop).
function Shot {
  param([IntPtr]$Hwnd, [string]$Path)
  $r = New-Object W+RECT
  [void][W]::GetClientRect($Hwnd, [ref]$r)
  $tl = New-Object W+POINT; $tl.X = 0; $tl.Y = 0
  [void][W]::ClientToScreen($Hwnd, [ref]$tl)
  $w = $r.R - $r.L; $h = $r.B - $r.T
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($tl.X, $tl.Y, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Host "  saved $Path  (client ${w}x${h})"
}
