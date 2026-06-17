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

import {
  deriveUnitTestPath,
  deriveIntegrationTestPath,
  normalizeExtension,
  inferExtension,
  isTestFile,
  isSourceFile,
  isWithinProjectRoot,
  toPosixRelativePath,
  resolveTestPaths,
  runTestResolvePaths,
} from './test-resolve-paths';

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
// deriveUnitTestPath -- 纯函数确定性 (AC-1~AC-5)
// ===========================================================================

describe('deriveUnitTestPath -- 纯函数确定性', () => {
  it('应将 src/config.ts 推导为 src/config.test.ts (AC-1)', () => {
    expect(deriveUnitTestPath('src/config.ts')).toBe('src/config.test.ts');
  });

  it('应将 src/component/Button.tsx 推导为 src/component/Button.test.tsx (AC-2)', () => {
    expect(deriveUnitTestPath('src/component/Button.tsx')).toBe('src/component/Button.test.tsx');
  });

  it('应将 src/auth.py 推导为 src/test_auth.py (AC-3)', () => {
    expect(deriveUnitTestPath('src/auth.py')).toBe('src/test_auth.py');
  });

  it('应将 src/handler.go 推导为 src/handler_test.go (AC-4)', () => {
    expect(deriveUnitTestPath('src/handler.go')).toBe('src/handler_test.go');
  });

  it('应将 src/lib.rs 推导为 src/lib_test.rs (AC-5)', () => {
    expect(deriveUnitTestPath('src/lib.rs')).toBe('src/lib_test.rs');
  });
});

describe('deriveUnitTestPath -- JS/TS 扩展名全覆盖', () => {
  it('应为 .js 生成 <name>.test.js 同级路径', () => {
    expect(deriveUnitTestPath('src/util.js')).toBe('src/util.test.js');
  });

  it('应为 .jsx 生成 <name>.test.jsx 同级路径', () => {
    expect(deriveUnitTestPath('src/App.jsx')).toBe('src/App.test.jsx');
  });

  it('应为 .mjs 生成 <name>.test.mjs 同级路径', () => {
    expect(deriveUnitTestPath('src/module.mjs')).toBe('src/module.test.mjs');
  });

  it('应为 .cjs 生成 <name>.test.cjs 同级路径', () => {
    expect(deriveUnitTestPath('src/module.cjs')).toBe('src/module.test.cjs');
  });
});

describe('deriveUnitTestPath -- 边界', () => {
  it('相同输入多次调用应产生相同输出（不依赖 fs）', () => {
    const input = 'src/config.ts';
    const first = deriveUnitTestPath(input);
    const second = deriveUnitTestPath(input);
    expect(first).toBe(second);
    expect(first).toBe('src/config.test.ts');
  });

  it('源路径含空格时 basename 应正确保留', () => {
    expect(deriveUnitTestPath('src/my module/config.ts')).toBe('src/my module/config.test.ts');
  });

  it('源路径含 Unicode 字符时 basename 应正确保留', () => {
    expect(deriveUnitTestPath('src/测试/模块.ts')).toBe('src/测试/模块.test.ts');
  });
});

// ===========================================================================
// deriveIntegrationTestPath -- 纯函数 (AC-7)
// ===========================================================================

describe('deriveIntegrationTestPath -- 纯函数', () => {
  it('scenario="api-flow", ext="ts" 应生成 __tests__/api-flow/api-flow.test.ts (AC-7)', () => {
    expect(deriveIntegrationTestPath('api-flow', 'ts')).toBe('__tests__/api-flow/api-flow.test.ts');
  });
});

// ===========================================================================
// normalizeExtension -- 纯函数
// ===========================================================================

describe('normalizeExtension -- 正向', () => {
  it('应将 ".ts" 规范为 "ts"', () => {
    expect(normalizeExtension('.ts')).toBe('ts');
  });

  it('应将 "TS" 规范为小写 "ts"', () => {
    expect(normalizeExtension('TS')).toBe('ts');
  });
});

describe('normalizeExtension -- 边界', () => {
  it('空字符串 "" 应回退默认或按实现约定处理', () => {
    // TODO: 依实现确认空字符串行为（回退 ts 或原样传递）
    const result = normalizeExtension('');
    expect(typeof result).toBe('string');
  });
});

// ===========================================================================
// inferExtension -- 纯函数 (AC-8)
// ===========================================================================

describe('inferExtension -- 正向', () => {
  it('源文件均为 .py 时应推断 py (AC-8)', () => {
    expect(inferExtension(['src/auth.py', 'src/util.py'])).toBe('py');
  });

  it('显式 extension 应优先于众数推断', () => {
    expect(inferExtension(['src/a.py', 'src/b.py'], 'ts')).toBe('ts');
  });
});

describe('inferExtension -- 边界', () => {
  it('无有效源文件时应默认 ts', () => {
    expect(inferExtension([])).toBe('ts');
  });

  it('混合扩展名时应取众数（2×.ts + 1×.py → ts）', () => {
    expect(inferExtension(['src/a.ts', 'src/b.ts', 'src/c.py'])).toBe('ts');
  });
});

