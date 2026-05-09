# push-tag.ps1 - 从 Cargo.toml 读取版本号，创建 tag 并推送到远端
# 用法: .\scripts\push-tag.ps1 [-Remote github]

param(
    [string]$Remote = "github"
)

$ErrorActionPreference = "Stop"

# 从 Cargo.toml 读取版本号
$cargoToml = Get-Content -Path "$PSScriptRoot\..\Cargo.toml" -Raw
if ($cargoToml -match 'version\s*=\s*"([^"]+)"') {
    $version = $Matches[1]
} else {
    Write-Error "无法从 Cargo.toml 中读取版本号"
    exit 1
}

$tag = "v$version"
Write-Host "版本: $version, Tag: $tag, Remote: $Remote" -ForegroundColor Cyan

# 检查本地是否已有该 tag
$localTag = git tag -l $tag 2>&1
if ($localTag -ne $tag) {
    Write-Host "创建本地 tag: $tag" -ForegroundColor Yellow
    git tag $tag
    if ($LASTEXITCODE -ne 0) {
        Write-Error "创建本地 tag 失败"
        exit 1
    }
} else {
    Write-Host "本地 tag $tag 已存在" -ForegroundColor Green
}

# 检查远端是否已有该 tag
Write-Host "检查远端 $Remote 是否存在 tag $tag ..." -ForegroundColor Cyan
$remoteTag = git ls-remote --tags $Remote "refs/tags/$tag" 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "无法连接远端 $Remote，请检查网络"
    exit 1
}

if ($remoteTag -and $remoteTag -match "refs/tags/$tag") {
    Write-Host "远端 $Remote 已存在 tag $tag，无需推送" -ForegroundColor Green
    exit 0
}

# 推送 tag
Write-Host "推送 tag $tag 到 $Remote ..." -ForegroundColor Yellow
git push $Remote $tag
if ($LASTEXITCODE -ne 0) {
    Write-Error "推送 tag 失败"
    exit 1
}

Write-Host "✅ 成功推送 tag $tag 到 $Remote" -ForegroundColor Green
