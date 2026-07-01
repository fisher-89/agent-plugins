/**
 * Tests for test-resolve-paths -- MCP tool that derives unit/integration test
 * file paths from module lists (files or directories).
 *
 * Covers AC-1~AC-9 from openspec/changes/add-test-path-resolver-api/test-design.md
 * AND AC-1~AC-6 from openspec/changes/test-resolve-paths-config-dirs/test-design.md
 *
 * @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
 * @see openspec/changes/add-test-path-resolver-api/test-design.md
 * @see openspec/changes/add-test-path-resolver-api/specs/test-path-resolver/spec.md
 */

import { execSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runTestDetectFrameworks } from './test-detect-frameworks';
import { runTestResolvePaths } from './test-resolve-paths';

// ---------------------------------------------------------------------------
// Mocks: wrap runTestDetectFrameworks + execSync in vi.fn() so new tests can
// override return values without breaking existing tests (pass-through by
// default).  vi.mock is hoisted by vitest and runs before module evaluation.
// ---------------------------------------------------------------------------

vi.mock('./test-detect-frameworks', async () => {
  const actual = await vi.importActual<{
    runTestDetectFrameworks: typeof runTestDetectFrameworks;
  }>('./test-detect-frameworks');
  return {
    ...actual,
    runTestDetectFrameworks: vi.fn(actual.runTestDetectFrameworks),
  };
});

vi.mock('child_process', async () => {
  const actual = await vi.importActual<{
    execSync: typeof execSync;
  }>('child_process');
  return {
    ...actual,
    execSync: vi.fn(actual.execSync),
  };
});

// ---------------------------------------------------------------------------
// Helpers: create temp project directories with source files
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'resolve-paths-test-'));
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

function writeFile(projectRoot: string, relativePath: string, content = ''): void {
  const absPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(absPath), { recursive: true });
  fs.writeFileSync(absPath, content, 'utf-8');
}

function writeDir(projectRoot: string, relativeDir: string): void {
  fs.mkdirSync(path.join(projectRoot, relativeDir), { recursive: true });
}

// ===========================================================================
// runTestResolvePaths -- 文件路径解析 (AC-6)
// ===========================================================================

describe('runTestResolvePaths -- 文件路径解析', () => {
  it('多个源文件路径应各自返回同级测试文件 (AC-6)', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/commands/foo.ts', 'src/commands/bar.ts'],
      });

      expect(result.unit_tests).toHaveLength(2);
      expect(result.unit_tests).toContainEqual({
        source: 'src/commands/bar.ts',
        test_file: 'src/commands/bar.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'src/commands/foo.ts',
        test_file: 'src/commands/foo.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('unit_tests 应按 source 字典序排序且去重', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/z.ts', 'src/a.ts', 'src/a.ts'],
      });

      const sources = result.unit_tests.map((e) => e.source);
      expect(sources).toEqual([...sources].sort());
      expect(new Set(sources).size).toBe(sources.length);
    } finally {
      project.cleanup();
    }
  });

  it('已是测试文件的路径应写入 errors 而非 unit_tests', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/foo.ts', 'src/foo.test.ts'],
      });

      const sources = result.unit_tests.map((e) => e.source);
      expect(sources).toContain('src/foo.ts');
      expect(sources).not.toContain('src/foo.test.ts');
      expect(result.errors.some((e) => e.path === 'src/foo.test.ts')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('非源文件（.md、.json、.yaml 等）路径应写入 errors', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/app.ts', 'README.md', 'config.json', 'data.yaml', 'notes.txt'],
      });

      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0]).toEqual({
        source: 'src/app.ts',
        test_file: 'src/app.test.ts',
      });
      expect(result.errors).toHaveLength(4);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- 集成测试路径 (AC-7, AC-8)
// ===========================================================================

