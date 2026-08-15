# setup-msvc.ps1 — 下载并配置目标架构 MSVC 编译环境（GitHub Actions 用）。
#
# 替代 loonghao/msvc-kit action（其下载 URL 拼错资产名导致 404）。
# 复刻 action 逻辑：下载 msvc-kit.exe → download 目标架构组件 → setup 导出 env。
#
# 用法：.\setup-msvc.ps1 -Arch x64|arm64|x86 [-HostArch x64] [-MsvcVersion 14.44]

param(
    [Parameter(Mandatory = $true)][string]$Arch,
    [string]$HostArch = "x64",
    [string]$MsvcVersion = "14.44"
)

$ErrorActionPreference = "Stop"
$asset = "msvc-kit-x86_64-windows.exe"
$kitDir = Join-Path $env:RUNNER_TEMP "msvc-kit"
New-Item -ItemType Directory -Path $kitDir -Force | Out-Null

# 1. 下载 msvc-kit（真实资产名，无版本号）
$exe = Join-Path $kitDir "msvc-kit.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Downloading msvc-kit ($asset)..."
    gh release download --repo loonghao/msvc-kit --pattern $asset --dir $kitDir
    Move-Item (Join-Path $kitDir $asset) $exe -Force
}

# 2. 下载目标架构 MSVC + SDK
Write-Host "Downloading MSVC $MsvcVersion ($Arch host=$HostArch)..."
& $exe download --arch $Arch --host-arch $HostArch --msvc-version $MsvcVersion
if ($LASTEXITCODE -ne 0) { throw "msvc-kit download failed (arch=$Arch)" }

# 3. 生成环境变量脚本并写入 GITHUB_ENV / GITHUB_PATH
Write-Host "Setting up toolchain env..."
$envScript = & $exe setup --script --shell powershell --arch $Arch --host-arch $HostArch
if ($LASTEXITCODE -ne 0) { throw "msvc-kit setup failed (arch=$Arch)" }

foreach ($line in $envScript) {
    # 形如：$env:XXX = "value"
    if ($line -match '^\s*\$env:([^=]+)\s*=\s*"?(.*?)"?\s*$') {
        $key = $matches[1]
        $value = $matches[2]
        Add-Content $env:GITHUB_ENV "$key=$value"
    }
}

# PATH 处理（msvc-kit 输出的 PATH 命令）
foreach ($line in $envScript) {
    if ($line -match 'PATH.*\$env:PATH') {
        # setup 脚本可能用 "$env:PATH=..." 形式；msvc-kit 若输出 PATH 追加，
        # 这里解析成 GITHUB_PATH 追加项。
        if ($line -match '\$env:PATH.*["''](.*?)["'']') {
            Add-Content $env:GITHUB_PATH $matches[1]
        }
    }
}

Write-Host "MSVC $MsvcVersion toolchain ready for arch=$Arch"
