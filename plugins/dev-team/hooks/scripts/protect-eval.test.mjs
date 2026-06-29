/**
 * 单元测试: protect-eval.mjs — PreToolUse eval.json 保护逻辑
 *
 * @see openspec/changes/rewrite-hooks-to-node/test-design.md
 *
 * 用法: node --test protect-eval.test.mjs
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(new URL("./protect-eval.mjs", import.meta.url));
const {
  isEvalJsonPath,
  detectBashWrite,
  detectPowerShellWrite,
  extractChangeName,
  buildDenyReason,
  parseInput,
  outputDeny,
} = await import("./protect-eval.mjs");

function parseHookStdout(stdout) {
  const parsed = JSON.parse(stdout.trim());
  return parsed.hookSpecificOutput?.permissionDecision ?? "";
}

// ---------------------------------------------------------------------------
// isEvalJsonPath — 路径匹配 (AC-3)
// ---------------------------------------------------------------------------

describe("isEvalJsonPath — 路径匹配", () => {
  it("相对路径 openspec/changes/test-change/eval.json 应返回 true", () => {
    assert.equal(isEvalJsonPath("openspec/changes/test-change/eval.json"), true);
  });

  it("绝对路径 D:/Projects/.../eval.json 应返回 true", () => {
    assert.equal(
      isEvalJsonPath("D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json"),
      true,
    );
  });

  it("非 eval 文件 openspec/changes/test/design.md 应返回 false", () => {
    assert.equal(isEvalJsonPath("openspec/changes/test/design.md"), false);
  });

  it("changes 目录外路径应返回 false", () => {
    assert.equal(isEvalJsonPath("plugins/dev-team/bin/src/commands/phase-log.ts"), false);
  });

  it("反斜杠归一化 openspec\\changes\\test\\eval.json 应返回 true", () => {
    assert.equal(isEvalJsonPath("openspec\\changes\\test\\eval.json"), true);
  });

  it("空字符串应返回 false", () => {
    assert.equal(isEvalJsonPath(""), false);
  });

  it("超长路径仍含 eval.json 后缀时应返回 true", () => {
    const prefix = "a".repeat(1000);
    const path = `${prefix}/openspec/changes/test-change/eval.json`;
    assert.equal(isEvalJsonPath(path), true);
  });
});

// ---------------------------------------------------------------------------
// detectBashWrite — Bash 写入检测与豁免 (AC-4)
// ---------------------------------------------------------------------------

describe("detectBashWrite — 写入模式", () => {
  it("> 重定向到 eval.json 应返回 true", () => {
    assert.equal(detectBashWrite("echo '[]' > openspec/changes/test/eval.json"), true);
  });

  it(">> 追加到 eval.json 应返回 true", () => {
    assert.equal(detectBashWrite("echo '[]' >> openspec/changes/test/eval.json"), true);
  });

  it("tee 写入 eval.json 应返回 true", () => {
    assert.equal(detectBashWrite("echo '[]' | tee openspec/changes/test/eval.json"), true);
  });

  it("tee -a 追加 eval.json 应返回 true", () => {
    assert.equal(detectBashWrite("echo '[]' | tee -a openspec/changes/test/eval.json"), true);
  });

  it("heredoc 写入 eval.json 应返回 true", () => {
    const cmd = "cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF";
    assert.equal(detectBashWrite(cmd), true);
  });

  it(">& 重定向到 eval.json 应返回 true", () => {
    assert.equal(detectBashWrite("echo '[]' >& openspec/changes/test/eval.json"), true);
  });

  it("含 -> 但无 eval.json 写入时应返回 false", () => {
    assert.equal(detectBashWrite("echo foo -> bar"), false);
  });
});

describe("detectBashWrite — 豁免与只读", () => {
  it("python 命令应豁免并返回 false", () => {
    assert.equal(
      detectBashWrite("python plugins/dev-team/utils/eval-check.py --change test"),
      false,
    );
  });

  it("python3 命令应豁免并返回 false", () => {
    assert.equal(detectBashWrite('python3 -c "print(1)"'), false);
  });

  it("node 命令应豁免并返回 false", () => {
    assert.equal(detectBashWrite("node scripts/write-eval.mjs"), false);
  });

  it("只读 cat eval.json 应返回 false", () => {
    assert.equal(detectBashWrite("cat openspec/changes/test/eval.json"), false);
  });

  it("无 eval.json 的命令 ls -la 应返回 false", () => {
    assert.equal(detectBashWrite("ls -la"), false);
  });

  it("空命令应返回 false", () => {
    assert.equal(detectBashWrite(""), false);
  });
});

// ---------------------------------------------------------------------------
// detectPowerShellWrite — PowerShell 写入检测 (AC-4)
// ---------------------------------------------------------------------------

describe("detectPowerShellWrite — 写入模式", () => {
  it("Set-Content 写入 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite('Set-Content -Path openspec/changes/test/eval.json -Value "[]"'),
      true,
    );
  });

  it("Set-Content 流水线写入 eval.json 应返回 true", () => {
    assert.equal(detectPowerShellWrite('"[]" | Set-Content openspec/changes/test/eval.json'), true);
  });

  it("Set-Content 大小写不敏感应返回 true", () => {
    assert.equal(
      detectPowerShellWrite('set-content -path openspec/changes/test/eval.json -value "[]"'),
      true,
    );
  });

  it("Out-File 写入 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite(
        "Get-Content data.txt | Out-File -FilePath openspec/changes/test/eval.json",
      ),
      true,
    );
  });

  it("Add-Content 追加 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite('Add-Content -Path openspec/changes/test/eval.json -Value "new entry"'),
      true,
    );
  });

  it("Export-Csv 写入 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite("$data | Export-Csv -Path openspec/changes/test/eval.json"),
      true,
    );
  });

  it("Export-CliXml 写入 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite("$data | Export-CliXml -Path openspec/changes/test/eval.json"),
      true,
    );
  });

  it("Tee-Object 写入 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite(
        "Get-Content data.txt | Tee-Object -FilePath openspec/changes/test/eval.json",
      ),
      true,
    );
  });

  it("> 重定向到 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite("Get-Content data.txt > openspec/changes/test/eval.json"),
      true,
    );
  });

  it(">> 追加到 eval.json 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite("Get-Content data.txt >> openspec/changes/test/eval.json"),
      true,
    );
  });

  it("*> 合并流到 eval.json 应返回 true", () => {
    assert.equal(detectPowerShellWrite("Get-ChildItem *> openspec/changes/test/eval.json"), true);
  });

  it("[System.IO.File]::WriteAllText 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite(
        '[System.IO.File]::WriteAllText("openspec/changes/test/eval.json", "[]")',
      ),
      true,
    );
  });

  it("[System.IO.File]::AppendAllText 应返回 true", () => {
    assert.equal(
      detectPowerShellWrite(
        '[System.IO.File]::AppendAllText("openspec/changes/test/eval.json", "data")',
      ),
      true,
    );
  });
});

describe("detectPowerShellWrite — 豁免与只读", () => {
  it("python 命令应豁免并返回 false", () => {
    assert.equal(
      detectPowerShellWrite("python plugins/dev-team/utils/eval-check.py --change test"),
      false,
    );
  });

  it("node 命令应豁免并返回 false", () => {
    assert.equal(detectPowerShellWrite("node scripts/write-eval.mjs"), false);
  });

  it("只读 Get-Content eval.json 应返回 false", () => {
    assert.equal(detectPowerShellWrite("Get-Content openspec/changes/test/eval.json"), false);
  });

  it("无 eval.json 的命令 Get-ChildItem 应返回 false", () => {
    assert.equal(detectPowerShellWrite("Get-ChildItem -Path src/"), false);
  });

  it("空命令应返回 false", () => {
    assert.equal(detectPowerShellWrite(""), false);
  });

  it("echo eval.json 不应误报", () => {
    assert.equal(detectPowerShellWrite('Write-Output "processing eval.json file"'), false);
  });
});

// ---------------------------------------------------------------------------
// extractChangeName / buildDenyReason (AC-3)
// ---------------------------------------------------------------------------

describe("extractChangeName", () => {
  it("应从 eval.json 路径提取变更名 my-feature", () => {
    assert.equal(extractChangeName("openspec/changes/my-feature/eval.json"), "my-feature");
  });

  it("无匹配路径应返回空字符串", () => {
    assert.equal(extractChangeName("src/utils/helper.ts"), "");
  });
});

describe("buildDenyReason", () => {
  it("拒绝原因应含 phase_log、eval.json 及变更名", () => {
    const reason = buildDenyReason("my-change", "Write");
    assert.match(reason, /phase_log/);
    assert.match(reason, /eval\.json/);
    assert.match(reason, /my-change/);
  });
});

// ---------------------------------------------------------------------------
// parseInput — fail-open (AC-9)
// ---------------------------------------------------------------------------

describe("parseInput — fail-open", () => {
  it("空 stdin 应触发 allow", () => {
    const result = parseInput("");
    assert.equal(result.decision, "allow");
  });

  it("无效 JSON 应触发 allow", () => {
    const result = parseInput("{not json");
    assert.equal(result.decision, "allow");
  });

  it("缺失 tool_name 应触发 allow", () => {
    const result = parseInput("{}");
    assert.equal(result.decision, "allow");
  });

  it("Write 缺失 tool_input.file_path 应触发 allow", () => {
    const result = parseInput(JSON.stringify({ tool_name: "Write", tool_input: {} }));
    assert.equal(result.decision, "allow");
  });

  it("未知工具 Read 应触发 allow", () => {
    const result = parseInput(
      JSON.stringify({ tool_name: "Read", tool_input: { file_path: "foo.txt" } }),
    );
    assert.equal(result.decision, "allow");
  });

  it("PowerShell Set-Content eval.json 应触发 deny", () => {
    const input = JSON.stringify({
      tool_name: "PowerShell",
      tool_input: { command: 'Set-Content -Path openspec/changes/test/eval.json -Value "[]"' },
    });
    const result = parseInput(input);
    assert.equal(result.decision, "deny");
  });

  it("PowerShell Get-Content eval.json 应触发 allow", () => {
    const input = JSON.stringify({
      tool_name: "PowerShell",
      tool_input: { command: "Get-Content openspec/changes/test/eval.json" },
    });
    const result = parseInput(input);
    assert.equal(result.decision, "allow");
  });

  it("PowerShell 缺失 command 应触发 allow", () => {
    const result = parseInput(JSON.stringify({ tool_name: "PowerShell", tool_input: {} }));
    assert.equal(result.decision, "allow");
  });

  it("PowerShell 不涉及 eval.json 应触发 allow", () => {
    const result = parseInput(
      JSON.stringify({
        tool_name: "PowerShell",
        tool_input: { command: "Get-ChildItem -Path src/" },
      }),
    );
    assert.equal(result.decision, "allow");
  });
});

// ---------------------------------------------------------------------------
// outputDeny — JSON 输出格式
// ---------------------------------------------------------------------------

describe("outputDeny — JSON 特殊字符", () => {
  it("reason 含换行、引号、反斜杠时 stdout 可 JSON.parse", () => {
    const reason = 'line1\nline2\t"quoted"\r\nbackslash\\test';
    const stdout = outputDeny(reason);
    assert.doesNotThrow(() => JSON.parse(stdout));
    const parsed = JSON.parse(stdout);
    assert.equal(parsed.hookSpecificOutput.permissionDecision, "deny");
    assert.ok(parsed.hookSpecificOutput.permissionDecisionReason.includes("line1"));
  });
});

// ---------------------------------------------------------------------------
// 黑盒: spawnSync 注入 stdin（Mock 策略 — 未导出函数时的 fail-open 回归）
// ---------------------------------------------------------------------------

describe("protect-eval.mjs — 黑盒 fail-open (AC-9)", () => {
  it("空 stdin 应输出 allow", () => {
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input: "",
      encoding: "utf-8",
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), "allow");
  });

  it("无效 JSON stdin 应输出 allow", () => {
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input: "{not json",
      encoding: "utf-8",
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), "allow");
  });

  it("未知工具 Read 应输出 allow", () => {
    const input = JSON.stringify({
      tool_name: "Read",
      tool_input: { file_path: "foo.txt" },
    });
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input,
      encoding: "utf-8",
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), "allow");
  });

  it("PowerShell Set-Content 写入 eval.json 应输出 deny", () => {
    const input = JSON.stringify({
      tool_name: "PowerShell",
      tool_input: {
        command: 'Set-Content -Path openspec/changes/test/eval.json -Value "[]"',
      },
    });
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input,
      encoding: "utf-8",
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), "deny");
  });

  it("PowerShell Get-Content 只读应输出 allow", () => {
    const input = JSON.stringify({
      tool_name: "PowerShell",
      tool_input: {
        command: "Get-Content openspec/changes/test/eval.json",
      },
    });
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input,
      encoding: "utf-8",
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), "allow");
  });
});