describe('runTestResolvePaths -- 集成测试路径', () => {
  it('integration_scenarios: ["api-flow"] + extension: "ts" (AC-7)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: '__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('modules: ["src/auth.py"] + integration_scenarios: ["db-roundtrip"] 无 extension (AC-8)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/auth.py', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/auth.py'],
        integration_scenarios: ['db-roundtrip'],
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'db-roundtrip',
        test_file: '__tests__/db-roundtrip/db-roundtrip.test.py',
      });
    } finally {
      project.cleanup();
    }
  });

  it('无有效源文件时集成测试应默认 .test.ts 扩展名', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'README.md', '# readme');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['README.md'],
        integration_scenarios: ['smoke'],
      });

      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.integration_tests).toContainEqual({
        scenario: 'smoke',
        test_file: '__tests__/smoke/smoke.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('多个 integration_scenarios 应按 scenario 字典序排序', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/a.ts'],
        integration_scenarios: ['zebra', 'alpha', 'middle'],
        extension: 'ts',
      });

      const scenarios = result.integration_tests.map((e) => e.scenario);
      expect(scenarios).toEqual([...scenarios].sort());
    } finally {
      project.cleanup();
    }
  });

  it('integration_scenarios 未传或为空数组时 integration_tests 应为空', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.ts', '');

      const without = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/a.ts'],
      });
      expect(without.integration_tests).toEqual([]);

      const empty = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/a.ts'],
        integration_scenarios: [],
      });
      expect(empty.integration_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- 错误收集 (AC-9)
// ===========================================================================

describe('runTestResolvePaths -- 错误收集', () => {
  it('不存在的源文件路径仍应推导出测试路径（无需判断文件是否存在）(AC-9)', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/missing.ts', 'src/config.ts'],
      });

      expect(result.errors).toHaveLength(0);
      expect(result.unit_tests).toContainEqual({
        source: 'src/missing.ts',
        test_file: 'src/missing.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'src/config.ts',
        test_file: 'src/config.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('README.md 应写入 errors，unit_tests 应为空', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'README.md', '#');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['README.md'],
      });

      expect(result.errors.some((e) => e.path === 'README.md')).toBe(true);
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('传入已是测试文件的路径应写入 errors', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.test.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.test.ts'],
      });

      expect(result.errors.some((e) => e.path === 'src/config.test.ts')).toBe(true);
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('路径穿越 ../../../outside.ts 应写入 errors', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['../../../outside.ts'],
      });

      expect(result.errors.some((e) => e.path === '../../../outside.ts')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('errors 应按 path 字典序排序', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['z-file.yaml', 'a-file.json', 'm-file.md'],
      });

      expect(result.errors).toHaveLength(3);
      const paths = result.errors.map((e) => e.path);
      expect(paths).toEqual([...paths].sort());
    } finally {
      project.cleanup();
    }
  });

  it('单条失败不应中断其余 modules 条目的处理', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['README.md', 'src/valid.ts', 'src/missing.ts'],
      });

      expect(result.errors).toHaveLength(1);
      expect(result.errors[0].path).toBe('README.md');
      expect(result.unit_tests).toContainEqual({
        source: 'src/valid.ts',
        test_file: 'src/valid.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'src/missing.ts',
        test_file: 'src/missing.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- modules 边界
// ===========================================================================

describe('runTestResolvePaths -- modules 边界', () => {
  it('modules 含单元素文件路径时应返回单条 unit_tests', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/single.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/single.ts'],
      });

      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0]).toEqual({
        source: 'src/single.ts',
        test_file: 'src/single.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('modules 含大量文件路径（100+ 条）时不应抛错且全部解析', () => {
    const project = createTempProject();
    try {
      const modules: string[] = [];
      for (let i = 0; i < 100; i++) {
        const rel = `src/file${i}.ts`;
        writeFile(project.root, rel, '');
        modules.push(rel);
      }

      const result = runTestResolvePaths({
        project_root: project.root,
        modules,
      });

      expect(result.unit_tests).toHaveLength(100);
    } finally {
      project.cleanup();
    }
  });

  it('modules 含多个不同目录的文件路径时应各自返回同级测试文件', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/standalone.ts', 'src/dir/inner.ts'],
      });

      const sources = result.unit_tests.map((e) => e.source);
      expect(sources).toContain('src/standalone.ts');
      expect(sources).toContain('src/dir/inner.ts');
      expect(result.unit_tests).toContainEqual({
        source: 'src/dir/inner.ts',
        test_file: 'src/dir/inner.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- 端到端编排 (AC-1~AC-9 集成)
// ===========================================================================

describe('runTestResolvePaths -- 端到端编排', () => {
  it('传入 modules、integration_scenarios、extension 应返回完整解析结构', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');
      writeDir(project.root, 'src/commands');
      writeFile(project.root, 'src/commands/foo.ts', '');

      const result = runTestResolvePaths({
        modules: ['src/config.ts', 'src/commands/'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        project_root: project.root,
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/config.ts',
        test_file: 'src/config.test.ts',
      });
      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: '__tests__/api-flow/api-flow.test.ts',
      });
      expect(result.errors.some((e) => e.path === 'src/commands/')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('project_root 指向绝对路径且源文件存在时相对路径解析应正确', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.ts', '');

      const result = runTestResolvePaths({
        modules: ['src/a.ts'],
        project_root: project.root,
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/a.ts',
        test_file: 'src/a.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// add-integration-root-param — integration_root via runTestResolvePaths
// @see openspec/changes/add-integration-root-param/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- integration_root 向后兼容', () => {
  it('未传 integration_root + integration_scenarios: ["api-flow"] + extension: "ts" → __tests__/api-flow/api-flow.test.ts (AC-1)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: '__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestResolvePaths -- integration_root 为点号', () => {
  it('integration_root: "." + 同上场景 → 与 AC-1 相同 (AC-2)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: '.',
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: '__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestResolvePaths -- integration_root 子目录前缀', () => {
  it('integration_root: "plugins/dev-team/bin" + integration_scenarios: ["api-flow"] → 带子目录前缀路径 (AC-3)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: 'plugins/dev-team/bin',
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: 'plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('integration_root: "plugins/dev-team/bin/" 经 runTestResolvePaths 映射后路径与 AC-3 一致 (AC-5)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: 'plugins/dev-team/bin/',
        project_root: project.root,
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: 'plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestResolvePaths -- integration_root 与 unit_tests 隔离', () => {
  it('modules: ["src/config.ts"] + integration_root: "plugins/dev-team/bin" 时 unit_tests[0].test_file 仍为 src/config.test.ts (AC-6)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        integration_root: 'plugins/dev-team/bin',
        extension: 'ts',
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/config.ts',
        test_file: 'src/config.test.ts',
      });
      expect(result.integration_tests[0]?.test_file).toContain('plugins/dev-team/bin/__tests__/');
    } finally {
      project.cleanup();
    }
  });

  it('多个 modules 含不同语言源文件时 integration_root 仅改变 integration_tests (AC-6)', () => {
    const project = createTempProject();
    try {
      const modules = ['src/a.ts', 'src/b.py', 'src/c.go'];

      const withoutRoot = runTestResolvePaths({
        project_root: project.root,
        modules,
        integration_scenarios: ['api-flow'],
        extension: 'ts',
      });

      const withRoot = runTestResolvePaths({
        project_root: project.root,
        modules,
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: 'plugins/dev-team/bin',
      });

      expect(withRoot.unit_tests).toEqual(withoutRoot.unit_tests);
      expect(withRoot.integration_tests[0]?.test_file).toMatch(
        /^plugins\/dev-team\/bin\/__tests__\//,
      );
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestResolvePaths -- integration_root 路径穿越', () => {
  it('integration_root: "../outside" 含 .. 段时写入 errors 且 integration_tests 为空 (D7)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        integration_root: '../outside',
        extension: 'ts',
      });

      expect(result.integration_tests).toEqual([]);
      expect(
        result.errors.some((e) => e.path.includes('integration') || e.message.length > 0),
      ).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestResolvePaths -- integration_root snake_case 映射', () => {
  it('runTestResolvePaths({ integration_root: "plugins/dev-team/bin" }) 应生成带子目录前缀的集成测试路径', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: 'plugins/dev-team/bin',
        project_root: project.root,
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: 'plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('未传 integration_root 时 runTestResolvePaths 应使用默认 __tests__ 前缀（回归）', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = runTestResolvePaths({
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        project_root: project.root,
      });

      expect(result.integration_tests).toContainEqual({
        scenario: 'api-flow',
        test_file: '__tests__/api-flow/api-flow.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- config-driven 自动扫描 (AC-1, AC-2)
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- config-driven 自动扫描', () => {
  beforeEach(() => {
    vi.mocked(runTestDetectFrameworks).mockReset();
    vi.mocked(execSync).mockReset();
  });

  it('modules: [] 且 test.overrides 配置有效时，unit_tests 包含扫描到的源文件推导结果 (AC-1)', () => {
    const project = createTempProject();
    try {
      // Mock auto-scan → return plan with src/ directory
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        if (opts.files === undefined) {
          return {
            detected: [],
            frameworks: ['vite-plus'],
            plan: [
              {
                directory: 'src',
                framework: 'vite-plus' as const,
                test_cmd: 'vp test --coverage --coverage.reporter=json-summary {files}',
                coverage_cmd: 'vp test --coverage --coverage.reporter=json-summary',
                coverage_format: 'istanbul' as const,
                coverage_output: 'coverage/coverage-summary.json',
                coverage_artifacts: ['coverage/coverage-summary.json'],
                coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
                script:
                  '#!/bin/bash\nset -e\ncd src\nrm -rf coverage\nrm -rf .nyc_output\nrm -rf test-stderr.txt\nvp test --coverage --coverage.reporter=json-summary\n',
              },
            ],
          };
        }
        return { detected: [], frameworks: [], plan: [] };
      });

      writeFile(project.root, 'src/foo.ts', '');
      writeFile(project.root, 'src/bar.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/foo.ts',
        test_file: 'src/foo.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'src/bar.ts',
        test_file: 'src/bar.test.ts',
      });
      expect(result.errors).toHaveLength(0);
    } finally {
      project.cleanup();
    }
  });

  it('modules: [] 时 runTestDetectFrameworks 被调用（通过 spy 验证）(AC-1)', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockReturnValue({
        detected: [],
        frameworks: [],
        plan: [],
      });

      runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

      expect(runTestDetectFrameworks).toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: project.root }),
      );
      // files 参数应被省略（触发 auto-scan）
      expect(runTestDetectFrameworks).toHaveBeenCalledWith(
        expect.not.objectContaining({ files: expect.anything() }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('modules: [] 且 plan 为空时 errors 包含 "No test configuration" 提示，unit_tests 为空 (AC-2)', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockReturnValue({
        detected: [],
        frameworks: [],
        plan: [],
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

      expect(result.errors.some((e) => e.path === 'config')).toBe(true);
      expect(result.errors[0].message).toContain('No test configuration');
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- config-driven 过滤 (AC-3)
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- config-driven 过滤', () => {
  beforeEach(() => {
    vi.mocked(runTestDetectFrameworks).mockReset();
  });

  it('modules: ["src/config.ts"] 在 test config 覆盖范围内时正常返回 unit_tests (AC-3)', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          frameworks: ['vitest'],
          plan: [],
        };
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/config.ts'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/config.ts',
        test_file: 'src/config.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('modules: ["scripts/not-covered.ts"] 不在 test config 范围内时该文件不出现在 unit_tests 中，errors 包含跳过的提示 (AC-3)', () => {
    const project = createTempProject();
    try {
      // 模拟仅 "src/" 范围内的文件被 detected
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const detected = opts
          .files!.map((f) => ({
            file: path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
            f,
          }))
          .filter(({ f }) => {
            const rel = f.replace(/\\/g, '/');
            return rel.startsWith('src/');
          })
          .map(({ file }) => ({ file, framework: 'vitest' as const }));
        return { detected, frameworks: ['vitest'], plan: [] };
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['scripts/not-covered.ts'],
      });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors.some((e) => e.path === 'scripts/not-covered.ts')).toBe(true);
      expect(result.errors.some((e) => e.message === 'Not in test config scope')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('modules 混合范围内外文件，仅范围内的文件进入 unit_tests (AC-3)', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const detected = opts
          .files!.map((f) => ({
            file: path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
            f,
          }))
          .filter(({ f }) => {
            const rel = f.replace(/\\/g, '/');
            return rel.startsWith('src/');
          })
          .map(({ file }) => ({ file, framework: 'vitest' as const }));
        return { detected, frameworks: ['vitest'], plan: [] };
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/in.ts', 'out/not-covered.ts'],
      });

      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe('src/in.ts');
      expect(result.errors.some((e) => e.path === 'out/not-covered.ts')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- git-change 模式 (AC-5)
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- git-change 模式', () => {
  beforeEach(() => {
    vi.mocked(execSync).mockReset();
    vi.mocked(runTestDetectFrameworks).mockReset();
  });

  it('modules: "git-change" 且 git diff 返回变更文件时，返回对应 unit_tests (AC-5)', () => {
    const project = createTempProject();
    try {
      vi.mocked(execSync).mockReturnValue('src/foo.ts\nsrc/bar.ts\n');
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const detected = opts.files!.map((f) => ({
          file: path.resolve(opts.projectRoot!, f),
          framework: 'vitest' as const,
        }));
        return { detected, frameworks: ['vitest'], plan: [] };
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(execSync).toHaveBeenCalledWith(
        'git diff HEAD --name-only',
        expect.objectContaining({ cwd: project.root }),
      );
      expect(result.unit_tests).toContainEqual({
        source: 'src/foo.ts',
        test_file: 'src/foo.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'src/bar.ts',
        test_file: 'src/bar.test.ts',
      });
    } finally {
      project.cleanup();
    }
  });

  it('modules: "git-change" 且 git 命令失败时（非 git 仓库），errors 包含错误消息 (AC-5)', () => {
    const project = createTempProject();
    try {
      vi.mocked(execSync).mockImplementation(() => {
        throw new Error('Not a git repository');
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(result.errors.some((e) => e.path === 'git')).toBe(true);
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('modules: "git-change" 返回的变更文件均不在 test config 范围内时 unit_tests 为空 (AC-5)', () => {
    const project = createTempProject();
    try {
      vi.mocked(execSync).mockReturnValue('outside/foo.ts\n');
      vi.mocked(runTestDetectFrameworks).mockReturnValue({
        detected: [],
        frameworks: [],
        plan: [],
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors.some((e) => e.message === 'Not in test config scope')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('modules: "git-change" 且 git diff 返回空（无变更）时 unit_tests 为空 (AC-5)', () => {
    const project = createTempProject();
    try {
      vi.mocked(execSync).mockReturnValue('');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors).toHaveLength(0);
    } finally {
      project.cleanup();
    }
  });

  it('modules: "git-change" 且 git diff 返回大量文件（100+）时不应抛错 (AC-5)', () => {
    const project = createTempProject();
    try {
      const files = Array.from({ length: 150 }, (_, i) => `src/file${i}.ts`);
      vi.mocked(execSync).mockReturnValue(files.join('\n') + '\n');
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const detected = opts.files!.map((f) => ({
          file: path.resolve(opts.projectRoot!, f),
          framework: 'vitest' as const,
        }));
        return { detected, frameworks: ['vitest'], plan: [] };
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(result.unit_tests).toHaveLength(150);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- 去重 (AC-6)
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- 去重', () => {
  beforeEach(() => {
    vi.mocked(runTestDetectFrameworks).mockReset();
  });

  it('多个 override 指向同一目录时扫描结果去重 (AC-6)', () => {
    const project = createTempProject();
    try {
      // 模拟两个 override 都指向 src/ 目录 → plan 有两个相同 directory
      vi.mocked(runTestDetectFrameworks).mockReturnValue({
        detected: [],
        frameworks: ['vitest', 'jest'],
        plan: [
          {
            directory: 'src',
            framework: 'vitest' as const,
            test_cmd:
              'npx vitest run --reporter=json --coverage --coverage.reporter=json-summary {files}',
            coverage_cmd: 'npx vitest run --coverage',
            coverage_format: 'istanbul' as const,
            coverage_output: 'coverage/coverage-summary.json',
            coverage_artifacts: ['coverage/coverage-summary.json'],
            coverage_cleanup: ['coverage', '.nyc_output'],
            script: '#!/bin/bash\nset -e\ncd src\nrm -rf coverage\nnpx vitest run --coverage\n',
          },
          {
            directory: 'src',
            framework: 'jest' as const,
            test_cmd:
              'npx jest --verbose --json --coverage --coverageReporters=json-summary {files}',
            coverage_cmd: 'npx jest --coverage',
            coverage_format: 'istanbul' as const,
            coverage_output: 'coverage/coverage-summary.json',
            coverage_artifacts: ['coverage/coverage-summary.json'],
            coverage_cleanup: ['coverage', '.nyc_output'],
            script: '#!/bin/bash\nset -e\ncd src\nrm -rf coverage\nnpx jest --coverage\n',
          },
        ],
      });

      writeFile(project.root, 'src/foo.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

      // 尽管两个 override 都指向 src/，但 src/foo.ts 只出现一次
      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe('src/foo.ts');
    } finally {
      project.cleanup();
    }
  });

  it('modules 包含重复文件路径时 unit_tests 去重 (AC-6)', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const detected = opts.files!.map((f) => ({
          file: path.resolve(opts.projectRoot!, f),
          framework: 'vitest' as const,
        }));
        return { detected, frameworks: ['vitest'], plan: [] };
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/a.ts', 'src/a.ts', 'src/a.ts'],
      });

      const sources = result.unit_tests.map((e) => e.source);
      expect(new Set(sources).size).toBe(sources.length);
      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('config-driven 扫描与 modules 显式传入产生重叠时去重 (AC-6)', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        if (opts.files === undefined) {
          // auto-scan mode — plan for src/ directory
          return {
            detected: [],
            frameworks: ['vitest'],
            plan: [
              {
                directory: 'src',
                framework: 'vitest' as const,
                test_cmd:
                  'npx vitest run --reporter=json --coverage --coverage.reporter=json-summary {files}',
                coverage_cmd: 'vp test',
                coverage_format: 'istanbul' as const,
                coverage_output: 'coverage/coverage-summary.json',
                coverage_artifacts: ['coverage/coverage-summary.json'],
                coverage_cleanup: ['coverage'],
                script: '#!/bin/bash\nset -e\ncd src\nrm -rf coverage\nvp test\n',
              },
            ],
          };
        }
        return { detected: [], frameworks: [], plan: [] };
      });

      // 既有 src/ 目录下的文件，又显式传入 src/shared.ts
      writeFile(project.root, 'src/shared.ts', '');
      writeFile(project.root, 'src/unique.ts', '');

      // 先用 empty modules 做一次扫描 (config-driven)
      const first = runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

      // 再换 mock 为显式传入模式
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        if (opts.files && opts.files.length > 0) {
          const detected = opts.files.map((f) => ({
            file: path.resolve(opts.projectRoot!, f),
            framework: 'vitest' as const,
          }));
          return { detected, frameworks: ['vitest'], plan: [] };
        }
        return { detected: [], frameworks: [], plan: [] };
      });

      // 模拟第二次调用时传入的 modules 包含 src/shared.ts（与扫描重叠）
      const second = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/shared.ts'],
      });

      // config-driven 扫描结果包含 src/shared.ts
      expect(first.unit_tests.some((e) => e.source === 'src/shared.ts')).toBe(true);

      // 各自调用去重逻辑正常
      expect(first.unit_tests).toHaveLength(2);
      expect(second.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });
});
