$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

Set-Location -LiteralPath $PSScriptRoot

$outputEncoding = New-Object System.Text.UTF8Encoding($false)
[Console]::InputEncoding = $outputEncoding
[Console]::OutputEncoding = $outputEncoding
$OutputEncoding = $outputEncoding

foreach ($command in @('node.exe', 'cargo.exe', 'pnpm.cmd')) {
    if (-not (Get-Command -Name $command -ErrorAction SilentlyContinue)) {
        throw "Required command not found: $command"
    }
}

Write-Output "node: $(node.exe --version)"
Write-Output "cargo: $(cargo.exe --version)"

$logPath = Join-Path -Path $PSScriptRoot -ChildPath 'hd2cn-debug.log'
Set-Content -LiteralPath $logPath -Value '' -Encoding UTF8
Write-Output "debug log: $logPath"

pnpm.cmd tauri dev 2>&1 | ForEach-Object {
    $line = $_.ToString()
    Write-Output $line
    Add-Content -LiteralPath $logPath -Value $line -Encoding UTF8
}
