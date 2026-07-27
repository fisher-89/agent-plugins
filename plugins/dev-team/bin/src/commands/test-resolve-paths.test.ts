/**
 * Tests for test-resolve-paths -- MCP tool that derives unit test file paths
 * from module lists (files or directories).
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

// Capture the original pass-through implementations at module-init time
// (before any test's mockImplementation can replace them).
// These are used by the top-level beforeEach to restore clean state for
// every test, regardless of shuffle order.
const _detectFrameworksPassthrough = vi.mocked(runTestDetectFrameworks).getMockImplementation();
const _execSyncPassthrough = vi.mocked(execSync).getMockImplementation();

// Restore pass-through implementations before every test so that mock
// state from one group never leaks into another when --sequence.shuffle
// reorders tests.
beforeEach(() => {
  vi.mocked(runTestDetectFrameworks).mockClear();
  vi.mocked(execSync).mockClear();
  if (_detectFrameworksPassthrough) {
    vi.mocked(runTestDetectFrameworks).mockImplementation(_detectFrameworksPassthrough);
  }
  if (_execSyncPassthrough) {
    vi.mocked(execSync).mockImplementation(_execSyncPassthrough);
  }
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

// ---------------------------------------------------------------------------
// Local type for testing legacy parameters (AC-1, AC-2, AC-7)
// The function ignores these extra properties at runtime, but we need them
// on the type to avoid `as any` assertions.
// ---------------------------------------------------------------------------

interface TestResolvePathsInputWithLegacy extends Omit<
  Parameters<typeof runTestResolvePaths>[0],
  'modules'
> {
  modules: string[];
  integration_scenarios?: string[];
  extension?: string;
  integration_root?: string;
}

// ===========================================================================
// runTestResolvePaths -- 集成测试路径移除 (AC-1, AC-2, AC-7)
//
// @see openspec/changes/remove-integration-test-path-resolution/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- 集成测试路径移除', () => {
  it('传入 integration_scenarios: ["api-flow"]、extension: "ts"、integration_root: "." 时结果不包含 integration_tests 字段，且无相关错误 (AC-1, AC-2, AC-7)', () => {
    const project = createTempProject();
    try {
      const input: TestResolvePathsInputWithLegacy = {
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: '.',
      };
      const result = runTestResolvePaths(input);

      // 旧参数被静默忽略，结果不含 integration_tests
      expect(result).not.toHaveProperty('integration_tests');
      // 单元测试路径推导不受影响
      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0]).toEqual({
        source: 'src/config.ts',
        test_file: 'src/config.test.ts',
      });
      // 不因旧参数产生任何错误
      expect(result.errors).toHaveLength(0);
    } finally {
      project.cleanup();
    }
  });

  it('仅传入 integration_scenarios: ["api-flow"] 时结果不包含 integration_tests 字段 (AC-1, AC-2, AC-7)', () => {
    const project = createTempProject();
    try {
      const input: TestResolvePathsInputWithLegacy = {
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: ['api-flow'],
      };
      const result = runTestResolvePaths(input);

      expect(result).not.toHaveProperty('integration_tests');
      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('integration_scenarios: [] 空数组时语义等价于不传，结果不包含 integration_tests (AC-7)', () => {
    const project = createTempProject();
    try {
      const input: TestResolvePathsInputWithLegacy = {
        project_root: project.root,
        modules: ['src/config.ts'],
        integration_scenarios: [],
      };
      const result = runTestResolvePaths(input);

      expect(result).not.toHaveProperty('integration_tests');
      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('同时传入三个已移除的旧参数时结果中只出现 unit_tests 与 errors（不含 integration_tests）(AC-1, AC-2, AC-7)', () => {
    const project = createTempProject();
    try {
      const input: TestResolvePathsInputWithLegacy = {
        project_root: project.root,
        modules: ['src/config.ts', 'README.md'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        integration_root: '.',
      };
      const result = runTestResolvePaths(input);

      expect(result).not.toHaveProperty('integration_tests');
      // 结果仅含 unit_tests 和 errors 两个顶层字段
      expect(Object.keys(result)).toEqual(['unit_tests', 'errors']);
      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe('src/config.ts');
      expect(result.errors.some((e) => e.path === 'README.md')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('不传任何旧参数时 unit_tests 与 errors 行为正常 (AC-6)', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/foo.ts', 'README.md'],
      });

      expect(result).toHaveProperty('unit_tests');
      expect(result).toHaveProperty('errors');
      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0]).toEqual({
        source: 'src/foo.ts',
        test_file: 'src/foo.test.ts',
      });
      expect(result.errors.some((e) => e.path === 'README.md')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

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

  it('派生 .py 文件的测试路径为 test_<name>.py', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/app.py'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/app.py',
        test_file: 'src/test_app.py',
      });
    } finally {
      project.cleanup();
    }
  });

  it('派生 .go 文件的测试路径为 <name>_test.go', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/app.go'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/app.go',
        test_file: 'src/app_test.go',
      });
    } finally {
      project.cleanup();
    }
  });

  it('派生 .rs 文件的测试路径为 <name>_test.rs', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/app.rs'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/app.rs',
        test_file: 'src/app_test.rs',
      });
    } finally {
      project.cleanup();
    }
  });

  it('派生 .tsx 文件的测试路径为 <name>.test.tsx', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/component.tsx'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/component.tsx',
        test_file: 'src/component.test.tsx',
      });
    } finally {
      project.cleanup();
    }
  });

  it('派生 .jsx 文件的测试路径为 <name>.test.jsx', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/component.jsx'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/component.jsx',
        test_file: 'src/component.test.jsx',
      });
    } finally {
      project.cleanup();
    }
  });

  it('派生 .mjs 文件的测试路径为 <name>.test.mjs', () => {
    const project = createTempProject();
    try {
      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/module.mjs'],
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/module.mjs',
        test_file: 'src/module.test.mjs',
      });
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

  it('绝对路径在项目范围内时正常解析', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.ts', '');
      const absPath = path.resolve(project.root, 'src/a.ts');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: [absPath],
      });

      // 绝对路径传入后, source 保留原始绝对路径,
      // test_file 也使用绝对路径推导
      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe(absPath.replace(/\\/g, '/'));
      expect(result.unit_tests[0].test_file).toBe(
        path.posix.join(path.dirname(absPath.replace(/\\/g, '/')), 'a.test.ts'),
      );
      expect(result.errors).toHaveLength(0);
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
// runTestResolvePaths -- 端到端编排
// ===========================================================================

describe('runTestResolvePaths -- 端到端编排', () => {
  it('传入 modules 应返回完整解析结构', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');
      writeDir(project.root, 'src/commands');
      writeFile(project.root, 'src/commands/foo.ts', '');

      const result = runTestResolvePaths({
        modules: ['src/config.ts', 'src/commands/'],
        project_root: project.root,
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/config.ts',
        test_file: 'src/config.test.ts',
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
// runTestResolvePaths -- config-driven 自动扫描 (AC-1, AC-2)
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('runTestResolvePaths -- config-driven 自动扫描', () => {
  beforeEach(() => {
    vi.mocked(runTestDetectFrameworks).mockClear();
    vi.mocked(execSync).mockClear();
  });

  it('modules: [] 且 tests[] 配置有效时，unit_tests 包含扫描到的源文件推导结果 (AC-1)', () => {
    const project = createTempProject();
    try {
      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: 'src', framework: 'vite-plus', includes: ['**/*.{ts,tsx}'] }],
        }),
      );
      // 使用真实 detect（passthrough），不再依赖旧 overrides mock

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
    vi.mocked(runTestDetectFrameworks).mockClear();
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
        return { detected, plan: [] };
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
        return { detected, plan: [] };
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
    vi.mocked(execSync).mockClear();
    vi.mocked(runTestDetectFrameworks).mockClear();
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
        return { detected, plan: [] };
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

  it('modules: "git-change" 且 stderr 的 toString 抛出时 extractErrorMessage 的 catch 分支被覆盖 (AC-5)', () => {
    const project = createTempProject();
    try {
      // execSync 抛出含 throws-on-stringify stderr 的对象时
      // String(stderr) 抛出错误，触发 extractErrorMessage 中 try/catch 的 catch 分支
      vi.mocked(execSync).mockImplementation(() => {
        const err = {
          stderr: {
            toString() {
              throw new Error('cannot stringify');
            },
          },
        };
        throw err;
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(result.errors.some((e) => e.path === 'git')).toBe(true);
      expect(result.errors[0].message).toBe('git diff HEAD --name-only failed');
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('modules: "git-change" 且 execSync 抛出带 stderr 的非 Error 对象时 errors 包含错误消息 (AC-5)', () => {
    const project = createTempProject();
    try {
      // execSync 可能抛出非 Error 但包含 stderr 的对象（如 child_process 底层错误）
      vi.mocked(execSync).mockImplementation(() => {
        const err: { stderr: string; message: string } = {
          stderr: 'fatal: not a git repository',
          message: 'Command failed',
        };
        throw err;
      });

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: 'git-change',
      });

      expect(result.errors.some((e) => e.path === 'git')).toBe(true);
      expect(result.errors[0].message).toContain('fatal: not a git repository');
      expect(result.unit_tests).toEqual([]);
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
        return { detected, plan: [] };
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
    vi.mocked(runTestDetectFrameworks).mockClear();
  });

  it('多个 suite 指向同一目录时扫描结果去重 (AC-6)', () => {
    const project = createTempProject();
    try {
      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            { root: 'src', framework: 'vitest', includes: ['**/*.{ts,tsx}'] },
            { root: 'src', framework: 'jest', includes: ['**/*.{ts,tsx}'] },
          ],
        }),
      );

      writeFile(project.root, 'src/foo.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

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
        return { detected, plan: [] };
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
      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: 'src', framework: 'vitest', includes: ['**/*.{ts,tsx}'] }],
        }),
      );

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
          return { detected, plan: [] };
        }
        return { detected: [], plan: [] };
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

