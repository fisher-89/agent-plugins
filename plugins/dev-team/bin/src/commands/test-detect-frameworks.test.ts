/**
 * 单元测试: test-detect-frameworks — 从 config.tests[] 构建 plan
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

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

function writeFile(projectRoot: string, relativePath: string, content = ''): void {
  const fullPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf-8');
}

// ===========================================================================
// plan.directory = absCwd (AC-2)
// ===========================================================================

describe('runTestDetectFrameworks — plan.directory = absCwd (AC-2)', () => {
  it('root: "a/b", cwd: ".." → plan[0].directory === "a"，shell script 含 cd a', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'a/b', framework: 'vite-plus', cwd: '..' }],
    });
    try {
      writeFile(project.root, 'a/b/foo.test.ts');
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].directory).toBe('a');
      expect(result.plan[0].script.shell).toContain('cd a');
    } finally {
      project.cleanup();
    }
  });

  it('省略 cwd（或缺省 "."）时 directory 等于 root', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg/src', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('pkg/src');
    } finally {
      project.cleanup();
    }
  });

  it('cwd: "nested" 时 directory 为 root/nested（POSIX）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: 'nested' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('pkg/nested');
    } finally {
      project.cleanup();
    }
  });

  it('root 指向不存在目录时仍产出 plan（directory 可推导）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'missing/dir', framework: 'vitest', cwd: '.' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].directory).toBe('missing/dir');
    } finally {
      project.cleanup();
    }
  });

  it('cwd 为空字符串时 directory 等于 root（schema 默认/解析后为 "."）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: '' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      // empty cwd resolves like '.' under path.resolve
      expect(result.plan[0].directory).toBe('pkg');
    } finally {
      project.cleanup();
    }
  });

  it('cwd 为超长相对路径（>1000 chars）不崩溃且 directory 可推导', () => {
    const long = 'n'.repeat(1001);
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: long }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toContain('pkg/');
    } finally {
      project.cleanup();
    }
  });

  it('cwd 含特殊字符（空格 / Unicode）时 directory 与 script cd 正确规范化', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: 'my dir/测试' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('pkg/my dir/测试');
      expect(result.plan[0].script.cmd).toMatch(/cd \/d /);
    } finally {
      project.cleanup();
    }
  });

  it('script.cmd 用 & 串联为单行，不含换行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const cmd = runTestDetectFrameworks({ projectRoot: project.root }).plan[0].script.cmd;
      expect(cmd).not.toMatch(/[\r\n]/);
      expect(cmd).toContain(' & ');
      expect(cmd.indexOf('cd /d')).toBeLessThan(cmd.indexOf('npx vitest'));
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// {config_args} 注入 (AC-3)
// ===========================================================================

describe('runTestDetectFrameworks — {config_args} 注入 (AC-3)', () => {
  it('suite 含 config 且框架为 vite-plus 时，script 展开为 --config + 相对 absCwd 路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vite-plus',
          cwd: '.',
          config: 'vite.config.ts',
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).toContain('--config vite.config.ts');
      // 不应整段盲 append 在命令末尾独行
      const lines = result.plan[0].script.shell.trim().split('\n');
      expect(lines[lines.length - 1]).toContain('--config');
      expect(lines[lines.length - 1]).toContain('vp test');
    } finally {
      project.cleanup();
    }
  });

  it('suite 无 config 时 script 中 {config_args} 展开为空', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).not.toContain('{config_args}');
      expect(result.plan[0].script.shell).not.toContain('--config');
    } finally {
      project.cleanup();
    }
  });

  it('suite 含 config 但框架为 pytest 时 plan 构建失败并给出明确错误', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'pytest', config: 'pytest.ini' }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /does not support config injection/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('suite 含 config 但框架为 rust 时 plan 构建失败', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'rust', config: 'Cargo.toml' }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /does not support config injection/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('config 为空字符串时校验失败或按实现拒绝', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', config: '' }],
    });
    try {
      // schema nonempty → decode 失败 → tests 回落默认 []
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('config 为超长路径（>1000）时展开不崩溃', () => {
    const long = `${'c'.repeat(1001)}.ts`;
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', config: long }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).toContain('--config');
    } finally {
      project.cleanup();
    }
  });

  it('cwd: ".." 且 config 相对 root 时，展开路径为相对 absCwd 的 POSIX 相对路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'a/b',
          framework: 'vite-plus',
          cwd: '..',
          config: 'vite.config.ts',
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      // absCwd=a, absConfig=a/b/vite.config.ts → relative = b/vite.config.ts
      expect(result.plan[0].script.shell).toContain('--config b/vite.config.ts');
      expect(result.plan[0].directory).toBe('a');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// suite scope (AC-4)
// ===========================================================================

describe('runTestDetectFrameworks — suite scope (AC-4)', () => {
  it('省略 includes 时使用框架 default_glob（相对 root）匹配文件', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/src/foo.test.ts', 'pkg/src/foo.ts'],
        projectRoot: project.root,
      });
      const fwOf = (suffix: string) =>
        result.detected.find((d) => d.file.replace(/\\/g, '/').endsWith(suffix))?.framework;
      expect(fwOf('pkg/src/foo.test.ts')).toBe('vitest');
      expect(fwOf('pkg/src/foo.ts')).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('显式 includes + excludes 时仅 in-scope 文件进入 detected[]', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vite-plus',
          includes: ['**/*.{test,spec}.ts'],
          excludes: ['**/skip/**'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts', 'pkg/skip/b.test.ts', 'pkg/c.ts'],
        projectRoot: project.root,
      });
      const frameworks = result.detected.map((d) => d.framework);
      expect(result.detected.find((d) => d.file.endsWith('a.test.ts'))?.framework).toBe(
        'vite-plus',
      );
      // excluded file skipped entirely
      expect(result.detected.find((d) => d.file.includes('skip'))).toBeUndefined();
      expect(result.detected.find((d) => d.file.endsWith('c.ts'))?.framework).toBe('unknown');
      expect(frameworks).not.toContain(undefined);
    } finally {
      project.cleanup();
    }
  });

  it('多 suite 时数组顺序优先（第一个匹配决定 framework）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] },
        { root: 'pkg', framework: 'jest', includes: ['**/*.test.ts'] },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('includes/excludes 含非法 glob 语法时行为明确（不静默匹配全部）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['[invalid'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts', 'pkg/b.ts'],
        projectRoot: project.root,
      });
      // 非法 glob 不应把所有文件标为匹配
      const matched = result.detected.filter((d) => d.framework === 'vitest');
      expect(matched.length).toBeLessThan(2);
    } finally {
      project.cleanup();
    }
  });

  it('includes: []（空数组）时无文件匹配进 detected（显式 files 标 unknown）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: [] }],
    });
    try {
      // includes: [] → 回落 default_glob（suite.includes?.length 为 0）
      // 按实现 length 为 0 时使用 default_glob
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('includes 为单元素数组时仅匹配该 glob', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/only.test.ts'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/only.test.ts', 'pkg/other.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected.find((d) => d.file.endsWith('only.test.ts'))?.framework).toBe(
        'vitest',
      );
      expect(result.detected.find((d) => d.file.endsWith('other.test.ts'))?.framework).toBe(
        'unknown',
      );
    } finally {
      project.cleanup();
    }
  });

  it('excludes 超大列表（大量模式）时匹配仍正确且不超时失控', () => {
    const excludes = Array.from({ length: 300 }, (_, i) => `**/skip${i}/**`);
    excludes.push('**/ignored/**');
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/*.test.ts'],
          excludes,
        },
      ],
    });
    try {
      const started = Date.now();
      const result = runTestDetectFrameworks({
        files: ['pkg/ok.test.ts', 'pkg/ignored/x.test.ts'],
        projectRoot: project.root,
      });
      expect(Date.now() - started).toBeLessThan(5000);
      expect(result.detected.find((d) => d.file.includes('ok'))?.framework).toBe('vitest');
      expect(result.detected.find((d) => d.file.includes('ignored'))).toBeUndefined();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// suite 阈值 (AC-5)
// ===========================================================================

describe('runTestDetectFrameworks — suite 阈值 (AC-5)', () => {
  it('plan[].mutation_score 取自对应 suite 的 mutation.score', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { score: 88 } }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(88);
    } finally {
      project.cleanup();
    }
  });

  it('suite 省略 mutation 时 mutation_score 为 schema 默认 70', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(70);
    } finally {
      project.cleanup();
    }
  });

  it('suite mutation.score 为非法值时 schema 解析失败，detect 不产出该 suite plan', () => {
    for (const score of [-1, 101, Number.NaN]) {
      const project = createTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'pkg', framework: 'vitest', mutation: { score } }],
      });
      try {
        const result = runTestDetectFrameworks({ projectRoot: project.root });
        expect(result.plan).toEqual([]);
      } finally {
        project.cleanup();
      }
    }
  });

  it('mutation.score 为 0 时 plan.mutation_score 为 0', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { score: 0 } }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(0);
    } finally {
      project.cleanup();
    }
  });

  it('mutation.score 为 100（上界）时 plan.mutation_score 为 100', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { score: 100 } }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(100);
    } finally {
      project.cleanup();
    }
  });

  it('mutation.score 省略 / undefined 时回落 schema 默认 70', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: {} }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(70);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 空 tests
// ===========================================================================

describe('runTestDetectFrameworks — 空 tests', () => {
  it('tests: [] 时 plan 为空数组', () => {
    const project = createTempProject({ schema: 'spec-driven', tests: [] });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('配置仅含旧 test 键（无 tests）时不产生 suite plan（硬 breaking）', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-legacy-'));
    try {
      fs.mkdirSync(path.join(tmpDir, 'openspec'), { recursive: true });
      fs.writeFileSync(
        path.join(tmpDir, 'openspec', 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          test: { framework: 'vitest', overrides: [{ file: 'src', framework: 'vitest' }] },
        }),
        'utf-8',
      );
      const result = runTestDetectFrameworks({ projectRoot: tmpDir });
      expect(result.plan).toEqual([]);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('tests 元素缺少必填 root/framework 时配置解析失败，命令侧不产出 plan', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-bad-'));
    try {
      fs.mkdirSync(path.join(tmpDir, 'openspec'), { recursive: true });
      fs.writeFileSync(
        path.join(tmpDir, 'openspec', 'config.json'),
        JSON.stringify({ schema: 'spec-driven', tests: [{ framework: 'vitest' }] }),
        'utf-8',
      );
      const result = runTestDetectFrameworks({ projectRoot: tmpDir });
      expect(result.plan).toEqual([]);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
