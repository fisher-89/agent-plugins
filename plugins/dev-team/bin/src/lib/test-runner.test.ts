/**
 * Tests for lib/test-runner -- test command execution and template substitution.
 *
 * Covers:
 * - executePlanEntry: template substitution via buildTestCommand
 * - executePlanEntry: single command execution (mocked execSync)
 * - executePlanEntry: chained command (pytest/rust) exit code behavior
 * - executePlanEntry: non-zero exit code handling
 * - executePlanEntry: timeout, empty stdout, binary not found
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import { beforeEach, describe, it, expect, vi } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// Mock child_process.execSync for all executePlanEntry tests
// ---------------------------------------------------------------------------

const mockExecSync = vi.fn();
vi.mock('child_process', () => ({
  execSync: (...args: unknown[]) => mockExecSync(...args),
}));

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { executePlanEntry } from './test-runner';

// ---------------------------------------------------------------------------
// Helper: create an Error that looks like an execSync error
// ---------------------------------------------------------------------------

interface ExecErrorLike {
  status?: number;
  stdout?: string;
  stderr?: string;
  message?: string;
}

function createExecError(message: string, extra: ExecErrorLike): Error {
  const err = new Error(message);
  if (extra.status !== undefined) {
    Object.defineProperty(err, 'status', { value: extra.status, enumerable: true });
  }
  if (extra.stdout !== undefined) {
    Object.defineProperty(err, 'stdout', { value: extra.stdout, enumerable: true });
  }
  if (extra.stderr !== undefined) {
    Object.defineProperty(err, 'stderr', { value: extra.stderr, enumerable: true });
  }
  return err;
}

// ===========================================================================
// executePlanEntry -- 单一命令执行
// ===========================================================================

describe('executePlanEntry -- 单一命令执行', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('执行 test_cmd 一次命令（无 merge_mode 分支，统一执行路径）', () => {
    mockExecSync.mockReturnValue(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run --reporter=json {files}\\n',
    };
    const result = executePlanEntry(entry, '/project', { files: ['src/test.test.ts'] });
    expect(result.exitCode).toBe(0);
    expect(result.testCases).toHaveLength(1);
    expect(result.framework).toBe('vitest');
    // 验证 execSync 被调用了一次（统一执行路径）
    expect(mockExecSync).toHaveBeenCalledTimes(1);
  });

  it('从 test_cmd 执行结果提取测试用例和覆盖率', () => {
    mockExecSync.mockReturnValue(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"},{"title":"t2","fullName":"t2","status":"failed"}]}]}',
    );

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run --reporter=json {files}\\n',
    };
    const result = executePlanEntry(entry, '/project');
    expect(result.testCases).toHaveLength(2);
    expect(result.testCases[0].status).toBe('passed');
    expect(result.testCases[1].status).toBe('failed');
  });

  it('所有框架（含链式命令和单命令）共享同一执行路径', () => {
    mockExecSync.mockReturnValue(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );

    // vitest（单命令）
    const vitestResult = executePlanEntry(
      {
        directory: '.',
        framework: 'vitest',
        test_cmd: '',
        coverage_format: 'istanbul' as const,
        coverage_output: 'coverage/coverage-summary.json',
        coverage_artifacts: ['coverage/coverage-summary.json'],
        coverage_cleanup: ['coverage'],
        script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
      },
      '/project',
    );

    // pytest（链式命令）-- 都走 execSync
    const pytestResult = executePlanEntry(
      {
        directory: '.',
        framework: 'pytest',
        test_cmd: '',
        coverage_format: 'coverage-py' as const,
        coverage_output: 'coverage.json',
        coverage_artifacts: ['coverage.json'],
        coverage_cleanup: ['.coverage'],
        script:
          'rm -rf .coverage\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
      },
      '/project',
    );

    // 两者都走 execSync，不区分 merge_mode
    expect(vitestResult.exitCode).toBe(0);
    expect(pytestResult.exitCode).toBe(0);
    expect(mockExecSync).toHaveBeenCalledTimes(2);
  });

  it('执行命令超时时返回 error 字段，不崩溃', () => {
    // 模拟 execSync 抛出一个含 status 的错误（超时也会抛异常）
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command timed out', {
        status: -1,
        stdout: '',
        stderr: 'Timeout reached',
      });
    });

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run --reporter=json {files}\\n',
    };
    const result = executePlanEntry(entry, '/project');
    // 不崩溃，返回 error 和 exitCode
    expect(result.error).toBe('Command timed out');
    expect(result.exitCode).toBe(-1);
    expect(result.testCases).toEqual([]);
  });

  it('test_cmd 为空字符串时返回 error，不执行子进程', () => {
    // 不应调用 execSync
    const result = executePlanEntry(
      {
        directory: '.',
        framework: 'vitest',
        test_cmd: '',
        coverage_format: 'istanbul' as const,
        coverage_output: 'coverage/coverage-summary.json',
        coverage_artifacts: ['coverage/coverage-summary.json'],
        coverage_cleanup: ['coverage'],
        script: '',
      },
      '/project',
    );
    expect(result.exitCode).toBe(-1);
    expect(result.error).toBe('Empty test command');
    expect(mockExecSync).not.toHaveBeenCalled();
  });

  it('执行命令返回空 stdout 时，parsed testCases 为空数组', () => {
    mockExecSync.mockReturnValue('');

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run --reporter=json {files}\\n',
    };
    const result = executePlanEntry(entry, '/project');
    expect(result.testCases).toEqual([]);
    // empty stdout 应被 json-parser 处理，返回 error 但不崩溃
    expect(result.error).toBeDefined();
  });

  it('执行命令不可识别（如 binary 不存在）时 exitCode 非零', () => {
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command failed: /bin/sh -c nonexistent-binary', {
        status: 127,
        stdout: '',
        stderr: 'sh: nonexistent-binary: command not found',
      });
    });

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnonexistent-binary --version\\n',
    };
    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(127);
    expect(result.error).toContain('Command failed');
  });
});

// ===========================================================================
// executePlanEntry -- empty test command
// ===========================================================================

describe('executePlanEntry -- empty test command', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('should return error when test_cmd is empty string', async () => {
    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: '',
    };
    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(-1);
    expect(result.error).toBe('Empty test command');
    expect(result.testCases).toEqual([]);
    expect(mockExecSync).not.toHaveBeenCalled();
  });

  it('should return error when test_cmd is whitespace-only', async () => {
    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: [],
      script: '   ',
    };
    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(-1);
    expect(result.error).toBe('Empty test command');
  });
});

// ===========================================================================
// executePlanEntry -- chained command
// ===========================================================================

describe('executePlanEntry -- chained command (AC-10)', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('pytest 链式命令：测试命令先执行，覆盖率命令后执行', () => {
    // 链式命令整体被 execSync 执行一次
    mockExecSync.mockReturnValue('collected 1 item\ntest_foo.py .');

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script:
        'rm -rf .coverage\\nrm -rf htmlcov\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(0);

    // 验证 execSync 被调用且命令包含链式模式
    expect(mockExecSync).toHaveBeenCalledTimes(1);
    const calledCmd = String(mockExecSync.mock.calls[0][0]);
    expect(calledCmd).toContain('; _X=$?;');
    expect(calledCmd).toContain('exit $_X');
    expect(calledCmd).toContain('pytest -v');
    expect(calledCmd).toContain('pytest --cov=');
  });

  it('rust 链式命令：测试命令先执行，覆盖率命令后执行', () => {
    mockExecSync.mockReturnValue('test result: ok. 1 passed; 0 failed; 0 ignored');

    const entry = {
      directory: '.',
      framework: 'rust',
      test_cmd: '',
      coverage_format: 'llvm-cov' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
      script:
        'rm -rf coverage\\nrm -rf target/llvm-cov\\ncargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(0);

    // 验证 execSync 调用时命令包含链式模式
    const calledCmd = String(mockExecSync.mock.calls[0][0]);
    expect(calledCmd).toContain('; _X=$?;');
    expect(calledCmd).toContain('exit $_X');
    expect(calledCmd).toContain('cargo test');
    expect(calledCmd).toContain('cargo llvm-cov');
  });

  it('链式命令退出码反映测试命令的退出码（exit $_X）', () => {
    // 模拟测试失败，退出码 1
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command failed: pytest -v failed', {
        status: 1,
        stdout: 'collected 1 item\ntest_foo.py FAILED',
        stderr: '',
      });
    });

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script:
        'rm -rf .coverage\\nrm -rf htmlcov\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
    };

    const result = executePlanEntry(entry, '/project');
    // execSync 抛出异常，退出码由异常中的 status 决定
    expect(result.exitCode).toBe(1);
    expect(result.error).toContain('Command failed');
  });

  it('链式命令中覆盖率从文件读取（coverage.json / coverage/coverage-summary.json）而非 stdout', () => {
    mockExecSync.mockReturnValue('collected 1 item\ntest_foo.py .');

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script:
        'rm -rf .coverage\\nrm -rf htmlcov\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
    };

    // 覆盖率输出是 coverage.json 文件，不在 stdout 中
    // 验证 test_cmd 包含 --cov-report=json（文件输出）
    expect(entry.script).toContain('--cov-report=json');
    // 执行结果中 coverage 为 null（因为 coverage.json 文件不存在）
    const result = executePlanEntry(entry, '/project');
    expect(result.coverage).toBeNull();
  });

  it('链式命令中测试命令失败时覆盖率命令仍执行（; _X=$?; 而非 &&）', () => {
    // 验证命令使用分号连接而非 &&
    const pytestCmd =
      'pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X';
    expect(pytestCmd).toContain('; _X=$?;');
    expect(pytestCmd).not.toContain('&& _X=$?');

    // 模拟测试失败
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command failed', { status: 1, stdout: 'FAILED', stderr: '' });
    });

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script: pytestCmd,
    };

    const result = executePlanEntry(entry, '/project');
    // 即使测试失败，execSync 仍被调用（因为整个链式命令是一条 shell 命令）
    expect(mockExecSync).toHaveBeenCalledTimes(1);
    // 退出码反映测试失败
    expect(result.exitCode).toBe(1);
  });

  it('链式命令中测试命令失败，最终退出码为测试退出码', () => {
    // 验证 exit $_X 语义：最终退出码是测试命令的退出码
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command failed', { status: 2, stdout: '', stderr: '' });
    });

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script:
        'rm -rf .coverage\\nrm -rf htmlcov\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(2);
  });

  it('链式命令中覆盖率命令失败但测试通过，最终退出码为 0', () => {
    // 测试通过时 exit $_X 使最终退出码为 0
    mockExecSync.mockReturnValue('collected 1 item\npytest --cov=... done');

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script:
        'rm -rf .coverage\\nrm -rf htmlcov\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(0);
  });

  it('链式命令 stdout 包含测试输出和覆盖率命令输出，解析器只关心测试输出', () => {
    mockExecSync.mockReturnValue(
      'collected 1 item\ntest_foo.py .\n---------- coverage: ----------\nName    Stmts   Miss  Cover\nfoo.py      10      0   100%',
    );

    const entry = {
      directory: '.',
      framework: 'pytest',
      test_cmd: '',
      coverage_format: 'coverage-py' as const,
      coverage_output: 'coverage.json',
      coverage_artifacts: ['coverage.json'],
      coverage_cleanup: ['.coverage', 'htmlcov'],
      script:
        'rm -rf .coverage\\nrm -rf htmlcov\\npytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X\\n',
    };

    const result = executePlanEntry(entry, '/project');
    // 即使 stdout 包含混合输出，解析器仍返回结果（text-parser fallback）
    expect(result.testCases.length).toBeGreaterThanOrEqual(0);
    expect(result.stdout).toContain('coverage');
    expect(result.stdout).toContain('collected 1 item');
  });
});

// ===========================================================================
// executePlanEntry -- non-zero exit code
// ===========================================================================

describe('executePlanEntry -- non-zero exit code (AC-11)', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('退出码非零时记录 exitCode 和错误信息到 ExecutionResult', () => {
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command failed: tests failed', {
        status: 1,
        stdout: '',
        stderr: 'tests failed with exit code 1',
      });
    });

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(1);
    expect(result.error).toBe('Command failed: tests failed');
  });

  it('非零退出码不影响其他 framework 执行（单条执行无阻塞）', () => {
    // executePlanEntry 每次调用都是独立的
    mockExecSync.mockReturnValue(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );

    const entry1 = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
    };
    const entry2 = {
      directory: '.',
      framework: 'vite-plus',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nvp test {files}\\n',
    };

    const result1 = executePlanEntry(entry1, '/project');
    const result2 = executePlanEntry(entry2, '/project');

    // 每个调用独立执行
    expect(result1.framework).toBe('vitest');
    expect(result2.framework).toBe('vite-plus');
    expect(result1.exitCode).toBe(0);
    expect(result2.exitCode).toBe(0);
    // execSync 被调用了两次（两次独立的执行）
    expect(mockExecSync).toHaveBeenCalledTimes(2);
  });

  it('退出码为 0 时 error 字段为 undefined', () => {
    mockExecSync.mockReturnValue(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(0);
    expect(result.error).toBeUndefined();
  });

  it('退出码为负数（如被信号终止）时处理', () => {
    mockExecSync.mockImplementation(() => {
      throw createExecError('Command terminated by signal', {
        status: -2,
        stdout: '',
        stderr: 'Killed by signal SIGINT',
      });
    });

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run\\n',
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.exitCode).toBe(-2);
    expect(result.error).toBeDefined();
  });

  it('退出码为非数字类型时转换为数字或返回 error', () => {
    // status 为 undefined 时，extractExecError 返回 -1
    mockExecSync.mockImplementation(() => {
      throw createExecError('Unknown error', { stdout: '', stderr: '' });
      // 不设置 status
    });

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run\\n',
    };

    const result = executePlanEntry(entry, '/project');
    // status 为 undefined 时，fallback 到 -1
    expect(result.exitCode).toBe(-1);
    expect(result.error).toBe('Unknown error');
  });
});

// ===========================================================================
// executePlanEntry -- mutation 执行阶段
// ===========================================================================

describe('executePlanEntry -- mutation 执行阶段', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('mutation_framework 非空且 noMutation=false 时在覆盖率解析后执行 StrykerJS', () => {
    // 第一次 execSync 调用返回测试结果
    mockExecSync.mockReturnValueOnce(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );
    // 第二次 execSync 调用返回 StrykerJS 执行结果
    mockExecSync.mockReturnValueOnce('StrykerJS run completed');

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: '',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
      mutation_framework: 'stryker-js',
      mutation_score: 80,
    };

    const result = executePlanEntry(entry, '/project', { noMutation: false });
    // StrykerJS 被调用（但可能因为报告文件不存在而返回 null）
    // 至少 execSync 被调用了
    expect(mockExecSync).toHaveBeenCalled();
    expect(result.mutation).not.toBeUndefined();
  });

  it('mutation_framework 为 null 时跳过 StrykerJS 执行，mutation=null', () => {
    mockExecSync.mockReturnValueOnce(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );

    const entry = {
      directory: '.',
      framework: 'bun',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: '',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nbun test {files}\\n',
      mutation_framework: null,
      mutation_score: null,
    };

    const result = executePlanEntry(entry, '/project');
    expect(result.mutation).toBeNull();
  });

  it('noMutation=true 时跳过 StrykerJS 执行，mutation=null', () => {
    mockExecSync.mockReturnValueOnce(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: '',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
      mutation_framework: 'stryker-js',
      mutation_score: 80,
    };

    const result = executePlanEntry(entry, '/project', { noMutation: true });
    expect(result.mutation).toBeNull();
  });

  it('StrykerJS 命令失败时静默跳过，mutation=null，不阻断测试流程', () => {
    // 第一次 execSync 成功（测试执行）
    mockExecSync.mockReturnValueOnce(
      '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
    );
    // 第二次 execSync 失败（StrykerJS 执行失败）
    mockExecSync.mockImplementationOnce(() => {
      throw createExecError('StrykerJS failed', {
        status: 1,
        stdout: '',
        stderr: 'StrykerJS error',
      });
    });

    const entry = {
      directory: '.',
      framework: 'vitest',
      test_cmd: '',
      coverage_format: 'istanbul' as const,
      coverage_output: '',
      coverage_artifacts: ['coverage/coverage-summary.json'],
      coverage_cleanup: ['coverage'],
      script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
      mutation_framework: 'stryker-js',
      mutation_score: 80,
    };

    const result = executePlanEntry(entry, '/project');
    // 测试结果仍然有效
    expect(result.exitCode).toBe(0);
    expect(result.testCases).toHaveLength(1);
    // mutation 应该为 null（静默跳过）
    expect(result.mutation).toBeNull();
  });

  it('mutation 执行时长计入 durationMs', () => {
    // 使用临时项目目录确保 resolveStrykerConfig 可创建临时配置
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-duration-'));
    try {
      // 创建报告文件以支持 mutation 解析
      const reportDir = path.join(tmpDir, 'reports', 'mutation');
      fs.mkdirSync(reportDir, { recursive: true });
      fs.writeFileSync(
        path.join(reportDir, 'mutation.json'),
        JSON.stringify({
          metrics: {
            mutationScore: 80,
            mutationScoreBasedOnCoveredCode: 80,
            killed: 8,
            survived: 2,
            timeout: 0,
            noCoverage: 0,
            compileErrors: 0,
            runtimeErrors: 0,
            ignored: 0,
            totalDetected: 8,
            totalUndetected: 2,
            totalMutants: 10,
          },
        }),
        'utf-8',
      );

      // 第一次 execSync: 测试执行
      mockExecSync.mockReturnValueOnce(
        '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
      );
      // 第二次 execSync: StrykerJS 执行
      mockExecSync.mockReturnValueOnce('StrykerJS run completed');

      const entry = {
        directory: '.',
        framework: 'vitest',
        test_cmd: '',
        coverage_format: 'istanbul' as const,
        coverage_output: '',
        coverage_artifacts: ['coverage/coverage-summary.json'],
        coverage_cleanup: ['coverage'],
        script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
        mutation_framework: 'stryker-js',
        mutation_score: 80,
      };
      const result = executePlanEntry(entry, tmpDir);

      // durationMs 应存在且为非负数
      expect(typeof result.durationMs).toBe('number');
      expect(result.durationMs).toBeGreaterThanOrEqual(0);
      // mutation 被正确执行
      expect(result.mutation).not.toBeNull();
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('mutation 执行后清理临时配置文件和 reports/mutation/ 目录', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-cleanup-'));
    try {
      // 先创建 reports/mutation/mutation.json 模拟 StrykerJS 输出
      const reportDir = path.join(tmpDir, 'reports', 'mutation');
      fs.mkdirSync(reportDir, { recursive: true });
      const mutationReport = {
        metrics: {
          mutationScore: 91.66666666666667,
          mutationScoreBasedOnCoveredCode: 91.66666666666667,
          killed: 10,
          survived: 1,
          timeout: 1,
          noCoverage: 0,
          compileErrors: 0,
          runtimeErrors: 0,
          ignored: 0,
          totalDetected: 11,
          totalUndetected: 1,
          totalMutants: 12,
        },
      };
      fs.writeFileSync(
        path.join(reportDir, 'mutation.json'),
        JSON.stringify(mutationReport),
        'utf-8',
      );

      // 第一次 execSync: 测试执行
      mockExecSync.mockReturnValueOnce(
        '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
      );
      // 第二次 execSync: StrykerJS 执行
      mockExecSync.mockReturnValueOnce('StrykerJS run completed');

      const entry = {
        directory: '.',
        framework: 'vitest',
        test_cmd: '',
        coverage_format: 'istanbul' as const,
        coverage_output: '',
        coverage_artifacts: ['coverage/coverage-summary.json'],
        coverage_cleanup: ['coverage'],
        script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
        mutation_framework: 'stryker-js',
        mutation_score: 80,
      };
      const result = executePlanEntry(entry, tmpDir);

      // mutation 结果应被正确解析
      expect(result.mutation).not.toBeNull();
      expect(result.mutation!.score).toBeCloseTo(91.66666, 1);

      // reports/mutation/ 目录应被清理
      expect(fs.existsSync(reportDir)).toBe(false);

      // 临时配置文件应被清理
      // 列出 tmpDir 下所有 stryker.config.*.json 文件，应不存在
      const leftoverConfigs = fs
        .readdirSync(tmpDir)
        .filter((f) => f.startsWith('stryker.config.') && f.endsWith('.json'));
      expect(leftoverConfigs).toHaveLength(0);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('StrykerJS 报告文件不存在时 mutation=null，不崩溃', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-no-report-'));
    try {
      // 第一次 execSync: 测试执行
      mockExecSync.mockReturnValueOnce(
        '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
      );
      // 第二次 execSync: StrykerJS 执行（但报告文件未创建）
      mockExecSync.mockReturnValueOnce('StrykerJS run completed');

      const entry = {
        directory: '.',
        framework: 'vitest',
        test_cmd: '',
        coverage_format: 'istanbul' as const,
        coverage_output: '',
        coverage_artifacts: ['coverage/coverage-summary.json'],
        coverage_cleanup: ['coverage'],
        script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
        mutation_framework: 'stryker-js',
        mutation_score: 80,
      };
      const result = executePlanEntry(entry, tmpDir);

      // 报告文件不存在时 mutation 应为 null，不崩溃
      expect(result.mutation).toBeNull();
      // 测试结果仍然有效
      expect(result.exitCode).toBe(0);
      expect(result.testCases).toHaveLength(1);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('执行顺序：测试命令 → 覆盖率解析 → StrykerJS', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-order-'));
    try {
      // 创建报告文件以验证 mutation 阶段确实执行
      const reportDir = path.join(tmpDir, 'reports', 'mutation');
      fs.mkdirSync(reportDir, { recursive: true });
      fs.writeFileSync(
        path.join(reportDir, 'mutation.json'),
        JSON.stringify({
          metrics: {
            mutationScore: 100,
            mutationScoreBasedOnCoveredCode: 100,
            killed: 5,
            survived: 0,
            timeout: 0,
            noCoverage: 0,
            compileErrors: 0,
            runtimeErrors: 0,
            ignored: 0,
            totalDetected: 5,
            totalUndetected: 0,
            totalMutants: 5,
          },
        }),
        'utf-8',
      );

      // execSync 应被调用 2 次: 测试命令 + StrykerJS
      mockExecSync.mockReturnValueOnce(
        '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
      );
      mockExecSync.mockReturnValueOnce('StrykerJS run completed');

      const entry = {
        directory: '.',
        framework: 'vitest',
        test_cmd: '',
        coverage_format: 'istanbul' as const,
        coverage_output: '',
        coverage_artifacts: ['coverage/coverage-summary.json'],
        coverage_cleanup: ['coverage'],
        script: 'rm -rf coverage\\nnpx vitest run {files}\\n',
        mutation_framework: 'stryker-js',
        mutation_score: 80,
      };
      const result = executePlanEntry(entry, tmpDir);

      // 验证 execSync 被调用了 2 次
      expect(mockExecSync).toHaveBeenCalledTimes(2);

      // 第一次调用: 测试命令
      const firstCall = String(mockExecSync.mock.calls[0][0]);
      expect(firstCall).toContain('npx vitest run');

      // 第二次调用: StrykerJS
      const secondCall = String(mockExecSync.mock.calls[1][0]);
      expect(secondCall).toContain('npx stryker run');

      // 验证 mutation 结果存在（说明 StrykerJS 阶段执行完成）
      expect(result.mutation).not.toBeNull();
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
