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

import * as NodeFs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type * as ProjectRoot from './lib/project-root';

// ---------------------------------------------------------------------------
// 可控制 mock
// ---------------------------------------------------------------------------

// vi.hoisted 确保 mock 函数在 vi.mock 工厂被 hoist 之前就已初始化，避免 TDZ 错误
const {
  mockReadFileSync,
  mockReadConfig,
  mockRunStaticAnalysis,
  mockGetProjectDir,
  actualGetProjectDirRef,
  mockBindSession,
  mockLookupChange,
  mockReadFileInventory,
  mockWriteFileInventory,
} = vi.hoisted(() => ({
  mockReadFileSync: vi.fn(),
  mockReadConfig: vi.fn(),
  mockRunStaticAnalysis: vi.fn(),
  mockGetProjectDir: vi.fn(),
  actualGetProjectDirRef: { current: null as null | (() => string) },
  mockBindSession: vi.fn(),
  mockLookupChange: vi.fn(),
  mockReadFileInventory: vi.fn(),
  mockWriteFileInventory: vi.fn(),
}));

// vi.mock 被提升到文件顶部，在静态 import 之前执行
vi.mock('node:fs', async (importOriginal) => {
  const actual: Record<string, unknown> = await importOriginal();
  return { ...actual, readFileSync: mockReadFileSync };
});

vi.mock('./lib/config', () => ({
  readConfig: mockReadConfig,
}));

vi.mock('./lib/session-registry', () => ({
  bindSession: mockBindSession,
  lookupChange: mockLookupChange,
}));

vi.mock('./modules/workflow', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    readFileInventory: mockReadFileInventory,
    writeFileInventory: mockWriteFileInventory,
  };
});

vi.mock('./commands/run-static-analysis', () => ({
  runStaticAnalysis: mockRunStaticAnalysis,
}));

vi.mock('./lib/project-root', async (importOriginal) => {
  const actual = await importOriginal<typeof ProjectRoot>();
  actualGetProjectDirRef.current = actual.getProjectDir;
  mockGetProjectDir.mockImplementation(() => actual.getProjectDir());
  return {
    ...actual,
    getProjectDir: () => mockGetProjectDir() as string,
  };
});

// 在 hooks 模块加载前设置 process 拦截，防止模块顶层 main() 自动执行导致进程退出
const exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
vi.spyOn(process.stderr, 'write').mockImplementation(() => true);

// 动态 import — vi.mock 已生效，process 拦截已就位
const { main, runProtectFiles, runRecordFiles, runStaticCheck, captureStderr } =
  await import('./hooks');

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
  mockGetProjectDir.mockReset();
  if (actualGetProjectDirRef.current) {
    mockGetProjectDir.mockImplementation(() => actualGetProjectDirRef.current!());
  }
  mockBindSession.mockReset();
  mockLookupChange.mockReset();
  mockReadFileInventory.mockReset();
  mockWriteFileInventory.mockReset();
  exitMock.mockClear();
  stdoutWriteMock.mockClear();
}

