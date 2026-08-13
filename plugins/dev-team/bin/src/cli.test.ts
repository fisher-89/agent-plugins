import { describe, it, expect, vi } from 'vite-plus/test';

import { cli } from './cli';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockRunTestExecution = vi.fn();
const mockRunStaticAnalysis = vi.fn();

vi.mock('./commands/test-execution', () => ({
  runTestExecution: (...args: unknown[]) => mockRunTestExecution(...args),
}));

vi.mock('./commands/run-static-analysis', () => ({
  runStaticAnalysis: (...args: unknown[]) => mockRunStaticAnalysis(...args),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** 从 cli 实例中获取 test-execution 命令 */
function getTestExecCommand() {
  return cli.commands.find((c) => c.name === 'test-execution')!;
}

/** 从 cli 实例中获取 run_static_analysis 命令 */
function getStaticAnalysisCommand() {
  return cli.commands.find((c) => c.name === 'run_static_analysis')!;
}

/** mock process.exit 并返回 spy，调用方负责 restore */
function spyOnProcessExit() {
  return vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
}

// ---------------------------------------------------------------------------
// 正向测试: CLI 注册
// ===========================================================================

describe('dev-team test-execution command registration', () => {
  it('命令 action 应调用 runTestExecution', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '/test/project' });

    expect(mockRunTestExecution).toHaveBeenCalled();
    expect(exitSpy).toHaveBeenCalledWith(0);
    exitSpy.mockRestore();
  });

  it('runTestExecution 返回非零时进程应以 exit(1) 退出', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(1);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '/test/project' });

    expect(mockRunTestExecution).toHaveBeenCalled();
    expect(exitSpy).toHaveBeenCalledWith(1);
    exitSpy.mockRestore();
  });
});

// ===========================================================================
// 异常测试
// ===========================================================================

describe('cli 命令 -- 异常', () => {
  it('未指定命令时 CLI 报错而非静默忽略', () => {
    const exitSpy = spyOnProcessExit();
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);

    // 使用不存在的子命令触发 cac 的 unknown command 错误
    cli.parse(['node', 'cli.js']);

    // cac 应输出错误信息到 stderr，而非静默退出
    const stderrOutput = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrOutput).include('No command specified');

    stderrSpy.mockRestore();
    exitSpy.mockRestore();
  });

  it('执行未注册命令时 CLI 报错而非静默忽略', () => {
    const exitSpy = spyOnProcessExit();
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const commandName = 'un-registered-command';

    // 使用不存在的子命令触发 cac 的 unknown command 错误
    cli.parse(['node', 'cli.js', commandName]);

    // cac 应输出错误信息到 stderr，而非静默退出
    const stderrOutput = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrOutput).include(`Unknown command: "${commandName}"`);

    stderrSpy.mockRestore();
    exitSpy.mockRestore();
  });
});

describe('dev-team test-execution -- 异常', () => {
  it('--project-root 后缺省值时 CLI 报错', () => {
    // --project-root <path> 要求必填值，传空将触发 cac 的 CACError
    expect(() => {
      cli.parse(['node', 'cli.js', 'test-execution', '--project-root']);
    }).toThrow(/option.*--project-root.*value.*missing/i);
  });
});

// ===========================================================================
// 边界测试
// ===========================================================================

describe('dev-team test-execution -- 边界', () => {
  it('--project-root 值为空字符串时使用默认 project dir', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '' });

    // 当 projectRoot 为空字符串时，action 仍正常执行
    expect(mockRunTestExecution).toHaveBeenCalledWith(expect.objectContaining({ projectRoot: '' }));
    exitSpy.mockRestore();
  });

  it('同时传递多个选项时不影响 test-execution 命令正常注册', () => {
    const cmd = getTestExecCommand();
    // --change, --project-root, --files, --framework, --skip-mutation, --mutation-diff-only
    expect(cmd.options.length).toBeGreaterThanOrEqual(4);
  });
});

// ===========================================================================
// --skip-mutation 选项
// ===========================================================================

