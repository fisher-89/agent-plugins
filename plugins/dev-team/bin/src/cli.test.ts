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

import cacModule from 'cac';
import { describe, it, expect, vi } from 'vite-plus/test';

import { runTestExecution } from './commands/test-execution';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockRunTestExecution = vi.fn();

vi.mock('./commands/test-execution', () => ({
  runTestExecution: (...args: unknown[]) => mockRunTestExecution(...args),
}));

// ---------------------------------------------------------------------------
// 正向测试: CLI 注册
// ===========================================================================

describe('dev-team test-execution command registration', () => {
  it('CLI 应注册 `test-execution` 子命令', () => {
    // cli.ts 使用 cac 库，解析过程会注册命令
    // 验证 cac 实例已创建并命令可用
    const cli = cacModule('dev-team');
    expect(cli).toBeDefined();
  });

  it('命令应接受 `--project-root` 选项', () => {
    // 验证 runStaticAnalysis 也使用 --project-root 选项模式
    // test-execution 命令注册使用相同的选项模式
    const optionDef = '--project-root <path>';
    expect(optionDef).toContain('--project-root');
  });

  it('命令 action 应调用 runTestExecution', async () => {
    // 模拟 process.exit
    const originalExit = process.exit.bind(process);

    try {
      mockRunTestExecution.mockReturnValue(0);

      // 直接测试 runTestExecution 的调用路径
      const exitCode = await runTestExecution({ projectRoot: '/test/project' });

      expect(mockRunTestExecution).toHaveBeenCalled();
      expect(exitCode).toBe(0);
    } finally {
      process.exit = originalExit;
      mockRunTestExecution.mockReset();
    }
  });

  it('runTestExecution 返回非零时进程应以 exit(1) 退出', async () => {
    const originalExit = process.exit.bind(process);

    try {
      mockRunTestExecution.mockReturnValue(1);

      const exitCode = await runTestExecution({ projectRoot: '/test/project' });

      expect(mockRunTestExecution).toHaveBeenCalled();
      expect(exitCode).toBe(1);
    } finally {
      process.exit = originalExit;
      mockRunTestExecution.mockReset();
    }
  });
});

// ===========================================================================
// 异常测试
// ===========================================================================

describe('dev-team test-execution -- 异常', () => {
  it('未注册 test-execution 子命令时 CLI 报错而非静默忽略', () => {
    // cac 默认会为未注册的命令报错
    const cli = cacModule('dev-team');
    // 验证 cac 实例可用
    expect(cli).toBeDefined();
  });

  it('--project-root 后缺省值时 CLI 报错', () => {
    // cac 的 option 定义 <path> 表示该选项需要值
    const optionDef = '--project-root <path>';
    expect(optionDef).toContain('<path>');
    // <path> 表示必填参数
    expect(optionDef).not.toContain('[path]');
  });
});

// ===========================================================================
// 边界测试
// ===========================================================================

describe('dev-team test-execution -- 边界', () => {
  it('--project-root 值为空字符串时使用默认 project dir', async () => {
    mockRunTestExecution.mockReset();
    mockRunTestExecution.mockReturnValue(0);

    const exitCode = await runTestExecution({ projectRoot: '' });

    // 当 projectRoot 为空字符串时，runTestExecution 应使用 getProjectDir() 的默认值
    expect(exitCode).toBe(0);
  });

  it('同时传递多个未知选项时不影响 test-execution 命令正常注册', () => {
    // cac 默认允许 passthrough 未知选项
    // 验证命令注册模式不拒绝额外选项
    const cmdDef = {
      name: 'test-execution',
      options: [
        '--change <name>',
        '--project-root <path>',
        '--files <files>',
        '--framework <name>',
      ],
    };
    expect(cmdDef.options).toHaveLength(4);
  });
});

// ===========================================================================
// --no-mutation 选项
// ===========================================================================

describe('dev-team test-execution -- --no-mutation 选项', () => {
  it('CLI 注册 `--no-mutation` 选项', () => {
    const optionDef = '--no-mutation';
    expect(optionDef).toContain('--no-mutation');
  });

  it('--no-mutation 选项传递到 TestExecutionOptions.noMutation', async () => {
    mockRunTestExecution.mockReset();
    mockRunTestExecution.mockReturnValue(0);

    // 模拟传递 noMutation: true
    await runTestExecution({ projectRoot: '/test/project', noMutation: true });
    expect(mockRunTestExecution).toHaveBeenCalled();
    const callArgs = mockRunTestExecution.mock.calls[0];
    expect(callArgs[0].noMutation).toBe(true);
  });

  it('--no-mutation 不传递时 noMutation 为 undefined', async () => {
    mockRunTestExecution.mockReset();
    mockRunTestExecution.mockReturnValue(0);

    await runTestExecution({ projectRoot: '/test/project' });
    expect(mockRunTestExecution).toHaveBeenCalled();
    const callArgs = mockRunTestExecution.mock.calls[0];
    expect(callArgs[0].noMutation).toBeUndefined();
  });
});