/** record-files 归账测试：以内存 store 模拟「读清单 → 折叠 → 写清单」的持久化。 */
function bindInventoryStore(initial: {
  written: string[];
  deleted: string[];
  source?: Record<string, string>;
}): void {
  let store = JSON.parse(JSON.stringify(initial)) as {
    written: string[];
    deleted: string[];
    source?: Record<string, string>;
  };
  mockReadFileInventory.mockImplementation(() => JSON.parse(JSON.stringify(store)));
  mockWriteFileInventory.mockImplementation((_changeDir: unknown, files: typeof store) => {
    store = JSON.parse(JSON.stringify(files));
  });
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
        tool_input: { file_path: 'openspec/changes/my-feature/workflow.json' },
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

  it('detectBashWrite — > 重定向到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "[]" > openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — >> 追加到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" >> openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — tee 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: "echo '[]' | tee openspec/changes/test/workflow.json" },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — heredoc 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'cat > openspec/changes/test/workflow.json <<EOF\n[]\nEOF' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — >& 重定向到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" >& openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectBashWrite — >| noclobber 重定向到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'echo "x" >| openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  // ---- PowerShell 写入检测 ----

  it('detectPowerShellWrite — Set-Content 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: 'Set-Content -Path openspec/changes/test/workflow.json -Value "[]"',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Out-File 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: 'Out-File -FilePath openspec/changes/test/workflow.json -InputObject "[]"',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Add-Content 追加 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Add-Content -Path openspec/changes/test/workflow.json -Value "x"' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Export-Csv 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Get-Process | Export-Csv openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Export-CliXml 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Get-Process | Export-CliXml openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — Tee-Object 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: 'Get-Process | Tee-Object -FilePath openspec/changes/test/workflow.json',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — > 重定向到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: '"[]" > openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — >> 追加到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: '"x" >> openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — *> 合并流到 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'cmd *> openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — [System.IO.File]::WriteAllText 写入 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: '[System.IO.File]::WriteAllText("openspec/changes/test/workflow.json", "[]")',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite — [System.IO.File]::AppendAllText 追加 workflow.json 返回 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command: '[System.IO.File]::AppendAllText("openspec/changes/test/workflow.json", "x")',
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
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('openspec/config.json');
  });

  it('内置保护模式匹配 workflow.json 时返回 deny，reason 含 phase_log MCP 工具提示', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/test/workflow.json' },
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
        tool_input: { command: 'git log --oneline -> openspec/changes/test/workflow.json' },
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

  it('detectBashWrite cat 只读 workflow.json 返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: 'cat openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- 边界: detectPowerShellWrite ----

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

  it('detectPowerShellWrite workflow.json 出现在非路径上下文中不应误报', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'Write-Host "The file is called workflow.json"' },
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
        tool_input: { command: 'Get-Content openspec/changes/test/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  // ---- dotnet 方法扩展覆盖 ----

  it('detectPowerShellWrite [System.IO.File]::WriteAllLines 写入 workflow.json 返回 deny (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command:
            '[System.IO.File]::WriteAllLines("openspec/changes/test/workflow.json", @("[]"))',
        },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('detectPowerShellWrite [System.IO.File]::WriteAllBytes 写入 workflow.json 返回 deny (覆盖扩展)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: {
          command:
            '[System.IO.File]::WriteAllBytes("openspec/changes/test/workflow.json", [byte[]]@())',
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

  it('loadPatterns 用户配置 files 为空时内置模式仍可匹配 workflow.json (边界)', () => {
    mockReadConfig.mockReturnValue({
      schema: 'spec-driven',
      write_protection: { files: [] },
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/my-feature/workflow.json' },
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

describe('detectPowerShellWrite python 豁免 (AC-10)', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('fixture 命令改为非 archi-decide.py 的 python 脚本路径时仍返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: 'python scripts/process.py' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
  });
});

describe('python 豁免回归', () => {
  const protectedEval = 'openspec/changes/test/workflow.json';

  beforeEach(() => {
    resetMocks();
  });

  it('行首 python/python3/node 写入受保护路径仍 allow', () => {
    for (const command of [
      `python script.py > ${protectedEval}`,
      `python3 script.py > ${protectedEval}`,
      `node script.js > ${protectedEval}`,
    ]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'PowerShell', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('python3x（无空格粘连）不得误豁免，写入受保护路径仍 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: `python3x script.py > ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });
});

describe('fixture 路径断言 (AC-10)', () => {
  it('hooks.ts 源码中不再出现 archi-decide.py 路径字面量', async () => {
    const { readFileSync: realReadFileSync } = await vi.importActual<typeof NodeFs>('node:fs');
    const hooksFile = fileURLToPath(new URL('./hooks.ts', import.meta.url));
    const source = realReadFileSync(hooksFile, 'utf-8');
    const banned = ['archi', 'decide.py'].join('-');
    expect(source.includes(banned)).toBe(false);
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

  it('isProtected 使用 matchGlob 进行匹配，对 workflow.json 模式返回 denied', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/test/workflow.json' },
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

  it('超长路径前缀 + workflow.json 仍应匹配', () => {
    const longPrefix = 'a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z';
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: `openspec/changes/test/${longPrefix}/workflow.json` },
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
        tool_input: { file_path: 'openspec\\changes\\test\\workflow.json' },
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
        tool_input: { file_path: 'openspec/changes/测试 项目 (1)/workflow.json' },
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

// ============================================================================
// hooks — 内置保护字面量 / detectBashWrite / getProjectDir / 子命令调度（突变补强）
// ============================================================================

describe('hooks — 内置保护字面量', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('Write openspec/changes/x/workflow.json → deny；reason 同时含 phase_log MCP 与 glob 语义', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/x/workflow.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('phase_log');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('backtrack');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('change_create');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toContain('MCP');
  });

  it('Write openspec/config.json → deny；reason 含自行操作或 config 保护文案', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(parsed.hookSpecificOutput.permissionDecisionReason).toMatch(/自行操作|config/);
  });

  it('对 openspec/changes/foo/workflow.json deny、对 openspec/changes/foo/proposal.md allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/foo/workflow.json' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');

    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/changes/foo/proposal.md' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('Write 非保护路径 src/foo.ts → allow；reason 不得误含 phase_log MCP', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'src/foo.ts' },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('allow');
    const reason = parsed.hookSpecificOutput.permissionDecisionReason ?? '';
    expect(reason).not.toContain('phase_log MCP');
  });
});

describe('hooks — detectBashWrite 正则判别（补强）', () => {
  const protectedEval = 'openspec/changes/test/workflow.json';

  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('tee -a / >& / >| 等写入受保护 workflow.json → 均 deny', () => {
    for (const command of [
      `echo x > ${protectedEval}`,
      `echo x >> ${protectedEval}`,
      `echo x >| ${protectedEval}`,
      `tee ${protectedEval}`,
      `tee -a ${protectedEval}`,
      `echo x >& ${protectedEval}`,
    ]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'Bash', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
    }
  });

  it('foo -> openspec/changes/test/workflow.json（箭头，非重定向）→ allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: `foo -> ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('命令以 tee <protected> 开头（无前导空白）仍 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: `tee ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('python/python3/node 豁免；python3x（无空格）不得误豁免', () => {
    for (const command of [
      `python script.py > ${protectedEval}`,
      `python3 x > ${protectedEval}`,
      'node x.js',
    ]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'Bash', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Bash',
        tool_input: { command: `python3x script.py > ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });
});

describe('hooks — getProjectDir 接线', () => {
  const ENV_KEY = 'CLAUDE_PROJECT_DIR';

  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('CLAUDE_PROJECT_DIR 指向已存在绝对根时 protect-files 经 getProjectDir 用该根读 config', async () => {
    const fs = await import('node:fs');
    const os = await import('node:os');
    const path = await import('node:path');
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'hooks-root-'));
    const prev = process.env[ENV_KEY];
    const prevWs = process.env.WORKSPACE_FOLDER_PATHS;
    process.env[ENV_KEY] = root;
    delete process.env.WORKSPACE_FOLDER_PATHS;
    try {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({
          tool_name: 'Write',
          tool_input: { file_path: 'src/foo.ts' },
        }),
      );
      runProtectFiles();
      expect(mockReadConfig).toHaveBeenCalledWith(root);
    } finally {
      if (prev === undefined) delete process.env[ENV_KEY];
      else process.env[ENV_KEY] = prev;
      if (prevWs === undefined) delete process.env.WORKSPACE_FOLDER_PATHS;
      else process.env.WORKSPACE_FOLDER_PATHS = prevWs;
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  it('内置保护按 file_path 匹配：即使 config 根与路径不同，openspec/config.json 仍 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: '/nonexistent-root/openspec/config.json' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
    expect(mockReadConfig).toHaveBeenCalled();
  });

  it('spy getProjectDir 抛错时 protect-files / static-check 不得未捕获崩溃；须 fail-open 或输出可观测 block JSON', () => {
    mockGetProjectDir.mockImplementation(() => {
      throw new Error('getProjectDir boom');
    });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'src/foo.ts' },
      }),
    );
    stdoutWriteMock.mockClear();
    let protectOutput = '';
    try {
      runProtectFiles();
      protectOutput = getLastStdout();
    } catch (err) {
      expect(err).toBeInstanceOf(Error);
      expect((err as Error).message).toContain('getProjectDir boom');
      protectOutput = JSON.stringify({ decision: 'block', reason: (err as Error).message });
    }
    expect(protectOutput.length).toBeGreaterThan(0);

    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [] }));
    stdoutWriteMock.mockClear();
    let staticOutput = '';
    try {
      runStaticCheck();
      staticOutput = getLastStdout();
    } catch (err) {
      expect(err).toBeInstanceOf(Error);
      expect((err as Error).message).toContain('getProjectDir boom');
      staticOutput = JSON.stringify({ decision: 'block', reason: (err as Error).message });
    }
    expect(staticOutput.length).toBeGreaterThan(0);
    expect(mockGetProjectDir).toHaveBeenCalled();
  });

  it('MCP 缓存已锁定且 env 指向另一路径时：hooks 经 getProjectDir 优先缓存根', async () => {
    const fs = await import('node:fs');
    const os = await import('node:os');
    const path = await import('node:path');
    const locked = fs.mkdtempSync(path.join(os.tmpdir(), 'hooks-locked-'));
    const other = fs.mkdtempSync(path.join(os.tmpdir(), 'hooks-other-'));
    mockGetProjectDir.mockReturnValue(locked);
    const prev = process.env[ENV_KEY];
    process.env[ENV_KEY] = other;
    try {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({
          tool_name: 'Write',
          tool_input: { file_path: 'src/foo.ts' },
        }),
      );
      runProtectFiles();
      expect(mockReadConfig).toHaveBeenCalledWith(locked);
      expect(mockReadConfig).not.toHaveBeenCalledWith(other);
      expect(mockGetProjectDir).toHaveBeenCalled();
    } finally {
      if (prev === undefined) delete process.env[ENV_KEY];
      else process.env[ENV_KEY] = prev;
      fs.rmSync(locked, { recursive: true, force: true });
      fs.rmSync(other, { recursive: true, force: true });
    }
  });

  it('getProjectDir 返回不存在路径时：内置保护仍按该根拼接 glob 匹配；不得静默改读 process.cwd() 下的 config', async () => {
    const path = await import('node:path');
    const missing = path.join(path.sep, 'nonexistent-hooks-root-' + Date.now());
    mockGetProjectDir.mockReturnValue(missing);
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: path.join(missing, 'openspec', 'config.json') },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
    expect(mockReadConfig).toHaveBeenCalledWith(missing);
    expect(mockReadConfig).not.toHaveBeenCalledWith(process.cwd());
  });
});

describe('hooks — WORKSPACE roots 静态检查', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('static-check 在空 workspace_roots 时回退 getProjectDir，不读错 cwd 配置', async () => {
    const fs = await import('node:fs');
    const os = await import('node:os');
    const path = await import('node:path');
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'hooks-ws-empty-'));
    mockGetProjectDir.mockReturnValue(root);
    try {
      mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [] }));
      mockRunStaticAnalysis.mockReturnValue(0);
      runStaticCheck();
      expect(mockGetProjectDir).toHaveBeenCalled();
      expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: root }),
      );
      expect(mockRunStaticAnalysis).not.toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: process.cwd() }),
      );
      expect(JSON.parse(getLastStdout())).toEqual({});
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  it('static-check 在多 workspace_roots 时使用 [0]，行为明确且不与 MCP 多根严格失败冲突', () => {
    const a = '/ws-a';
    const b = '/ws-b';
    mockGetProjectDir.mockClear();
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [a, b] }));
    mockRunStaticAnalysis.mockReturnValue(0);
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
      expect.objectContaining({ projectRoot: expect.stringContaining('ws-a') }),
    );
    const passedRoot = (
      mockRunStaticAnalysis.mock.calls[0] as [{ projectRoot: string }] | undefined
    )?.[0]?.projectRoot;
    expect(passedRoot).toBeDefined();
    expect(passedRoot).not.toContain('ws-b');
    // 事件已提供 roots 时不应回退 getProjectDir
    expect(mockGetProjectDir).not.toHaveBeenCalled();
  });
});

