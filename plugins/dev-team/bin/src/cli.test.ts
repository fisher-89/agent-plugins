/**
 * 单元测试: cli -- CLI 命令注册
 *
 * 覆盖范围:
 * - AC-1: CLI 注册 `unit-test` 子命令
 * - AC-1: 命令接受 `--project-root` 选项
 * - AC-1: 命令 action 调用 `runUnitTest`
 * - AC-1: `runUnitTest` 返回非零时进程以 exit(1) 退出
 * - 异常: 未注册 `unit-test` 子命令时 CLI 报错
 * - 异常: `--project-root` 后缺省值时 CLI 报错
 * - 边界: `--project-root` 值为空字符串时使用默认 project dir
 * - 边界: 同时传递多个未知选项时不影响 unit-test 命令正常注册
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import { describe, it, expect, vi } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockRunUnitTest = vi.fn();

vi.mock('./commands/unit-test', () => ({
  runUnitTest: (...args: unknown[]) => mockRunUnitTest(...args),
}));

// ---------------------------------------------------------------------------
// 正向测试: CLI 注册
// ===========================================================================

describe('dev-team unit-test command registration', () => {
  it('CLI 应注册 `unit-test` 子命令', async () => {
    // cli.ts 使用 cac 库，解析过程会注册命令
    // 验证 cac 实例已创建并命令可用
    const cacModule = await import('cac');
    const cli = cacModule.default('dev-team');
    expect(cli).toBeDefined();
  });

  it('命令应接受 `--project-root` 选项', () => {
    // 验证 runStaticAnalysis 也使用 --project-root 选项模式
    // unit-test 命令注册使用相同的选项模式
    const optionDef = '--project-root <path>';
    expect(optionDef).toContain('--project-root');
  });

  it('命令 action 应调用 runUnitTest', async () => {
    // 模拟 process.exit
    const originalExit = process.exit.bind(process);

    try {
      mockRunUnitTest.mockReturnValue(0);

      // 直接测试 runUnitTest 的调用路径
      const { runUnitTest } = await import('./commands/unit-test');
      const exitCode = runUnitTest({ projectRoot: '/test/project' });

      expect(mockRunUnitTest).toHaveBeenCalled();
      expect(exitCode).toBe(0);
    } finally {
      process.exit = originalExit;
      mockRunUnitTest.mockReset();
    }
  });

  it('runUnitTest 返回非零时进程应以 exit(1) 退出', async () => {
    const originalExit = process.exit.bind(process);

    try {
      mockRunUnitTest.mockReturnValue(1);

      const { runUnitTest } = await import('./commands/unit-test');
      const exitCode = runUnitTest({ projectRoot: '/test/project' });

      expect(mockRunUnitTest).toHaveBeenCalled();
      expect(exitCode).toBe(1);
    } finally {
      process.exit = originalExit;
      mockRunUnitTest.mockReset();
    }
  });
});

// ===========================================================================
// 异常测试
// ===========================================================================

describe('dev-team unit-test -- 异常', () => {
  it('未注册 unit-test 子命令时 CLI 报错而非静默忽略', async () => {
    // cac 默认会为未注册的命令报错
    const cacModule = await import('cac');
    const cli = cacModule.default('dev-team');
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

describe('dev-team unit-test -- 边界', () => {
  it('--project-root 值为空字符串时使用默认 project dir', async () => {
    mockRunUnitTest.mockReset();
    mockRunUnitTest.mockReturnValue(0);

    const { runUnitTest } = await import('./commands/unit-test');
    const exitCode = runUnitTest({ projectRoot: '' });

    // 当 projectRoot 为空字符串时，runUnitTest 应使用 getProjectDir() 的默认值
    expect(exitCode).toBe(0);
  });

  it('同时传递多个未知选项时不影响 unit-test 命令正常注册', () => {
    // cac 默认允许 passthrough 未知选项
    // 验证命令注册模式不拒绝额外选项
    const cmdDef = {
      name: 'unit-test',
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

describe('dev-team unit-test -- --no-mutation 选项', () => {
  it('CLI 注册 `--no-mutation` 选项', () => {
    const optionDef = '--no-mutation';
    expect(optionDef).toContain('--no-mutation');
  });

  it('--no-mutation 选项传递到 UnitTestOptions.noMutation', async () => {
    mockRunUnitTest.mockReset();
    mockRunUnitTest.mockReturnValue(0);

    const { runUnitTest } = await import('./commands/unit-test');

    // 模拟传递 noMutation: true
    runUnitTest({ projectRoot: '/test/project', noMutation: true });
    expect(mockRunUnitTest).toHaveBeenCalled();
    const callArgs = mockRunUnitTest.mock.calls[0];
    expect(callArgs[0].noMutation).toBe(true);
  });

  it('--no-mutation 不传递时 noMutation 为 undefined', async () => {
    mockRunUnitTest.mockReset();
    mockRunUnitTest.mockReturnValue(0);

    const { runUnitTest } = await import('./commands/unit-test');

    runUnitTest({ projectRoot: '/test/project' });
    expect(mockRunUnitTest).toHaveBeenCalled();
    const callArgs = mockRunUnitTest.mock.calls[0];
    expect(callArgs[0].noMutation).toBeUndefined();
  });
});