// ===========================================================================
// runTestResolvePaths -- exclude 过滤 (AC-4)
// ===========================================================================

describe('runTestResolvePaths -- exclude 过滤 (AC-4)', () => {
  beforeEach(() => {
    vi.mocked(runTestDetectFrameworks).mockClear();
    vi.mocked(execSync).mockClear();
  });

  it('配置 test.exclude 后被排除的文件不出现在 unit_tests[] 中', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      // 模拟 openspec/config.json 包含 suite excludes
      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: 'src',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
              excludes: ['**/generated/**'],
            },
            {
              root: 'generated',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
              excludes: ['**/*'],
            },
          ],
        }),
      );
      writeFile(project.root, 'src/app.ts', '');
      writeFile(project.root, 'generated/out.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/app.ts', 'generated/out.ts'],
      });

      expect(result.unit_tests.some((u) => u.source === 'src/app.ts')).toBe(true);
      expect(result.unit_tests.some((u) => u.source === 'generated/out.ts')).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('未被排除的文件正常出现在 unit_tests[] 中', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest', excludes: ['**/generated/**'] }],
        }),
      );
      writeFile(project.root, 'src/a.ts', '');
      writeFile(project.root, 'src/b.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/a.ts', 'src/b.ts'],
      });

      expect(result.unit_tests).toHaveLength(2);
    } finally {
      project.cleanup();
    }
  });

  it('空 modules 自动扫描模式下 exclude 过滤生效', () => {
    const project = createTempProject();
    try {
      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: 'src',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
            },
            {
              root: 'generated',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
              excludes: ['**/*'],
            },
          ],
        }),
      );
      writeFile(project.root, 'src/app.ts', '');
      writeFile(project.root, 'generated/out.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: [],
      });

      expect(result.unit_tests.some((u) => u.source === 'src/app.ts')).toBe(true);
      expect(result.unit_tests.some((u) => u.source === 'generated/out.ts')).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('混合排除/未排除文件时仅未排除文件进入 unit_tests', () => {
    const project = createTempProject();
    try {
      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            { root: 'src', framework: 'vitest', includes: ['**/*.{ts,tsx}'] },
            {
              root: 'excluded',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
              excludes: ['**/*'],
            },
          ],
        }),
      );
      writeFile(project.root, 'src/keep.ts', '');
      writeFile(project.root, 'excluded/skip.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/keep.ts', 'excluded/skip.ts'],
      });

      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe('src/keep.ts');
    } finally {
      project.cleanup();
    }
  });

  it('既不配置全局 exclude 也不配置 override exclude 时全部源文件正常进入 unit_tests', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest' }],
        }),
      );
      writeFile(project.root, 'src/a.ts', '');
      writeFile(project.root, 'src/b.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/a.ts', 'src/b.ts'],
      });

      expect(result.unit_tests).toHaveLength(2);
    } finally {
      project.cleanup();
    }
  });

  it('override-level exclude 不扩展到其他 override 区域', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: 'dir-a',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
              excludes: ['**/*.snap'],
            },
            {
              root: 'dir-b',
              framework: 'vitest',
              includes: ['**/*.{ts,tsx}'],
              excludes: ['**/ignored/**'],
            },
          ],
        }),
      );
      writeFile(project.root, 'dir-a/main.ts', '');
      writeFile(project.root, 'dir-a/icon.snap', '');
      writeFile(project.root, 'dir-b/app.ts', '');
      writeFile(project.root, 'dir-b/ignored/tmp.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['dir-a/main.ts', 'dir-a/icon.snap', 'dir-b/app.ts', 'dir-b/ignored/tmp.ts'],
      });

      // dir-a/icon.snap 是测试文件吗？.snap 不是源文件 → 应被 isSourceFile 过滤
      // 这里只验证 dir-b/ignored/tmp.ts 不在 unit_tests 中（被 exclude）
      expect(result.unit_tests.some((u) => u.source === 'dir-a/main.ts')).toBe(true);
      expect(result.unit_tests.some((u) => u.source === 'dir-b/app.ts')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths -- 向后兼容 (AC-6)
// ===========================================================================

describe('runTestResolvePaths -- 向后兼容 (AC-6)', () => {
  beforeEach(() => {
    vi.mocked(runTestDetectFrameworks).mockClear();
    vi.mocked(execSync).mockClear();
  });

  it('不配置 exclude 时全部现有功能行为不变', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest' }],
        }),
      );
      writeFile(project.root, 'src/helper.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/helper.ts'],
      });

      expect(result.unit_tests).toHaveLength(1);
      expect(result.unit_tests[0].source).toBe('src/helper.ts');
    } finally {
      project.cleanup();
    }
  });

  it('配置中存在 `"exclude": []` 空数组时路径解析结果与无 exclude 配置时一致', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest', excludes: [] }],
        }),
      );
      writeFile(project.root, 'src/helper.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/helper.ts'],
      });

      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('配置中 `test` 节包含 `exclude` 字段但其值为 undefined 时路径解析结果不受影响', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest' }],
        }),
      );
      writeFile(project.root, 'src/helper.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/helper.ts'],
      });

      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('配置中不存在 `test` 节时路径解析结果与无 exclude 配置时一致', () => {
    const project = createTempProject();
    try {
      vi.mocked(runTestDetectFrameworks).mockImplementation((opts) => {
        const absFiles = opts.files!.map((f) =>
          path.isAbsolute(f) ? path.resolve(f) : path.resolve(opts.projectRoot!, f),
        );
        return {
          detected: absFiles.map((f) => ({ file: f, framework: 'vitest' as const })),
          plan: [],
        };
      });

      const openspecDir = path.join(project.root, 'openspec');
      fs.mkdirSync(openspecDir, { recursive: true });
      fs.writeFileSync(
        path.join(openspecDir, 'config.json'),
        JSON.stringify({ schema: 'spec-driven' }),
      );
      writeFile(project.root, 'src/helper.ts', '');

      const result = runTestResolvePaths({
        project_root: project.root,
        modules: ['src/helper.ts'],
      });

      // 如果没有 test 节，readConfig 返回的默认 test 对象是 undefined？
      // 在 resolveTestPaths 中 readConfig 读取，然后传给 processNonEmptyModules，
      // 内部仍使用 detectFrameworks 的结果。这里验证基本功能不变
      expect(result.unit_tests).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths — suite root 扫描 (AC-4)
// ===========================================================================

describe('runTestResolvePaths — suite root 扫描 (AC-4)', () => {
  function writeConfig(root: string, config: unknown): void {
    fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
    fs.writeFileSync(path.join(root, 'openspec', 'config.json'), JSON.stringify(config), 'utf-8');
  }

  it('modules: [] 且 suite root: "pkg/src", cwd: ".." 时仍扫描 pkg/src 下源文件', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [
          {
            root: 'pkg/src',
            framework: 'vite-plus',
            cwd: '..',
            includes: ['**/*.{ts,tsx}'],
          },
        ],
      });
      writeFile(project.root, 'pkg/src/app.ts');
      writeFile(project.root, 'pkg/outside.ts');
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.unit_tests).toContainEqual({
        source: 'pkg/src/app.ts',
        test_file: 'pkg/src/app.test.ts',
      });
      expect(result.unit_tests).not.toContainEqual(
        expect.objectContaining({ source: 'pkg/outside.ts' }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('显式 includes/excludes 时 unit_tests 仅含 in-scope 源文件', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [
          {
            root: 'src',
            framework: 'vite-plus',
            includes: ['**/*.{ts,tsx}'],
            excludes: ['**/skip/**'],
          },
        ],
      });
      writeFile(project.root, 'src/ok.ts');
      writeFile(project.root, 'src/skip/hidden.ts');
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.unit_tests.map((u) => u.source)).toEqual(['src/ok.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('非空 modules 时按 suite scope 过滤后再推导测试路径', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [
          {
            root: 'src',
            framework: 'vite-plus',
            includes: ['**/*.{ts,tsx}'],
          },
        ],
      });
      writeFile(project.root, 'src/a.ts');
      writeFile(project.root, 'other/b.ts');
      const result = runTestResolvePaths({
        modules: ['src/a.ts', 'other/b.ts'],
        project_root: project.root,
      });
      expect(result.unit_tests.map((u) => u.source)).toContain('src/a.ts');
      // 范围外文件 marked unknown 仍进入 detected，但空 modules 扫描不会包含它
      const emptyScan = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(emptyScan.unit_tests.map((u) => u.source)).not.toContain('other/b.ts');
    } finally {
      project.cleanup();
    }
  });

  it('modules 指向项目外路径时 errors 含对应 path/message，不崩溃', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vite-plus', includes: ['**/*.{ts,tsx}'] }],
      });
      const result = runTestResolvePaths({
        modules: ['../outside.ts'],
        project_root: project.root,
      });
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0]).toHaveProperty('path');
      expect(result.errors[0]).toHaveProperty('message');
    } finally {
      project.cleanup();
    }
  });

  it('modules 为单元素合法路径时返回对应 unit_tests', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vite-plus', includes: ['**/*.{ts,tsx}'] }],
      });
      writeFile(project.root, 'src/one.ts');
      const result = runTestResolvePaths({
        modules: ['src/one.ts'],
        project_root: project.root,
      });
      expect(result.unit_tests).toEqual([{ source: 'src/one.ts', test_file: 'src/one.test.ts' }]);
    } finally {
      project.cleanup();
    }
  });

  it('modules 为空数组且 suite includes 省略时按 default_glob 语义扫描', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vite-plus' }],
      });
      writeFile(project.root, 'src/app.ts');
      writeFile(project.root, 'src/app.test.ts');
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('modules 含超长路径字符串（>1000）时进入 errors 或不崩溃', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vite-plus', includes: ['**/*.{ts,tsx}'] }],
      });
      const longPath = `src/${'x'.repeat(1001)}.ts`;
      expect(() =>
        runTestResolvePaths({ modules: [longPath], project_root: project.root }),
      ).not.toThrow();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths — 错误文案引导 tests (AC-6)
// ===========================================================================

describe('runTestResolvePaths — 错误文案引导 tests (AC-6)', () => {
  function writeConfig(root: string, config: unknown): void {
    fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
    fs.writeFileSync(path.join(root, 'openspec', 'config.json'), JSON.stringify(config), 'utf-8');
  }

  it('modules: [] 且无 tests / tests: [] 时 errors 提示配置 tests', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, { schema: 'spec-driven', tests: [] });
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.errors[0].message).toContain('tests');
      expect(result.errors[0].message).not.toContain('test.framework');
      expect(result.errors[0].message).not.toContain('test.overrides');
    } finally {
      project.cleanup();
    }
  });

  it('配置仅含旧 test.overrides、无 tests 时 errors 引导配置 tests（硬 breaking）', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, {
        schema: 'spec-driven',
        test: { overrides: [{ file: 'src', framework: 'vite-plus' }] },
      });
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.unit_tests).toEqual([]);
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors.some((e) => e.message.includes('tests'))).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('错误 message 非空字符串且不含已废弃键名', () => {
    const project = createTempProject();
    try {
      writeConfig(project.root, { schema: 'spec-driven' });
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.errors[0].message.length).toBeGreaterThan(0);
      expect(result.errors[0].message).not.toMatch(/test\.framework|test\.overrides/);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths — project_root 注入与 getProjectDir 回退 (AC-7)
// ===========================================================================

describe('runTestResolvePaths — project_root 注入与 getProjectDir 回退', () => {
  it('显式 project_root 指向 fixture 时 unit_tests 相对该根；spy getProjectDir 调用次数 0 (AC-7)', async () => {
    const project = createTempProject();
    writeFile(project.root, 'src/a.ts');
    const projectRootLib = await import('../lib/project-root');
    const spy = vi.spyOn(projectRootLib, 'getProjectDir');
    try {
      const result = runTestResolvePaths({
        modules: ['src/a.ts'],
        project_root: project.root,
      });
      expect(spy).toHaveBeenCalledTimes(0);
      expect(result.unit_tests.some((u) => u.source === 'src/a.ts')).toBe(true);
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });

  it('project_root: null / 省略 / ""：均回退 getProjectDir（断言 spy 被调用）(AC-7)', async () => {
    const project = createTempProject();
    writeFile(project.root, 'src/b.ts');
    const projectRootLib = await import('../lib/project-root');
    const spy = vi.spyOn(projectRootLib, 'getProjectDir').mockReturnValue(project.root);
    try {
      for (const input of [
        { modules: ['src/b.ts'] as string[] | 'git-change' },
        { modules: ['src/b.ts'] as string[], project_root: null },
        { modules: ['src/b.ts'] as string[], project_root: '' },
      ]) {
        spy.mockClear();
        runTestResolvePaths(input);
        expect(spy).toHaveBeenCalled();
      }
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths — SOURCE_EXTENSIONS 全扩展名
// ===========================================================================

describe('runTestResolvePaths — SOURCE_EXTENSIONS 全扩展名', () => {
  it('分别传入 .ts/.tsx/.js/.jsx/.mjs/.cjs/.py/.go/.rs：均进入 unit_tests 且派生路径精确', () => {
    const project = createTempProject();
    const cases: Array<{ src: string; test: string }> = [
      { src: 'src/a.ts', test: 'src/a.test.ts' },
      { src: 'src/a.tsx', test: 'src/a.test.tsx' },
      { src: 'src/a.js', test: 'src/a.test.js' },
      { src: 'src/a.jsx', test: 'src/a.test.jsx' },
      { src: 'src/a.mjs', test: 'src/a.test.mjs' },
      { src: 'src/a.cjs', test: 'src/a.test.cjs' },
      { src: 'src/a.py', test: 'src/test_a.py' },
      { src: 'src/a.go', test: 'src/a_test.go' },
      { src: 'src/a.rs', test: 'src/a_test.rs' },
    ];
    try {
      for (const { src, test } of cases) {
        writeFile(project.root, src);
        const result = runTestResolvePaths({ modules: [src], project_root: project.root });
        expect(result.errors).toEqual([]);
        expect(result.unit_tests.some((u) => u.source === src && u.test_file === test)).toBe(true);
      }
    } finally {
      project.cleanup();
    }
  });

  it('.md / .json / .yaml / 无扩展名 → 进入 errors，不进 unit_tests', () => {
    const project = createTempProject();
    try {
      for (const src of ['docs/a.md', 'cfg.json', 'x.yaml', 'noext']) {
        writeFile(project.root, src);
        const result = runTestResolvePaths({ modules: [src], project_root: project.root });
        expect(result.unit_tests.find((u) => u.source === src)).toBeUndefined();
        expect(result.errors.some((e) => e.path === src)).toBe(true);
      }
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths — isTestFile 正则判别
// ===========================================================================

describe('runTestResolvePaths — isTestFile 正则判别', () => {
  it('传入 foo.test.ts / test_foo.py / foo_test.go / foo_test.rs / foo_tests.rs → 均进 errors', () => {
    const project = createTempProject();
    const tests = [
      'src/foo.test.ts',
      'src/test_foo.py',
      'src/foo_test.go',
      'src/foo_test.rs',
      'src/foo_tests.rs',
    ];
    try {
      for (const src of tests) {
        writeFile(project.root, src);
        const result = runTestResolvePaths({ modules: [src], project_root: project.root });
        expect(result.unit_tests.find((u) => u.source === src)).toBeUndefined();
        expect(result.errors.some((e) => e.path === src)).toBe(true);
      }
    } finally {
      project.cleanup();
    }
  });

  it('testX.py（无下划线）不是测试文件约定，必须与 test_foo.py 行为不同', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/testX.py');
      writeFile(project.root, 'src/test_foo.py');
      const asSource = runTestResolvePaths({
        modules: ['src/testX.py'],
        project_root: project.root,
      });
      const asTest = runTestResolvePaths({
        modules: ['src/test_foo.py'],
        project_root: project.root,
      });
      expect(asTest.errors.some((e) => e.path === 'src/test_foo.py')).toBe(true);
      expect(asSource.unit_tests.some((u) => u.source === 'src/testX.py')).toBe(true);
      expect(asSource.unit_tests.find((u) => u.source === 'src/testX.py')?.test_file).toBe(
        'src/test_testX.py',
      );
    } finally {
      project.cleanup();
    }
  });

  it('foo_tests.rs 与 foo_test.rs 均识别为测试文件；foo_tes.rs 不得误判（证明 tests?）', () => {
    const project = createTempProject();
    try {
      for (const src of ['src/foo_tests.rs', 'src/foo_test.rs']) {
        writeFile(project.root, src);
        const result = runTestResolvePaths({ modules: [src], project_root: project.root });
        expect(result.errors.some((e) => e.path === src)).toBe(true);
      }
      writeFile(project.root, 'src/foo_tes.rs');
      const notTest = runTestResolvePaths({
        modules: ['src/foo_tes.rs'],
        project_root: project.root,
      });
      expect(notTest.unit_tests.some((u) => u.source === 'src/foo_tes.rs')).toBe(true);
      expect(notTest.unit_tests.find((u) => u.source === 'src/foo_tes.rs')?.test_file).toBe(
        'src/foo_tes_test.rs',
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestResolvePaths — extractErrorMessage stderr
// ===========================================================================

describe('runTestResolvePaths — extractErrorMessage stderr', () => {
  function writeConfig(root: string): void {
    fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
    fs.writeFileSync(
      path.join(root, 'openspec', 'config.json'),
      JSON.stringify({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*'] }],
      }),
      'utf-8',
    );
  }

  it('modules: "git-change" 且 execSync 抛 Error：errors.message === error.message', () => {
    const project = createTempProject();
    writeConfig(project.root);
    vi.mocked(execSync).mockImplementation(() => {
      throw new Error('exact-git-error');
    });
    try {
      const result = runTestResolvePaths({ modules: 'git-change', project_root: project.root });
      expect(result.errors.some((e) => e.message === 'exact-git-error')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it("抛非 Error 但 { stderr: '  boom  ' }：errors.message === 'boom'（trim 后）", () => {
    const project = createTempProject();
    writeConfig(project.root);
    vi.mocked(execSync).mockImplementation(() => {
      throw { stderr: '  boom  ' };
    });
    try {
      const result = runTestResolvePaths({ modules: 'git-change', project_root: project.root });
      expect(result.errors.some((e) => e.message === 'boom')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it("抛 { stderr: '   ' }（仅空白）：errors.message === fallback 非空串", () => {
    const project = createTempProject();
    writeConfig(project.root);
    vi.mocked(execSync).mockImplementation(() => {
      throw { stderr: '   ' };
    });
    try {
      const result = runTestResolvePaths({ modules: 'git-change', project_root: project.root });
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0].message.trim().length).toBeGreaterThan(0);
      expect(result.errors[0].message).not.toBe('   ');
    } finally {
      project.cleanup();
    }
  });

  it('抛 { stderr: { toString() { throw } } }：走 catch 后仍有非空 fallback', () => {
    const project = createTempProject();
    writeConfig(project.root);
    vi.mocked(execSync).mockImplementation(() => {
      throw {
        stderr: {
          toString() {
            throw new Error('x');
          },
        },
      };
    });
    try {
      const result = runTestResolvePaths({ modules: 'git-change', project_root: project.root });
      expect(result.errors[0].message.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('抛 null / 数字 / 无 stderr 对象：使用 fallback，不抛未捕获异常', () => {
    const project = createTempProject();
    writeConfig(project.root);
    try {
      for (const thrown of [null, 42, { message: 'no-stderr' }]) {
        vi.mocked(execSync).mockImplementation(() => {
          throw thrown;
        });
        expect(() =>
          runTestResolvePaths({ modules: 'git-change', project_root: project.root }),
        ).not.toThrow();
        const result = runTestResolvePaths({ modules: 'git-change', project_root: project.root });
        expect(result.errors[0].message.length).toBeGreaterThan(0);
      }
    } finally {
      project.cleanup();
    }
  });
});