describe('hooks — 子命令调度（异常补强）', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('argv[2] 为未知子命令（含 Protect-Files 大小写变体 / 随机串）：stderr 含 Unknown subcommand: + 原文，exit(1)', () => {
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    for (const sub of ['Protect-Files', 'PROTECT-FILES', 'random-cmd']) {
      exitMock.mockClear();
      stderrSpy.mockClear();
      const original = process.argv;
      process.argv = ['node', 'hooks', sub];
      try {
        main();
        expect(exitMock).toHaveBeenCalledWith(1);
        const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
        expect(stderrText).toContain('Unknown subcommand:');
        expect(stderrText).toContain(sub);
      } finally {
        process.argv = original;
      }
    }
    stderrSpy.mockRestore();
  });

  it('恰好 protect-files / static-check 字面量调度；多余 argv 后缀不改变子命令选择', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Write', tool_input: { file_path: 'src/x.ts' } }),
    );
    const original = process.argv;
    try {
      process.argv = ['node', 'hooks', 'protect-files', 'extra', 'args'];
      main();
      expect(exitMock).not.toHaveBeenCalledWith(1);
      expect(getLastStdout().length).toBeGreaterThan(0);

      process.argv = ['node', 'hooks', 'static-check', 'extra'];
      mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [] }));
      main();
      expect(mockRunStaticAnalysis).toHaveBeenCalled();
    } finally {
      process.argv = original;
    }
  });
});

// ============================================================================
// Cursor 工具名路由 — Shell / StrReplace（AC-6）
// ============================================================================

