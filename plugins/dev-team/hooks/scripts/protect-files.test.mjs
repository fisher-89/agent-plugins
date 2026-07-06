/**
 * 单元测试: protect-files.mjs — 配置驱动的写入保护 Hook
 *
 * @see openspec/changes/write-protection-config/test-design.md
 *
 * 用法: node --test protect-files.test.mjs
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

const mod = await import('./protect-files.mjs');

const {
  globToRegex,
  loadConfig,
  loadPatterns,
  isProtected,
  buildDenyReason,
  detectBashWrite,
  detectPowerShellWrite,
  extractChangeName,
  outputAllow,
  outputDeny,
  parseInput,
} = mod;

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

function createTempProject(configContent = null) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'protect-files-test-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  if (configContent !== null) {
    fs.writeFileSync(path.join(openspecDir, 'config.json'), configContent, 'utf-8');
  }
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

// ---------------------------------------------------------------------------
// globToRegex
// ---------------------------------------------------------------------------

describe('globToRegex — ** 通配符 (AC-4)', () => {
  it('openspec/**/eval.json 应匹配 openspec/changes/test/eval.json', () => {
    const re = globToRegex('openspec/**/eval.json');
    assert.ok(re.test('openspec/changes/test/eval.json'));
  });

  it('openspec/**/eval.json 应匹配 openspec/a/b/c/eval.json', () => {
    const re = globToRegex('openspec/**/eval.json');
    assert.ok(re.test('openspec/a/b/c/eval.json'));
  });

  it('openspec/**/eval.json 应匹配 openspec/eval.json (零段深度)', () => {
    const re = globToRegex('openspec/**/eval.json');
    assert.ok(re.test('openspec/eval.json'));
  });

  it('仅含 ** 应匹配任意路径', () => {
    const re = globToRegex('**');
    assert.ok(re.test('any/path/to/file.json'));
    assert.ok(re.test('file.json'));
  });
});

describe('globToRegex — * 通配符 (AC-4)', () => {
  it('openspec/data/*.json 应匹配 openspec/data/settings.json', () => {
    const re = globToRegex('openspec/data/*.json');
    assert.ok(re.test('openspec/data/settings.json'));
  });

  it('openspec/data/*.json 不应匹配 openspec/data/sub/config.json', () => {
    const re = globToRegex('openspec/data/*.json');
    assert.ok(!re.test('openspec/data/sub/config.json'));
  });

  it('* 不应跨越路径分隔符', () => {
    const re = globToRegex('*.json');
    assert.ok(!re.test('sub/file.json'));
  });
});

describe('globToRegex — ? 通配符 (AC-4)', () => {
  it('config?.json 应匹配 config1.json', () => {
    const re = globToRegex('config?.json');
    assert.ok(re.test('config1.json'));
  });

  it('config?.json 不应匹配 config10.json (? 只匹配单字符)', () => {
    const re = globToRegex('config?.json');
    assert.ok(!re.test('config10.json'));
  });
});

describe('globToRegex — 边界', () => {
  it('空 glob 字符串应返回空正则（匹配空字符串）', () => {
    const re = globToRegex('');
    assert.ok(re.test(''));
    assert.ok(!re.test('a'));
  });

  it('glob 含多个连续 * 时应正确处理', () => {
    const re = globToRegex('***');
    // 第一个 ** 消耗为跨段通配，后续 * 消耗为段内通配
    assert.ok(re.test('anything'));
  });

  it('glob 含特殊字符时应正确转义', () => {
    const re = globToRegex('file[name].txt');
    assert.ok(re.test('file[name].txt'));
    assert.ok(!re.test('filenametxt'));
  });
});

// ---------------------------------------------------------------------------
// loadConfig
// ---------------------------------------------------------------------------

