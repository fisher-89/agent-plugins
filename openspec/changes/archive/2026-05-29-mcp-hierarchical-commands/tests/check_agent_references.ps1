<#
.SYNOPSIS
验证所有 agent 文件引用新的 MCP tool 层级名称，确保无旧 FQN 残留。

.DESCRIPTION
AC-5: 所有 agent 文件引用新 MCP tool 名称。
逐文件 grep 检查 10 个 agent 文件，确保旧名称没有残留。

引用更新映射:
  archi_query     -> archi/query
  archi_validate  -> archi/validate
  archi_write     -> archi/write
  archi_check     -> archi/check
  eval_log        -> eval/log
  eval_check      -> eval/check

使用:
    powershell -File check_agent_references.ps1
#>

# ===========================================================================
# 配置
# ===========================================================================

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = Resolve-Path "$SCRIPT_DIR/../../../.."

$AGENTS_DIR = Join-Path $PROJECT_ROOT "plugins/dev-team/agents"

# 所有需要检查的 agent 文件
$ALL_AGENTS = @(
    "architecture",
    "requirements-evaluator",
    "dev-design-evaluator",
    "test-design-evaluator",
    "test-gen-evaluator",
    "implementation-evaluator",
    "unit-test-evaluator",
    "code-review-evaluator",
    "integration-test-evaluator",
    "acceptance-evaluator"
)

# 旧名称模式 (在 FQN 上下文中)
$OLD_FQN_PATTERNS = @(
    "mcp__plugin_dev-team_dev-team__eval_log",
    "mcp__plugin_dev-team_dev-team__eval_check",
    "mcp__plugin_dev-team_dev-team__archi_query",
    "mcp__plugin_dev-team_dev-team__archi_validate",
    "mcp__plugin_dev-team_dev-team__archi_write",
    "mcp__plugin_dev-team_dev-team__archi_check"
)

# 新 FQN 模式
$NEW_FQN_EVAL_LOG   = "mcp__plugin_dev-team_dev-team__eval/log"
$NEW_FQN_EVAL_CHECK = "mcp__plugin_dev-team_dev-team__eval/check"
$NEW_FQN_ARCHI_QUERY    = "mcp__plugin_dev-team_dev-team__archi/query"
$NEW_FQN_ARCHI_VALIDATE = "mcp__plugin_dev-team_dev-team__archi/validate"
$NEW_FQN_ARCHI_WRITE    = "mcp__plugin_dev-team_dev-team__archi/write"
$NEW_FQN_ARCHI_CHECK    = "mcp__plugin_dev-team_dev-team__archi/check"

# 旧短名称
$OLD_SHORT_NAMES = @(
    "eval_log",
    "eval_check",
    "archi_query",
    "archi_validate",
    "archi_write",
    "archi_check"
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
# 测试: AC-5 — 每个 agent 文件使用新名称
# ===========================================================================

function Test-Ac5AgentReferences {
    Write-Host "`n=== AC-5: 验证 agent 文件引用新 MCP tool 名称 ===" -ForegroundColor Cyan

    foreach ($agent in $ALL_AGENTS) {
        $agentFile = Join-Path $AGENTS_DIR "$agent.md"

        if (-not (Test-Path $agentFile)) {
            Fail "AC-5: $agent.md 存在" "文件不存在: $agentFile"
            continue
        }
        Pass "AC-5.a: $agent.md 存在"

        try {
            $content = Get-Content -Path $agentFile -Raw -ErrorAction Stop
        } catch {
            Fail "AC-5: 读取 $agent.md" "读取失败: $_"
            continue
        }

        # 检查旧 FQN 模式是否完全没有残留
        foreach ($oldPattern in $OLD_FQN_PATTERNS) {
            $oldFound = $content -match $oldPattern
            Assert-False $oldFound "AC-5.b: $agent 不含旧 FQN `"$oldPattern`"" "文件包含旧 FQN: $oldPattern"
        }

        # 检查旧短名称
        foreach ($oldShort in $OLD_SHORT_NAMES) {
            # 旧短名称可能出现在解释性文本中，所以我们重点检查 mcp__ 前缀上下文中的
            # 但这里我们仍然检查裸短名称，因为旧名称不应该以任何形式出现在 agent 的 tool 引用中
            $shortFound = $content -match "(?<!archi/)$oldShort"
            if ($shortFound) {
                # 确认这个旧短名称不是新名称的一部分 (例如 "eval/log" 不包含 "eval_log")
                # 并且不是以 "archi/" 前缀出现
                Fail "AC-5.c: $agent 不含旧短名称 `"$oldShort`"" "文件包含旧短名称: $oldShort"
            } else {
                Pass "AC-5.c: $agent 不含旧短名称 `"$oldShort`""
            }
        }

        # 对于 architecture agent，检查 archi 工具的新名称
        # architecture.md 使用 MCP prefix + 短名称的文档格式，所以匹配新短名称即可
        if ($agent -eq "architecture") {
            $hasArchiQuery = $content -match "archi/query"
            $hasArchiValidate = $content -match "archi/validate"
            $hasArchiWrite = $content -match "archi/write"
            $hasArchiCheck = $content -match "archi/check"

            Assert-True $hasArchiQuery    "AC-5.d: architecture 使用 archi/query"    "未找到 archi/query"
            Assert-True $hasArchiValidate "AC-5.d: architecture 使用 archi/validate" "未找到 archi/validate"
            Assert-True $hasArchiWrite    "AC-5.d: architecture 使用 archi/write"    "未找到 archi/write"
            Assert-True $hasArchiCheck    "AC-5.d: architecture 使用 archi/check"    "未找到 archi/check"
            # architecture agent 仅使用 archi/* 工具，不使用 eval/log
        }

        # 对于 evaluator agent，检查 eval/log
        if ($agent -match "-evaluator$") {
            $hasEvalLog = $content -match [regex]::Escape($NEW_FQN_EVAL_LOG)
            Assert-True $hasEvalLog "AC-5.e: $agent 使用 eval/log" "未找到 $NEW_FQN_EVAL_LOG"
        }
    }
}

# ===========================================================================
# 运行器
# ===========================================================================

function Main {
    Write-Host "==================================================" -ForegroundColor Magenta
    Write-Host "  Agent 文件 MCP Tool 名称引用检查" -ForegroundColor Magenta
    Write-Host "  Agent 目录: $AGENTS_DIR" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    Test-Ac5AgentReferences

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