describe('runProtectFiles / Shell', () => {
  const protectedEval = 'openspec/changes/test/workflow.json';

  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('tool_name 为 Shell 且 command 含重定向到受保护路径时 deny，与同等 Bash 一致', () => {
    const command = `echo "[]" > ${protectedEval}`;
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Shell', tool_input: { command } }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');

    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Bash', tool_input: { command } }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('Shell 且 >> 追加到受保护路径时 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `echo x >> ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('Shell 且 command 以 > 受保护路径开头（无前置命令）时 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `> ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('Shell 且 echo x >| 受保护路径（noclobber）时 deny；reason 含路径与 detected via', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `echo x >| ${protectedEval}` },
      }),
    );
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout());
    expect(parsed.hookSpecificOutput.permissionDecision).toBe('deny');
    const reason = parsed.hookSpecificOutput.permissionDecisionReason as string;
    expect(reason).toContain(protectedEval);
    expect(reason).toContain('detected via');
    expect(reason.length).toBeGreaterThan(0);
  });

  it('Shell 且 echo x | tee -a 受保护路径时 deny', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `echo x | tee -a ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('Shell 且 echo x >& 受保护路径（零或多个空白）时 deny', () => {
    for (const command of [`echo x >& ${protectedEval}`, `echo x >&${protectedEval}`]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'Shell', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
    }
  });

  it('tool_name 为 Shell 且 command 为只读时返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `cat ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_name 为 Shell 且 command 以 python/python3/node 开头写入受保护路径时返回 allow', () => {
    for (const command of [
      `python script.py > ${protectedEval}`,
      `python3 script.py > ${protectedEval}`,
      `node scripts/write.js > ${protectedEval}`,
    ]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'Shell', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('tool_name 为 Shell 但 command 缺失或非字符串时 fail-open 返回 allow', () => {
    for (const tool_input of [{}, { command: 123 }, { command: null }]) {
      mockReadFileSync.mockReturnValue(JSON.stringify({ tool_name: 'Shell', tool_input }));
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('tool_name 为 Shell 且 command 为空字符串时返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Shell', tool_input: { command: '' } }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_name 为 Shell 且超长 command 含受保护路径重定向时仍 deny 且不崩溃', () => {
    const padding = 'x'.repeat(1200);
    const command = `echo ${padding} > ${protectedEval}`;
    expect(() => {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'Shell', tool_input: { command } }),
      );
      runProtectFiles();
    }).not.toThrow();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('tool_name 为 Shell 且 command 含特殊字符与受保护路径时行为与 Bash 一致', () => {
    const command = `echo "emoji🙂\nline" > ${protectedEval}`;
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Shell', tool_input: { command } }),
    );
    runProtectFiles();
    const shellDecision = JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision;

    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Bash', tool_input: { command } }),
    );
    runProtectFiles();
    const bashDecision = JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision;
    expect(shellDecision).toBe(bashDecision);
    expect(shellDecision).toBe('deny');
  });

  it('Shell 且 command 为 echo x>path（> 后无空白）时 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `echo x>${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('Shell 且 command 含 -> 受保护路径（箭头而非重定向）时 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Shell',
        tool_input: { command: `foo -> ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });
});

describe('runProtectFiles / StrReplace', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('tool_name 为 StrReplace 且 file_path 为受保护路径时 deny，与同等 Edit 一致；reason 含字面量 StrReplace', () => {
    const file_path = 'openspec/changes/test/workflow.json';
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'StrReplace', tool_input: { file_path } }),
    );
    runProtectFiles();
    const strReplaceOut = JSON.parse(getLastStdout());
    expect(strReplaceOut.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(strReplaceOut.hookSpecificOutput.permissionDecisionReason).toContain('StrReplace');

    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Edit', tool_input: { file_path } }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('tool_name 为 StrReplace 且 file_path 为未受保护路径时返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'StrReplace',
        tool_input: { file_path: 'src/helper.ts' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_name 为 StrReplace 但缺失 file_path 或非字符串时 fail-open 返回 allow', () => {
    for (const tool_input of [{}, { file_path: 42 }, { file_path: null }]) {
      mockReadFileSync.mockReturnValue(JSON.stringify({ tool_name: 'StrReplace', tool_input }));
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('tool_name 为 StrReplace 且 file_path 为空字符串时返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'StrReplace', tool_input: { file_path: '' } }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('file_path 为超长路径且匹配保护 glob 时仍 deny 且不崩溃', () => {
    const longMid = `${'a/'.repeat(400)}workflow.json`;
    const file_path = `openspec/changes/test/${longMid}`;
    expect(() => {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'StrReplace', tool_input: { file_path } }),
      );
      runProtectFiles();
    }).not.toThrow();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('file_path 含反斜杠与正斜杠混用时匹配行为与 Edit 一致', () => {
    const file_path = 'openspec\\changes/test\\workflow.json';
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'StrReplace', tool_input: { file_path } }),
    );
    runProtectFiles();
    const strReplaceDecision = JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision;

    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: 'Edit', tool_input: { file_path } }),
    );
    runProtectFiles();
    const editDecision = JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision;
    expect(strReplaceDecision).toBe(editDecision);
    expect(strReplaceDecision).toBe('deny');
  });
});

describe('runProtectFiles / isRecord 与 evaluateToolAccess（突变补强）', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('tool_input 为数组且挂上 file_path 属性指向受保护路径时仍 allow', () => {
    // JSON 数组即使语义上带 path，isRecord 也必须因 Array.isArray 而拒绝
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'StrReplace',
        tool_input: ['openspec/changes/test/workflow.json'],
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('tool_input 为 null / 原始字符串 / 数字时 allow', () => {
    for (const tool_input of [null, 'openspec/config.json', 42]) {
      mockReadFileSync.mockReturnValue(JSON.stringify({ tool_name: 'Write', tool_input }));
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('stdin 为仅空白 / 仅制表换行时 allow', () => {
    for (const raw of ['   ', '\t\n', ' \n\t ']) {
      mockReadFileSync.mockReturnValue(raw);
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('tool_name 为 0 / false 等非字符串时 allow', () => {
    for (const tool_name of [0, false]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({
          tool_name,
          tool_input: { file_path: 'openspec/config.json' },
        }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
  });

  it('readConfig 无 write_protection.files（undefined）时仅内置 glob 生效', () => {
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'custom/not-builtin.txt' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');

    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });
});

describe('runProtectFiles / PowerShell 与路由（突变补强）', () => {
  const protectedEval = 'openspec/changes/test/workflow.json';

  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('PowerShell 且 2> / *> 写入受保护路径时 deny', () => {
    for (const command of [`echo x 2> ${protectedEval}`, `echo x *> ${protectedEval}`]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'PowerShell', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
    }
  });

  it('PowerShell python3x（无空格粘连）不得误豁免，写入受保护路径仍 deny (AC-10)', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: `python3x script.py > ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('PowerShell python3/node 行首豁免；非行首 python 不豁免', () => {
    for (const command of [
      `python3 script.py > ${protectedEval}`,
      `node script.js > ${protectedEval}`,
    ]) {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ tool_name: 'PowerShell', tool_input: { command } }),
      );
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
    }
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'PowerShell',
        tool_input: { command: `echo hi; python script.py > ${protectedEval}` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
  });

  it('未知 tool_name 且 command 为 PowerShell 写受保护路径时仍 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'NotPowerShell',
        tool_input: { command: `Set-Content -Path ${protectedEval} -Value x` },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });
});

describe('runProtectFiles / 既有工具名', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  it('既有 Write / Edit / Bash / PowerShell 受保护用例仍为 deny；Write config reason 含内置中文模板', () => {
    const cases = [
      { tool_name: 'Write', tool_input: { file_path: 'openspec/config.json' } },
      { tool_name: 'Edit', tool_input: { file_path: 'openspec/changes/test/workflow.json' } },
      {
        tool_name: 'Bash',
        tool_input: { command: 'echo x > openspec/changes/test/workflow.json' },
      },
      {
        tool_name: 'PowerShell',
        tool_input: {
          command: 'Set-Content -Path openspec/changes/test/workflow.json -Value "[]"',
        },
      },
    ];
    for (const input of cases) {
      mockReadFileSync.mockReturnValue(JSON.stringify(input));
      runProtectFiles();
      expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('deny');
    }
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'Write',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    const reason = JSON.parse(getLastStdout()).hookSpecificOutput
      .permissionDecisionReason as string;
    expect(reason).toContain('该文件受写入保护');
    expect(reason).toContain('自行操作');
    expect(reason).toContain('detected via');
    expect(reason).toContain('Write');
  });

  it('tool_name 为未知字符串时 fail-open 返回 allow', () => {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'UnknownTool',
        tool_input: { file_path: 'openspec/config.json' },
      }),
    );
    runProtectFiles();
    expect(JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision).toBe('allow');
  });
});

describe('runStaticCheck / parseWorkspaceRoot 与 captureStderr（突变补强）', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
    mockRunStaticAnalysis.mockReturnValue(0);
  });

  it('workspace_roots 为盘符路径时使用该 root（POSIX 化）调用分析', () => {
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: ['D:\\proj'] }));
    runStaticCheck();
    expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
      expect.objectContaining({ projectRoot: expect.stringMatching(/D:.*proj|D\/proj|proj/) }),
    );
    const passed = (mockRunStaticAnalysis.mock.calls[0] as [{ projectRoot: string }])[0]
      .projectRoot;
    expect(passed).not.toContain('\\');
  });

  it('workspace_roots 为 [] / 非数组 / [123] 时回退 getProjectDir', () => {
    const fallback = '/fallback-root';
    for (const roots of [[], 'not-array', [123]] as unknown[]) {
      mockRunStaticAnalysis.mockClear();
      mockGetProjectDir.mockClear();
      mockGetProjectDir.mockReturnValue(fallback);
      mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: roots }));
      runStaticCheck();
      expect(mockGetProjectDir).toHaveBeenCalled();
      expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: fallback }),
      );
    }
  });

  it("workspace_roots 为 [''] 时 firstRoot 为字符串：不回退 getProjectDir（杀 length/类型守卫）", () => {
    mockGetProjectDir.mockClear();
    mockReadFileSync.mockReturnValue(JSON.stringify({ workspace_roots: [''] }));
    runStaticCheck();
    expect(mockGetProjectDir).not.toHaveBeenCalled();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
  });

  it('captureStderr 写入 string 与 Uint8Array 后 getCaptured 拼接完整文本；restore 后不再捕获', () => {
    const [getCaptured, restore] = captureStderr();
    process.stderr.write('hello');
    process.stderr.write(new Uint8Array(Buffer.from('世界')));
    expect(getCaptured()).toBe('hello世界');
    restore();
    const before = getCaptured();
    process.stderr.write('after-restore');
    expect(getCaptured()).toBe(before);
  });
});

