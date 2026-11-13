#!/usr/bin/env pwsh
# Build basalt-tui into bin/basalt.exe with a version stamp.
$ErrorActionPreference = "Stop"
$version = if ($args.Count -gt 0) { $args[0] } else { "0.1.0" }
New-Item -ItemType Directory -Force -Path bin | Out-Null
go build -trimpath -ldflags "-s -w -X github.com/juang/basalt-tui/internal/tui.Version=$version" -o bin/basalt.exe .
Write-Host "built bin/basalt.exe ($version)"