// ===========================================================================
// isTestFile / isSourceFile -- 纯函数
// ===========================================================================

describe('isTestFile / isSourceFile -- 正向', () => {
  it('应识别 foo.test.ts 为测试文件', () => {
    expect(isTestFile('src/foo.test.ts')).toBe(true);
    expect(isSourceFile('src/foo.test.ts')).toBe(false);
  });

  it('应识别 test_auth.py 为测试文件', () => {
    expect(isTestFile('src/test_auth.py')).toBe(true);
  });

  it('应识别 handler_test.go 为测试文件', () => {
    expect(isTestFile('src/handler_test.go')).toBe(true);
  });

  it('应识别 lib_test.rs 为测试文件', () => {
    expect(isTestFile('src/lib_test.rs')).toBe(true);
  });
});

describe('isTestFile / isSourceFile -- 异常', () => {
  it('README.md 应识别为非源文件', () => {
    expect(isSourceFile('README.md')).toBe(false);
    expect(isTestFile('README.md')).toBe(false);
  });

  it('config.json 应识别为非源文件', () => {
    expect(isSourceFile('config.json')).toBe(false);
  });
});

// ===========================================================================
// isWithinProjectRoot -- 纯函数
// ===========================================================================

describe('isWithinProjectRoot -- 异常', () => {
  it('../../../outside.ts 解析后越出 project_root 应返回 false', () => {
    const projectRoot = '/project/root';
    expect(isWithinProjectRoot(projectRoot, '../../../outside.ts')).toBe(false);
  });
});

// ===========================================================================
// toPosixRelativePath -- 纯函数
// ===========================================================================

describe('toPosixRelativePath -- 边界', () => {
  it('应将 Windows 反斜杠路径规范为 POSIX / 分隔符', () => {
    expect(toPosixRelativePath('src\\config.ts')).toBe('src/config.ts');
    expect(toPosixRelativePath('src\\component\\Button.tsx')).toBe('src/component/Button.tsx');
  });
});

// ===========================================================================
// resolveTestPaths -- 目录展开 (AC-6)
// ===========================================================================