describe('loadConfig — 读取 config.json (AC-2)', () => {
  it('有效的 config.json 含 write_protection 配置时应返回完整配置对象', () => {
    const tmp = createTempProject(
      JSON.stringify({
        write_protection: { files: [{ glob: 'openspec/data/*.json', reason: '受保护 %s' }] },
      }),
    );
    try {
      const result = loadConfig(tmp.root);
      assert.deepEqual(result, {
        files: [{ glob: 'openspec/data/*.json', reason: '受保护 %s' }],
      });
    } finally {
      tmp.cleanup();
    }
  });

  it('config.json 中无 write_protection 时返回空对象', () => {
    const tmp = createTempProject(JSON.stringify({ schema: 'spec-driven' }));
    try {
      const result = loadConfig(tmp.root);
      assert.deepEqual(result, {});
    } finally {
      tmp.cleanup();
    }
  });

  it('config.json 不存在时返回空对象', () => {
    const tmp = createTempProject(null);
    try {
      const result = loadConfig(tmp.root);
      assert.deepEqual(result, {});
    } finally {
      tmp.cleanup();
    }
  });

  it('config.json 包含无效 JSON 时返回空对象 (fail-open)', () => {
    const tmp = createTempProject('not valid json');
    try {
      const result = loadConfig(tmp.root);
      assert.deepEqual(result, {});
    } finally {
      tmp.cleanup();
    }
  });

  it('projectRoot 为空字符串时返回空对象', () => {
    const result = loadConfig('');
    assert.deepEqual(result, {});
  });
});

// ---------------------------------------------------------------------------
// loadPatterns
// ---------------------------------------------------------------------------

describe('loadPatterns — 合并模式列表 (AC-2, AC-3)', () => {
  it('未配置 write_protection 时应返回内置默认模式', () => {
    const tmp = createTempProject(JSON.stringify({ schema: 'spec-driven' }));
    try {
      const patterns = loadPatterns(tmp.root);
      assert.ok(Array.isArray(patterns));
      assert.equal(patterns.length, 2);
      assert.ok(patterns.some((p) => p.glob.includes('eval.json')));
      assert.ok(patterns.some((p) => p.glob.includes('config.json')));
      assert.ok(patterns.every((p) => typeof p.match.test === 'function'));
    } finally {
      tmp.cleanup();
    }
  });

  it('用户配置了 write_protection.files 时返回内置默认 + 用户模式合并列表', () => {
    const tmp = createTempProject(
      JSON.stringify({
        write_protection: { files: [{ glob: 'openspec/data/*.json', reason: '受保护 %s' }] },
      }),
    );
    try {
      const patterns = loadPatterns(tmp.root);
      assert.equal(patterns.length, 3); // 2 built-in + 1 user
      assert.ok(patterns.some((p) => p.glob === 'openspec/data/*.json'));
    } finally {
      tmp.cleanup();
    }
  });

  it('用户配置的 files 为空数组时仅返回内置默认模式', () => {
    const tmp = createTempProject(
      JSON.stringify({ write_protection: { files: [] } }),
    );
    try {
      const patterns = loadPatterns(tmp.root);
      assert.equal(patterns.length, 2);
    } finally {
      tmp.cleanup();
    }
  });

  it('用户配置的 glob 与内置默认重复时不应影响数量', () => {
    const tmp = createTempProject(
      JSON.stringify({
        write_protection: { files: [{ glob: 'openspec/changes/**/eval.json' }] },
      }),
    );
    try {
      const patterns = loadPatterns(tmp.root);
      assert.equal(patterns.length, 3); // 2 built-in + 1 duplicate user (no dedup)
    } finally {
      tmp.cleanup();
    }
  });

  it('configDir 不存在时返回仅内置默认模式 (fail-open)', () => {
    const patterns = loadPatterns('/nonexistent/path');
    assert.equal(patterns.length, 2);
  });
});

// ---------------------------------------------------------------------------
// isProtected
// ---------------------------------------------------------------------------

describe('isProtected — 内置默认模式匹配 (AC-3)', () => {
  const builtInPatterns = loadPatterns('/nonexistent'); // only built-in

  it('openspec/changes/test/eval.json 应匹配内置 eval.json 模式', () => {
    const result = isProtected('openspec/changes/test/eval.json', builtInPatterns);
    assert.equal(result.matched, true);
    assert.ok(result.matchedGlob.includes('eval.json'));
  });

  it('openspec/config.json 应匹配内置 config.json 模式', () => {
    const result = isProtected('openspec/config.json', builtInPatterns);
    assert.equal(result.matched, true);
    assert.ok(result.matchedGlob.includes('config.json'));
  });

  it('Windows 反斜杠路径归一化后应匹配', () => {
    const result = isProtected('openspec\\changes\\test\\eval.json', builtInPatterns);
    assert.equal(result.matched, true);
  });

  it('超长路径前缀 + eval.json 仍应匹配', () => {
    const longPath = 'openspec/changes/' + 'a/'.repeat(200) + 'eval.json';
    assert.ok(longPath.length > 400);
    const result = isProtected(longPath, builtInPatterns);
    assert.equal(result.matched, true);
  });
});

