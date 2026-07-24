/**
 * 集成测试: mutation 执行全流程（tests[] + absCwd）
 *
 * @see openspec/changes/tests-array-cwd-config/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';
import { executePlanEntry } from '../../src/lib/test-runner';
import { type OpenSpecConfigInput } from '../../src/schemas';

const mockExecSync = vi.fn();
vi.mock('child_process', () => ({
  execSync: (...args: unknown[]) => mockExecSync(...args),
}));

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(config: OpenSpecConfigInput | Record<string, unknown>): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-flow-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(path.join(openspecDir, 'config.json'), JSON.stringify(config, null, 2), 'utf-8');
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

describe('mutation 执行全流程 -- 正向', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('完整配置（vitest + mutation.score=80）→ 框架检测 → plan 生成 → executePlanEntry', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest', mutation: { score: 80 } }],
    });
    try {
      mockExecSync.mockReturnValue(
        '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
      );

      const detectResult = runTestDetectFrameworks({ projectRoot: project.root });
      expect(detectResult.plan).toHaveLength(1);
      expect(detectResult.plan[0].mutation_framework).toBe('stryker-js');
      expect(detectResult.plan[0].mutation_score).toBe(80);

      const result = executePlanEntry(detectResult.plan[0], project.root, { noMutation: false });
      expect(result.framework).toBe('vitest');
      expect(result.testCases).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('--no-mutation 标志跳过 mutation 阶段，其余流程正常', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest', mutation: { score: 80 } }],
    });
    try {
      mockExecSync.mockReturnValue(
        '{"testResults":[{"assertionResults":[{"title":"t1","fullName":"t1","status":"passed"}]}]}',
      );

      const detectResult = runTestDetectFrameworks({ projectRoot: project.root });
      expect(detectResult.plan[0].mutation_framework).toBe('stryker-js');

      const result = executePlanEntry(detectResult.plan[0], project.root, { noMutation: true });
      expect(result.mutation).toBeNull();
      expect(result.testCases).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('不支持框架（如 bun）配置时 mutation_framework 为 null', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'bun' }],
    });
    try {
      mockExecSync.mockReturnValue('{"testResults":[{"title":"t1","status":"passed"}]}');

      const detectResult = runTestDetectFrameworks({ projectRoot: project.root });
      expect(detectResult.plan).toHaveLength(1);
      expect(detectResult.plan[0].mutation_framework).toBeNull();
      expect(detectResult.plan[0].mutation_score).toBe(70);
    } finally {
      project.cleanup();
    }
  });

  it('suite 自定义 mutation.score 写入 plan.mutation_score', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'src', framework: 'vitest', mutation: { score: 90 } }],
    });
    try {
      const detectResult = runTestDetectFrameworks({ projectRoot: project.root });
      expect(detectResult.plan).toHaveLength(1);
      expect(detectResult.plan[0].mutation_framework).toBe('stryker-js');
      expect(detectResult.plan[0].mutation_score).toBe(90);
    } finally {
      project.cleanup();
    }
  });
});

describe('变异 cwd 与 mutate 相对路径', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
  });

  it('mutation 执行 cwd = absCwd；临时 stryker.config.* 位于 absCwd', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-abscwd-'));
    try {
      fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
      fs.writeFileSync(
        path.join(root, 'openspec', 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: 'a/b', framework: 'vitest', cwd: '..', mutation: { score: 70 } }],
        }),
        'utf-8',
      );
      fs.mkdirSync(path.join(root, 'a', 'b'), { recursive: true });
      fs.writeFileSync(path.join(root, 'a', 'b', 'foo.ts'), 'export {}');
      const reportDir = path.join(root, 'a', 'reports', 'mutation');
      fs.mkdirSync(reportDir, { recursive: true });
      fs.writeFileSync(
        path.join(reportDir, 'mutation.json'),
        JSON.stringify({
          metrics: {
            mutationScore: 100,
            killed: 1,
            survived: 0,
            timeout: 0,
            noCoverage: 0,
            compileErrors: 0,
            runtimeErrors: 0,
            ignored: 0,
            totalDetected: 1,
            totalUndetected: 0,
            totalMutants: 1,
          },
        }),
        'utf-8',
      );

      mockExecSync.mockReturnValueOnce(
        JSON.stringify({
          testResults: [
            {
              name: 'a/b/foo.test.ts',
              assertionResults: [{ title: 't1', fullName: 't1', status: 'passed' }],
            },
          ],
        }),
      );
      mockExecSync.mockReturnValueOnce('ok');

      const detect = runTestDetectFrameworks({ projectRoot: root });
      expect(detect.plan[0].directory).toBe('a');
      executePlanEntry(detect.plan[0], root, { noMutation: false });
      expect(mockExecSync).toHaveBeenCalledTimes(2);
      expect(mockExecSync.mock.calls[1][1].cwd).toBe(path.resolve(root, 'a'));
      expect(String(mockExecSync.mock.calls[1][0]).replace(/\\/g, '/')).toContain(
        '/a/stryker.config.',
      );
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  it('sourceFiles 为空或 exclude 后为空时跳过 mutation', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vitest',
          excludes: ['**/*'],
          mutation: { score: 70 },
        },
      ],
    });
    try {
      mockExecSync.mockReturnValueOnce(
        JSON.stringify({
          testResults: [
            {
              name: 'src/app.test.ts',
              assertionResults: [{ title: 't1', fullName: 't1', status: 'passed' }],
            },
          ],
        }),
      );
      const detect = runTestDetectFrameworks({ projectRoot: project.root });
      const result = executePlanEntry(detect.plan[0], project.root);
      expect(result.mutation).toBeNull();
      expect(mockExecSync).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });

  it('不支持 mutation 的框架时变异阶段不执行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'pytest' }],
    });
    try {
      mockExecSync.mockReturnValue('ok');
      const detect = runTestDetectFrameworks({ projectRoot: project.root });
      const result = executePlanEntry(detect.plan[0], project.root);
      expect(detect.plan[0].mutation_framework).toBeNull();
      expect(result.mutation).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('mutate 列表路径相对 absCwd，且遵守 suite excludes', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-mutate-rel-'));
    try {
      fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
      fs.writeFileSync(
        path.join(root, 'openspec', 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: 'a/b',
              framework: 'vitest',
              cwd: '..',
              excludes: ['**/skip.ts'],
              mutation: { score: 70 },
            },
          ],
        }),
        'utf-8',
      );
      fs.mkdirSync(path.join(root, 'a', 'b'), { recursive: true });
      fs.writeFileSync(path.join(root, 'a', 'b', 'keep.ts'), 'export const keep = 1;');
      fs.writeFileSync(path.join(root, 'a', 'b', 'skip.ts'), 'export const skip = 1;');

      const reportDir = path.join(root, 'a', 'reports', 'mutation');
      fs.mkdirSync(reportDir, { recursive: true });
      fs.writeFileSync(
        path.join(reportDir, 'mutation.json'),
        JSON.stringify({
          metrics: {
            mutationScore: 100,
            killed: 1,
            survived: 0,
            timeout: 0,
            noCoverage: 0,
            compileErrors: 0,
            runtimeErrors: 0,
            ignored: 0,
            totalDetected: 1,
            totalUndetected: 0,
            totalMutants: 1,
          },
        }),
        'utf-8',
      );

      mockExecSync.mockReturnValueOnce(
        JSON.stringify({
          testResults: [
            {
              name: 'a/b/keep.test.ts',
              assertionResults: [{ title: 't1', fullName: 't1', status: 'passed' }],
            },
            {
              name: 'a/b/skip.test.ts',
              assertionResults: [{ title: 't2', fullName: 't2', status: 'passed' }],
            },
          ],
        }),
      );

      let capturedMutate: string[] | null = null;
      mockExecSync.mockImplementationOnce((_cmd: unknown, opts?: { cwd?: string }) => {
        const cwd = opts?.cwd ?? path.resolve(root, 'a');
        const configs = fs.readdirSync(cwd).filter((f) => f.startsWith('stryker.config.'));
        expect(configs.length).toBeGreaterThanOrEqual(1);
        const configJson = JSON.parse(fs.readFileSync(path.join(cwd, configs[0]), 'utf-8'));
        capturedMutate = configJson.mutate.map((p: string) => p.replace(/\\/g, '/'));
        return 'ok';
      });

      const detect = runTestDetectFrameworks({ projectRoot: root });
      expect(detect.plan[0].directory).toBe('a');
      executePlanEntry(detect.plan[0], root, { noMutation: false });

      expect(capturedMutate).not.toBeNull();
      // mutate 相对 absCwd(=a)，且 skip 被 suite excludes 剔除
      expect(capturedMutate).toContain('b/keep.ts');
      expect(capturedMutate).not.toContain('b/skip.ts');
      expect(capturedMutate).not.toContain('a/b/keep.ts');
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  it('夹具从 test.overrides 迁到 tests[] 后 mutation-execution-flow 场景仍通过', () => {
    // 旧形状: test.framework + test.mutation + test.overrides[{ file, framework }]
    // 迁移后: tests[{ root, framework, mutation, includes }]
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vitest',
          includes: ['**/*.{test,spec}.{ts,tsx,js,jsx}'],
          mutation: { score: 80 },
        },
      ],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'src'), { recursive: true });
      fs.writeFileSync(path.join(project.root, 'src', 'foo.ts'), 'export const x = 1;');
      fs.writeFileSync(path.join(project.root, 'src', 'foo.test.ts'), '// test');

      const reportDir = path.join(project.root, 'src', 'reports', 'mutation');
      fs.mkdirSync(reportDir, { recursive: true });
      fs.writeFileSync(
        path.join(reportDir, 'mutation.json'),
        JSON.stringify({
          metrics: {
            mutationScore: 90,
            killed: 9,
            survived: 1,
            timeout: 0,
            noCoverage: 0,
            compileErrors: 0,
            runtimeErrors: 0,
            ignored: 0,
            totalDetected: 9,
            totalUndetected: 1,
            totalMutants: 10,
          },
        }),
        'utf-8',
      );

      mockExecSync.mockReturnValueOnce(
        JSON.stringify({
          testResults: [
            {
              name: 'src/foo.test.ts',
              assertionResults: [{ title: 't1', fullName: 't1', status: 'passed' }],
            },
          ],
        }),
      );
      mockExecSync.mockReturnValueOnce('ok');

      const detect = runTestDetectFrameworks({ projectRoot: project.root });
      expect(detect.plan).toHaveLength(1);
      expect(detect.plan[0].directory).toBe('src');
      expect(detect.plan[0].mutation_framework).toBe('stryker-js');
      expect(detect.plan[0].mutation_score).toBe(80);

      const result = executePlanEntry(detect.plan[0], project.root, { noMutation: false });
      expect(result.framework).toBe('vitest');
      expect(result.testCases).toHaveLength(1);
      expect(result.mutation).not.toBeNull();
      expect(result.mutation!.threshold).toBe(80);
      expect(mockExecSync).toHaveBeenCalledTimes(2);
    } finally {
      project.cleanup();
    }
  });

  it('仅旧 test.overrides、无 tests[] 时不产生 mutation plan（硬 breaking）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      // 故意写入已废弃键，验证不再被解析为 suite
      test: {
        framework: 'vitest',
        mutation: { score: 80 },
        overrides: [{ file: 'src/**', framework: 'vitest' }],
      },
    });
    try {
      const detect = runTestDetectFrameworks({ projectRoot: project.root });
      expect(detect.plan).toHaveLength(0);
    } finally {
      project.cleanup();
    }
  });
});