describe('resolveTestPaths -- 文件路径解析', () => {
  it('多个源文件路径应各自返回同级测试文件 (AC-6)', () => {
    const project = createTempProject();
    try {
      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
// resolveTestPaths -- 集成测试路径 (AC-7, AC-8)
// ===========================================================================

describe('resolveTestPaths -- 集成测试路径', () => {
  it('integration_scenarios: ["api-flow"] + extension: "ts" (AC-7)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
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

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/auth.py'],
        integrationScenarios: ['db-roundtrip'],
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

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['README.md'],
        integrationScenarios: ['smoke'],
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

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/a.ts'],
        integrationScenarios: ['zebra', 'alpha', 'middle'],
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

      const without = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/a.ts'],
      });
      expect(without.integration_tests).toEqual([]);

      const empty = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/a.ts'],
        integrationScenarios: [],
      });
      expect(empty.integration_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// resolveTestPaths -- 错误收集 (AC-9)
// ===========================================================================

describe('resolveTestPaths -- 错误收集', () => {
  it('不存在的源文件路径仍应推导出测试路径（无需判断文件是否存在）(AC-9)', () => {
    const project = createTempProject();
    try {
      const result = resolveTestPaths({
        projectRoot: project.root,
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

      const result = resolveTestPaths({
        projectRoot: project.root,
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

      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
// resolveTestPaths -- modules 边界
// ===========================================================================

describe('resolveTestPaths -- modules 边界', () => {
  it('modules 含单元素文件路径时应返回单条 unit_tests', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/single.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
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

      const result = resolveTestPaths({
        projectRoot: project.root,
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
      const result = resolveTestPaths({
        projectRoot: project.root,
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
  it('传入 modules、integration_scenarios、extension 应返回与 resolveTestPaths 一致的结构', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');
      writeDir(project.root, 'src/commands');
      writeFile(project.root, 'src/commands/foo.ts', '');

      const direct = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts', 'src/commands/'],
        integrationScenarios: ['api-flow'],
        extension: 'ts',
      });

      const viaRunner = runTestResolvePaths({
        modules: ['src/config.ts', 'src/commands/'],
        integration_scenarios: ['api-flow'],
        extension: 'ts',
        project_root: project.root,
      });

      expect(viaRunner).toEqual(direct);
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
// add-integration-root-param — deriveIntegrationTestPath + integrationRoot
// @see openspec/changes/add-integration-root-param/test-design.md
// ===========================================================================

describe('deriveIntegrationTestPath -- integrationRoot 纯函数', () => {
  it('未传 integrationRoot 第三参数时 ("api-flow", "ts") → __tests__/api-flow/api-flow.test.ts (AC-1)', () => {
    expect(deriveIntegrationTestPath('api-flow', 'ts')).toBe('__tests__/api-flow/api-flow.test.ts');
  });

  it('integrationRoot: "." 时 → __tests__/api-flow/api-flow.test.ts (AC-2)', () => {
    expect(deriveIntegrationTestPath('api-flow', 'ts', '.')).toBe(
      '__tests__/api-flow/api-flow.test.ts',
    );
  });

  it('("api-flow", "ts", "plugins/dev-team/bin") → plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts (AC-4)', () => {
    expect(deriveIntegrationTestPath('api-flow', 'ts', 'plugins/dev-team/bin')).toBe(
      'plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts',
    );
  });

  it('integrationRoot: "plugins/dev-team/bin/" 尾部斜杠规范化后无 // 重复 (AC-5)', () => {
    const result = deriveIntegrationTestPath('api-flow', 'ts', 'plugins/dev-team/bin/');
    expect(result).toBe('plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts');
    expect(result).not.toMatch(/\/\//);
  });

  it('integrationRoot: "" 空字符串时行为与未传或 "." 一致（无前缀）', () => {
    expect(deriveIntegrationTestPath('api-flow', 'ts', '')).toBe(
      '__tests__/api-flow/api-flow.test.ts',
    );
  });

  it('Windows 反斜杠 integrationRoot: "plugins\\dev-team\\bin" 规范为 POSIX / 分隔符', () => {
    expect(deriveIntegrationTestPath('api-flow', 'ts', 'plugins\\dev-team\\bin')).toBe(
      'plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts',
    );
  });
});

describe('resolveTestPaths -- integration_root 向后兼容', () => {
  it('未传 integrationRoot + integration_scenarios: ["api-flow"] + extension: "ts" → __tests__/api-flow/api-flow.test.ts (AC-1)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
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

describe('resolveTestPaths -- integration_root 为点号', () => {
  it('integrationRoot: "." + 同上场景 → 与 AC-1 相同 (AC-2)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
        extension: 'ts',
        integrationRoot: '.',
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

describe('resolveTestPaths -- integration_root 子目录前缀', () => {
  it('integrationRoot: "plugins/dev-team/bin" + integration_scenarios: ["api-flow"] → 带子目录前缀路径 (AC-3)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
        extension: 'ts',
        integrationRoot: 'plugins/dev-team/bin',
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

describe('resolveTestPaths -- integration_root 与 unit_tests 隔离', () => {
  it('modules: ["src/config.ts"] + integrationRoot: "plugins/dev-team/bin" 时 unit_tests[0].test_file 仍为 src/config.test.ts (AC-6)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
        integrationRoot: 'plugins/dev-team/bin',
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

  it('多个 modules 含不同语言源文件时 integrationRoot 仅改变 integration_tests (AC-6)', () => {
    const project = createTempProject();
    try {
      const modules = ['src/a.ts', 'src/b.py', 'src/c.go'];

      const withoutRoot = resolveTestPaths({
        projectRoot: project.root,
        modules,
        integrationScenarios: ['api-flow'],
        extension: 'ts',
      });

      const withRoot = resolveTestPaths({
        projectRoot: project.root,
        modules,
        integrationScenarios: ['api-flow'],
        extension: 'ts',
        integrationRoot: 'plugins/dev-team/bin',
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

describe('resolveTestPaths -- integration_root 路径穿越', () => {
  it('integrationRoot: "../outside" 含 .. 段时写入 errors 且 integration_tests 为空 (D7)', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const result = resolveTestPaths({
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
        integrationRoot: '../outside',
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
  it('runTestResolvePaths({ integration_root: "plugins/dev-team/bin" }) 与 resolveTestPaths({ integrationRoot }) 结果一致', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const modules = ['src/config.ts'];
      const integrationScenarios = ['api-flow'];
      const extension = 'ts';
      const integrationRoot = 'plugins/dev-team/bin';

      const direct = resolveTestPaths({
        projectRoot: project.root,
        modules,
        integrationScenarios,
        extension,
        integrationRoot,
      });

      const viaRunner = runTestResolvePaths({
        modules,
        integration_scenarios: integrationScenarios,
        extension,
        integration_root: integrationRoot,
        project_root: project.root,
      });

      expect(viaRunner).toEqual(direct);
    } finally {
      project.cleanup();
    }
  });

  it('未传 integration_root 时 runTestResolvePaths 与 resolveTestPaths 行为与变更前一致（回归）', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/config.ts', '');

      const params = {
        projectRoot: project.root,
        modules: ['src/config.ts'],
        integrationScenarios: ['api-flow'],
        extension: 'ts',
      };

      const direct = resolveTestPaths(params);
      const viaRunner = runTestResolvePaths({
        modules: params.modules,
        integration_scenarios: params.integrationScenarios,
        extension: params.extension,
        project_root: project.root,
      });

      expect(viaRunner).toEqual(direct);
    } finally {
      project.cleanup();
    }
  });
});