describe('isProtected — 自定义 glob 匹配 (AC-4)', () => {
  const customPatterns = [
    { match: globToRegex('openspec/data/*.json'), glob: 'openspec/data/*.json', reason: '受保护' },
  ];

  it('配置 glob openspec/data/*.json 后，openspec/data/settings.json 应匹配', () => {
    const result = isProtected('openspec/data/settings.json', customPatterns);
    assert.equal(result.matched, true);
    assert.equal(result.matchedGlob, 'openspec/data/*.json');
  });

  it('路径包含特殊字符（空格、括号）时 glob 匹配正确', () => {
    const pattern = [{ match: globToRegex('my project/**/*.json'), glob: 'my project/**/*.json' }];
    const result = isProtected('my project/data/特殊文件[name].json', pattern);
    assert.equal(result.matched, true);
  });
});

describe('isProtected — 不匹配路径 (AC-4)', () => {
  const patterns = loadPatterns('/nonexistent');

  it('src/utils/helper.ts 不应匹配任何模式', () => {
    const result = isProtected('src/utils/helper.ts', patterns);
    assert.equal(result.matched, false);
  });
});

describe('isProtected — 异常输入', () => {
  it('filePath 为空字符串时返回 {matched: false}', () => {
    const result = isProtected('', [{ match: globToRegex('*.json'), glob: '*.json' }]);
    assert.equal(result.matched, false);
  });

  it('patterns 为空数组时返回 {matched: false}', () => {
    const result = isProtected('test.json', []);
    assert.equal(result.matched, false);
  });

  it('filePath 为 undefined/null 时返回 {matched: false}', () => {
    assert.equal(isProtected(undefined, []).matched, false);
    assert.equal(isProtected(null, []).matched, false);
  });
});

// ---------------------------------------------------------------------------
// buildDenyReason
// ---------------------------------------------------------------------------

describe('buildDenyReason — 自定义原因占位符替换 (AC-5)', () => {
  it('reason 含 %s 被替换为文件路径', () => {
    const pattern = { reason: '文件 %s 受保护', match: globToRegex('*'), glob: '*' };
    const reason = buildDenyReason(pattern, 'test.json', 'Bash');
    assert.equal(reason, '文件 test.json 受保护');
  });

  it('reason 含 %t 被替换为工具名称', () => {
    const pattern = { reason: '通过 %t 检测', match: globToRegex('*'), glob: '*' };
    const reason = buildDenyReason(pattern, 'test.json', 'Bash');
    assert.equal(reason, '通过 Bash 检测');
  });

  it('reason 同时含 %s 和 %t 均被正确替换', () => {
    const pattern = { reason: '%s 受保护, via %t', match: globToRegex('*'), glob: '*' };
    const reason = buildDenyReason(pattern, 'test.json', 'PowerShell');
    assert.equal(reason, 'test.json 受保护, via PowerShell');
  });
});

describe('buildDenyReason — 默认原因回退 (AC-5)', () => {
  it('pattern 无自定义 reason 时使用内置通用拒绝文案', () => {
    const pattern = { match: globToRegex('*'), glob: '*' }; // no reason
    const reason = buildDenyReason(pattern, 'eval.json', 'Bash');
    assert.ok(reason.includes('eval.json'));
    assert.ok(reason.includes('Bash'));
    assert.ok(reason.includes('受写入保护'));
  });
});

describe('buildDenyReason — 边界', () => {
  it('filePath 为空字符串时占位符替换仍安全', () => {
    const pattern = { reason: '路径: %s', match: globToRegex('*'), glob: '*' };
    const reason = buildDenyReason(pattern, '', 'Bash');
    assert.equal(reason, '路径: ');
  });

  it('toolName 为空字符串时占位符替换仍安全', () => {
    const pattern = { reason: '工具: %t', match: globToRegex('*'), glob: '*' };
    const reason = buildDenyReason(pattern, 'file.json', '');
    assert.equal(reason, '工具: ');
  });

  it('reason 中含特殊字符（换行、引号、emoji）时输出仍为有效 JSON', () => {
    const pattern = { reason: '换行\n引号"反斜杠\\emoji\u{1F600}', match: globToRegex('*'), glob: '*' };
    const reason = buildDenyReason(pattern, 'f.json', 'Bash');
    const serialized = JSON.stringify({ reason });
    assert.doesNotThrow(() => JSON.parse(serialized));
    const reparsed = JSON.parse(serialized);
    assert.ok(reparsed.reason.includes('emoji'));
  });

  it('pattern 为 null 时使用默认回退', () => {
    const reason = buildDenyReason(null, 'file.json', 'Bash');
    assert.ok(reason.includes('file.json'));
    assert.ok(reason.includes('Bash'));
  });
});