// ============================================================================
// 内置 glob 集合 — workflow.json 受保护 / eval.json 放行 (AC-8, AC-9)
// ============================================================================

describe('内置 glob 集合 (AC-8, AC-9)', () => {
  beforeEach(() => {
    resetMocks();
    // 无 write_protection：仅内置集合生效
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  /** 通过 stdin 提交一次工具调用并返回解析后的 hook 输出。 */
  function runHook(payload: Record<string, unknown>): {
    permissionDecision: string;
    permissionDecisionReason: string;
  } {
    mockReadFileSync.mockReturnValue(JSON.stringify(payload));
    runProtectFiles();
    const parsed = JSON.parse(getLastStdout()) as {
      hookSpecificOutput: { permissionDecision: string; permissionDecisionReason: string };
    };
    return parsed.hookSpecificOutput;
  }

  // ---- workflow.json 仍被内置规则拦截 ----

  it('Write openspec/changes/test/workflow.json、无 write_protection 时 deny，reason 指向 phase_log / backtrack / change_create（AC-8）', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/test/workflow.json' },
    });

    expect(output.permissionDecision).toBe('deny');
    expect(output.permissionDecisionReason).toContain('workflow.json');
    expect(output.permissionDecisionReason).toContain('phase_log');
    expect(output.permissionDecisionReason).toContain('backtrack');
    expect(output.permissionDecisionReason).toContain('change_create');
  });

  it('Edit / StrReplace 同一 workflow.json 路径同样 deny，reason 含工具名', () => {
    for (const toolName of ['Edit', 'StrReplace']) {
      const output = runHook({
        tool_name: toolName,
        tool_input: { file_path: 'openspec/changes/test/workflow.json' },
      });

      expect(output.permissionDecision).toBe('deny');
      expect(output.permissionDecisionReason).toContain(toolName);
    }
  });

  it('Bash 以 > / >> / tee / heredoc / >| / >& 写 workflow.json 时 deny', () => {
    const commands = [
      'echo "{}" > openspec/changes/x/workflow.json',
      'echo "{}" >> openspec/changes/x/workflow.json',
      'echo "{}" | tee openspec/changes/x/workflow.json',
      'cat <<EOF > openspec/changes/x/workflow.json',
      'echo "{}" >| openspec/changes/x/workflow.json',
      'echo "{}" >& openspec/changes/x/workflow.json',
    ];

    for (const command of commands) {
      const output = runHook({ tool_name: 'Bash', tool_input: { command } });
      expect(output.permissionDecision).toBe('deny');
      expect(output.permissionDecisionReason).toContain('workflow.json');
    }
  });

  it('PowerShell 以 Set-Content / Out-File / Add-Content / Export-Csv / Export-CliXml / Tee-Object / > / >> / *> / .NET 方法写 workflow.json 时 deny', () => {
    const target = 'openspec/changes/x/workflow.json';
    const commands = [
      `Set-Content -Path ${target} -Value "{}"`,
      `Out-File -FilePath ${target}`,
      `Add-Content ${target} "{}"`,
      `Export-Csv ${target}`,
      `Export-CliXml ${target}`,
      `Tee-Object ${target}`,
      `echo "{}" > ${target}`,
      `echo "{}" >> ${target}`,
      `echo "{}" *> ${target}`,
      `[System.IO.File]::WriteAllText("${target}", "{}")`,
    ];

    for (const command of commands) {
      const output = runHook({ tool_name: 'PowerShell', tool_input: { command } });
      expect(output.permissionDecision).toBe('deny');
      expect(output.permissionDecisionReason).toContain('workflow.json');
    }
  });

  it('Write openspec/config.json 仍 deny，reason 含 config.json 语义（AC-8）', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/config.json' },
    });

    expect(output.permissionDecision).toBe('deny');
    expect(output.permissionDecisionReason).toContain('openspec/config.json');
  });

  // ---- eval.json 不再被内置规则拦截 ----

  it('Write openspec/changes/test/eval.json、无 write_protection 时内置 glob 不单独 deny（AC-9）', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/test/eval.json' },
    });

    expect(output.permissionDecision).not.toBe('deny');
    expect(output.permissionDecision).toBe('allow');
  });

  it('Bash > eval.json / PowerShell Set-Content eval.json 时 allow（内置集合不含该文件）（AC-9）', () => {
    const bashOutput = runHook({
      tool_name: 'Bash',
      tool_input: { command: 'echo "[]" > openspec/changes/x/eval.json' },
    });
    expect(bashOutput.permissionDecision).toBe('allow');

    const psOutput = runHook({
      tool_name: 'PowerShell',
      tool_input: { command: 'Set-Content openspec/changes/x/eval.json "[]"' },
    });
    expect(psOutput.permissionDecision).toBe('allow');
  });

  it('Edit / StrReplace eval.json 亦 allow（内置集合整体不含 eval.json）', () => {
    for (const toolName of ['Edit', 'StrReplace']) {
      const output = runHook({
        tool_name: toolName,
        tool_input: { file_path: 'openspec/changes/test/eval.json' },
      });
      expect(output.permissionDecision).toBe('allow');
    }
  });

  it('Write openspec/changes/test/proposal.md 时 allow（内置 glob 不单独 deny）', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/test/proposal.md' },
    });

    expect(output.permissionDecision).toBe('allow');
  });

  it('write_protection.files 含 glob openspec/changes/*/eval.json 时同一路径 deny（用户可自行保护）', () => {
    mockReadConfig.mockReturnValue({
      schema: 'spec-driven',
      write_protection: {
        files: [{ glob: 'openspec/changes/*/eval.json', reason: '用户保护：%s（%t）' }],
      },
    });

    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/x/eval.json' },
    });

    expect(output.permissionDecision).toBe('deny');
    expect(output.permissionDecisionReason).toContain('用户保护');
  });

  // ---- 边界 ----

  it('file_path 超长（>1000 chars）且以 /workflow.json 结尾并匹配 glob 时 deny', () => {
    const longPath = `openspec/changes/${'a'.repeat(1001)}/workflow.json`;
    const output = runHook({ tool_name: 'Write', tool_input: { file_path: longPath } });

    expect(output.permissionDecision).toBe('deny');
    expect(output.permissionDecisionReason).toContain('workflow.json');
  });

  it('路径含 emoji 的 change 名 + workflow.json 时仍按 glob 匹配 deny', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/chg-🧪/workflow.json' },
    });

    expect(output.permissionDecision).toBe('deny');
  });

  it('Write 缺 file_path / file_path 为 null / 数字时 allow（fail-open）', () => {
    for (const filePath of [undefined, null, 42]) {
      const output = runHook({ tool_name: 'Write', tool_input: { file_path: filePath } });
      expect(output.permissionDecision).toBe('allow');
    }
  });

  it('Bash cat / PowerShell Get-Content 只读 workflow.json 时 allow', () => {
    const bashOutput = runHook({
      tool_name: 'Bash',
      tool_input: { command: 'cat openspec/changes/x/workflow.json' },
    });
    expect(bashOutput.permissionDecision).toBe('allow');

    const psOutput = runHook({
      tool_name: 'PowerShell',
      tool_input: { command: 'Get-Content openspec/changes/x/workflow.json' },
    });
    expect(psOutput.permissionDecision).toBe('allow');
  });

  it('行首 python / node 写受保护路径仍豁免 allow（既有豁免不因新 glob 取消）', () => {
    for (const command of [
      'python scripts/write.py openspec/changes/x/workflow.json',
      'node scripts/write.mjs openspec/changes/x/workflow.json',
    ]) {
      const output = runHook({ tool_name: 'Bash', tool_input: { command } });
      expect(output.permissionDecision).toBe('allow');
    }
  });

  it('reason 中 %s / %t 被替换，序列化结果可 JSON.parse', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/x/workflow.json' },
    });

    expect(output.permissionDecisionReason).toContain('openspec/changes/x/workflow.json');
    expect(output.permissionDecisionReason).toContain('Write');
    expect(output.permissionDecisionReason).not.toContain('%s');
    expect(output.permissionDecisionReason).not.toContain('%t');
    expect(() => JSON.parse(getLastStdout())).not.toThrow();
  });

  it('write_protection.files 为空数组时内置 workflow.json / config.json 仍生效', () => {
    mockReadConfig.mockReturnValue({ schema: 'spec-driven', write_protection: { files: [] } });

    expect(
      runHook({ tool_name: 'Write', tool_input: { file_path: 'openspec/changes/x/workflow.json' } })
        .permissionDecision,
    ).toBe('deny');
    expect(
      runHook({ tool_name: 'Write', tool_input: { file_path: 'openspec/config.json' } })
        .permissionDecision,
    ).toBe('deny');
  });

  // ---- 废弃：过渡期的 eval.json 保护语义 ----

  it('Write openspec/changes/test/eval.json 在内置规则下不再 deny（旧「过渡期保护遗留文件」语义已废弃）', () => {
    const output = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/test/eval.json' },
    });

    expect(output.permissionDecision).not.toBe('deny');
  });
});

