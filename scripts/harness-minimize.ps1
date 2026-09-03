# Arnes de reproduccion del congelamiento al minimizar/restaurar.
#
# Lanza la app reproduciendo una pista y cicla: minimizar -> esperar ->
# restaurar COMO LO HACE LA BARRA DE TAREAS (WM_SYSCOMMAND/SC_RESTORE),
# muestreando IsIconic para ver la trayectoria. Si el primer intento no
# restaura, escala a ShowWindow(SW_RESTORE), que es el "insistir" del usuario.
#
#   ./scripts/harness-minimize.ps1 -Cycles 10
#   ./scripts/harness-minimize.ps1 -Cycles 10 -Env POSIBLE_NO_SMTC=1
#
param(
    [string]$Exe = "target/debug/myglossmusic.exe",
    [string]$PlayId = "jig2aRZbHm4",
    [int]$Cycles = 10,
    [string[]]$Env = @()
)

$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsHungAppWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint f, uint t, out IntPtr r);
}
'@

function Test-Responde([IntPtr]$h, [uint32]$timeoutMs = 2000) {
    $r = [IntPtr]::Zero
    $ok = [Win]::SendMessageTimeout($h, 0, [IntPtr]::Zero, [IntPtr]::Zero, 2, $timeoutMs, [ref]$r)
    return ($ok -ne [IntPtr]::Zero)
}

# Espera hasta $ms a que deje de estar minimizada; devuelve los ms que tardo o -1.
function Wait-Deiconify([IntPtr]$h, [int]$ms) {
    for ($t = 0; $t -lt $ms; $t += 100) {
        Start-Sleep -Milliseconds 100
        if (-not [Win]::IsIconic($h)) { return $t + 100 }
    }
    return -1
}

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = (Resolve-Path $Exe).Path
$psi.UseShellExecute = $false
$psi.EnvironmentVariables["POSIBLE_PLAY_TEST"] = $PlayId
foreach ($kv in $Env) {
    $pair = $kv.Split("=", 2)
    $psi.EnvironmentVariables[$pair[0]] = $pair[1]
}
$proc = [System.Diagnostics.Process]::Start($psi)
Write-Host "PID $($proc.Id)  env extra: $($Env -join ' ')"

try {
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 100 -and $h -eq [IntPtr]::Zero; $i++) {
        Start-Sleep -Milliseconds 200
        $proc.Refresh()
        $h = $proc.MainWindowHandle
    }
    if ($h -eq [IntPtr]::Zero) { throw "la ventana principal no aparecio" }
    Start-Sleep -Seconds 6

    $fallosSuaves = 0   # el SC_RESTORE de la barra de tareas fue ignorado
    $fallosDuros = 0    # ni insistiendo volvio, o la ventana dejo de responder
    for ($c = 1; $c -le $Cycles; $c++) {
        [Win]::ShowWindow($h, 6) | Out-Null            # SW_MINIMIZE
        Start-Sleep -Seconds (3 + ($c % 3))

        # Restaurar como la barra de tareas: WM_SYSCOMMAND + SC_RESTORE.
        [Win]::PostMessageW($h, 0x0112, [IntPtr]0xF120, [IntPtr]::Zero) | Out-Null
        $t1 = Wait-Deiconify $h 3000

        $detalle = "sc_restore=${t1}ms"
        if ($t1 -lt 0) {
            $fallosSuaves++
            # Insistir: varios ShowWindow, como el usuario a clics.
            $t2 = -1
            for ($r2 = 1; $r2 -le 4 -and $t2 -lt 0; $r2++) {
                [Win]::ShowWindow($h, 9) | Out-Null    # SW_RESTORE
                $t2 = Wait-Deiconify $h 1500
            }
            $detalle += " insistir=${t2}ms(intentos=$r2)"
            if ($t2 -lt 0) { $fallosDuros++ }
        }
        [Win]::SetForegroundWindow($h) | Out-Null
        Start-Sleep -Milliseconds 400

        $resp = Test-Responde $h
        $hung = [Win]::IsHungAppWindow($h)
        $proc.Refresh()
        $alive = -not $proc.HasExited
        Write-Host ("ciclo {0}: {1} responde={2} colgada={3} viva={4}" -f $c, $detalle, $resp, $hung, $alive)
        if (-not $alive) { $fallosDuros++; Write-Host "PROCESO MUERTO exit=0x$('{0:X}' -f $proc.ExitCode)"; break }
        if (-not $resp) { $fallosDuros++ }
    }
    Write-Host "RESULTADO: ciclos=$Cycles fallosSuaves=$fallosSuaves fallosDuros=$fallosDuros"
}
finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}