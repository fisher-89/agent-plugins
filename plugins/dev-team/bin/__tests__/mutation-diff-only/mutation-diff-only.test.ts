/**
 * 集成测试: --mutation-diff-only CLI 参数端到端
 *
 * 覆盖范围:
 * - mutationDiffOnly=true: 真实 git 仓库中识别变更文件，注入 executePlanEntry
 * - mutationDiffOnly=true + 多 framework: 每个 framework 收到相同 mutationDiffFiles
 * - mutationDiffOnly=false: 跳过 git diff 解析，mutationDiffFiles 为 undefined
 * - mutationDiffOnly=true + --change: 报告写入 openspec/changes/<name>/reports/
 * - mutationDiffOnly + --framework 过滤: 仅匹配的 framework 收到 mutationDiffFiles
 *
 * 外部边界:
 * - 真实 git 仓库 (simple-git 对接真实 git binary)
 * - 真实文件系统 (config.json / 报告文件 / 源文件)
 * - 真实 runTestDetectFrameworks (自动扫描项目文件)
 * - 真实 generateSubReport / generateSummaryReport (写入实际报告文件)
 * - 仅 mock executePlanEntry (避免执行真实 shell 测试命令)
 *
 * @see plugins/dev-team/bin/src/commands/test-execution.ts
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runTestExecution } from '../../src/commands/test-execution';
import type { ExecutionResult } from '../../src/lib/test-runner';
import type { TestExecutionSubReport } from '../../src/schemas';

// ---------------------------------------------------------------------------
// 仅 mock executePlanEntry —— 其余依赖全部走真实实现
// ---------------------------------------------------------------------------

const mockExecutePlanEntry = vi.fn();

vi.mock('../../src/lib/test-runner', () => ({
  executePlanEntry: (...args: unknown[]) => mockExecutePlanEntry(...args),
}));

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

/**
 * 创建一个拥有真实 git 仓库的临时项目:
 * 1. 写入所有源文件 (包括后续需要修改的文件)
 * 2. 初始化 git 仓库并做首次 commit
 * 3. 写入 openspec/config.json (首次 commit 之后修改以产生 diff)
 * 4. 修改 dirtyFiles 中指定的文件产生 dirty working tree
 *
 * 注意: 所有 dirtyFiles 必须先存在于首次 commit 中再修改，
 * 否则 git diff HEAD --name-only 不会列出未跟踪文件。
 */
function createProjectWithGit(
  options: {
    framework?: string;
    dirtyFiles?: string[];
  } = {},
): TempProject {
  const { framework = 'vitest', dirtyFiles = [] } = options;

  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-diff-it-'));
  const execSync = require('child_process').execSync;
  const runGit = (cmd: string) => execSync(cmd, { cwd: root, encoding: 'utf-8', stdio: 'pipe' });

  // 1. 初始化 git 仓库
  runGit('git init');
  runGit('git config user.email "test@test.test"');
  runGit('git config user.name "Test"');

  // 2. 创建所有源文件（包括后续要修改的 dirty files）
  const srcDir = path.join(root, 'src');
  fs.mkdirSync(srcDir, { recursive: true });
  fs.writeFileSync(path.join(srcDir, 'foo.test.ts'), '// test file', 'utf-8');
  fs.writeFileSync(path.join(srcDir, 'foo.ts'), 'export const x = 1;', 'utf-8');
  fs.writeFileSync(path.join(srcDir, 'bar.ts'), 'export const y = 2;', 'utf-8');

  // 为 dirty files 创建初始文件（确保它们被 git track）
  for (const file of dirtyFiles) {
    const filePath = path.join(root, file);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    // 如果文件还未创建，写入初始内容
    if (!fs.existsSync(filePath)) {
      fs.writeFileSync(filePath, `// initial content for ${file}`, 'utf-8');
    }
  }

  const libDir = path.join(root, 'lib');
  if (!fs.existsSync(libDir)) {
    fs.mkdirSync(libDir, { recursive: true });
  }

  // 3. 创建 openspec/config.json
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(
      {
        schema: 'spec-driven',
        tests: [{ root: '.', framework, includes: ['**/*.{test,spec}.{ts,tsx,js,jsx}'] }],
      },
      null,
      2,
    ),
    'utf-8',
  );

  // 4. 首次 commit —— 所有文件进入 HEAD
  runGit('git add -A');
  runGit('git commit -m "initial commit"');

  // 5. 修改 dirtyFiles 中的文件，使 working tree 变脏
  for (const file of dirtyFiles) {
    const filePath = path.join(root, file);
    fs.appendFileSync(filePath, '\n// modified for mutation-diff-only test', 'utf-8');
  }

  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function makeExecutionResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

