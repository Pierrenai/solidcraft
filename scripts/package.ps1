param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if (!$SkipBuild) {
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Compilation impossible.' }
    }
    $raw = cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Lecture des dependances impossible.' }
    $metadata = $raw | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq 'solidcraft' -and !$_.source } | Select-Object -First 1
    $version = $package.version
    $name = "SolidCraft-$version-windows-x86_64"
    $out = Join-Path $root "release/$name"
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Copy-Item -LiteralPath 'target/release/solidcraft.exe' -Destination (Join-Path $out 'SolidCraft.exe') -Force
    foreach ($file in @('LICENSE','README.md','BUILDING.md','Lancer SolidCraft.cmd')) {
        Copy-Item -LiteralPath $file -Destination $out -Force
    }
    Copy-Item -LiteralPath 'Exemples' -Destination $out -Recurse -Force
    $notices = @('# Third-party components', '', 'SolidCraft is GPL-3.0-only. Dependencies retain their own licenses.', 'This inventory conservatively includes the Windows dependency graph and build dependencies.', '', '| Component | License expression | Source |', '| --- | --- | --- |')
    foreach ($dep in ($metadata.packages | Where-Object { $_.source } | Sort-Object name,version)) {
        $depRoot = Split-Path $dep.manifest_path -Parent
        $id = "$($dep.name)-$($dep.version)"
        $destination = Join-Path $out "LICENSES/$id"
        $notices += "| $id | $($dep.license) | https://crates.io/crates/$($dep.name)/$($dep.version) |"
        $files = Get-ChildItem -LiteralPath $depRoot -File -Recurse | Where-Object { $_.Name -match '(?i)(licen[cs]e|copying|copyright|notice|^ofl([.-]|$))' }
        if (!$files) {
            $depRoot = Join-Path $root "third-party/$($dep.name)"
            if (Test-Path -LiteralPath $depRoot) { $files = Get-ChildItem -LiteralPath $depRoot -File -Recurse }
            if (!$files) { throw "Aucun texte de licence trouve pour $id." }
        }
        foreach ($file in $files) {
            $relative = $file.FullName.Substring($depRoot.Length).TrimStart([char[]]"\/")
            $target = Join-Path $destination $relative
            New-Item -ItemType Directory -Force -Path (Split-Path $target -Parent) | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $target -Force
        }
    }
    $notices | Set-Content -LiteralPath (Join-Path $out 'THIRD_PARTY.md') -Encoding UTF8
    $zip = Join-Path $root "release/$name.zip"
    # Cargo archives may contain timestamps before the ZIP epoch (1980).
    Get-ChildItem -LiteralPath $out -Recurse -File | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = [datetime]::new(1980,1,1) }
    Compress-Archive -Path "$out/*" -DestinationPath $zip -Force
    $exe = Join-Path $root "release/$name.exe"
    Copy-Item -LiteralPath (Join-Path $out 'SolidCraft.exe') -Destination $exe -Force
    $hashes = foreach ($file in @($zip,$exe)) {
        $hash = Get-FileHash -LiteralPath $file -Algorithm SHA256
        "$($hash.Hash.ToLowerInvariant())  $(Split-Path $file -Leaf)"
    }
    $hashes | Set-Content -LiteralPath (Join-Path $root 'release/SHA256SUMS.txt') -Encoding ASCII
    Write-Host "Package: $zip"
} finally { Pop-Location }
