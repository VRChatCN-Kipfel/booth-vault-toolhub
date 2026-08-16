# setup-msvc.ps1 — 下载并配置目标架构 MSVC 编译环境（GitHub Actions 用）。
#
# 基于 msvc-kit CLI（v0.2.15）实测：
#   download --arch <x64|x86|arm64> -t <dir> [--no-verify]
#   setup --script --shell powershell --arch <arch> -d <dir>
# 该版本无 --host-arch（host 自动检测）。
# setup 输出格式：标量 $env:XXX = "val"；数组 @(...) -join ";"；PATH 前缀拼接。
# 这里只导出 INCLUDE/LIB 等编译必需变量（GitHub runner 已自带完整 PATH）。
#
# 用法：.\setup-msvc.ps1 -Arch x64|arm64|x86

param(
    [Parameter(Mandatory = $true)][string]$Arch
)

$ErrorActionPreference = "Stop"
$asset = "msvc-kit-x86_64-windows.exe"
$kitDir = Join-Path $env:RUNNER_TEMP "msvc-kit"
$installDir = Join-Path $env:RUNNER_TEMP "msvc-kit-toolchain"
New-Item -ItemType Directory -Path $kitDir -Force | Out-Null
New-Item -ItemType Directory -Path $installDir -Force | Out-Null

# 1. 下载 msvc-kit（真实资产名）
$exe = Join-Path $kitDir "msvc-kit.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Downloading msvc-kit ($asset)..."
    gh release download --repo loonghao/msvc-kit --pattern $asset --dir $kitDir
    Move-Item (Join-Path $kitDir $asset) $exe -Force
}

# 2. 下载目标架构 MSVC + SDK
Write-Host "Downloading MSVC (arch=$Arch)..."
& $exe download --arch $Arch -t $installDir --no-verify
if ($LASTEXITCODE -ne 0) { throw "msvc-kit download failed (arch=$Arch)" }

# 3. 生成激活脚本
Write-Host "Generating activation script (arch=$Arch)..."
$envScript = & $exe setup --script --shell powershell --arch $Arch -d $installDir
if ($LASTEXITCODE -ne 0) { throw "msvc-kit setup failed (arch=$Arch)" }

# 解析：标量 $env:NAME = "value"；数组 @(...) -join ";"。
# 合并数组段（INCLUDE/LIB 是多行数组）。
$curName = $null
$curValue = ""
$inArray = $false

foreach ($line in $envScript) {
    $t = $line.Trim()
    if ($t -eq "" -or $t.StartsWith("#")) { continue }

    # 数组起始：$env:NAME = @(
    if ($t -match '^\$env:([^=]+)\s*=\s*@\($') {
        FlushVar $curName $curValue
        $curName = $matches[1].Trim()
        $curValue = ""
        $inArray = $true
        continue
    }
    if ($inArray) {
        # 数组元素 "a",
        if ($t -match '^"([^"]*)",?\s*$') {
            if ($curValue) { $curValue += ";" }
            $curValue += $matches[1]
            continue
        }
        # 数组结束 ) -join ";"
        if ($t.StartsWith(")")) {
            FlushVar $curName $curValue
            $curName = $null
            $curValue = ""
            $inArray = $false
            continue
        }
        continue
    }
    # 标量：$env:NAME = "value"（跳过 PATH，它引用 $NewPaths 变量）
    if ($t -match '^\$env:([^=]+)\s*=\s*"(.*)"\s*$') {
        FlushVar $curName $curValue
        $curName = $matches[1].Trim()
        $curValue = $matches[2]
        if ($curName -eq "PATH") {
            # PATH 跳过（runner 已有系统 PATH）
            $curName = $null
            $curValue = ""
        }
        continue
    }
}
FlushVar $curName $curValue

Write-Host "MSVC toolchain ready for arch=$Arch"
exit 0

function FlushVar {
    param($Name, $Value)
    if (-not $Name -or -not $Value) { return }
    Write-Host "GITHUB_ENV: $Name=$Value"
    Add-Content $env:GITHUB_ENV "$Name=$Value"
}
