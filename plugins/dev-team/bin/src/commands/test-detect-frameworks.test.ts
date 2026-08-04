/**
 * 单元测试: test-detect-frameworks — 占位符延迟展开、coverage 约定、版本探测
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it, vi } from 'vite-plus/test';

import * as projectRootLib from '../lib/project-root';
import * as testFramework from '../lib/test-framework';
import { type OpenSpecConfigInput } from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: OpenSpecConfigInput): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-fw-test-'));
  const openspecDir = path.join(tmpDir, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(configData, null, 2),
    'utf-8',
  );
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

describe('runTestDetectFrameworks — plan.cwd / root', () => {
  it('root + cwd 解析为 absCwd 相对路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'a/b', framework: 'vite-plus', cwd: '..' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].cwd).toBe('a');
      expect(result.plan[0].root).toBe('a/b');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — 占位符延迟展开 (AC-7)', () => {
  it('产出 script 含未展开的 report 占位符', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'jest' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).toContain('{results_file}');
      expect(plan.script.shell).toContain('{report_dir}');
      expect(plan.script.shell).toContain('{config_args}');
      expect(plan.script.cmd).toContain('{results_file}');
    } finally {
      project.cleanup();
    }
  });

  it('coverage_output 为相对 reportDir 的垂直约定名', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: '.', framework: 'jest' },
        { root: 'go-pkg', framework: 'go' },
        { root: 'py', framework: 'pytest' },
      ],
    });
    try {
      const plans = runTestDetectFrameworks({ projectRoot: project.root }).plan;
      const byFw = Object.fromEntries(plans.map((p) => [p.framework, p]));
      expect(byFw.jest.coverage_output).toBe('coverage-summary.json');
      expect(byFw.go.coverage_output).toBe('func-summary.txt');
      expect(byFw.pytest.coverage_output).toBe('coverage.json');
      expect(byFw.jest.coverage_output.startsWith('coverage/')).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('bun plan 的 coverage_format 为 lcov', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'bun' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.coverage_format).toBe('lcov');
      expect(plan.coverage_output).toBe('lcov.info');
    } finally {
      project.cleanup();
    }
  });

  it('无 tests 配置 → 空 plan', () => {
    const project = createTempProject({ schema: 'spec-driven' });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('script 中不出现已展开的绝对 reports/test/... 路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).not.toMatch(/reports[/\\]test[/\\]/);
      expect(plan.script.cmd).not.toMatch(/reports[/\\]test[/\\]/);
      expect(plan.script.shell).not.toContain(project.root);
    } finally {
      project.cleanup();
    }
  });

  it('不注入 rm -rf coverage 等 suite cwd cleanup；不 mkdir reportDir', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).not.toContain('rm -rf coverage');
      expect(plan.script.shell).not.toContain('rm -rf .nyc_output');
      expect(plan.script.shell).not.toMatch(/^cd /);
      expect(fs.existsSync(path.join(project.root, 'reports', 'test'))).toBe(false);
      expect(fs.readdirSync(project.root).some((n) => n.startsWith('bunfig.dev-team-'))).toBe(
        false,
      );
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — detectFrameworkVersion', () => {
  it('以 suite framework 与 absCwd 调用 detectFrameworkVersion', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'jest', cwd: '.' }],
    });
    try {
      runTestDetectFrameworks({ projectRoot: project.root });
      expect(testFramework.detectFrameworkVersion).toHaveBeenCalledTimes(1);
      expect(testFramework.detectFrameworkVersion).toHaveBeenCalledWith(
        'jest',
        path.resolve(project.root, 'pkg'),
      );
    } finally {
      project.cleanup();
    }
  });

  it('多 suite 时按各自 framework / absCwd 分别探测版本', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'js', framework: 'vitest' },
        { root: 'go-pkg', framework: 'go' },
      ],
    });
    try {
      runTestDetectFrameworks({ projectRoot: project.root });
      expect(testFramework.detectFrameworkVersion).toHaveBeenCalledTimes(2);
      expect(testFramework.detectFrameworkVersion).toHaveBeenCalledWith(
        'vitest',
        path.resolve(project.root, 'js'),
      );
      expect(testFramework.detectFrameworkVersion).toHaveBeenCalledWith(
        'go',
        path.resolve(project.root, 'go-pkg'),
      );
    } finally {
      project.cleanup();
    }
  });

  it('jest >= 29.5.0 → script 含 --randomize', () => {
    vi.mocked(testFramework.detectFrameworkVersion).mockReturnValue('29.5.0');
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'jest' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).toContain('--randomize');
      expect(plan.script.cmd).toContain('--randomize');
    } finally {
      project.cleanup();
    }
  });

  it('jest < 29.5.0 → script 不含 --randomize', () => {
    vi.mocked(testFramework.detectFrameworkVersion).mockReturnValue('29.4.0');
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'jest' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).not.toContain('--randomize');
      expect(plan.script.cmd).not.toContain('--randomize');
    } finally {
      project.cleanup();
    }
  });

  it('版本探测失败（空字符串）→ jest 按不支持 --randomize 生成脚本', () => {
    vi.mocked(testFramework.detectFrameworkVersion).mockReturnValue('');
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'jest' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).not.toContain('--randomize');
      expect(plan.script.cmd).not.toContain('--randomize');
      expect(plan.script.shell).toContain('{results_file}');
    } finally {
      project.cleanup();
    }
  });

  it('root + cwd 解析后，version 探测 cwd 为 absCwd 而非 suite.root', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'a/b', framework: 'vite-plus', cwd: '..' }],
    });
    try {
      runTestDetectFrameworks({ projectRoot: project.root });
      expect(testFramework.detectFrameworkVersion).toHaveBeenCalledWith(
        'vite-plus',
        path.resolve(project.root, 'a'),
      );
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — mutation-score 补强', () => {
  it('显式 files 命中 suite → detected.framework 正确且 plan 含对应 framework', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'jest', includes: ['**/*.test.ts'] }],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'pkg'), { recursive: true });
      const result = runTestDetectFrameworks({
        projectRoot: project.root,
        files: ['pkg/a.test.ts'],
      });
      expect(result.detected[0].framework).toBe('jest');
      expect(result.detected[0].file.replace(/\\/g, '/')).toContain('pkg/a.test.ts');
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].framework).toBe('jest');
    } finally {
      project.cleanup();
    }
  });

  it('files: [] → { detected: [], plan: [] }（即使有 suites）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root, files: [] });
      expect(result).toEqual({ detected: [], plan: [] });
    } finally {
      project.cleanup();
    }
  });

  it('省略 files → auto-scan 收集命中 includes 的文件', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] }],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'pkg'), { recursive: true });
      fs.writeFileSync(path.join(project.root, 'pkg', 'hit.test.ts'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'pkg', 'miss.ts'), '', 'utf-8');
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(
        result.detected.some(
          (d) => d.file.replace(/\\/g, '/').endsWith('pkg/hit.test.ts') && d.framework === 'vitest',
        ),
      ).toBe(true);
      expect(result.detected.some((d) => d.file.endsWith('miss.ts'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('auto-scan 不进入 node_modules/.git/dist/build/target/.vp/coverage/.nyc_output/.claude', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] }],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'pkg'), { recursive: true });
      const banned = [
        'node_modules',
        '.git',
        'dist',
        'build',
        'target',
        '.vp',
        'coverage',
        '.nyc_output',
        '.claude',
      ];
      for (const d of banned) {
        const dir = path.join(project.root, 'pkg', d);
        fs.mkdirSync(dir, { recursive: true });
        fs.writeFileSync(path.join(dir, 'secret.test.ts'), '', 'utf-8');
      }
      fs.writeFileSync(path.join(project.root, 'pkg', 'ok.test.ts'), '', 'utf-8');
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(
        result.detected.some((d) => d.file.replace(/\\/g, '/').endsWith('pkg/ok.test.ts')),
      ).toBe(true);
      for (const d of banned) {
        expect(
          result.detected.some(
            (x) => x.file.includes(`${path.sep}${d}${path.sep}`) || x.file.includes(`/${d}/`),
          ),
        ).toBe(false);
      }
    } finally {
      project.cleanup();
    }
  });

  it('两 suite 均可匹配时数组顺序前者胜出', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'pkg', framework: 'jest', includes: ['**/*.test.ts'] },
        { root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        projectRoot: project.root,
        files: ['pkg/x.test.ts'],
      });
      expect(result.detected[0].framework).toBe('jest');
    } finally {
      project.cleanup();
    }
  });

  it('suite.excludes 命中的文件不出现在 detected', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/*.test.ts'],
          excludes: ['skip/**'],
        },
      ],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'pkg', 'skip'), { recursive: true });
      fs.writeFileSync(path.join(project.root, 'pkg', 'skip', 'a.test.ts'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'pkg', 'keep.test.ts'), '', 'utf-8');
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.detected.some((d) => d.file.includes('keep.test.ts'))).toBe(true);
      expect(
        result.detected.some(
          (d) => d.file.includes(`${path.sep}skip${path.sep}`) || d.file.includes('/skip/'),
        ),
      ).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('显式 files 未匹配 → unknown；auto-scan 未匹配则省略', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] }],
    });
    try {
      const explicit = runTestDetectFrameworks({
        projectRoot: project.root,
        files: ['readme.md'],
      });
      expect(explicit.detected[0].framework).toBe('unknown');

      fs.mkdirSync(path.join(project.root, 'pkg'), { recursive: true });
      fs.writeFileSync(path.join(project.root, 'pkg', 'readme.md'), '', 'utf-8');
      const scanned = runTestDetectFrameworks({ projectRoot: project.root });
      expect(scanned.detected.some((d) => d.file.endsWith('readme.md'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('无 tests / tests=[]：显式 files → 全 unknown 且 plan=[]；auto-scan → detected=[]', () => {
    for (const tests of [undefined, []] as const) {
      const project = createTempProject(
        tests === undefined ? { schema: 'spec-driven' } : { schema: 'spec-driven', tests: [] },
      );
      try {
        const explicit = runTestDetectFrameworks({
          projectRoot: project.root,
          files: ['a.ts'],
        });
        expect(explicit.detected).toEqual([{ file: expect.any(String), framework: 'unknown' }]);
        expect(explicit.plan).toEqual([]);

        fs.writeFileSync(path.join(project.root, 'a.ts'), '', 'utf-8');
        const scanned = runTestDetectFrameworks({ projectRoot: project.root });
        expect(scanned.detected).toEqual([]);
        expect(scanned.plan).toEqual([]);
      } finally {
        project.cleanup();
      }
    }
  });

  it('suite 声明 config 但 framework config_flag===null（go）→ 抛错含 does not support config injection', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'go', config: 'go.mod' }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /does not support config injection/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('script.shell/cmd 仍含占位符，无 cd 前缀、无已展开 reports 绝对路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vite-plus' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).toContain('{results_file}');
      expect(plan.script.shell).toContain('{report_dir}');
      expect(plan.script.shell).toContain('{config_args}');
      expect(plan.script.cmd).toContain('{results_file}');
      expect(plan.script.shell).not.toMatch(/^cd\s/);
      expect(plan.script.cmd).not.toMatch(/^cd\s/);
      expect(plan.script.shell).not.toContain(project.root);
      expect(plan.script.shell).not.toMatch(/reports[/\\]test[/\\]/);
    } finally {
      project.cleanup();
    }
  });

  it('coverage_format/output、mutation_script 来自 registry；mutation_score 来自 suite 或缺省 null', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: '.', framework: 'vitest', mutation: { score: 66 } },
        { root: 'go-pkg', framework: 'go' },
      ],
    });
    try {
      const plans = runTestDetectFrameworks({ projectRoot: project.root }).plan;
      const vitest = plans.find((p) => p.framework === 'vitest')!;
      const go = plans.find((p) => p.framework === 'go')!;
      expect(vitest.coverage_format).toBe('istanbul');
      expect(vitest.coverage_output).toBe('coverage-summary.json');
      expect(vitest.mutation_score).toBe(66);
      expect(vitest.mutation_script).toEqual(
        expect.objectContaining({
          shell: expect.stringContaining('stryker'),
          cmd: expect.stringContaining('stryker'),
        }),
      );
      expect(go.coverage_format).toBe('go-cover');
      expect(go.mutation_script).toBeNull();
      // schema 对 suite.mutation.score 有默认 70；未显式声明时仍可能得到默认值
      expect(go.mutation_score === null || go.mutation_score === 70).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('两 suite 解析到相同 directory+framework → plan 仅一条', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: '.', framework: 'vitest', includes: ['a/**'] },
        { root: '.', framework: 'vitest', includes: ['b/**'] },
      ],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('suite 无 includes → 使用 framework default_glob；自定义 includes 覆盖默认', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'def', framework: 'pytest' },
        { root: 'custom', framework: 'pytest', includes: ['**/special_*.py'] },
      ],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'def'), { recursive: true });
      fs.mkdirSync(path.join(project.root, 'custom'), { recursive: true });
      fs.writeFileSync(path.join(project.root, 'def', 'test_default.py'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'custom', 'special_x.py'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'custom', 'test_ignored.py'), '', 'utf-8');
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(
        result.detected.some((d) => d.file.includes('test_default.py') && d.framework === 'pytest'),
      ).toBe(true);
      expect(
        result.detected.some((d) => d.file.includes('special_x.py') && d.framework === 'pytest'),
      ).toBe(true);
      expect(result.detected.some((d) => d.file.includes('test_ignored.py'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('全局 exclude（任一 suite.excludes）与 suite.excludes 命中的文件不出现在 detected', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/*.test.ts'],
          excludes: ['legacy/**'],
        },
        {
          root: 'other',
          framework: 'jest',
          includes: ['**/*.test.ts'],
          excludes: ['**/vendor.test.ts'],
        },
      ],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'pkg', 'legacy'), { recursive: true });
      fs.mkdirSync(path.join(project.root, 'other'), { recursive: true });
      fs.writeFileSync(path.join(project.root, 'pkg', 'legacy', 'old.test.ts'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'pkg', 'keep.test.ts'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'other', 'vendor.test.ts'), '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'other', 'ok.test.ts'), '', 'utf-8');

      const scanned = runTestDetectFrameworks({ projectRoot: project.root });
      expect(scanned.detected.some((d) => d.file.includes('keep.test.ts'))).toBe(true);
      expect(scanned.detected.some((d) => d.file.includes('ok.test.ts'))).toBe(true);
      expect(scanned.detected.some((d) => d.file.includes('legacy'))).toBe(false);
      expect(scanned.detected.some((d) => d.file.includes('vendor.test.ts'))).toBe(false);

      // 显式 files 同样受全局 isFileExcluded 过滤（不进入 detected）
      const explicit = runTestDetectFrameworks({
        projectRoot: project.root,
        files: ['pkg/legacy/old.test.ts', 'other/vendor.test.ts', 'pkg/keep.test.ts'],
      });
      expect(explicit.detected).toHaveLength(1);
      expect(explicit.detected[0].file.replace(/\\/g, '/')).toContain('pkg/keep.test.ts');
    } finally {
      project.cleanup();
    }
  });

  it('省略 projectRoot 时走 getProjectDir；绝对/相对 files 均可', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] }],
    });
    const spy = vi.spyOn(projectRootLib, 'getProjectDir').mockReturnValue(project.root);
    try {
      fs.mkdirSync(path.join(project.root, 'pkg'), { recursive: true });
      const absFile = path.join(project.root, 'pkg', 'abs.test.ts');
      fs.writeFileSync(absFile, '', 'utf-8');
      fs.writeFileSync(path.join(project.root, 'pkg', 'rel.test.ts'), '', 'utf-8');

      spy.mockClear();
      const omitted = runTestDetectFrameworks({
        files: ['pkg/rel.test.ts', absFile],
      });
      expect(spy).toHaveBeenCalled();
      expect(omitted.detected).toHaveLength(2);
      expect(omitted.detected.every((d) => d.framework === 'vitest')).toBe(true);
      expect(
        omitted.detected.some((d) => d.file.replace(/\\/g, '/').endsWith('pkg/rel.test.ts')),
      ).toBe(true);
      expect(omitted.detected.some((d) => path.resolve(d.file) === path.resolve(absFile))).toBe(
        true,
      );

      spy.mockClear();
      const withRoot = runTestDetectFrameworks({
        projectRoot: project.root,
        files: ['pkg/rel.test.ts'],
      });
      expect(spy).not.toHaveBeenCalled();
      expect(withRoot.detected[0].framework).toBe('vitest');
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });
});