// ---------------------------------------------------------------------------
// extractChangeName
// ---------------------------------------------------------------------------

describe('extractChangeName — 路径提取', () => {
  it('从 openspec/changes/my-feature/eval.json 提取 my-feature', () => {
    assert.equal(extractChangeName('openspec/changes/my-feature/eval.json'), 'my-feature');
  });

  it('从 Windows 路径提取 change name', () => {
    assert.equal(extractChangeName('openspec\\changes\\my-change\\eval.json'), 'my-change');
  });

  it('路径中不含 openspec/changes/ 时返回空字符串', () => {
    assert.equal(extractChangeName('src/file.json'), '');
  });

  it('路径含 extra/changes/ 等无关前缀不误匹配', () => {
    assert.equal(extractChangeName('extra/changes/file.json'), '');
  });

  it('空路径返回空字符串', () => {
    assert.equal(extractChangeName(''), '');
  });
});

// ---------------------------------------------------------------------------
// detectBashWrite
// ---------------------------------------------------------------------------

describe('detectBashWrite — eval.json 写入检测 (AC-10)', () => {
  const patterns = loadPatterns('/nonexistent'); // built-in only

  it('> 重定向到 eval.json 返回 deny', () => {
    const result = detectBashWrite('echo "[]" > openspec/changes/test/eval.json', patterns);
    assert.equal(result.decision, 'deny');
  });

  it('>> 追加到 eval.json 返回 deny', () => {
    const result = detectBashWrite('echo "[]" >> openspec/changes/test/eval.json', patterns);
    assert.equal(result.decision, 'deny');
  });

  it('tee 写入 eval.json 返回 deny', () => {
    const result = detectBashWrite("echo '[]' | tee openspec/changes/test/eval.json", patterns);
    assert.equal(result.decision, 'deny');
  });

  it('heredoc 写入 eval.json 返回 deny', () => {
    const result = detectBashWrite('cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF', patterns);
    assert.equal(result.decision, 'deny');
  });

  it('>& 重定向到 eval.json 返回 deny', () => {
    const result = detectBashWrite('somecmd >& openspec/changes/test/eval.json', patterns);
    assert.equal(result.decision, 'deny');
  });

  it('>| noclobber 重定向到 eval.json 返回 deny', () => {
    const result = detectBashWrite('echo "test" >| openspec/changes/test/eval.json', patterns);
    assert.equal(result.decision, 'deny');
  });
});

