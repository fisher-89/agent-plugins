/**
 * 单元测试: hooks.ts — hooks 单入口（protect-files / static-check）
 *
 * 覆盖范围:
 * - AC-1: hooks 子命令调度
 * - AC-2: protect-files 功能等价迁移
 * - AC-3: protect-files 复用 picomatch
 * - AC-4: static-check 进程内调用
 * - AC-5: static-check 功能等价迁移
 */

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 可控制 mock
// ---------------------------------------------------------------------------

// vi.hoisted 确保 mock 函数在 vi.mock 工厂被 hoist 之前就已初始化，避免 TDZ 错误
const { mockReadFileSync, mockReadConfig, mockRunStaticAnalysis } = vi.hoisted(() => ({
  mockReadFileSync: vi.fn(),
  mockReadConfig: vi.fn(),
  mockRunStaticAnalysis: vi.fn(),
}));

// vi.mock 被提升到文件顶部，在静态 import 之前执行
vi.mock('node:fs', async (importOriginal) => {
  const actual: Record<string, unknown> = await importOriginal();
  return { ...actual, readFileSync: mockReadFileSync };
});

vi.mock('./lib/config', () => ({
  readConfig: mockReadConfig,
}));

vi.mock('./commands/run-static-analysis', () => ({
  runStaticAnalysis: mockRunStaticAnalysis,
}));

// 在 hooks 模块加载前设置 process 拦截，防止模块顶层 main() 自动执行导致进程退出
// eslint-disable-next-line typescript/no-unsafe-type-assertion
const exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
vi.spyOn(process.stderr, 'write').mockImplementation(() => true);

// 动态 import — vi.mock 已生效，process 拦截已就位
const { main, runProtectFiles, runStaticCheck, captureStderr } = await import('./hooks');

// 模块加载完成后设置 stdout spy（auto-execution 未写 stdout）
const stdoutWriteMock = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/** 重置所有 mock 并设置默认行为 */
function resetMocks(): void {
  mockReadFileSync.mockReset();
  mockReadConfig.mockReset().mockReturnValue({ schema: 'spec-driven' });
  mockRunStaticAnalysis.mockReset().mockReturnValue(0);
  exitMock.mockClear();
  stdoutWriteMock.mockClear();
}

/** 从 stdout spy calls 中提取最后写入的字符串 */
function getLastStdout(): string {
  return String(stdoutWriteMock.mock.lastCall?.[0] ?? '');
}

// ============================================================================
// hooks 子命令调度 — AC-1
// ============================================================================

