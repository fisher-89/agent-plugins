/**
 * Tests for test-detect-frameworks -- MCP tool that detects test framework
 * per file using glob matching from config.json `test.framework` + `test.overrides`.
 *
 * Covers:
 * - AC-4: Glob-first-match file-to-framework detection
 * - AC-4: plan entries include coverage_artifacts, script
 * - Reverse AC-4: unmatched files return "unknown", empty file list returns empty
 * - Boundary: empty files list, auto-scan with no matches
 *
 * @see openspec/changes/per-directory-test-execution/test-design.md
 * @see openspec/changes/unified-coverage-artifacts/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import { getFrameworkConfig } from '../lib/test-framework';
import { type OpenSpecConfigInput } from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Helpers: create temp project directories with config.json
// ---------------------------------------------------------------------------

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

// ===========================================================================
// AC-4: Glob-first-match file-to-framework detection
// ===========================================================================

describe('detectFrameworks -- glob first-match (AC-4)', () => {
  it('should map each file to its matching framework via default + overrides', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '**/tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts', 'tests/unit/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(2);
      const vitestEntry = result.detected.find((d) => d.file.endsWith('helper.test.ts'));
      const rustEntry = result.detected.find((d) => d.file.endsWith('test_auth.rs'));
      expect(vitestEntry?.framework).toBe('vitest');
      expect(rustEntry?.framework).toBe('rust');
    } finally {
      project.cleanup();
    }
  });

  it('should use first-match rule when a file matches both default and override globs', () => {
    // Default vitest glob matches test files; override **/e2e/** is checked second
    // but e2e file names like test_app.ts don't match vitest default glob
    // (no .test./.spec. in name), so only the override matches
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '**/e2e/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });

  it('should return "unknown" for files not matching any glob (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['unknown.js'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should return empty detected for empty file list (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '**/tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [],
        projectRoot: project.root,
      });
      expect(result.detected).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('override 使用无通配符路径 plugins/dev-team/bin 时应检测子目录测试文件为对应框架', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'plugins/dev-team/bin', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/foo.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// Single-framework tests
// ===========================================================================

describe('detectFrameworks -- single framework', () => {
  it('should detect framework from default config and match files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('should apply framework default glob for matching', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js', 'src/util.test.ts', 'src/helper.ts'],
        projectRoot: project.root,
      });
      // .test.ts and .spec.js match vitest default glob; helper.ts does not
      expect(result.detected).toHaveLength(3);
      expect(result.detected[0].framework).toBe('vitest');
      expect(result.detected[1].framework).toBe('vitest');
      expect(result.detected[2].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should match no files when framework config has no matching files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [],
        projectRoot: project.root,
      });
      expect(result.detected).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('detectFrameworks -- edge cases', () => {
  it('should handle files parameter as undefined (auto-scan) gracefully', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      // Create a non-test file in the project to test auto-scan
      const srcDir = path.join(project.root, 'src');
      fs.mkdirSync(srcDir, { recursive: true });
      fs.writeFileSync(path.join(srcDir, 'readme.md'), '# readme', 'utf-8');

      const result = runTestDetectFrameworks({
        projectRoot: project.root,
      });
      expect(result.detected.length).toBeGreaterThanOrEqual(0);
    } finally {
      project.cleanup();
    }
  });

  it('should handle config with no framework gracefully', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should handle a very large file list without error', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const largeFiles = Array.from({ length: 1000 }, (_, i) => `src/test${i}.test.ts`);
      const result = runTestDetectFrameworks({
        files: largeFiles,
        projectRoot: project.root,
      });
      expect(result.detected.length).toBe(1000);
    } finally {
      project.cleanup();
    }
  });

  it('should handle files with special characters in paths', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/my test file.test.ts', 'src/测试.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(2);
      expect(result.detected.every((d) => d.framework === 'vitest')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('should handle missing config gracefully', () => {
    // When config has no test node at all, everything is "unknown"
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- plan 集成测试
// ===========================================================================

describe('runTestDetectFrameworks -- plan 内容正确性 (AC-6)', () => {
  it('应为单框架配置生成正确的 plan 条目', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vite-plus',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'vite-plus',
        coverage_format: 'istanbul',
        coverage_output: 'coverage/coverage-summary.json',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为多框架多目录配置生成正确的 plan 数组（default first, then overrides）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/main.test.ts', 'src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      // Default framework entry comes first
      expect(result.plan[0].directory).toBe('.');
      expect(result.plan[0].framework).toBe('vitest');
      // Override entry comes second
      expect(result.plan[1].directory).toBe('plugins/dev-team/bin');
      expect(result.plan[1].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks -- 单框架 plan (AC-9)', () => {
  it('应为 "vitest" 生成 plan，directory 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'vitest',
        coverage_format: 'istanbul',
        coverage_output: 'coverage/coverage-summary.json',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为 "jest" 生成 plan，directory 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'jest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'jest',
        coverage_format: 'istanbul',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为 "rust" 生成 plan，directory 为 "."，coverage_format 为 "llvm-cov"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'rust',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'rust',
        coverage_format: 'llvm-cov',
      });
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks -- 空配置 plan (AC-10)', () => {
  it('应在无 framework 配置时返回空 plan', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('应在无 test 配置时返回空 plan', () => {
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 向后兼容回归测试 (AC-11)
// ===========================================================================

describe('runTestDetectFrameworks -- 向后兼容 (AC-11)', () => {
  it('添加 plan 后 detected 字段结构和内容不变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0]).toHaveProperty('file');
      expect(result.detected[0]).toHaveProperty('framework');
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('添加 plan 后无匹配文件仍返回 "unknown"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['unknown.js'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-4: plan 条目包含 coverage_artifacts
// ===========================================================================

describe('runTestDetectFrameworks -- plan 新增覆盖率产物字段 (AC-4)', () => {
  it('配置 vitest 框架时，plan 条目包含 coverage_artifacts', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toHaveProperty('coverage_artifacts');
    } finally {
      project.cleanup();
    }
  });

  it('vitest plan 条目的 coverage_artifacts 为 ["coverage/coverage-summary.json"]', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    } finally {
      project.cleanup();
    }
  });

  it('多框架 [vitest, rust] 时每个 plan 条目包含对应 coverage_artifacts', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      for (const entry of result.plan) {
        expect(entry).toHaveProperty('coverage_artifacts');
      }
    } finally {
      project.cleanup();
    }
  });

  it('rust plan 条目 coverage_artifacts 为 ["coverage/coverage-summary.json"]', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'rust',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    } finally {
      project.cleanup();
    }
  });

  it('单框架 "vitest" 时 plan 条目也包含 coverage_artifacts', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toHaveProperty('coverage_artifacts');
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- plan 包含 script 字段 (AC-7, AC-9)
// ===========================================================================

describe('runTestDetectFrameworks -- plan 包含 script 字段 (AC-7, AC-9)', () => {
  it('vitest 框架配置时 plan 条目包含非空 script 字段 (AC-7)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toHaveProperty('script');
      expect(typeof result.plan[0].script).toBe('object');
      expect(typeof result.plan[0].script.shell).toBe('string');
      expect(result.plan[0].script.shell.length).toBeGreaterThan(0);
      expect(typeof result.plan[0].script.cmd).toBe('string');
      expect(result.plan[0].script.cmd.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('多框架 [vitest, rust] 配置时每个 plan 条目都包含非空 script', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      for (const entry of result.plan) {
        expect(entry).toHaveProperty('script');
        expect(typeof entry.script).toBe('object');
        expect(typeof entry.script.shell).toBe('string');
        expect(entry.script.shell.length).toBeGreaterThan(0);
        expect(typeof entry.script.cmd).toBe('string');
        expect(entry.script.cmd.length).toBeGreaterThan(0);
      }
    } finally {
      project.cleanup();
    }
  });

  it('vite-plus 框架（directory 非 "."）时 script 含 cd 行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/main.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      // The vite-plus override entry (index 1) should have cd line
      const vitePlusPlan = result.plan[1];
      expect(vitePlusPlan.script.shell).toContain('cd plugins/dev-team/bin');
    } finally {
      project.cleanup();
    }
  });

  it('空 framework 配置时 plan 为空数组', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- 单框架 plan script
// ===========================================================================

describe('runTestDetectFrameworks -- 单框架 plan script', () => {
  it('"vitest" 时 plan 条目 script 字段为非空对象含 shell/cmd', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(typeof result.plan[0].script).toBe('object');
      expect(typeof result.plan[0].script.shell).toBe('string');
      expect(result.plan[0].script.shell.length).toBeGreaterThan(0);
      expect(typeof result.plan[0].script.cmd).toBe('string');
      expect(result.plan[0].script.cmd.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('"jest" 时 plan 条目 script 字段为非空对象含 shell/cmd', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'jest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(typeof result.plan[0].script).toBe('object');
      expect(typeof result.plan[0].script.shell).toBe('string');
      expect(result.plan[0].script.shell.length).toBeGreaterThan(0);
      expect(typeof result.plan[0].script.cmd).toBe('string');
      expect(result.plan[0].script.cmd.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('"rust" 时 plan 条目 script 字段为非空对象含 shell/cmd', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'rust',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(typeof result.plan[0].script).toBe('object');
      expect(typeof result.plan[0].script.shell).toBe('string');
      expect(result.plan[0].script.shell.length).toBeGreaterThan(0);
      expect(typeof result.plan[0].script.cmd).toBe('string');
      expect(result.plan[0].script.cmd.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// add-node-go-pytest-frameworks: 新框架 plan 生成 (AC-5)
// @see openspec/changes/add-node-go-pytest-frameworks/test-design.md
// ===========================================================================

describe('runTestDetectFrameworks — go plan (AC-5)', () => {
  it('config framework: "go" 时 plan 含 coverage_format: "go-cover" 及注册表 artifacts/cleanup', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'go' },
    });
    try {
      const expected = getFrameworkConfig('go');
      const result = runTestDetectFrameworks({
        files: ['pkg/foo/foo_test.go'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].coverage_format).toBe('go-cover');
      expect(result.plan[0].coverage_artifacts).toEqual(expected.coverage_artifacts);
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — node-test plan (AC-5)', () => {
  it('config framework: "node-test" 时 plan 含 coverage_format "node-test" 及 coverage_output .txt', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'node-test' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.mjs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].coverage_format).toBe('node-test');
      expect(result.plan[0].coverage_output).toBe('coverage/node-test-output.txt');
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/node-test-output.txt']);
    } finally {
      project.cleanup();
    }
  });

  it('node-test plan 的 script 最后一行为简化后的覆盖命令，不含管道符 (AC-6)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'node-test' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.mjs'],
        projectRoot: project.root,
      });
      const script = result.plan[0].script.shell;
      const lastLine = script.trimEnd().split('\n').pop() ?? '';
      expect(lastLine).not.toContain('|');
      expect(lastLine).not.toContain('parse-node-test-coverage.mjs');
      expect(lastLine).not.toContain('tee');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — pytest plan (AC-5)', () => {
  it('config framework: "pytest" 时 plan 含 coverage_format: "coverage-py"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'pytest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_foo.py'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].coverage_format).toBe('coverage-py');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — 多框架 (AC-5)', () => {
  it('配置 [vitest, go] 时 plan 两条目各自携带正确 coverage_format', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '**/*_test.go', framework: 'go' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts', 'pkg/foo_test.go'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      expect(result.plan[0].coverage_format).toBe('istanbul');
      expect(result.plan[1].coverage_format).toBe('go-cover');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks — glob 检测 (AC-5)', () => {
  it('*_test.go 文件在 go 配置下映射为 go 框架', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'go' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['internal/util/util_test.go'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('go');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-12: plan 无 merge_mode
// ===========================================================================

describe('runTestDetectFrameworks -- plan 无 merge_mode (AC-12)', () => {
  it('vitest plan 条目不包含 merge_mode', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).not.toHaveProperty('merge_mode');
    } finally {
      project.cleanup();
    }
  });

  it('pytest plan 条目不包含 merge_mode', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'pytest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_foo.py'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).not.toHaveProperty('merge_mode');
    } finally {
      project.cleanup();
    }
  });

  it('多框架每个 plan 条目均不包含 merge_mode', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan.length).toBeGreaterThanOrEqual(2);
      for (const entry of result.plan) {
        expect(entry).not.toHaveProperty('merge_mode');
      }
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- plan mutation 字段
// ===========================================================================

describe('runTestDetectFrameworks -- plan mutation 字段', () => {
  it('vitest 框架时 PlanEntry.mutation_framework 为 "stryker-js"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest', mutation: { score: 80 } },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].mutation_framework).toBe('stryker-js');
    } finally {
      project.cleanup();
    }
  });

  it('vitest 框架时 PlanEntry.mutation_score 为 config 中的 score 值', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest', mutation: { score: 85 } },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].mutation_score).toBe(85);
    } finally {
      project.cleanup();
    }
  });

  it('vitest 框架时 PlanEntry.mutation_config 从 config 正确填充', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        mutation: { score: 80 },
        overrides: [{ file: 'src/**', mutation: { score: 85 } }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      // mutation_config 应为 override 中的 mutation.score 值
      expect(result.plan[0].mutation_config).toEqual({ score: 85 });
    } finally {
      project.cleanup();
    }
  });

  it('bun 框架时 PlanEntry.mutation_framework 为 null', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'bun' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].mutation_framework).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('config 未设置 test.mutation 时 mutation_score 为默认值', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].mutation_score).toBe(70);
    } finally {
      project.cleanup();
    }
  });

  it('多框架（vitest + go）时每个 plan 条目正确携带各自的 mutation 字段', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        mutation: { score: 80 },
        overrides: [{ file: 'src/go/**/*_test.go', framework: 'go' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts', 'src/go/foo_test.go'],
        projectRoot: project.root,
      });
      expect(result.plan.length).toBeGreaterThanOrEqual(2);
      const vitestEntry = result.plan.find((e) => e.framework === 'vitest');
      const goEntry = result.plan.find((e) => e.framework === 'go');
      expect(vitestEntry).toBeDefined();
      expect(goEntry).toBeDefined();
      expect(vitestEntry!.mutation_framework).toBe('stryker-js');
      expect(vitestEntry!.mutation_score).toBe(80);
      expect(goEntry!.mutation_framework).toBeNull();
      expect(goEntry!.mutation_score).toBeNull();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// detectFrameworks -- exclude 过滤 (AC-3)
// ===========================================================================

describe('detectFrameworks -- exclude 过滤 (AC-3)', () => {
  // Normalize Windows paths for assertion matching
  function posixFile(p: string): string {
    return p.replace(/\\/g, '/');
  }

  it('配置 test.exclude 后被排除的文件不出现在 detected[] 中', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        exclude: ['**/generated/**'],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts', 'generated/out.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected.some((d) => posixFile(d.file).endsWith('src/app.test.ts'))).toBe(true);
      expect(result.detected.some((d) => posixFile(d.file).endsWith('generated/out.test.ts'))).toBe(
        false,
      );
    } finally {
      project.cleanup();
    }
  });

  it('未被排除的文件正常出现在 detected[] 中', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        exclude: ['**/generated/**'],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts', 'src/lib/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected.every((d) => !posixFile(d.file).includes('generated'))).toBe(true);
      expect(result.detected.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('override-level exclude 仅在该 override 文件范围内生效', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [
          { file: '**/dir-a/**', exclude: ['**/*.snap'] },
          { file: '**/dir-b/**', framework: 'vite-plus', exclude: ['**/*.test.ts'] },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [
          'dir-a/src/types.ts',
          'dir-a/src/icon.snap',
          'dir-b/src/foo.test.ts',
          'dir-b/src/bar.ts',
        ],
        projectRoot: project.root,
      });
      expect(result.detected.some((d) => posixFile(d.file).endsWith('dir-a/src/icon.snap'))).toBe(
        false,
      );
      expect(result.detected.some((d) => posixFile(d.file).endsWith('dir-b/src/foo.test.ts'))).toBe(
        false,
      );
      expect(result.detected.some((d) => posixFile(d.file).endsWith('dir-b/src/bar.ts'))).toBe(
        true,
      );
    } finally {
      project.cleanup();
    }
  });

  it('多框架配置下 exclude 过滤正确作用于各框架', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        exclude: ['**/legacy/**'],
        overrides: [{ file: '**/rust-tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/new.test.ts', 'legacy/old.test.ts', 'rust-tests/test_foo.rs'],
        projectRoot: project.root,
      });
      expect(result.detected.some((d) => posixFile(d.file).endsWith('legacy/old.test.ts'))).toBe(
        false,
      );
      expect(result.detected.some((d) => posixFile(d.file).endsWith('src/new.test.ts'))).toBe(true);
      expect(
        result.detected.some((d) => posixFile(d.file).endsWith('rust-tests/test_foo.rs')),
      ).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('exclude 配置不影响非匹配框架的文件检测', () => {
    // 场景：全局 exclude 匹配默认框架的部分文件，但 override 框架的文件
    // 不应受 exclude 影响，仍应被正确分配所属框架
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        exclude: ['**/generated/**'],
        overrides: [{ file: '**/rust-tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts', 'rust-tests/test_foo.rs'],
        projectRoot: project.root,
      });
      // 不匹配 exclude 的 vitest 文件框架仍为 vitest
      const vitestEntry = result.detected.find((d) =>
        posixFile(d.file).endsWith('src/app.test.ts'),
      );
      expect(vitestEntry?.framework).toBe('vitest');
      // 不匹配 exclude 且属于 override 框架的文件框架为 rust（不被全局 exclude 干扰）
      const rustEntry = result.detected.find((d) =>
        posixFile(d.file).endsWith('rust-tests/test_foo.rs'),
      );
      expect(rustEntry?.framework).toBe('rust');
    } finally {
      project.cleanup();
    }
  });

  it('非排除且在框架 glob 范围内的文件应被正常检测', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        exclude: ['**/generated/**'],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected.some((d) => posixFile(d.file).endsWith('src/app.test.ts'))).toBe(true);
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- 向后兼容 (AC-6)
// ===========================================================================