/**
 * 子报告路径: <reportsDir>/<framework>.json
 * （derivePlanId('.', framework) → `${framework}.json`）
 */
function readSubReport(reportsDir: string, framework: string): TestExecutionSubReport | null {
  const subPath = path.join(reportsDir, `${framework}.json`);
  if (!fs.existsSync(subPath)) return null;
  return JSON.parse(fs.readFileSync(subPath, 'utf-8'));
}

/**
 * 汇总报告路径: <reportsDir>/../test-execution.json
 * （generateSummaryReport 写回到 reportsDir 的父目录）
 */
function readSummaryReport(reportsDir: string): Record<string, unknown> | null {
  const summaryPath = path.join(reportsDir, '..', 'test-execution.json');
  if (!fs.existsSync(summaryPath)) return null;
  return JSON.parse(fs.readFileSync(summaryPath, 'utf-8'));
}

function makeAbsoluteFilePath(relativePath: string, projectRoot: string): string {
  return path.join(projectRoot, relativePath).replace(/\\/g, '/');
}

// ===========================================================================
// mutationDiffOnly=true: 真实 git diff 识别变更文件
// ===========================================================================

describe('--mutation-diff-only 真实 git diff', () => {
  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('mutationDiffOnly=true 时，真实 git diff 变更文件应注入 executePlanEntry', async () => {
    const dirtyFiles = ['src/foo.ts', 'src/bar.ts'];
    const project = createProjectWithGit({ dirtyFiles });
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
      });

      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      const options = mockExecutePlanEntry.mock.calls[0][2];
      // 两个文件都在首次 commit 中存在且被修改
      expect(options.mutationDiffFiles).toEqual(
        expect.arrayContaining(dirtyFiles.map((file) => makeAbsoluteFilePath(file, project.root))),
      );
      expect(options.mutationDiffFiles).toHaveLength(2);
    } finally {
      project.cleanup();
    }
  });

  it('无变更文件时 mutationDiffFiles 为空数组', async () => {
    const project = createProjectWithGit();
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
      });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ mutationDiffFiles: [] }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('仅修改一个文件时 mutationDiffFiles 仅包含该文件', async () => {
    const project = createProjectWithGit({
      dirtyFiles: ['src/foo.ts'],
    });
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
      });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({
          mutationDiffFiles: [makeAbsoluteFilePath('src/foo.ts', project.root)],
        }),
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// mutationDiffOnly=true: 真实报告文件生成
// ===========================================================================

describe('--mutation-diff-only 报告文件生成', () => {
  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('mutationDiffOnly=true 时，子报告和汇总报告应写入真实文件', async () => {
    const project = createProjectWithGit({
      dirtyFiles: ['src/foo.ts'],
    });
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
      });

      // 子报告: <root>/reports/test-execution/vitest.json
      const reportsDir = path.join(project.root, 'reports', 'test-execution');
      const subReport = readSubReport(reportsDir, 'vitest');
      expect(subReport).not.toBeNull();
      expect(subReport?.framework).toBe('vitest');
      expect(subReport?.summary?.total).toBe(1);

      // 汇总报告: <root>/reports/test-execution.json (reportsDir/..)
      const summaryReport = readSummaryReport(reportsDir);
      expect(summaryReport).not.toBeNull();
      expect(summaryReport!.phase).toBe('test-execution');
      expect(summaryReport!.total).toBe(1);
    } finally {
      project.cleanup();
    }
  });

  it('mutationDiffOnly=true + --change 时，报告写入 change 专属目录', async () => {
    const project = createProjectWithGit({
      dirtyFiles: ['src/foo.ts'],
    });
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
        change: 'my-feature',
      });

      // 子报告: openspec/changes/my-feature/reports/test-execution/vitest.json
      const reportsDir = path.join(
        project.root,
        'openspec',
        'changes',
        'my-feature',
        'reports',
        'test-execution',
      );
      const subReport = readSubReport(reportsDir, 'vitest');
      expect(subReport).not.toBeNull();

      // 汇总报告: openspec/changes/my-feature/reports/test-execution.json
      const summaryReport = readSummaryReport(reportsDir);
      expect(summaryReport).not.toBeNull();
      expect(summaryReport!.command).toBe('dev-team test-execution');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// mutationDiffOnly=false: 跳过 git diff
// ===========================================================================

