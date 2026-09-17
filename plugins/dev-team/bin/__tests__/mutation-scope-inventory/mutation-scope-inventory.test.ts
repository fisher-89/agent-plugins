/**
 * 集成测试: mutation-scope-inventory
 *
 * 验证 AC-7 的端到端链路: --change CLI 参数 → 清单突变 scope → 净归零去噪。
 * git 仓库 fixture 与 workflow.json 全部真实(不 mock),只 mock 测试执行边界
 * (runTestDetectFrameworks / executePlanEntry / generateSubReport /
 * generateSummaryReport),用真实 HEAD 内容裁判去噪漏斗方向(只向 overstate、不漏删)。
 *
 * @see openspec/changes/workflow-file-inventory/test-design.md — 集成测试
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vite-plus/test';

import { runTestExecution } from '../../src/commands/test-execution';
import type { ExecutionResult } from '../../src/lib/test-runner';
import { type TestPlan } from '../../src/schemas';
import type {
  SourceFileEntry,
  TestExecutionSubReport,
} from '../../src/schemas/test-execution-output.schema';

// ---------------------------------------------------------------------------
// Mocks — 只隔离测试执行边界;git 与文件清单走真实实现
// ---------------------------------------------------------------------------

const mockExecutePlanEntry = vi.fn();
const mockGenerateSubReport = vi.fn();
const mockGenerateSummaryReport = vi.fn();
const mockRunTestDetectFrameworks = vi.fn();

vi.mock('../../src/lib/test-runner', () => ({
  executePlanEntry: (...args: unknown[]) => mockExecutePlanEntry(...args),
}));

vi.mock('../../src/lib/test-report', () => ({
  generateSubReport: (...args: unknown[]) => mockGenerateSubReport(...args),
  generateSummaryReport: (...args: unknown[]) => mockGenerateSummaryReport(...args),
}));

vi.mock('../../src/commands/test-detect-frameworks', () => ({
  runTestDetectFrameworks: (...args: unknown[]) => mockRunTestDetectFrameworks(...args),
}));

// ---------------------------------------------------------------------------
// Helpers(形状与 src/commands/test-execution.test.ts 保持一致)
// ---------------------------------------------------------------------------

function sfe(file: string, overrides: Partial<SourceFileEntry['coverage']> = {}): SourceFileEntry {
  return {
    file,
    coverage: {
      lines: 0,
      branches: null,
      functions: null,
      total_lines: null,
      covered_lines: null,
      total_branches: null,
      covered_branches: null,
      total_functions: null,
      covered_functions: null,
      ...overrides,
    },
  };
}

function makePlanEntry(overrides: Partial<TestPlan> = {}): TestPlan {
  return {
    cwd: '.',
    root: '.',
    framework: 'vitest',
    coverage_format: 'istanbul',
    coverage_output: 'coverage-summary.json',
    mutation_cwd: '.',
    mutation_script: null,
    script: {
      shell: '#!/bin/bash\nset -e\n\nnpx vitest run --coverage --coverage.reporter=json-summary',
      cmd: 'npx vitest run --coverage --coverage.reporter=json-summary',
    },
    ...overrides,
  };
}

function makeExecutionResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
    coverage: null,
    mutation: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    planId: 'vitest',
    reportDir: '/tmp/reports/test/vitest',
    ...overrides,
  };
}

function makeSubReport(overrides: Partial<TestExecutionSubReport> = {}): TestExecutionSubReport {
  return {
    framework: 'vitest',
    root: '.',
    timestamp: '2026-09-17T00:00:00.000Z',
    exit_code: 0,
    duration_ms: 500,
    summary: { total: 1, passed: 1, failed: 0, skipped: 0 },
    error_cases: [],
    test_files: ['src/foo.test.ts'],
    source_files: [sfe('src/foo.ts')],
    coverage: null,
    mutation: null,
    ...overrides,
  };
}

function makeSummaryReport(): Record<string, unknown> {
  return {
    phase: 'test-execution',
    command: 'dev-team test-execution',
    timestamp: '2026-09-17T00:00:00.000Z',
    duration_seconds: 1,
    total: 1,
    passed: 1,
    failed: 0,
    skipped: 0,
    conclusion: 'pass',
    problems: [],
    coverage: null,
  };
}

interface GitProject {
  root: string;
  cleanup: () => void;
}

function runGit(cwd: string, args: string[]): void {
  execFileSync('git', args, { cwd, stdio: 'ignore' });
}

/** 真实 git 仓库 fixture:首次提交含 src/kept.ts 与 src/noisy.ts。 */
function createGitProject(): GitProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-scope-inventory-'));
  fs.mkdirSync(path.join(root, 'src'), { recursive: true });
  runGit(root, ['init']);
  runGit(root, ['config', 'user.email', 'test@example.com']);
  runGit(root, ['config', 'user.name', 'test']);
  // 字节级保真:禁止 autocrlf 改写行尾,保证 CRLF 用例的 HEAD 对比按原字节进行
  runGit(root, ['config', 'core.autocrlf', 'false']);
  runGit(root, ['config', 'commit.gpgsign', 'false']);
  fs.writeFileSync(path.join(root, 'src', 'kept.ts'), 'export const kept = 1;\n', 'utf-8');
  fs.writeFileSync(path.join(root, 'src', 'noisy.ts'), 'export const noisy = 1;\n', 'utf-8');
  runGit(root, ['add', '-A']);
  runGit(root, ['commit', '-m', 'init']);
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