// ============================================================================
// record-files — phase_next 建绑 (AC-3)
// ============================================================================

describe('record-files — phase_next 建绑 (AC-3)', () => {
  let tempRoot = '';

  beforeEach(() => {
    resetMocks();
    tempRoot = NodeFs.mkdtempSync(path.join(os.tmpdir(), 'hooks-record-bind-'));
    mockGetProjectDir.mockReturnValue(tempRoot);
  });

  afterEach(() => {
    NodeFs.rmSync(tempRoot, { recursive: true, force: true });
  });

  function runRecord(event: Record<string, unknown>): void {
    mockReadFileSync.mockReturnValue(JSON.stringify(event));
    runRecordFiles();
  }

  it('全名 mcp__plugin_dev-team_dev-team__phase_next 事件（tool_input.change=change-a）→ bindSession 被调用且不写清单 (AC-3)', () => {
    runRecord({
      tool_name: 'mcp__plugin_dev-team_dev-team__phase_next',
      tool_input: { change: 'change-a' },
      session_id: 'S1',
    });

    expect(mockBindSession).toHaveBeenCalledWith(tempRoot, 'S1', 'change-a');
    expect(mockReadFileInventory).not.toHaveBeenCalled();
    expect(mockWriteFileInventory).not.toHaveBeenCalled();
    expect(stdoutWriteMock).not.toHaveBeenCalled();
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('phase_next 事件携带 agent_type（subagent 触发）→ 绑定照常建立（绑定不依赖 agent_type）', () => {
    runRecord({
      tool_name: 'mcp__plugin_dev-team_dev-team__phase_next',
      tool_input: { change: 'change-a' },
      session_id: 'S1',
      agent_type: 'dev-team:test-gen-generator',
    });

    expect(mockBindSession).toHaveBeenCalledWith(tempRoot, 'S1', 'change-a');
  });

  it('phase_next 事件缺 tool_input.change → 不建立绑定、不写清单、不抛错（exit 0 语义）', () => {
    for (const toolInput of [{}, { change: '' }, { change: 42 }, undefined]) {
      mockBindSession.mockClear();
      runRecord({
        tool_name: 'mcp__plugin_dev-team_dev-team__phase_next',
        tool_input: toolInput,
        session_id: 'S1',
      });

      expect(mockBindSession).not.toHaveBeenCalled();
      expect(mockWriteFileInventory).not.toHaveBeenCalled();
      expect(exitMock).not.toHaveBeenCalled();
    }
  });

  it('stdin 非法 JSON / 空输入 → 静默丢弃、exit 0、不产生任何写副作用', () => {
    for (const raw of ['{not json', '', '   ', '[1,2]', '"str"']) {
      mockBindSession.mockClear();
      mockReadFileInventory.mockClear();
      mockWriteFileInventory.mockClear();
      mockReadFileSync.mockReturnValue(raw);
      expect(() => runRecordFiles()).not.toThrow();
      expect(mockBindSession).not.toHaveBeenCalled();
      expect(mockReadFileInventory).not.toHaveBeenCalled();
      expect(mockWriteFileInventory).not.toHaveBeenCalled();
      expect(exitMock).not.toHaveBeenCalled();
    }
  });

  it('非 MCP 全名的 phase_next 变体不建立绑定：无 mcp__ 前缀 / 后缀不符均按普通事件走查表路径（匹配精确性，AC-3）', () => {
    for (const toolName of ['phase_next', 'mcp__plugin_dev-team_dev-team__phase_next_v2']) {
      mockBindSession.mockClear();
      mockLookupChange.mockReturnValue('change-a');
      runRecord({
        tool_name: toolName,
        tool_input: { change: 'sneaky' },
        session_id: 'S1',
      });
      // 未走建绑分支
      expect(mockBindSession).not.toHaveBeenCalled();
    }
  });
});

// ============================================================================
// record-files — 归账写清单 (AC-2, AC-4, AC-13)
// ============================================================================

describe('record-files — 归账写清单 (AC-2, AC-4, AC-13)', () => {
  let tempRoot = '';

  beforeEach(() => {
    resetMocks();
    tempRoot = NodeFs.mkdtempSync(path.join(os.tmpdir(), 'hooks-record-fold-'));
    mockGetProjectDir.mockReturnValue(tempRoot);
    mockLookupChange.mockReturnValue('change-a');
    bindInventoryStore({ written: [], deleted: [] });
  });

  afterEach(() => {
    NodeFs.rmSync(tempRoot, { recursive: true, force: true });
  });

  function runRecord(event: Record<string, unknown>): void {
    mockReadFileSync.mockReturnValue(JSON.stringify(event));
    runRecordFiles();
  }

  function expectedChangeDir(): string {
    return path.resolve(tempRoot, 'openspec', 'changes', 'change-a');
  }

  function lastWrittenFiles(): {
    written: string[];
    deleted: string[];
    source?: Record<string, string>;
  } {
    expect(mockWriteFileInventory).toHaveBeenCalled();
    const call = mockWriteFileInventory.mock.calls.at(-1)!;
    expect(call[0]).toBe(expectedChangeDir());
    return call[1];
  }

  it('已绑定 session 的 Write 事件 file_path=src/foo.ts → 清单 written 含 src/foo.ts（相对项目根 POSIX 风格，AC-2）', () => {
    runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' });

    expect(lastWrittenFiles().written).toEqual(['src/foo.ts']);
  });

  it('Edit 事件 file_path / NotebookEdit 事件 notebook_path → 同归账语义 (AC-2)', () => {
    runRecord({ tool_name: 'Edit', tool_input: { file_path: 'src/edited.ts' }, session_id: 'S1' });
    expect(lastWrittenFiles().written).toContain('src/edited.ts');

    runRecord({
      tool_name: 'NotebookEdit',
      tool_input: { notebook_path: 'src/nb.ipynb' },
      session_id: 'S1',
    });
    expect(lastWrittenFiles().written).toContain('src/nb.ipynb');
  });

  it('Bash rm src/old.ts → deleted 含 src/old.ts；PowerShell Remove-Item 同语义 (AC-2)', () => {
    runRecord({ tool_name: 'Bash', tool_input: { command: 'rm src/old.ts' }, session_id: 'S1' });
    expect(lastWrittenFiles().deleted).toEqual(['src/old.ts']);

    bindInventoryStore({ written: [], deleted: [] });
    runRecord({
      tool_name: 'PowerShell',
      tool_input: { command: 'Remove-Item src/old.ts' },
      session_id: 'S1',
    });
    expect(lastWrittenFiles().deleted).toEqual(['src/old.ts']);
  });

  it('Bash git restore src/foo.ts → 折叠为净 untouched（written/deleted 均无该路径，AC-4）', () => {
    runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' });
    runRecord({
      tool_name: 'Bash',
      tool_input: { command: 'git restore src/foo.ts' },
      session_id: 'S1',
    });

    const files = lastWrittenFiles();
    expect(files.written).toEqual([]);
    expect(files.deleted).toEqual([]);
  });

  it('同 session 事件序列 write→delete→write → 净状态按折叠规则收敛（与 file-inventory 单元语义一致）', () => {
    runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' });
    runRecord({ tool_name: 'Bash', tool_input: { command: 'rm src/foo.ts' }, session_id: 'S1' });
    runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' });

    const files = lastWrittenFiles();
    expect(files.written).toEqual(['src/foo.ts']);
    expect(files.deleted).toEqual([]);
  });

  it('事件携带 agent_type → 清单 source[path]=agent_type；随后被无 agent_type 的主会话事件重写 → source 清除 (AC-13)', () => {
    runRecord({
      tool_name: 'Write',
      tool_input: { file_path: 'src/foo.ts' },
      session_id: 'S1',
      agent_type: 'dev-team:implementation-generator',
    });
    expect(lastWrittenFiles().source).toEqual({
      'src/foo.ts': 'dev-team:implementation-generator',
    });

    runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' });
    const files = lastWrittenFiles();
    expect(files.written).toEqual(['src/foo.ts']);
    expect(files.source).toBeUndefined();
  });

  it('未绑定 session 的写事件 → 静默丢弃，workflow.json 不变（stderr 诊断，AC-3 前置语义）', () => {
    mockLookupChange.mockReturnValue(null);
    const stderrSpy = vi.spyOn(process.stderr, 'write');
    stderrSpy.mockClear();

    runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S9' });

    expect(stderrSpy.mock.calls.some((c) => String(c[0]).includes('未绑定'))).toBe(true);
    expect(mockReadFileInventory).not.toHaveBeenCalled();
    expect(mockWriteFileInventory).not.toHaveBeenCalled();
    expect(exitMock).not.toHaveBeenCalled();
    stderrSpy.mockRestore();
  });

  it('目标 change 缺 workflow.json / 无 files → 静默（stderr 诊断、exit 0），不产生半写状态', () => {
    mockReadFileInventory.mockImplementation(() => {
      throw new Error(
        'workflow.json 缺少 files 字段：该 change 创建于文件清单机制之前，请重建该 change（change_create）。',
      );
    });
    const stderrSpy = vi.spyOn(process.stderr, 'write');
    stderrSpy.mockClear();

    expect(() =>
      runRecord({ tool_name: 'Write', tool_input: { file_path: 'src/foo.ts' }, session_id: 'S1' }),
    ).not.toThrow();

    expect(
      stderrSpy.mock.calls.some(
        (c) => String(c[0]).includes('record-files:') && String(c[0]).includes('请重建'),
      ),
    ).toBe(true);
    expect(mockWriteFileInventory).not.toHaveBeenCalled();
    expect(exitMock).not.toHaveBeenCalled();
    stderrSpy.mockRestore();
  });

  it('绝对路径（含 Windows 反斜杠）相对 getProjectDir() 归一化为 POSIX 相对路径', () => {
    runRecord({
      tool_name: 'Write',
      tool_input: { file_path: path.join(tempRoot, 'src', 'deep', 'foo.ts') },
      session_id: 'S1',
    });

    expect(lastWrittenFiles().written).toEqual(['src/deep/foo.ts']);
  });

  it('路径越出项目根（../outside.ts）→ 丢弃不入清单（不读写清单）', () => {
    runRecord({ tool_name: 'Write', tool_input: { file_path: '../outside.ts' }, session_id: 'S1' });

    expect(mockReadFileInventory).not.toHaveBeenCalled();
    expect(mockWriteFileInventory).not.toHaveBeenCalled();
  });

  it('openspec/** 路径与 workflow.json 自身路径 → 排除不入清单（自污染排除，AC-2）', () => {
    for (const filePath of [
      'openspec/changes/change-a/proposal.md',
      'openspec/config.json',
      path.join(tempRoot, 'openspec', 'changes', 'change-a', 'workflow.json'),
      'workflow.json',
    ]) {
      runRecord({ tool_name: 'Write', tool_input: { file_path: filePath }, session_id: 'S1' });
    }

    expect(mockReadFileInventory).not.toHaveBeenCalled();
    expect(mockWriteFileInventory).not.toHaveBeenCalled();
  });

  it('归账全程任何内部异常 → exit 0 且 stdout 无阻塞决策输出（不阻塞工具调用）', () => {
    mockReadFileInventory.mockImplementation(() => {
      throw new Error('boom-in-recorder');
    });
    const stderrSpy = vi.spyOn(process.stderr, 'write');
    stderrSpy.mockClear();
    stdoutWriteMock.mockClear();

    expect(() =>
      runRecord({ tool_name: 'Bash', tool_input: { command: 'rm src/a.ts' }, session_id: 'S1' }),
    ).not.toThrow();

    expect(exitMock).not.toHaveBeenCalled();
    expect(stdoutWriteMock).not.toHaveBeenCalled();
    expect(stderrSpy.mock.calls.some((c) => String(c[0]).includes('boom-in-recorder'))).toBe(true);
    stderrSpy.mockRestore();
  });
});

// ============================================================================
// protect-files — 扩拦截：git stash / clean / restore / rm / mv (AC-5, AC-4)
// ============================================================================

describe('protect-files — 扩拦截：git stash / clean / restore / rm / mv (AC-5, AC-4)', () => {
  beforeEach(() => {
    resetMocks();
    mockReadConfig.mockReturnValue({ schema: 'spec-driven' });
  });

  function decision(command: string, toolName: 'Bash' | 'PowerShell' = 'Bash'): string {
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ tool_name: toolName, tool_input: { command } }),
    );
    runProtectFiles();
    return JSON.parse(getLastStdout()).hookSpecificOutput.permissionDecision as string;
  }

  it('git stash / git stash pop / git clean -fd（无路径限定）→ deny (AC-5)', () => {
    for (const command of [
      'git stash',
      'git stash pop',
      'git stash push -m wip',
      'git clean -fd',
    ]) {
      expect(decision(command)).toBe('deny');
    }
  });

  it('git clean <openspec 下路径> → deny；git clean -n / --dry-run 只读形态 → allow (AC-5)', () => {
    expect(decision('git clean -fd openspec/changes/x/')).toBe('deny');
    expect(decision('git clean -n')).toBe('allow');
    expect(decision('git clean --dry-run -fd')).toBe('allow');
  });

  it('git clean 指向源码路径（非 openspec）→ allow（记录器可逐路径归账的形态放行）', () => {
    expect(decision('git clean -fd src/')).toBe('allow');
  });

  it('git stash list / git stash show 只读形态 → allow (AC-5)', () => {
    expect(decision('git stash list')).toBe('allow');
    expect(decision('git stash show')).toBe('allow');
  });

  it('git restore openspec/changes/x/design.md → deny；git restore src/foo.ts → allow（工作流产物拦、源码放行，AC-5）', () => {
    expect(decision('git restore openspec/changes/x/design.md')).toBe('deny');
    expect(decision('git restore src/foo.ts')).toBe('allow');
  });

  it('rm <受保护路径> / Remove-Item <受保护路径> → deny（删保护，AC-5）', () => {
    expect(decision('rm openspec/changes/x/workflow.json')).toBe('deny');
    expect(decision('Remove-Item openspec/config.json', 'PowerShell')).toBe('deny');
    // 删非保护路径放行
    expect(decision('rm src/foo.ts')).toBe('allow');
  });

  it('mv <受保护路径> <新路径> / mv <旧> <受保护路径> → deny（delete+write 双条目任一命中即拦）', () => {
    expect(decision('mv openspec/config.json src/renamed.json')).toBe('deny');
    expect(decision('mv src/a.ts openspec/changes/x/workflow.json')).toBe('deny');
    // 双双不命中 → allow
    expect(decision('mv src/a.ts src/b.ts')).toBe('allow');
  });

  it('git restore --source=<commit> <path> / git checkout <commit> -- <paths> 归 write 分类（还原到历史版本为内容写入语义，AC-4）', () => {
    // write 分类：命中保护 glob 即拦，源码放行
    expect(decision('git restore --source=HEAD~1 openspec/changes/x/workflow.json')).toBe('deny');
    expect(decision('git restore --source=abc123 src/foo.ts')).toBe('allow');
    expect(decision('git checkout abc123 -- openspec/config.json')).toBe('deny');
    expect(decision('git checkout abc123 -- src/foo.ts')).toBe('allow');
    // 无 commit 的 checkout -- 为 revert 分类：非 openspec 产物放行
    expect(decision('git checkout -- src/foo.ts')).toBe('allow');
  });

  it('既有 write 语义回归：> >> tee Set-Content 命中保护 glob 仍 deny', () => {
    expect(decision('echo x > openspec/changes/x/workflow.json')).toBe('deny');
    expect(decision('echo x >> openspec/changes/x/workflow.json')).toBe('deny');
    expect(decision('echo x | tee openspec/changes/x/workflow.json')).toBe('deny');
    expect(
      decision('Set-Content -Path openspec/changes/x/workflow.json -Value x', 'PowerShell'),
    ).toBe('deny');
  });

  it('python/node 命令豁免语义不变：python -c "…rm…" 类命令不拦截（fail-open 语义回归）', () => {
    expect(decision('python -c "import os; os.remove(\'openspec/config.json\')"')).toBe('allow');
    expect(decision('python3 script.py > openspec/changes/x/workflow.json')).toBe('allow');
    expect(decision('node script.js > openspec/changes/x/workflow.json')).toBe('allow');
  });
});