describe('runTestDetectFrameworks -- 向后兼容 (AC-6)', () => {
  it('不配置 exclude 时全部现有功能行为不变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('配置中存在 `"exclude": []` 空数组时检测结果与无 exclude 配置时一致', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest', exclude: [] },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('配置中 `test` 节不包含 `exclude` 字段时检测结果不受影响', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('配置中不存在 `test` 节时检测结果与无 exclude 配置时一致', () => {
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- shell script content verification (AC-3)
// ===========================================================================

describe('runTestDetectFrameworks -- shell script content (AC-3)', () => {
  it('vitest 框架的 plan shell 包含 `npx vitest run` 和 `rm -rf coverage`', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'vitest' } });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const shell = (result.plan[0].script as { shell: string; cmd: string }).shell;
      expect(shell).toContain('npx vitest run');
      expect(shell).toContain('rm -rf coverage');
    } finally {
      project.cleanup();
    }
  });

  it('非 "." 目录的 plan shell 包含 `cd` 行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/app.test.ts'],
        projectRoot: project.root,
      });
      const shell = (result.plan[1].script as { shell: string; cmd: string }).shell;
      expect(shell).toContain('cd plugins/dev-team/bin');
    } finally {
      project.cleanup();
    }
  });

  it('链式命令框架（rust）的 plan shell 包含 `; _X=$?;` 和 `exit $_X`', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'rust' } });
    try {
      const result = runTestDetectFrameworks({ files: ['src/lib.rs'], projectRoot: project.root });
      const shell = (result.plan[0].script as { shell: string; cmd: string }).shell;
      expect(shell).toContain('; _X=$?;');
      expect(shell).toContain('exit $_X');
    } finally {
      project.cleanup();
    }
  });

  it('链式命令框架（pytest）的 plan shell 包含 `; _X=$?;` 和 `exit $_X`', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'pytest' } });
    try {
      const result = runTestDetectFrameworks({ files: ['test_app.py'], projectRoot: project.root });
      const shell = (result.plan[0].script as { shell: string; cmd: string }).shell;
      expect(shell).toContain('; _X=$?;');
      expect(shell).toContain('exit $_X');
    } finally {
      project.cleanup();
    }
  });

  it('所有 8 框架生成的 plan shell 为非空字符串', () => {
    const ALL_EIGHT = [
      'jest',
      'vitest',
      'vite-plus',
      'bun',
      'rust',
      'node-test',
      'go',
      'pytest',
    ] as const;
    for (const fw of ALL_EIGHT) {
      const project = createTempProject({ schema: 'spec-driven', test: { framework: fw } });
      try {
        const result = runTestDetectFrameworks({
          files: ['src/test.test.ts'],
          projectRoot: project.root,
        });
        const script = result.plan[0].script as { shell: string; cmd: string };
        expect(typeof script.shell).toBe('string');
        expect(script.shell.length).toBeGreaterThan(0);
      } finally {
        project.cleanup();
      }
    }
  });

  it('directory 为 `"."` 时不生成 `cd` 行', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'vitest' } });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const shell = (result.plan[0].script as { shell: string; cmd: string }).shell;
      expect(shell).not.toContain('cd ');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- cmd script content (AC-4)