/** 直接写 change 的 workflow.json;files 缺省时构造机制前旧 change。 */
function writeWorkflowJson(
  root: string,
  change: string,
  files?: { written: string[]; deleted: string[] },
): void {
  const changeDir = path.join(root, 'openspec', 'changes', change);
  fs.mkdirSync(changeDir, { recursive: true });
  const doc: Record<string, unknown> = { workflow_type: 'requirement', created: '2026-09-17' };
  if (files !== undefined) {
    doc.files = files;
  }
  fs.writeFileSync(
    path.join(changeDir, 'workflow.json'),
    `${JSON.stringify(doc, null, 2)}\n`,
    'utf-8',
  );
}

function abs(root: string, rel: string): string {
  return path.resolve(root, rel).replace(/\\/g, '/');
}

/** executePlanEntry 收到的 options.mutationDiffFiles。 */
function mutationDiffFilesPassed(): string[] | undefined {
  return mockExecutePlanEntry.mock.calls[0][2].mutationDiffFiles as string[] | undefined;
}

function stubHappyRun(): void {
  mockRunTestDetectFrameworks.mockReturnValue({ detected: [], plan: [makePlanEntry()] });
  mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
  mockGenerateSubReport.mockReturnValue(makeSubReport());
  mockGenerateSummaryReport.mockReturnValue(makeSummaryReport());
}

// ---------------------------------------------------------------------------
// 场景: 真实仓库去噪漏斗
// ---------------------------------------------------------------------------

