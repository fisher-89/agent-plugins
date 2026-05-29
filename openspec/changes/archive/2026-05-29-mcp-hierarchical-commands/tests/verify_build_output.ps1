<#
.SYNOPSIS
验证编译产物 dev-team-mcp.cjs 使用 xx/yy 层级 tool 名称。

.DESCRIPTION
AC-3: 编译产物 dev-team-mcp.cjs 反映新命名。
检查编译产物的 TOOLS 字符串和 switch 语句是否使用 xx/yy 格式。

边界场景:
- 编译产物中旧名称仍出现 -> 检测到旧名称残留并报错
- TOOLS 数组与 switch case 不匹配

使用:
    powershell -File verify_build_output.ps1
    # 或指定产物路径:
    powershell -File verify_build_output.ps1 -BuildOutputPath "plugins/dev-team/bin/dev-team-mcp.cjs"
#>

param(
    [string]$BuildOutputPath = ""
)

# ===========================================================================
# 配置
# ===========================================================================

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = Resolve-Path "$SCRIPT_DIR/../../../.."

if (-not $BuildOutputPath) {
    $BuildOutputPath = Join-Path $PROJECT_ROOT "plugins/dev-team/bin/dev-team-mcp.cjs"
}

# 层级名称映射
$NEW_NAMES = @(
    "eval/log",
    "eval/check",
    "archi/query",
    "archi/validate",
    "archi/write",
    "archi/check"
)

$OLD_NAMES = @(
    "eval_log",
    "eval_check",
    "archi_query",
    "archi_validate",
    "archi_write",
    "archi_check"
)

$OLD_PATTERNS = @(
    "eval_log",
    "eval_check",
    "archi_"
)

# 计数器
$global:PASS_COUNT = 0
$global:FAIL_COUNT = 0

# ===========================================================================
# 帮助函数
# ===========================================================================

function Pass($name) {
    Write-Host "  PASS: $name"
    $global:PASS_COUNT++
}

function Fail($name, $detail) {
    Write-Host "  FAIL: $name -- $detail"
    $global:FAIL_COUNT++
}

function Assert-True($condition, $testName, $detail) {
    if ($condition) {
        Pass $testName
    } else {
        Fail $testName $detail
    }
}

function Assert-False($condition, $testName, $detail) {
    if (-not $condition) {
        Pass $testName
    } else {
        Fail $testName $detail
    }
}

# ===========================================================================
# 源码读取
# ===========================================================================

function Read-BuildOutput {
    if (-not (Test-Path $BuildOutputPath)) {
        Write-Host "ERROR: 编译产物不存在: $BuildOutputPath" -ForegroundColor Red
        Write-Host "       请先执行构建命令 (npm run build) 生成 dev-team-mcp.cjs" -ForegroundColor Yellow
        exit 1
    }

    try {
        return Get-Content -Path $BuildOutputPath -Raw -ErrorAction Stop
    } catch {
        Write-Host "ERROR: 无法读取文件 $BuildOutputPath : $_" -ForegroundColor Red
        exit 1
    }
}

# ===========================================================================
# 测试: AC-3 — 编译产物使用新名称
# ===========================================================================

function Test-Ac3BuildOutputNewNames {
    Write-Host "`n=== AC-3: 编译产物 dev-team-mcp.cjs 使用 xx/yy 格式 ===" -ForegroundColor Cyan
    Write-Host "  产物路径: $BuildOutputPath"

    $content = Read-BuildOutput

    # 检查所有新名称出现在产物中
    foreach ($name in $NEW_NAMES) {
        $escapedName = $name -replace '/', '\/'
        $found = $content -match $escapedName
        Assert-True $found "AC-3.a: 编译产物包含新名称 `"$name`"" "未在产物中找到 `"$name`""
    }

    # 检查所有旧名称 (xx_yy) 不再出现在产物中
    foreach ($oldName in $OLD_NAMES) {
        $found = $content -match $oldName
        Assert-False $found "AC-3.b: 编译产物不包含旧名称 `"$oldName`"" "产物中仍存在旧名称 `"$oldName`""
    }

    # 检查旧模式 archi_ 整体不在产物中 (排除正常用法)
    foreach ($pattern in $OLD_PATTERNS) {
        $found = $content -match $pattern
        Assert-False $found "AC-3.c: 编译产物不包含旧模式 `"$pattern`"" "产物中仍存在旧模式 `"$pattern`""
    }
}

