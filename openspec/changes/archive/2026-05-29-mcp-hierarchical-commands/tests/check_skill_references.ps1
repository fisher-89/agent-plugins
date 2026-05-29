<#
.SYNOPSIS
验证所有 skill 文件引用新的 MCP tool 层级名称，确保无旧 FQN 残留。

.DESCRIPTION
AC-4: 所有 skill 文件引用新 MCP tool 名称。
逐文件 grep 检查 10 个 skill 文件，确保完全限定名已更新为 xx/yy 格式。

引用更新映射:
  eval_check -> eval/check
  eval_log   -> eval/log
  archi_*    -> archi/*

使用:
    powershell -File check_skill_references.ps1
#>

# ===========================================================================
# 配置
# ===========================================================================

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = Resolve-Path "$SCRIPT_DIR/../../../.."

$SKILLS_DIR = Join-Path $PROJECT_ROOT "plugins/dev-team/skills"

# 预期使用 eval/check 的技能 (10 个)
$SKILLS_USING_EVAL_CHECK = @(
    "phase-requirements",
    "phase-dev-design",
    "phase-test-design",
    "phase-test-gen",
    "phase-implement",
    "phase-code-review",
    "phase-acceptance",
    "openspec-archive-change"
)

# 预期使用 eval/log 的技能 (2 个 - 注意它们只在新名称下符合)
$SKILLS_USING_EVAL_LOG = @(
    "phase-unit-test",
    "phase-integration-test"
)

# 所有 10 个需要检查的技能
$ALL_SKILLS = @(
    "phase-requirements",
    "phase-dev-design",
    "phase-test-design",
    "phase-test-gen",
    "phase-implement",
    "phase-unit-test",
    "phase-code-review",
    "phase-integration-test",
    "phase-acceptance",
    "openspec-archive-change"
)

# 旧名称模式 (在 FQN 上下文中)
$OLD_FQN_PATTERNS = @(
    "mcp__plugin_dev-team_dev-team__eval_log",
    "mcp__plugin_dev-team_dev-team__eval_check",
    "mcp__plugin_dev-team_dev-team__archi_"
)

# 新名称模式 (在 FQN 上下文中)
$NEW_FQN_PATTERNS = @(
    "mcp__plugin_dev-team_dev-team__eval/log",
    "mcp__plugin_dev-team_dev-team__eval/check",
    "mcp__plugin_dev-team_dev-team__archi/"
)

# 旧短名称 (无 FQN 前缀)
$OLD_SHORT_NAMES = @(
    "eval_log",
    "eval_check"
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
# 测试: AC-4 — 每个 skill 文件使用新名称
# ===========================================================================

function Test-Ac4SkillReferences {
    Write-Host "`n=== AC-4: 验证 skill 文件引用新 MCP tool 名称 ===" -ForegroundColor Cyan

    foreach ($skill in $ALL_SKILLS) {
        $skillFile = Join-Path $SKILLS_DIR $skill "SKILL.md"

        if (-not (Test-Path $skillFile)) {
            Fail "AC-4: $skill/SKILL.md 存在" "文件不存在: $skillFile"
            continue
        }
        Pass "AC-4.a: $skill/SKILL.md 存在"

        try {
            $content = Get-Content -Path $skillFile -Raw -ErrorAction Stop
        } catch {
            Fail "AC-4: 读取 $skill/SKILL.md" "读取失败: $_"
            continue
        }

        # 检查旧 FQN 模式是否完全没有残留
        foreach ($oldPattern in $OLD_FQN_PATTERNS) {
            $oldFound = $content -match $oldPattern
            Assert-False $oldFound "AC-4.b: $skill 不含旧 FQN `"$oldPattern`"" "文件包含旧 FQN: $oldPattern"
        }

        # 检查新 FQN 模式存在 (该技能预期使用的)
        $expectedNewPattern = if ($skill -in $SKILLS_USING_EVAL_CHECK) {
            "mcp__plugin_dev-team_dev-team__eval/check"
        } elseif ($skill -in $SKILLS_USING_EVAL_LOG) {
            "mcp__plugin_dev-team_dev-team__eval/log"
        } else {
            $null
        }

        if ($expectedNewPattern) {
            $newFound = $content -match $expectedNewPattern
            Assert-True $newFound "AC-4.c: $skill 包含新 FQN `"$expectedNewPattern`"" "文件中未找到新 FQN: $expectedNewPattern"
        }

        # 检查旧短名称 (不含 FQN 前缀) 不存在
        foreach ($oldShort in $OLD_SHORT_NAMES) {
            $shortFound = $content -match $oldShort
            Assert-False $shortFound "AC-4.d: $skill 不含旧短名称 `"$oldShort`"" "文件包含旧短名称: $oldShort"
        }
    }
}

# ===========================================================================
# 运行器
# ===========================================================================

function Main {
    Write-Host "==================================================" -ForegroundColor Magenta
    Write-Host "  Skill 文件 MCP Tool 名称引用检查" -ForegroundColor Magenta
    Write-Host "  技能目录: $SKILLS_DIR" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    Test-Ac4SkillReferences

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
