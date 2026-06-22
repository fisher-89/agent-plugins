/**
 * Tests for test-resolve-paths -- MCP tool that derives unit/integration test
 * file paths from module lists (files or directories).
 *
 * Covers AC-1~AC-9 from openspec/changes/add-test-path-resolver-api/test-design.md
 *
 * @see openspec/changes/add-test-path-resolver-api/test-design.md
 * @see openspec/changes/add-test-path-resolver-api/specs/test-path-resolver/spec.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import { runTestResolvePaths } from './test-resolve-paths';

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