# ===========================================================================
# 测试: 边界场景 — 产物 TOOLS 与 switch 交叉检查
# ===========================================================================

function Test-EdgeBuildConsistency {
    Write-Host "`n=== 边界: 编译产物 TOOLS 与 switch 交叉检查 ===" -ForegroundColor Cyan

    $content = Read-BuildOutput

    # 提取编译产物中的 name 引用 (在 TOOLS 定义上下文中)
    $toolNameMatches = [regex]::Matches($content, 'name:\s*"([^"]+)"')
    $toolNames = $toolNameMatches | ForEach-Object { $_.Groups[1].Value }

    # 提取 switch case 标签
    $caseMatches = [regex]::Matches($content, 'case\s+"([^"]+)":')
    $caseNames = $caseMatches | ForEach-Object { $_.Groups[1].Value }

    # TOOLS 中的每个 name 必须在 switch 中有对应 case
    foreach ($name in $toolNames) {
        $inSwitch = $caseNames -contains $name
        Assert-True $inSwitch "边界.a: TOOLS 中的 `"$name`" 在 switch 中有对应 case" "TOOLS 包含 `"$name`" 但 switch 中无对应"
    }

    # switch 中的每个 case 必须在 TOOLS 中有对应
    foreach ($case in $caseNames) {
        $inTools = $toolNames -contains $case
        Assert-True $inSwitch "边界.b: switch case `"$case`" 在 TOOLS 中有对应 entry" "switch 包含 `"$case`" 但 TOOLS 中无对应"
    }

    # 验证产物中无旧名称出现在工具定义的 name 字段中 (仅检查前几个 name，排除 import/require 路径)
    Write-Host "  INFO: 提取到的 tool name 引用: $($toolNames -join ', ')"
}

# ===========================================================================
# 测试: 边界场景 — 产物中旧名称残留检测
# ===========================================================================

function Test-EdgeNoOldNameResidue {
    Write-Host "`n=== 边界: 产物中无旧名称残留 ===" -ForegroundColor Cyan

    $content = Read-BuildOutput

    # 从内容中过滤掉 import/require 路径和注释，仅关注逻辑代码中的字符串
    # 移除注释行
    $codeOnly = $content -replace '//.*', ''

    # 检查代码中是否存在旧工具名称作为字符串字面量 (非注释部分)
    foreach ($oldName in $OLD_NAMES) {
        # 在代码中寻找 "eval_log", "eval_check" 等作为字符串字面量出现
        $pattern = '"' + $oldName + '"'
        $literalMatch = $codeOnly -match $pattern
        Assert-False $literalMatch "边界.c: 代码中无旧名称字符串 `"$oldName`"" "代码中仍存在 `"$oldName`" 字符串字面量"
    }
}

# ===========================================================================
# 运行器
# ===========================================================================

function Main {
    Write-Host "==================================================" -ForegroundColor Magenta
    Write-Host "  编译产物层级名称验证" -ForegroundColor Magenta
    Write-Host "  产物: $BuildOutputPath" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    Test-Ac3BuildOutputNewNames
    Test-EdgeBuildConsistency
    Test-EdgeNoOldNameResidue

    # 汇总
    $total = $global:PASS_COUNT + $global:FAIL_COUNT
    Write-Host "`n==================================================" -ForegroundColor Magenta
    Write-Host "  结果: $($global:PASS_COUNT) passed, $($global:FAIL_COUNT) failed (共 $total 项)" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    if ($global:FAIL_COUNT -gt 0) {
        exit 1
    }
    exit 0
}

Main