describe('detectBashWrite — 豁免与只读 (AC-4)', () => {
  const patterns = loadPatterns('/nonexistent');

  it('python 命令豁免返回 allow', () => {
    const result = detectBashWrite('python scripts/write-eval.py', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('python3 命令豁免返回 allow', () => {
    const result = detectBashWrite('python3 scripts/tool.py', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('node 命令豁免返回 allow', () => {
    const result = detectBashWrite('node scripts/write.mjs', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('只读 cat eval.json 返回 allow', () => {
    const result = detectBashWrite('cat openspec/changes/test/eval.json', patterns);
    assert.equal(result.decision, 'allow');
  });
});

describe('detectBashWrite — 自定义模式匹配 (AC-4)', () => {
  const customPatterns = [
    { match: globToRegex('openspec/data/*.json'), glob: 'openspec/data/*.json', reason: '受保护 %s' },
  ];

  it('命令中含自定义保护文件路径时依据 patterns 拒绝', () => {
    const result = detectBashWrite('echo "x" > openspec/data/settings.json', customPatterns);
    assert.equal(result.decision, 'deny');
    assert.ok(result.reason.includes('openspec/data/settings.json'));
  });
});

describe('detectBashWrite — 边界', () => {
  const patterns = loadPatterns('/nonexistent');

  it('含 -> 但无写入操作时返回 allow', () => {
    const result = detectBashWrite('ls -la -> file.txt', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('空命令字符串返回 allow', () => {
    const result = detectBashWrite('', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('无写入操作符时返回 allow', () => {
    const result = detectBashWrite('ls -la', patterns);
    assert.equal(result.decision, 'allow');
  });
});

// ---------------------------------------------------------------------------
// detectPowerShellWrite
// ---------------------------------------------------------------------------

describe('detectPowerShellWrite — eval.json 写入检测 (AC-10)', () => {
  const patterns = loadPatterns('/nonexistent');

  it('Set-Content 写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      'Set-Content -Path openspec/changes/test/eval.json -Value "[]"',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('Set-Content 流水线写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '"[]" | Set-Content openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('Out-File 写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '"[]" | Out-File openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('Add-Content 追加 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '"x" | Add-Content openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('Export-Csv 写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '$data | Export-Csv openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('Export-CliXml 写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '$data | Export-CliXml openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('Tee-Object 写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '"x" | Tee-Object openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('> 重定向到 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '"[]" > openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('>> 追加到 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '"[]" >> openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('*> 合并流到 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      'somecmd *> openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('[System.IO.File]::WriteAllText 写入 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '[System.IO.File]::WriteAllText("openspec/changes/test/eval.json", "[]")',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });

  it('[System.IO.File]::AppendAllText 追加 eval.json 返回 deny', () => {
    const result = detectPowerShellWrite(
      '[System.IO.File]::AppendAllText("openspec/changes/test/eval.json", "more")',
      patterns,
    );
    assert.equal(result.decision, 'deny');
  });
});

describe('detectPowerShellWrite — 豁免与只读 (AC-4)', () => {
  const patterns = loadPatterns('/nonexistent');

  it('python 命令豁免返回 allow', () => {
    const result = detectPowerShellWrite('python scripts/write-eval.py', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('node 命令豁免返回 allow', () => {
    const result = detectPowerShellWrite('node scripts/write.mjs', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('只读 Get-Content eval.json 返回 allow', () => {
    const result = detectPowerShellWrite(
      'Get-Content openspec/changes/test/eval.json',
      patterns,
    );
    assert.equal(result.decision, 'allow');
  });

  it('不涉及受保护文件时返回 allow', () => {
    const result = detectPowerShellWrite(
      'Set-Content -Path src/test.js -Value "test"',
      patterns,
    );
    assert.equal(result.decision, 'allow');
  });

  it('命令字符串 eval.json 出现在非路径上下文中不应误报', () => {
    const result = detectPowerShellWrite(
      'Write-Host "eval.json is a filename"',
      patterns,
    );
    assert.equal(result.decision, 'allow');
  });
});

describe('detectPowerShellWrite — 自定义模式匹配 (AC-4)', () => {
  const customPatterns = [
    { match: globToRegex('openspec/data/*.json'), glob: 'openspec/data/*.json', reason: '受保护 %s' },
  ];

  it('命令中含自定义保护文件路径时依据 patterns 拒绝', () => {
    const result = detectPowerShellWrite(
      'Set-Content -Path openspec/data/settings.json -Value "x"',
      customPatterns,
    );
    assert.equal(result.decision, 'deny');
  });
});

describe('detectPowerShellWrite — 边界', () => {
  const patterns = loadPatterns('/nonexistent');
  it('空命令字符串返回 allow', () => {
    const result = detectPowerShellWrite('', patterns);
    assert.equal(result.decision, 'allow');
  });
});

// ---------------------------------------------------------------------------
// outputAllow / outputDeny
// ---------------------------------------------------------------------------

describe('outputAllow — 输出格式', () => {
  it('outputAllow 返回含 permissionDecision: "allow" 的有效 JSON', () => {
    const json = outputAllow();
    const parsed = JSON.parse(json);
    assert.equal(parsed.hookSpecificOutput.permissionDecision, 'allow');
    assert.equal(parsed.hookSpecificOutput.hookEventName, 'PreToolUse');
  });
});

describe('outputDeny — 输出格式', () => {
  it('outputDeny(reason) 返回含 permissionDecision: "deny" 和 reason 的有效 JSON', () => {
    const json = outputDeny('测试拒绝原因');
    const parsed = JSON.parse(json);
    assert.equal(parsed.hookSpecificOutput.permissionDecision, 'deny');
    assert.equal(parsed.hookSpecificOutput.permissionDecisionReason, '测试拒绝原因');
  });

  it('reason 含换行、引号、反斜杠时 stdout 可 JSON.parse', () => {
    const reason = '换行\n引号"反斜杠\\';
    const json = outputDeny(reason);
    const parsed = JSON.parse(json);
    assert.equal(parsed.hookSpecificOutput.permissionDecisionReason, reason);
  });
});

// ---------------------------------------------------------------------------
// parseInput — fail-open
// ---------------------------------------------------------------------------

describe('parseInput — fail-open (AC-8)', () => {
  const patterns = loadPatterns('/nonexistent');

  it('空字符串输入返回 {decision: "allow"}', () => {
    const result = parseInput('', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('仅空白输入返回 {decision: "allow"}', () => {
    const result = parseInput('   ', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('无效 JSON 输入返回 {decision: "allow"}', () => {
    const result = parseInput('{not json', patterns);
    assert.equal(result.decision, 'allow');
  });

  it('有效 JSON 缺失 tool_name 返回 {decision: "allow"}', () => {
    const result = parseInput(JSON.stringify({}), patterns);
    assert.equal(result.decision, 'allow');
  });

  it('Write 工具缺失 tool_input.file_path 返回 {decision: "allow"}', () => {
    const input = JSON.stringify({ tool_name: 'Write', tool_input: {} });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'allow');
  });

  it('未知工具名称返回 {decision: "allow"}', () => {
    const input = JSON.stringify({
      tool_name: 'Read',
      tool_input: { file_path: 'openspec/config.json' },
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'allow');
  });
});

// ---------------------------------------------------------------------------
// parseInput — 拒绝路径
// ---------------------------------------------------------------------------

describe('parseInput — 拒绝路径 (AC-2, AC-3)', () => {
  const patterns = loadPatterns('/nonexistent'); // built-in

  it('Write 受保护文件路径返回 {decision: "deny"}', () => {
    const input = JSON.stringify({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/config.json' },
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'deny');
  });

  it('Edit 受保护文件路径返回 {decision: "deny"}', () => {
    const input = JSON.stringify({
      tool_name: 'Edit',
      tool_input: { file_path: 'openspec/changes/test/eval.json' },
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'deny');
  });

  it('Bash 含受保护文件写入命令返回 {decision: "deny"}', () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: { command: 'echo "x" > openspec/config.json' },
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'deny');
  });

  it('PowerShell 含受保护文件写入命令返回 {decision: "deny"}', () => {
    const input = JSON.stringify({
      tool_name: 'PowerShell',
      tool_input: { command: 'Set-Content openspec/config.json -Value "x"' },
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'deny');
  });

  it('Bash 无命令时返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: {},
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'allow');
  });

  it('PowerShell 无命令时返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'PowerShell',
      tool_input: {},
    });
    const result = parseInput(input, patterns);
    assert.equal(result.decision, 'allow');
  });
});

describe('parseInput — 自定义模式 (AC-4)', () => {
  it('Bash 写入用户自定义 glob 模式文件返回 {decision: "deny"}', () => {
    const tmp = createTempProject(
      JSON.stringify({
        write_protection: { files: [{ glob: 'openspec/data/*.json', reason: '自定义保护 %s' }] },
      }),
    );
    try {
      const customPatterns = loadPatterns(tmp.root);
      const input = JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" > openspec/data/settings.json' },
      });
      const result = parseInput(input, customPatterns);
      assert.equal(result.decision, 'deny');
      assert.ok(result.reason.includes('自定义保护'));
    } finally {
      tmp.cleanup();
    }
  });

  it('PowerShell 写入用户自定义 glob 模式文件返回 {decision: "deny"}', () => {
    const tmp = createTempProject(
      JSON.stringify({
        write_protection: { files: [{ glob: 'openspec/data/*.json', reason: '自定义保护 %s' }] },
      }),
    );
    try {
      const customPatterns = loadPatterns(tmp.root);
      const input = JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Set-Content openspec/data/settings.json -Value "x"' },
      });
      const result = parseInput(input, customPatterns);
      assert.equal(result.decision, 'deny');
    } finally {
      tmp.cleanup();
    }
  });
});