describe('--mutation-diff-only=false', () => {
  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('mutationDiffOnly=false 时 mutationDiffFiles 为 undefined（即使有 dirty files）', async () => {
    const project = createProjectWithGit({
      dirtyFiles: ['src/foo.ts', 'src/bar.ts'],
    });
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: false,
      });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ mutationDiffFiles: undefined }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('不传 mutationDiffOnly 时默认 mutationDiffFiles 为 undefined', async () => {
    const project = createProjectWithGit({
      dirtyFiles: ['src/foo.ts'],
    });
    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      await runTestExecution({ projectRoot: project.root });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ mutationDiffFiles: undefined }),
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// mutationDiffOnly=true + 多 framework
// ===========================================================================

describe('--mutation-diff-only + 多 framework', () => {
  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('每个 framework 收到相同的 mutationDiffFiles', async () => {
    const dirtyFiles = ['src/foo.ts', 'src/bar.ts'];
    const project = createProjectWithGit({ framework: 'vitest', dirtyFiles });

    // 添加 pytest 测试文件 + override 配置以触发第二个 framework
    const testsDir = path.join(project.root, 'tests');
    fs.mkdirSync(testsDir, { recursive: true });
    fs.writeFileSync(path.join(testsDir, 'test_basic.py'), '# test placeholder', 'utf-8');

    const configPath = path.join(project.root, 'openspec', 'config.json');
    fs.writeFileSync(
      configPath,
      JSON.stringify(
        {
          schema: 'spec-driven',
          tests: [
            { root: '.', framework: 'vitest', includes: ['**/*.{test,spec}.{ts,tsx,js,jsx}'] },
            { root: 'tests', framework: 'pytest', includes: ['**/test_*.py'] },
          ],
        },
        null,
        2,
      ),
      'utf-8',
    );

    // 提交新增的 pytest 配置，然后再次修改 dirty files
    const execSync = require('child_process').execSync;
    execSync('git add -A && git commit -m "add pytest config"', {
      cwd: project.root,
      encoding: 'utf-8',
      stdio: 'pipe',
    });
    // 再次修改文件使 working tree 变脏
    fs.appendFileSync(path.join(project.root, 'src/foo.ts'), '\n// round 2', 'utf-8');
    fs.appendFileSync(path.join(project.root, 'src/bar.ts'), '\n// round 2', 'utf-8');

    try {
      mockExecutePlanEntry
        .mockReturnValueOnce(makeExecutionResult({ framework: 'vitest' }))
        .mockReturnValueOnce(makeExecutionResult({ framework: 'pytest' }));

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
      });

      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
      // vitest 和 pytest 收到相同的 mutationDiffFiles
      const diffFiles = dirtyFiles.map((file) => makeAbsoluteFilePath(file, project.root));
      expect(mockExecutePlanEntry).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({ framework: 'vitest' }),
        project.root,
        expect.objectContaining({ mutationDiffFiles: expect.arrayContaining(diffFiles) }),
      );
      expect(mockExecutePlanEntry).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({ framework: 'pytest' }),
        project.root,
        expect.objectContaining({ mutationDiffFiles: expect.arrayContaining(diffFiles) }),
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// mutationDiffOnly=true + --framework 过滤
// ===========================================================================

describe('--mutation-diff-only + --framework', () => {
  beforeEach(() => {
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('--framework 过滤时仅匹配的 framework 执行，但仍收到 mutationDiffFiles', async () => {
    const project = createProjectWithGit({
      framework: 'vitest',
      dirtyFiles: ['src/foo.ts'],
    });

    // 添加 pytest 配置
    const testsDir = path.join(project.root, 'tests');
    fs.mkdirSync(testsDir, { recursive: true });
    fs.writeFileSync(path.join(testsDir, 'test_basic.py'), '# test placeholder', 'utf-8');

    const configPath = path.join(project.root, 'openspec', 'config.json');
    fs.writeFileSync(
      configPath,
      JSON.stringify(
        {
          schema: 'spec-driven',
          tests: [
            { root: '.', framework: 'vitest', includes: ['**/*.{test,spec}.{ts,tsx,js,jsx}'] },
            { root: 'tests', framework: 'pytest', includes: ['**/test_*.py'] },
          ],
        },
        null,
        2,
      ),
      'utf-8',
    );

    const execSync = require('child_process').execSync;
    execSync('git add -A && git commit -m "add pytest config"', {
      cwd: project.root,
      encoding: 'utf-8',
      stdio: 'pipe',
    });
    fs.appendFileSync(path.join(project.root, 'src/foo.ts'), '\n// round 2', 'utf-8');

    try {
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult({ framework: 'vitest' }));

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
        framework: 'vitest',
      });

      // 仅 vitest 执行
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.objectContaining({ framework: 'vitest' }),
        project.root,
        expect.objectContaining({
          mutationDiffFiles: [makeAbsoluteFilePath('src/foo.ts', project.root)],
        }),
      );
    } finally {
      project.cleanup();
    }
  }, 10000);
});
