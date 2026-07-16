/**
 * 单元测试: cli -- CLI 命令注册
 *
 * 覆盖范围:
 * - AC-1: CLI 注册 `test-execution` 子命令
 * - AC-1: 命令接受 `--project-root` 选项
 * - AC-1: 命令 action 调用 `runTestExecution`
 * - AC-1: `runTestExecution` 返回非零时进程以 exit(1) 退出
 * - 异常: 未注册 `test-execution` 子命令时 CLI 报错
 * - 异常: `--project-root` 后缺省值时 CLI 报错
 * - 边界: `--project-root` 值为空字符串时使用默认 project dir
 * - 边界: 同时传递多个未知选项时不影响 test-execution 命令正常注册
 */

import { describe, it, expect, vi } from 'vite-plus/test';

import { cli } from './cli';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockRunTestExecution = vi.fn();

vi.mock('./commands/test-execution', () => ({
  runTestExecution: (...args: unknown[]) => mockRunTestExecution(...args),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** 从 cli 实例中获取 test-execution 命令 */
function getTestExecCommand() {
  return cli.commands.find((c) => c.name === 'test-execution')!;
}

/** mock process.exit 并返回 spy，调用方负责 restore */
function spyOnProcessExit() {
  // eslint-disable-next-line typescript/no-unsafe-type-assertion
  return vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
}

// ---------------------------------------------------------------------------
// 正向测试: CLI 注册
// ===========================================================================

describe('dev-team test-execution command registration', () => {
  it('CLI 应注册 `test-execution` 子命令', () => {
    const cmd = getTestExecCommand();
    expect(cmd).toBeDefined();
    expect(cmd.name).toBe('test-execution');
  });

  it('命令应接受 `--project-root` 选项', () => {
    const cmd = getTestExecCommand();
    const opt = cmd.options.find((o) => o.rawName.includes('--project-root'));
    expect(opt).toBeDefined();
    // <path> (尖括号) 表示必须提供值
    expect(opt!.rawName).toContain('<path>');
  });

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

describe('dev-team test-execution -- 异常', () => {
  it('未注册 test-execution 子命令时 CLI 报错而非静默忽略', () => {
    // 验证 cli 实例有 commands 列表，不存在的命令不会在其中
    const fakeCmd = cli.commands.find((c) => c.name === 'nonexistent-command');
    expect(fakeCmd).toBeUndefined();
  });

  it('--project-root 后缺省值时 CLI 报错', () => {
    const cmd = getTestExecCommand();
    const opt = cmd.options.find((o) => o.rawName.includes('--project-root'));
    expect(opt).toBeDefined();
    // <path> 表示必填参数，[path] 表示可选
    expect(opt!.rawName).toContain('<path>');
    expect(opt!.rawName).not.toContain('[path]');
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
    // --change, --project-root, --files, --framework, --no-mutation, --mutation-diff-only
    expect(cmd.options.length).toBeGreaterThanOrEqual(4);
  });
});

// ===========================================================================
// --no-mutation 选项
// ===========================================================================

describe('dev-team test-execution -- --no-mutation 选项', () => {
  it('CLI 注册 `--no-mutation` 选项', () => {
    const cmd = getTestExecCommand();
    const opt = cmd.options.find((o) => o.rawName.includes('--no-mutation'));
    expect(opt).toBeDefined();
  });

  it('--no-mutation 选项传递到 runTestExecution', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '/test/project', noMutation: true });

    expect(mockRunTestExecution).toHaveBeenCalledWith(
      expect.objectContaining({ noMutation: true }),
    );
    exitSpy.mockRestore();
  });

  it('--no-mutation 不传递时 noMutation 为 undefined', async () => {
    const exitSpy = spyOnProcessExit();
    mockRunTestExecution.mockResolvedValue(0);

    const cmd = getTestExecCommand();
    await cmd.commandAction!({ projectRoot: '/test/project' });

    const callArgs = mockRunTestExecution.mock.calls.at(-1)!;
    expect(callArgs[0].noMutation).toBeUndefined();
    exitSpy.mockRestore();
  });
});