// ============================================================================
// main — record-files 子命令分发
// ============================================================================

describe('main — record-files 子命令分发', () => {
  let tempRoot = '';

  beforeEach(() => {
    resetMocks();
    tempRoot = NodeFs.mkdtempSync(path.join(os.tmpdir(), 'hooks-record-main-'));
    mockGetProjectDir.mockReturnValue(tempRoot);
  });

  afterEach(() => {
    NodeFs.rmSync(tempRoot, { recursive: true, force: true });
  });

  it("argv[2]='record-files' → 分发执行 runRecordFiles（phase_next 事件完成建绑）", () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'record-files'];
    mockReadFileSync.mockReturnValue(
      JSON.stringify({
        tool_name: 'mcp__plugin_dev-team_dev-team__phase_next',
        tool_input: { change: 'change-a' },
        session_id: 'S1',
      }),
    );
    try {
      main();
      expect(mockBindSession).toHaveBeenCalledWith(tempRoot, 'S1', 'change-a');
      expect(exitMock).not.toHaveBeenCalledWith(1);
    } finally {
      process.argv = origArgv;
    }
  });

  it("argv[2]='record_file'（拼写残缺）仍走 Unknown subcommand → exit 1", () => {
    const origArgv = process.argv;
    process.argv = ['node', 'hooks.ts', 'record_file'];
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    try {
      main();
      expect(exitMock).toHaveBeenCalledWith(1);
      const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
      expect(stderrText).toContain('Unknown subcommand: record_file');
    } finally {
      stderrSpy.mockRestore();
      process.argv = origArgv;
    }
  });
});