describe('hooks 子命令调度 (AC-1)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'unprotected/file.txt' },
      }),
    );
  });

  it('process.argv[2] = "protect-files" 时 main 调用 runProtectFiles 并输出 allow JSON', () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'protect-files'];
    try {
      main();
      const output = getLastStdout();
      const parsed = JSON.parse(output);
      expect(parsed).toHaveProperty('hookSpecificOutput');
    } finally {
      process.argv = origArgv;
    }
  });

  it('process.argv[2] = "static-check" 时 main 调用 runStaticCheck 并输出 JSON', () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'static-check'];
    try {
      mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['/test'] }));
      main();
      const output = getLastStdout().trim();
      expect(output).toContain('{}');
    } finally {
      process.argv = origArgv;
    }
  });

  it('runProtectFiles 输出格式为 { hookSpecificOutput: { hookEventName, permissionDecision } }', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const output = getLastStdout();
    const parsed = JSON.parse(output);
    expect(parsed).toEqual({
      hookSpecificOutput: {
        hookEventName: 'PreToolUse',
        permissionDecision: 'deny',
        permissionDecisionReason: expect.any(String),
      },
    });
  });

  it('未知子命令 "unknown" 时 process.exit(1)，stderr 含子命令名', () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'unknown'];
    // 使用 dedicated spy 避免与 captureStderr 干扰
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    try {
      main();
      expect(exitMock).toHaveBeenCalledWith(1);
      expect(stderrSpy).toHaveBeenCalledWith(expect.stringContaining('Unknown subcommand'));
    } finally {
      stderrSpy.mockRestore();
      process.argv = origArgv;
    }
  });

  it('process.argv[2] 未定义时 process.exit(1)', () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts'];
    try {
      main();
      expect(exitMock).toHaveBeenCalledWith(1);
    } finally {
      process.argv = origArgv;
    }
  });

  it('process.argv[2]="" 空字符串时 process.exit(1) (边界)', () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', ''];
    try {
      main();
      expect(exitMock).toHaveBeenCalledWith(1);
    } finally {
      process.argv = origArgv;
    }
  });

  it('顶层 try-catch 包裹整个处理流程，任何未捕获异常输出 { decision: "block", reason } (边界)', async () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'protect-files'];

    // 让 mockReadFileSync 抛出异常，模拟未捕获异常传播到顶层 try-catch
    mockReadFileSync.mockImplementation(() => {
      throw new Error('模拟未捕获异常');
    });

    // 清除之前 stdout spy 的调用记录
    stdoutWriteMock.mockClear();

    // 重置模块缓存，强制 hooks.ts 重新执行模块级代码（即 try-catch 包裹的 main()）
    vi.resetModules();

    try {
      // 重新导入 — 模块级 main() 执行时 runProtectFiles 内部抛出异常，
      // 被 try-catch 捕获后输出 { decision: "block", reason }
      await import('./hooks');

      // 验证输出
      const output = String(stdoutWriteMock.mock.calls[0]?.[0] ?? '');
      const parsed = JSON.parse(output);
      expect(parsed.decision).toBe('block');
      expect(parsed.reason).toContain('模拟未捕获异常');
    } finally {
      process.argv = origArgv;
      mockReadFileSync.mockReset();
      stdoutWriteMock.mockClear();
    }
  });

  it('顶层 try-catch 捕获非 Error 对象（如字符串）时 String(error) 输出 { decision: "block", reason } (边界)', async () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'protect-files'];
    mockReadFileSync.mockImplementation(() => {
      throw '字符串异常';
    });
    stdoutWriteMock.mockClear();
    vi.resetModules();
    try {
      await import('./hooks');
      const output = String(stdoutWriteMock.mock.calls[0]?.[0] ?? '');
      const parsed = JSON.parse(output);
      expect(parsed.decision).toBe('block');
      expect(parsed.reason).toContain('字符串异常');
    } finally {
      process.argv = origArgv;
      mockReadFileSync.mockReset();
      stdoutWriteMock.mockClear();
    }
  });
});

// ============================================================================
// protect-files 功能等价迁移 — AC-2
// ============================================================================

