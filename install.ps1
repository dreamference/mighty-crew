# Install the latest crew release for Windows (x86_64) into %LOCALAPPDATA%\Programs\crew and add it to the user PATH.
#   irm https://raw.githubusercontent.com/dreamference/mighty-crew/main/install.ps1 | iex
# $env:CREW_VERSION = "0.1.1" pins a version.
$ErrorActionPreference = "Stop"
$repo = "dreamference/mighty-crew"
$target = "x86_64-pc-windows-msvc"
if ($env:CREW_VERSION) { $version = $env:CREW_VERSION } else {
  $version = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name.TrimStart("v")
}
$name = "crew-$version-$target"
$url = "https://github.com/$repo/releases/download/v$version/$name.zip"
$dir = Join-Path $env:LOCALAPPDATA "Programs\crew"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "crew-install-$([System.Guid]::NewGuid())"
New-Item -ItemType Directory -Force -Path $tmp, $dir | Out-Null
Write-Host "downloading $url"
Invoke-WebRequest -Uri $url -OutFile "$tmp\$name.zip"
try {
  $want = (Invoke-RestMethod "$url.sha256").Split(" ")[0].Trim()
  $have = (Get-FileHash "$tmp\$name.zip" -Algorithm SHA256).Hash.ToLower()
  if ($want -ne $have) { throw "checksum mismatch for $name.zip" }
} catch [System.Net.WebException] { }
Expand-Archive -Path "$tmp\$name.zip" -DestinationPath $tmp -Force
Copy-Item "$tmp\$name\crew.exe" "$dir\crew.exe" -Force
Remove-Item -Recurse -Force $tmp
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $dir) {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$dir", "User")
  Write-Host "added $dir to your user PATH; open a new terminal to use it"
}
Write-Host "installed crew $version to $dir\crew.exe"
& "$dir\crew.exe" --version
