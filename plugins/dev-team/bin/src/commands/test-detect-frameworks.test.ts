/**
 * 单元测试: test-detect-frameworks — 占位符延迟展开、coverage 约定、版本探测
 *
 * detectFrameworkVersion 默认由 bin/__tests__/test-setup.ts 全局 spy 为 '99.0.0'；
 * 本文件仅在需要断言调用或改返回值时操作该 spy。
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

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

beforeEach(() => {
  vi.mocked(testFramework.detectFrameworkVersion).mockClear();
  vi.mocked(testFramework.detectFrameworkVersion).mockReturnValue('99.0.0');
});

describe('runTestDetectFrameworks — plan.directory / scope', () => {
  it('root + cwd 解析为 absCwd 相对路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'a/b', framework: 'vite-plus', cwd: '..' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].directory).toBe('a');
      expect(result.plan[0].scope).toBe('b');
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
