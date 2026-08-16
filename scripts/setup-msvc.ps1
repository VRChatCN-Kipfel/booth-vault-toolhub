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
#
# GitHub API 限速：gh 命令用 workflow 传入的 GITHUB_TOKEN。
# 三个架构 job 并发下载，避免匿名限速墙。

param(
    [Parameter(Mandatory = $true)][string]$Arch
)

$ErrorActionPreference = "Stop"

# ---- helpers（必须在使用前定义）----

function FlushVar {
    param($Name, $Value)
    if (-not $Name -or -not $Value) { return }
    Write-Host "GITHUB_ENV: $Name=$Value"
    Add-Content $env:GITHUB_ENV "$Name=$Value"
}

# ---- main ----

$asset = "msvc-kit-x86_64-windows.exe"
$kitDir = Join-Path $env:RUNNER_TEMP "msvc-kit"
$installDir = Join-Path $env:RUNNER_TEMP "msvc-kit-toolchain"
New-Item -ItemType Directory -Path $kitDir -Force | Out-Null
New-Item -ItemType Directory -Path $installDir -Force | Out-Null

# 1. 下载 msvc-kit（真实资产名，用 GITHUB_TOKEN 避免限速）
$exe = Join-Path $kitDir "msvc-kit.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Downloading msvc-kit ($asset)..."
    gh release download --repo loonghao/msvc-kit --pattern $asset --dir $kitDir
    if ($LASTEXITCODE -ne 0) { throw "msvc-kit 下载失败" }
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

# 解析（避免复杂正则，用字符串处理）：
#   数组起始: $env:NAME = @(
#   数组元素: "path",
#   数组结束: ) -join ";"
#   标量:     $env:NAME = "value"
#   PATH:     $env:PATH = "$NewPaths;$env:PATH"   → 跳过
$curName = $null
$curValue = ""
$inArray = $false
# PATH 前缀段（msvc-kit 的 $NewPaths 数组 → 加入 GITHUB_PATH）
$pathSegs = @()
$inPathArray = $false

foreach ($line in $envScript) {
    $t = $line.Trim()
    if ($t -eq "" -or $t.StartsWith("#")) { continue }

    # $NewPaths = @( → PATH 数组起始
    if ($t.StartsWith('$NewPaths') -and $t.Contains('= @(')) {
        $inPathArray = $true
        continue
    }
    if ($inPathArray) {
        if ($t.StartsWith('"')) {
            $item = $t.Trim().TrimEnd(',').Trim('"')
            if ($item) { $pathSegs += $item }
            continue
        }
        if ($t.StartsWith(')')) {
            $inPathArray = $false
            continue
        }
        continue
    }

    # 数组起始：$env:NAME = @(
    if ($t.StartsWith('$env:') -and $t.Contains('= @(')) {
        FlushVar $curName $curValue
        $curName = $t.Substring(5).Split('=')[0].Trim()
        $curValue = ""
        $inArray = $true
        continue
    }
    if ($inArray) {
        # 数组元素 "xxx" 或 "xxx",
        if ($t.StartsWith('"')) {
            # 去首尾引号 + 尾部逗号
            $item = $t.Trim().TrimEnd(',').Trim('"')
            if ($curValue) { $curValue += ";" }
            $curValue += $item
            continue
        }
        # 数组结束
        if ($t.StartsWith(')')) {
            FlushVar $curName $curValue
            $curName = $null
            $curValue = ""
            $inArray = $false
            continue
        }
        continue
    }
    # 标量
    if ($t.StartsWith('$env:')) {
        FlushVar $curName $curValue
        $eqIdx = $t.IndexOf('=')
        if ($eqIdx -gt 0) {
            $name = $t.Substring(5, $eqIdx - 5).Trim()
            $value = $t.Substring($eqIdx + 1).Trim()
            if ($name -eq "PATH") {
                # PATH 跳过（runner 已有系统 PATH）
                $curName = $null
                $curValue = ""
                continue
            }
            # 去引号
            $value = $value.Trim('"', ' ')
            $curName = $name
            $curValue = $value
            FlushVar $curName $curValue
            $curName = $null
            $curValue = ""
        }
        continue
    }
}
FlushVar $curName $curValue

# 把 MSVC bin 目录加入 GITHUB_PATH（rustc 需要在此找 link.exe/rc.exe，
# 否则命中 Git Bash 的 /usr/bin/link.exe 导致链接失败）
foreach ($seg in $pathSegs) {
    if ($seg) {
        Write-Host "GITHUB_PATH: $seg"
        Add-Content $env:GITHUB_PATH $seg
    }
}

Write-Host "MSVC toolchain ready for arch=$Arch"