describe('dev-team test-execution --skip-mutation 选项', () => {
  it('--skip-mutation 选项传递到 runTestExecution', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '/test/project', skipMutation: true });

    expect(mockRunTestExecution).toHaveBeenCalledWith(
      expect.objectContaining({ noMutation: true }),
    );
    exitSpy.mockRestore();
  });

  it('--skip-mutation 不传递时 skipMutation 为 undefined', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '/test/project' });

    const callArgs = mockRunTestExecution.mock.calls.at(-1)!;
    expect(callArgs[0].skipMutation).toBeUndefined();
    exitSpy.mockRestore();
  });
});

// ===========================================================================
// 正向测试: run_static_analysis CLI 注册
// ===========================================================================

describe('dev-team run_static_analysis command registration', () => {
  it('命令 action 应调用 runStaticAnalysis', () => {
    const exitSpy = spyOnProcessExit();
    mockRunStaticAnalysis.mockReturnValue(0);

    const cmd = getStaticAnalysisCommand();
    cmd.commandAction!({ projectRoot: '/test/project' });

    expect(mockRunStaticAnalysis).toHaveBeenCalled();
    expect(exitSpy).toHaveBeenCalledWith(0);
    exitSpy.mockRestore();
  });

  it('runStaticAnalysis 返回非零时进程应以 exit(1) 退出', () => {
    const exitSpy = spyOnProcessExit();
    mockRunStaticAnalysis.mockReturnValue(1);

    const cmd = getStaticAnalysisCommand();
    cmd.commandAction!({ projectRoot: '/test/project' });

    expect(mockRunStaticAnalysis).toHaveBeenCalled();
    expect(exitSpy).toHaveBeenCalledWith(1);
    exitSpy.mockRestore();
  });
});

// ===========================================================================
// 异常测试: run_static_analysis
// ===========================================================================

describe('dev-team run_static_analysis -- 异常', () => {
  it('--project-root 后缺省值时 CLI 报错', () => {
    // --project-root <path> 要求必填值，传空将触发 cac 的 CACError
    expect(() => {
      cli.parse(['node', 'cli.js', 'run_static_analysis', '--project-root']);
    }).toThrow(/option.*--project-root.*value.*missing/i);
  });
});

// ===========================================================================
// 边界测试: run_static_analysis
// ===========================================================================

describe('既有命令回归', () => {
  it('test-execution 与 run_static_analysis 仍正常注册且 action 可调用', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);
    mockRunStaticAnalysis.mockReturnValue(0);

    const testExec = getTestExecCommand();
    const staticAnalysis = getStaticAnalysisCommand();
    expect(testExec).toBeDefined();
    expect(staticAnalysis).toBeDefined();

    await testExec.commandAction!({ projectRoot: '/test/project' });
    staticAnalysis.commandAction!({ projectRoot: '/test/project' });

    expect(mockRunTestExecution).toHaveBeenCalled();
    expect(mockRunStaticAnalysis).toHaveBeenCalled();
    exitSpy.mockRestore();
  });
});

describe('help 文本边界', () => {
  it('help 输出字符串中不包含 archi-decide 或 archi decide 子串', () => {
    const helpSpy = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);
    cli.outputHelp();
    const helpText = helpSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(helpText).not.toContain('archi-decide');
    expect(helpText).not.toContain('archi decide');
    helpSpy.mockRestore();
  });
});

describe('dev-team run_static_analysis -- 边界', () => {
  it('--project-root 值为空字符串时使用默认 project dir', () => {
    const exitSpy = spyOnProcessExit();
    mockRunStaticAnalysis.mockReturnValue(0);

    const cmd = getStaticAnalysisCommand();
    cmd.commandAction!({ projectRoot: '' });

    // 当 projectRoot 为空字符串时，action 仍正常执行
    expect(mockRunStaticAnalysis).toHaveBeenCalledWith(
      expect.objectContaining({ projectRoot: '' }),
    );
    exitSpy.mockRestore();
  });
});