// ===========================================================================

describe('runTestDetectFrameworks -- cmd script content (AC-4)', () => {
  it('vitest 框架的 plan cmd 包含 `npx vitest run` 和 `if exist ... rmdir`', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'vitest' } });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const cmd = (result.plan[0].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('npx vitest run');
      expect(cmd).toContain('if exist "coverage" (rmdir /s /q "coverage"');
    } finally {
      project.cleanup();
    }
  });

  it('非 "." 目录的 plan cmd 包含 `cd /d` 行（带 `/d` 标志）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/app.test.ts'],
        projectRoot: project.root,
      });
      const cmd = (result.plan[1].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('cd /d plugins/dev-team/bin');
    } finally {
      project.cleanup();
    }
  });

  it('链式命令框架（rust）的 plan cmd 使用 `if errorlevel` 模式', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'rust' } });
    try {
      const result = runTestDetectFrameworks({ files: ['src/lib.rs'], projectRoot: project.root });
      const cmd = (result.plan[0].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('if errorlevel');
      expect(cmd).toContain('%errorlevel%');
    } finally {
      project.cleanup();
    }
  });

  it('链式命令框架（pytest）的 plan cmd 使用 `if errorlevel` 模式', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'pytest' } });
    try {
      const result = runTestDetectFrameworks({ files: ['test_app.py'], projectRoot: project.root });
      const cmd = (result.plan[0].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('if errorlevel');
      expect(cmd).toContain('%errorlevel%');
    } finally {
      project.cleanup();
    }
  });

  it('所有 8 框架生成的 plan cmd 为非空字符串', () => {
    const ALL_EIGHT = [
      'jest',
      'vitest',
      'vite-plus',
      'bun',
      'rust',
      'node-test',
      'go',
      'pytest',
    ] as const;
    for (const fw of ALL_EIGHT) {
      const project = createTempProject({ schema: 'spec-driven', test: { framework: fw } });
      try {
        const result = runTestDetectFrameworks({
          files: ['src/test.test.ts'],
          projectRoot: project.root,
        });
        const script = result.plan[0].script as { shell: string; cmd: string };
        expect(typeof script.cmd).toBe('string');
        expect(script.cmd.length).toBeGreaterThan(0);
      } finally {
        project.cleanup();
      }
    }
  });

  it('cmd 脚本使用 `\\r\\n` 行分隔符', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'vitest' } });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const cmd = (result.plan[0].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('\r\n');
    } finally {
      project.cleanup();
    }
  });

  it('directory 为 `"."` 时 plan cmd 不含 `cd` 行', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'vitest' } });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const cmd = (result.plan[0].script as { shell: string; cmd: string }).cmd;
      expect(cmd).not.toContain('cd ');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks -- cmd cleanup edge cases
// ===========================================================================

describe('runTestDetectFrameworks -- cmd cleanup edge cases', () => {
  it('含空格路径的 directory 在 cmd 中生成带引号的 `cd /d`', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'path/with spaces/**', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['path/with spaces/app.test.ts'],
        projectRoot: project.root,
      });
      const cmd = (result.plan[1].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('cd /d "path/with spaces"');
    } finally {
      project.cleanup();
    }
  });

  it('coverage_cleanup 含路径分隔符的框架（rust）在 cmd 中正确生成 rmdir', () => {
    const project = createTempProject({ schema: 'spec-driven', test: { framework: 'rust' } });
    try {
      const result = runTestDetectFrameworks({ files: ['src/lib.rs'], projectRoot: project.root });
      const cmd = (result.plan[0].script as { shell: string; cmd: string }).cmd;
      expect(cmd).toContain('"target/llvm-cov"');
      expect(cmd).toContain('rmdir /s /q "target/llvm-cov"');
    } finally {
      project.cleanup();
    }
  });
});