describe('protect-files 功能等价迁移 (AC-2)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  // ---- Write / Edit 路径保护 ----

  it('parseInput 解析 Write 受保护文件路径返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('parseInput 解析 Edit 受保护文件路径返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Edit',
        tool_input: { file_path: 'openspec/changes/my-feature/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('parseInput 解析 Write 未受保护文件路径返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'src/helper.ts' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- Bash 写入检测 ----

  it('detectBashWrite — > 重定向到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "[]" > openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — >> 追加到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" >> openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — tee 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: "echo '[]' | tee openspec/changes/test/eval.json" },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — heredoc 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — >& 重定向到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" >& openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — >| noclobber 重定向到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" >| openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  // ---- PowerShell 写入检测 ----

  it('detectPowerShellWrite — Set-Content 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Set-Content -Path openspec/changes/test/eval.json -Value "[]"' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Out-File 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: 'Out-File -FilePath openspec/changes/test/eval.json -InputObject "[]"',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Add-Content 追加 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Add-Content -Path openspec/changes/test/eval.json -Value "x"' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Export-Csv 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Get-Process | Export-Csv openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Export-CliXml 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Get-Process | Export-CliXml openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Tee-Object 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: 'Get-Process | Tee-Object -FilePath openspec/changes/test/eval.json',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — > 重定向到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: '"[]" > openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — >> 追加到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: '"x" >> openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — *> 合并流到 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'cmd *> openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — [System.IO.File]::WriteAllText 写入 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: '[System.IO.File]::WriteAllText("openspec/changes/test/eval.json", "[]")',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — [System.IO.File]::AppendAllText 追加 eval.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: '[System.IO.File]::AppendAllText("openspec/changes/test/eval.json", "x")',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  // ---- 输出格式 ----

  it('outputAllow 返回含 permissionDecision: "allow" 的有效 JSON', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'safe/file.txt' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
    expect(parsed.hookSpecificOutput.hookEventName).toBe('PreToolUse');
  });

  it('outputDeny(reason) 返回含 permissionDecision: "deny" 和 reason 的有效 JSON', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toBeDefined();
    expect(typeof parsed.hookSpecificOutput.permissionDecisionReason).toBe('string');
  });

  it('buildDenyReason — 自定义 reason 含 %s 和 %t 占位符替换', () => {
    mockReadConfig.mockReturnValue({
      write_protection: {
        files: [{ glob: 'critical/*.json', reason: '禁止写入: %s, 工具: %t' }],
      },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'critical/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('critical/config.json');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('Write');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).not.toContain('%s');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).not.toContain('%t');
  });

  // ---- 异常: fail-open ----

  it('parseInput 空字符串输入返回 allow (fail-open)', () => {
    mockReadFileSync.mockReturnValue('');
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput 空白字符串输入返回 allow (fail-open)', () => {
    mockReadFileSync.mockReturnValue('   ');
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput 无效 JSON 输入返回 allow (fail-open)', () => {
    mockReadFileSync.mockReturnValue('{not json');
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput 有效 JSON 缺失 tool_name 返回 allow (fail-open)', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ tool_input: { file_path: 'test.txt' } }));
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput Write 工具缺失 tool_input.file_path 返回 allow (fail-open)', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ tool_name: 'Write', tool_input: {} }));
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput 未知工具名称 Read 返回 allow (fail-open)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Read',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectBashWrite 空命令字符串返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: '' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectPowerShellWrite 空命令字符串返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: '' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- 边界: isProtected ----

  it('isProtected filePath 为空字符串时 parseInput 返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: '' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('内置保护模式正确匹配受保护路径，reason 含 config_get MCP 工具提示', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('config_get');
  });

  it('内置保护模式匹配 eval.json 时返回 deny，reason 含 phase_log MCP 工具提示', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('phase_log');
  });

  // ---- 边界: buildDenyReason ----

  it('buildDenyReason pattern 无 reason 时使用默认回退文案', () => {
    mockReadConfig.mockReturnValue({
      write_protection: {
        files: [{ glob: 'custom/*.json' }], // 无 reason
      },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'custom/file.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('受写入保护');
  });

  it('buildDenyReason reason 含特殊字符（换行、引号、反斜杠、emoji）时序列化可 JSON.parse', () => {
    const specialReason = '包含换行\n引号"反斜杠\\emoji';
    mockReadConfig.mockReturnValue({
      write_protection: {
        files: [{ glob: 'special/*.json', reason: specialReason }],
      },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'special/file.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('换行');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('引号');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('反斜杠');
  });

  // ---- 边界: detectBashWrite ----

  it('detectBashWrite 含 "->" 但无写入操作时返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'git log --oneline -> openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectBashWrite node 命令豁免返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'node scripts/write-eval.mjs' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectBashWrite cat 只读 eval.json 返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'cat openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- 边界: detectPowerShellWrite ----

  it('detectPowerShellWrite python 命令豁免返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'python plugins/dev-team/utils/archi-decide.py list' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectPowerShellWrite node 命令豁免返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'node scripts/write-eval.mjs' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectPowerShellWrite eval.json 出现在非路径上下文中不应误报', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Write-Host "The file is called eval.json"' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('detectPowerShellWrite Get-Content 只读返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Get-Content openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- dotnet 方法扩展覆盖 ----

  it('detectPowerShellWrite [System.IO.File]::WriteAllLines 写入 eval.json 返回 deny (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: '[System.IO.File]::WriteAllLines("openspec/changes/test/eval.json", @("[]"))',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite [System.IO.File]::WriteAllBytes 写入 eval.json 返回 deny (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command:
            '[System.IO.File]::WriteAllBytes("openspec/changes/test/eval.json", [byte[]]@())',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  // ---- 命令豁免 ----

  it('detectBashWrite python3 命令豁免返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'python3 scripts/process.py' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- 只读 cmdlet 不误报 ----

  it('Select-Object 读操作不产生 deny（仅写 cmdlet 生效）', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Get-Process | Select-Object Name, CPU' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- parseInput 参数类型边界 ----

  it('parseInput Write 工具 tool_input 为数字时返回 allow (边界)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: 123,
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput Write 工具 tool_input 为 null 时返回 allow (边界)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: null,
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput 中 tool_input 为数组时 isRecord 返回 false (边界)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: ['not', 'a', 'record'],
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('parseInput 中 tool_input 为字符串时 isRecord 返回 false (边界)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: 'just a string',
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- loadPatterns 边界 ----

  it('loadPatterns 用户配置 files 数组为空时仅使用内置保护模式 (边界)', () => {
    mockReadConfig.mockReturnValue({
      schema: 'spec-driven',
      write_protection: { files: [] },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'src/random.txt' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('loadPatterns 用户配置 files 为空时内置模式仍可匹配 eval.json (边界)', () => {
    mockReadConfig.mockReturnValue({
      schema: 'spec-driven',
      write_protection: { files: [] },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/my-feature/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('loadPatterns 用户配置 file 条目缺失 glob 时跳过该条目 (边界)', () => {
    mockReadConfig.mockReturnValue({
      write_protection: {
        files: [{ reason: '无 glob 字段' }],
      },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    // 内置模式仍应匹配 protect-files 使用的 loadPatterns 中 glob 为空时跳过逻辑
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('loadPatterns 用户配置 file 条目 glob 为空字符串时跳过该条目 (边界)', () => {
    mockReadConfig.mockReturnValue({
      write_protection: {
        files: [{ glob: '' }],
      },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  // ---- 写入非受保护文件 ----

  it('detectPowerShellWrite 写入非受保护文件时返回 allow (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Set-Content -Path src/helper.ts -Value "code"' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });
});

// ============================================================================
// protect-files 复用 picomatch — AC-3
// ============================================================================

describe('protect-files 复用 picomatch (AC-3)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({
      write_protection: {
        files: [{ glob: 'secrets/**/*.env', reason: '机密文件 %s' }],
      },
    });
  });

  it('isProtected 使用 matchGlob 进行匹配，对 eval.json 模式返回 denied', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/test/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('用户配置自定义 glob 模式后 isProtected 正确匹配对应路径', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" > secrets/prod.env' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('用户自定义 glob 模式不匹配时允许写入', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" > src/helper.ts' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('超长路径前缀 + eval.json 仍应匹配', () => {
    const longPrefix = 'a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z';
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: `openspec/changes/test/${longPrefix}/eval.json` },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('Windows 反斜杠路径归一化后匹配', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec\\changes\\test\\eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('路径含特殊字符（空格、括号、Unicode）时 glob 匹配正确', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/测试 项目 (1)/eval.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });
});

// ============================================================================
// static-check 进程内调用 — AC-4
// ============================================================================

describe('static-check 进程内调用 (AC-4)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['/test/root'] }));
  });

  it('runStaticCheck 直接调用 runStaticAnalysis()', () => {
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('runStaticCheck 调用 runStaticAnalysis 时传递从 stdin 解析的 workspaceRoot', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['/custom/path'] }));
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
      expect.objectContaining({ projectRoot: expect.stringContaining('/custom/path') }),
    );
  });

  it('runStaticAnalysis 返回非零但无 stderr 时输出 { decision: "block" } 和默认 reason', () => {
    mockRunStaticAnalysis.mockReturnValue(1);
    runStaticCheck();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.decision).toBe('block');
    expect(parsed.reason).toBe('静态检查未通过，请修复以下错误后重新提交：\n\n');
  });

  it('captureStderr 处理 Uint8Array/Buffer 输入时正确转换为字符串 (覆盖扩展)', () => {
    const [getCaptured, restore] = captureStderr();
    process.stderr.write(Buffer.from('buffer content'));
    expect(getCaptured()).toBe('buffer content');
    restore();
  });

  it('runStaticAnalysis 抛异常时 runStaticCheck 输出 { decision: "block", reason } (覆盖异常路径)', async () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'static-check'];

    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['/test'] }));
    mockRunStaticAnalysis.mockImplementation(() => {
      throw new Error('模拟 static analysis 抛出异常');
    });

    stdoutWriteMock.mockClear();
    vi.resetModules();

    try {
      await import('./hooks');

      const output = String(stdoutWriteMock.mock.calls[0]?.[0] ?? '');
      const parsed = JSON.parse(output);
      expect(parsed.decision).toBe('block');
      expect(parsed.reason).toContain('模拟 static analysis 抛出异常');
    } finally {
      process.argv = origArgv;
      mockRunStaticAnalysis.mockReset().mockReturnValue(0);
      stdoutWriteMock.mockClear();
    }
  });
});

// ============================================================================
// static-check 功能等价迁移 — AC-5
// ============================================================================

describe('static-check 功能等价迁移 (AC-5)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['/test'] }));
  });

  it('formatOutput CLI exit 0 时应返回 {}', () => {
    mockRunStaticAnalysis.mockReturnValue(0);
    runStaticCheck();
    const output = getLastStdout().trim();
    expect(output).toBe('{}');
  });

  it('formatOutput CLI exit 非 0 时应返回 { decision: "block", reason }，reason 以 FOLLOWUP_PREFIX 开头', () => {
    mockRunStaticAnalysis.mockImplementation(() => {
      process.stderr.write('lint error found');
      return 1;
    });
    runStaticCheck();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.decision).toBe('block');
    expect(parsed.reason).toContain('lint error found');
    expect(parsed.reason).toContain('静态检查未通过');
  });

  it('formatOutput 应将 stderr 合并进 reason', () => {
    mockRunStaticAnalysis.mockImplementation(() => {
      process.stderr.write('stderr line 1\nstderr line 2');
      return 1;
    });
    runStaticCheck();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.decision).toBe('block');
    expect(parsed.reason).toContain('stderr line 1');
    expect(parsed.reason).toContain('stderr line 2');
  });

  it('parseWorkspaceRoot 从合法 event JSON 提取 workspace_roots[0]', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['/project/path'] }));
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
      expect.objectContaining({ projectRoot: expect.stringContaining('/project/path') }),
    );
  });

  it('parseWorkspaceRoot workspace_roots 为空数组时使用 fallback', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [] }));
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('parseWorkspaceRoot 无 workspace_roots 字段时使用 fallback', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ some_field: 'value' }));
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('parseWorkspaceRoot 非法 JSON 时返回 null 且不抛异常', () => {
    mockReadFileSync.mockReturnValue('not valid json');
    expect(() => runStaticCheck()).not.toThrow();
  });

  it('formatOutput CLI 输出含特殊字符时 JSON.stringify 可解析', () => {
    mockRunStaticAnalysis.mockImplementation(() => {
      process.stderr.write('error: "quote" and line\nbreak');
      return 1;
    });
    runStaticCheck();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.decision).toBe('block');
    expect(parsed.reason).toContain('quote');
    expect(parsed.reason).toContain('break');
  });

  it('CLI 文件缺失时 ENOENT 错误应正确传递', () => {
    mockRunStaticAnalysis.mockImplementation(() => {
      process.stderr.write('ENOENT: no such file or directory');
      return 1;
    });
    runStaticCheck();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.decision).toBe('block');
    expect(parsed.reason).toContain('ENOENT');
  });

  it.each([2, 127])(
    'runStaticAnalysis 返回 exit %i 时输出 { decision: "block" } (边界)',
    (exitCode) => {
      mockRunStaticAnalysis.mockReturnValue(exitCode);
      runStaticCheck();
      const parsed = JSON.parse(getLastStdout());
      expect(parsed.decision).toBe('block');
    },
  );

  it('parseWorkspaceRoot JSON 解析为数组（非 Record）时返回 null 且不抛异常 (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue('["item1", "item2"]');
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('parseWorkspaceRoot JSON 解析为字符串（非 Record）时返回 null 且不抛异常 (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue('"just a string"');
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });
});