describe('场景: 真实仓库去噪漏斗', () => {
  let logs: string[] = [];
  let logSpy: Mock;

  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    mockRunTestDetectFrameworks.mockReset();
    logs = [];
    logSpy = vi.spyOn(console, 'log').mockImplementation((...args: unknown[]) => {
      logs.push(args.map(String).join(' '));
    });
  });

  afterEach(() => {
    logSpy.mockRestore();
  });

  it('正向: scope 恰为改写文件与 HEAD 缺席的新文件,net-zero 文件被剔除 (AC-7)', async () => {
    const project = createGitProject();
    try {
      writeWorkflowJson(project.root, 'inv', {
        written: ['src/kept.ts', 'src/noisy.ts', 'src/new-file.ts'],
        deleted: [],
      });
      // 工作区改写 kept.ts (≠ HEAD);noisy.ts 保持与 HEAD 一致;new-file.ts 不落盘
      fs.writeFileSync(
        path.join(project.root, 'src', 'kept.ts'),
        'export const kept = 2;\n',
        'utf-8',
      );
      stubHappyRun();

      const exitCode = await runTestExecution({ projectRoot: project.root, change: 'inv' });

      expect(exitCode).toBe(0);
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      const received = mutationDiffFilesPassed();
      expect(received).toHaveLength(2);
      expect([...received!].sort()).toEqual(
        [abs(project.root, 'src/kept.ts'), abs(project.root, 'src/new-file.ts')].sort(),
      );
      expect(received).not.toContain(abs(project.root, 'src/noisy.ts'));
      // 诊断行改为清单来源前缀,git diff 前缀退场
      expect(logs.some((l) => l.startsWith('mutation scope (change inventory): 2 files'))).toBe(
        true,
      );
      expect(logs.some((l) => l.startsWith('mutation denoise: 1 net-zero files excluded'))).toBe(
        true,
      );
      expect(logs.some((l) => l.startsWith('--mutation-diff-only:'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('边界: written 全部与 HEAD 内容一致 → 空 scope 不报错,突变按空 scope 处理 (AC-7)', async () => {
    const project = createGitProject();
    try {
      writeWorkflowJson(project.root, 'inv', {
        written: ['src/kept.ts', 'src/noisy.ts'],
        deleted: [],
      });
      stubHappyRun();

      const exitCode = await runTestExecution({ projectRoot: project.root, change: 'inv' });

      expect(exitCode).toBe(0);
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      expect(mutationDiffFilesPassed()).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('边界: CRLF 行尾与反斜杠路径形态归一化后参与去噪漏斗 (AC-7)', async () => {
    const project = createGitProject();
    try {
      fs.writeFileSync(path.join(project.root, 'src', 'crlf.ts'), 'a\r\nb\r\n', 'utf-8');
      fs.writeFileSync(
        path.join(project.root, 'src', 'win.ts'),
        'export const win = 1;\n',
        'utf-8',
      );
      runGit(project.root, ['add', '-A']);
      runGit(project.root, ['commit', '-m', 'crlf and win']);
      // 提交后仅改写 win.ts;crlf.ts 工作区字节与 HEAD 完全一致(净归零 → 去噪命中)
      fs.writeFileSync(
        path.join(project.root, 'src', 'win.ts'),
        'export const win = 2;\n',
        'utf-8',
      );
      writeWorkflowJson(project.root, 'inv', {
        written: ['src/crlf.ts', 'src\\win.ts'],
        deleted: [],
      });
      stubHappyRun();

      const exitCode = await runTestExecution({ projectRoot: project.root, change: 'inv' });

      expect(exitCode).toBe(0);
      const received = mutationDiffFilesPassed();
      // crlf.ts 工作区字节 == HEAD → 去噪命中被剔除
      expect(received).not.toContain(abs(project.root, 'src/crlf.ts'));
      // 反斜杠拼写的 win.ts 工作区 ≠ HEAD → 保留,不因路径形态失效
      expect(received).toContain(abs(project.root, 'src/win.ts'));
      expect(received).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// 场景: 入口与守卫
// ---------------------------------------------------------------------------

describe('场景: 入口与守卫', () => {
  let logs: string[] = [];
  let logSpy: Mock;

  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    mockRunTestDetectFrameworks.mockReset();
    logs = [];
    logSpy = vi.spyOn(console, 'log').mockImplementation((...args: unknown[]) => {
      logs.push(args.map(String).join(' '));
    });
  });

  afterEach(() => {
    logSpy.mockRestore();
  });

  it('异常: 机制前旧 change (无 files) 传 change → 硬报错"请重建"且不产出执行报告 (AC-7)', async () => {
    const project = createGitProject();
    try {
      writeWorkflowJson(project.root, 'legacy');
      stubHappyRun();

      await expect(
        runTestExecution({ projectRoot: project.root, change: 'legacy' }),
      ).rejects.toThrow(/请重建/);

      expect(mockExecutePlanEntry).not.toHaveBeenCalled();
      expect(mockGenerateSummaryReport).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('正向: noMutation 与 change 同传 → 惰性解析,旧 change 不误报且不限圈 (AC-7)', async () => {
    const project = createGitProject();
    try {
      writeWorkflowJson(project.root, 'legacy');
      stubHappyRun();

      const exitCode = await runTestExecution({
        projectRoot: project.root,
        change: 'legacy',
        noMutation: true,
      });

      expect(exitCode).toBe(0);
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      expect(mutationDiffFilesPassed()).toBeUndefined();
      // 清单通道未激活:无 scope 诊断行
      expect(logs.some((l) => l.startsWith('mutation scope (change inventory):'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('边界: 不传 change → 突变不限圈,清单通道完全不激活 (AC-7)', async () => {
    const project = createGitProject(); // fixture 不含 openspec/workflow.json
    try {
      stubHappyRun();

      const exitCode = await runTestExecution({ projectRoot: project.root });

      expect(exitCode).toBe(0);
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      expect(mutationDiffFilesPassed()).toBeUndefined();
    } finally {
      project.cleanup();
    }
  });

  it('正向: written 含测试文件路径 → 反推同位源文件并入 scope (AC-7)', async () => {
    const project = createGitProject();
    try {
      // foo.test.ts 与反推出的 foo.ts 均不在磁盘且不在 HEAD → 双双保留(overstate 方向)
      writeWorkflowJson(project.root, 'inv', { written: ['src/foo.test.ts'], deleted: [] });
      stubHappyRun();

      const exitCode = await runTestExecution({ projectRoot: project.root, change: 'inv' });

      expect(exitCode).toBe(0);
      const received = mutationDiffFilesPassed();
      expect(received).toHaveLength(2);
      expect(received).toContain(abs(project.root, 'src/foo.test.ts'));
      expect(received).toContain(abs(project.root, 'src/foo.ts'));
    } finally {
      project.cleanup();
    }
  });
});