// ============================================================================
// captureStderr 辅助函数
// ============================================================================

describe('captureStderr 辅助函数', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('captureStderr 可捕获 stderr 输出并在 restore 后恢复', () => {
    const [getCaptured, restore] = captureStderr();
    process.stderr.write('captured content');
    expect(getCaptured()).toBe('captured content');
    restore();
  });

  it('captureStderr restore 后写入不被收集', () => {
    const [getCaptured, restore] = captureStderr();
    process.stderr.write('before');
    restore();
    process.stderr.write('after');
    const captured = getCaptured();
    expect(captured).toBe('before');
    expect(captured).not.toContain('after');
  });

  it('多次 captureStderr 嵌套时正确分层', () => {
    const [get1, restore1] = captureStderr();
    process.stderr.write('layer1');
    const [get2, restore2] = captureStderr();
    process.stderr.write('layer2');
    expect(get2()).toBe('layer2');
    restore2();
    expect(get1()).toBe('layer1');
    restore1();
  });
});

// ============================================================================
// 补充集成测试 — parseWorkspaceRoot 边界，杀死 L447 逻辑/等式突变体
// ============================================================================

describe('parseWorkspaceRoot 集成测试 (通过 runStaticCheck)', () => {
  beforeEach(() => {
    resetMocks();
    mockRunStaticAnalysis.mockReturnValue(0);
  });

  it('workspace_roots[0] 为数字时回退到 fallback 不抛异常', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [123] }));
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('workspace_roots[0] 为 null 时回退到 fallback', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [null] }));
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('workspace_roots[0] 为布尔值时回退到 fallback', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [true] }));
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('workspace_roots 为非数组时回退到 fallback', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: 'not-an-array' }));
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('workspace_roots 仅含空字符串时仍进入解析', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [''] }));
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('workspace_roots 元素为对象时回退到 fallback', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [{}] }));
    expect(() => runStaticCheck()).not.toThrow();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });
});

// ============================================================================
// 补充集成测试 — parseInput 边界，通过 runProtectFiles 杀死边界突变体
// ============================================================================

describe('parseInput 边界集成测试 (通过 runProtectFiles)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('tool_input 为数字 123 时 isRecord 返回 false → 允许', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: 123,
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_name 为数字 42 (非字符串类型) 时返回允许 (fail-open)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 42,
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_name 为布尔值 true 时返回允许 (fail-open)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: true,
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_name 为 null 时返回允许 (fail-open)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: null,
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('Bash tool_input 缺失 command 字段时返回允许', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { not_command: 'echo x' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('Bash tool_input.command 为数字时返回允许 (非字符串)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 123 },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('Bash tool_input 为 null 时返回允许', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: null,
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('PowerShell tool_input 缺失 command 字段时返回允许', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { not_command: 'echo x' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('PowerShell tool_input.command 为数字时返回允许 (非字符串)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 123 },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('PowerShell tool_input 为 null 时返回允许', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: null,
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });
});
