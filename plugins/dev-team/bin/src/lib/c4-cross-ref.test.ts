/**
 * 单元测试: lib/c4-cross-ref.ts — runCrossRefCheck
 *
 * 通过临时目录 + 模型文件 + 源文件夹具, 端到端测试交叉引用验证的全部路径:
 * - 模型未找到 → skipped
 * - 无变更文件 → no_changes
 * - 空模型 → clean (无 elements/relationships)
 * - 关系匹配 (clean)
 * - unmodeled_dependency (violation)
 * - unmapped_import_target (warning)
 * - unused_relationship (warning)
 * - path_not_found (warning)
 * - self-import 忽略
 * - 多文件 / 多元素
 * - TypeScript / Python 文件
 * - 文件列表 vs staged 模式
 *
 * @see plugins/dev-team/bin/src/lib/c4-cross-ref.ts
 */

import type * as LegacyChildProcess from 'child_process';
import * as fs from 'fs';
import type * as NodeChildProcess from 'node:child_process';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect, vi } from 'vite-plus/test';

import { runCrossRefCheck } from './c4-cross-ref';

// ---------------------------------------------------------------------------
// git 退场 / staged 废弃防回归：child_process 全程 spy（仅作 not-called 断言）
// ---------------------------------------------------------------------------

vi.mock('node:child_process', async () => {
  const actual = await vi.importActual<typeof NodeChildProcess>('node:child_process');
  return { ...actual, execSync: vi.fn(actual.execSync), execFileSync: vi.fn(actual.execFileSync) };
});

vi.mock('child_process', async () => {
  const actual = await vi.importActual<typeof LegacyChildProcess>('child_process');
  return { ...actual, execSync: vi.fn(actual.execSync), execFileSync: vi.fn(actual.execFileSync) };
});

import { execSync as spiedExecSync } from 'node:child_process';

// ===========================================================================
// 工具函数
// ===========================================================================

/** 创建临时项目目录结构, 返回 [projectRoot, cleanup] */
function createProject(options: {
  models?: Record<string, string>; // models/ 下的 .c4 文件名 → 内容
  files?: Record<string, string>; // 项目根下的源文件路径 → 内容
  extraDirs?: string[]; // models/ 之外需创建的目录
}): [string, () => void] {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'c4-xref-test-'));
  const modelsDir = path.join(root, 'openspec', 'architecture', 'models');

  if (options.models && Object.keys(options.models).length > 0) {
    fs.mkdirSync(modelsDir, { recursive: true });
    for (const [name, content] of Object.entries(options.models)) {
      fs.writeFileSync(path.join(modelsDir, name), content);
    }
  }

  if (options.files) {
    for (const [filePath, content] of Object.entries(options.files)) {
      const fullPath = path.join(root, filePath);
      fs.mkdirSync(path.dirname(fullPath), { recursive: true });
      fs.writeFileSync(fullPath, content);
    }
  }

  if (options.extraDirs) {
    for (const dir of options.extraDirs) {
      fs.mkdirSync(path.join(root, dir), { recursive: true });
    }
  }

  return [root, () => fs.rmSync(root, { recursive: true, force: true })];
}

// ===========================================================================
// runCrossRefCheck — 模型未找到 / 无文件
// ===========================================================================

describe('runCrossRefCheck — early exit', () => {
  it('模型目录不存在时应返回 skipped', async () => {
    const [root, cleanup] = createProject({});
    try {
      const result = await runCrossRefCheck(root, { files: ['src/a.ts'] });
      expect(result.status).toBe('skipped');
      expect(result.violations).toEqual([]);
      expect(result.warnings).toEqual([]);
    } finally {
      cleanup();
    }
  });

  it('changedFiles 为空数组时应返回 no_changes', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Pkg {
    metadata { path './src/' }
  }
}`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, { files: [] });
      expect(result.status).toBe('no_changes');
      expect(result.violations).toEqual([]);
    } finally {
      cleanup();
    }
  });

  it('不传 files 且不传 staged 时应返回 no_changes (resolveCheckFiles 返回空)', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Pkg {
    metadata { path './src/' }
  }
}`,
      },
    });
    try {
      const result = await runCrossRefCheck(root);
      expect(result.status).toBe('no_changes');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — 空模型 (clean)
// ===========================================================================

describe('runCrossRefCheck — empty model', () => {
  it('空模型无元素/关系时应返回 clean', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
}`,
      },
      files: {
        'src/foo.ts': `import { bar } from './bar';`,
        'src/bar.ts': `export const bar = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, { files: ['src/foo.ts', 'src/bar.ts'] });
      expect(result.status).toBe('clean');
      expect(result.violations).toEqual([]);
      // 文件未映射到任何元素
      expect(result.unmatched_files).toContain('src/foo.ts');
      expect(result.unmatched_files).toContain('src/bar.ts');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — clean: 有效关系
// ===========================================================================

describe('runCrossRefCheck — valid relationships', () => {
  it('import 关系与模型声明一致时应返回 clean', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  Frontend -> Backend "calls API"
}`,
      },
      files: {
        'src/frontend/app.ts': `import { api } from '../backend/api';`,
        'src/backend/api.ts': `export const api = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/frontend/app.ts', 'src/backend/api.ts'],
      });
      expect(result.status).toBe('clean');
      expect(result.violations).toEqual([]);
      expect(result.matched.length).toBe(2);
    } finally {
      cleanup();
    }
  });

  it('同一元素内 self-import 应被忽略', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
}`,
      },
      files: {
        'src/core/a.ts': `import { helper } from './b';`,
        'src/core/b.ts': `export const helper = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/a.ts', 'src/core/b.ts'],
      });
      expect(result.status).toBe('clean');
      expect(result.violations).toEqual([]);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — violation: unmodeled_dependency
// ===========================================================================

describe('runCrossRefCheck — unmodeled_dependency', () => {
  it('模型未声明关系时应返回 violations_found', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  // 故意不声明 Frontend -> Backend
}`,
      },
      files: {
        'src/frontend/app.ts': `import { api } from '../backend/api';`,
        'src/backend/api.ts': `export const api = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/frontend/app.ts', 'src/backend/api.ts'],
      });
      expect(result.status).toBe('violations_found');
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].type).toBe('unmodeled_dependency');
      expect(result.violations[0].source).toBe('Frontend');
      expect(result.violations[0].target).toBe('Backend');
    } finally {
      cleanup();
    }
  });

  it('声明的关系方向不对时也应报 violation', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
  B -> A "B calls A"
}`,
      },
      files: {
        'src/a/index.ts': `import { stuff } from '../b/index';`,
        'src/b/index.ts': `export const stuff = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/index.ts', 'src/b/index.ts'],
      });
      // A imports B but model says B -> A, direction mismatch
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].type).toBe('unmodeled_dependency');
      expect(result.violations[0].source).toBe('A');
      expect(result.violations[0].target).toBe('B');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — warning: unmapped_import_target
// ===========================================================================

describe('runCrossRefCheck — unmapped_import_target', () => {
  it('import 目标文件不在模型中时应产生 warning', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
}`,
      },
      files: {
        'src/frontend/app.ts': `import { unknown } from '../external/helper';`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/frontend/app.ts'],
      });
      const unmappedWarnings = result.warnings.filter((w) =>
        w.includes('not mapped to any model element'),
      );
      expect(unmappedWarnings.length).toBe(1);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — warning: unused_relationship
// ===========================================================================

describe('runCrossRefCheck — unused_relationship', () => {
  it('模型声明了关系但变更文件中无对应 import 时应产生 warning', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  Frontend -> Backend "calls API"
}`,
      },
      files: {
        'src/frontend/app.ts': `// 无 import 到 backend`,
        'src/backend/api.ts': `export const api = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/frontend/app.ts', 'src/backend/api.ts'],
      });
      const unusedWarnings = result.warnings.filter((w) => w.includes('no import evidence'));
      expect(unusedWarnings.length).toBe(1);
      expect(unusedWarnings[0]).toContain('Frontend');
      expect(unusedWarnings[0]).toContain('Backend');
    } finally {
      cleanup();
    }
  });

  it('关系被实际使用时不应产生 unused_relationship warning', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  Frontend -> Backend "calls API"
}`,
      },
      files: {
        'src/frontend/app.ts': `import { api } from '../backend/api';`,
        'src/backend/api.ts': `export const api = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/frontend/app.ts', 'src/backend/api.ts'],
      });
      // 不应有 unused_relationship warning
      const unusedWarnings = result.warnings.filter((w) => w.includes('no import evidence'));
      expect(unusedWarnings.length).toBe(0);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — warning: path_not_found
// ===========================================================================

describe('runCrossRefCheck — path_not_found', () => {
  it('元素 metadata.path 指向不存在的目录时应产生 warning', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Ghost {
    metadata { path './src/nonexistent/' }
  }
}`,
      },
      files: {
        'src/real/index.ts': `export const x = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/real/index.ts'],
      });
      const pathWarnings = result.warnings.filter((w) => w.includes('does not exist'));
      expect(pathWarnings.length).toBe(1);
      expect(pathWarnings[0]).toContain('Ghost');
      expect(pathWarnings[0]).toContain('src/nonexistent');
    } finally {
      cleanup();
    }
  });

  it('路径存在时不应产生 path_not_found warning', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Real {
    metadata { path './src/real/' }
  }
}`,
      },
      files: {
        'src/real/index.ts': `export const x = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/real/index.ts'],
      });
      const pathWarnings = result.warnings.filter((w) => w.includes('does not exist'));
      expect(pathWarnings.length).toBe(0);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — matched / unmatched
// ===========================================================================

describe('runCrossRefCheck — matched & unmatched_files', () => {
  it('应正确返回 matched 按元素分组', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
  package Utils {
    metadata { path './src/utils/' }
  }
  Core -> Utils "uses"
}`,
      },
      files: {
        'src/core/a.ts': `import { fn } from '../utils/b';`,
        'src/core/c.ts': `import { fn } from '../utils/b';`,
        'src/utils/b.ts': `export const fn = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/a.ts', 'src/core/c.ts', 'src/utils/b.ts'],
      });
      expect(result.matched.length).toBe(2);
      const core = result.matched.find((m) => m.element_id === 'Core');
      expect(core).toBeDefined();
      expect(core!.files.sort()).toEqual(['src/core/a.ts', 'src/core/c.ts'].sort());
      const utils = result.matched.find((m) => m.element_id === 'Utils');
      expect(utils).toBeDefined();
      expect(utils!.files).toEqual(['src/utils/b.ts']);
    } finally {
      cleanup();
    }
  });

  it('未映射到模型的文件应出现在 unmatched_files 中', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
}`,
      },
      files: {
        'src/core/a.ts': `export const x = 1;`,
        'src/wild/unmapped.ts': `export const y = 2;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/a.ts', 'src/wild/unmapped.ts'],
      });
      expect(result.unmatched_files).toContain('src/wild/unmapped.ts');
      expect(result.unmatched_files).not.toContain('src/core/a.ts');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — JS/TS 多种 import 语法
// ===========================================================================

describe('runCrossRefCheck — JS/TS import patterns', () => {
  it('应正确解析 default import', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
}`,
      },
      files: {
        'src/a/main.ts': `import thing from '../b/module';`,
        'src/b/module.ts': `export default {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/main.ts'],
      });
      // 未声明关系 → violation
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].type).toBe('unmodeled_dependency');
    } finally {
      cleanup();
    }
  });

  it('应正确解析 require() 语法', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
}`,
      },
      files: {
        'src/a/main.ts': `const mod = require('../b/module');`,
        'src/b/module.ts': `module.exports = {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/main.ts'],
      });
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].source).toBe('A');
      expect(result.violations[0].target).toBe('B');
    } finally {
      cleanup();
    }
  });

  it('应正确解析 import type 语法', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
  A -> B "uses"
}`,
      },
      files: {
        'src/a/main.ts': `import type { Item } from '../b/types';`,
        'src/b/types.ts': `export type Item = {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/main.ts', 'src/b/types.ts'],
      });
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });

  it('应正确解析 import * as 语法', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
  A -> B "uses"
}`,
      },
      files: {
        'src/a/main.ts': `import * as mod from '../b/all';`,
        'src/b/all.ts': `export const x = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/main.ts', 'src/b/all.ts'],
      });
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });

  it("应正确解析 side-effect import (import './path')", async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
}`,
      },
      files: {
        'src/a/main.ts': `import '../b/init';`,
        'src/b/init.ts': `console.log('init');`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/main.ts'],
      });
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].source).toBe('A');
      expect(result.violations[0].target).toBe('B');
    } finally {
      cleanup();
    }
  });

  it('import 外部包 (非 ./ ../ @/) 应被过滤不触发 violation', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
}`,
      },
      files: {
        'src/core/main.ts': `import * as lodash from 'lodash';`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/main.ts'],
      });
      // 外部包被过滤, 不产生 violation 也不产生 unmapped warning
      expect(result.violations.length).toBe(0);
      const unmappedWarnings = result.warnings.filter((w) =>
        w.includes('not mapped to any model element'),
      );
      expect(unmappedWarnings.length).toBe(0);
    } finally {
      cleanup();
    }
  });

  it('应正确解析 @/ alias import', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package A {
    metadata { path './src/a/' }
  }
  package B {
    metadata { path './src/b/' }
  }
  A -> B "uses"
}`,
      },
      files: {
        'src/a/main.ts': `import { foo } from '@/src/b/module';`,
        'src/b/module.ts': `export const foo = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/a/main.ts', 'src/b/module.ts'],
      });
      // @/ → 去掉前缀 → src/b/module → 应匹配到 B
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — Python import
// ===========================================================================

describe('runCrossRefCheck — Python imports', () => {
  it('应正确解析 from .module import 相对导入', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package ServiceA {
    metadata { path './src/service_a/' }
  }
  package ServiceB {
    metadata { path './src/service_b/' }
  }
  ServiceA -> ServiceB "calls"
}`,
      },
      files: {
        'src/service_a/main.py': `from . import helper`,
        'src/service_a/helper.py': `pass`,
        'src/service_b/api.py': `def get(): pass`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/service_a/main.py', 'src/service_a/helper.py'],
      });
      // . import 解析后两个文件在同一元素 ServiceA → self-import 忽略
      expect(result.status).toBe('clean');
      expect(result.violations.length).toBe(0);
    } finally {
      cleanup();
    }
  });

  it('应正确解析 from ..module import 相对导入', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package ServiceA {
    metadata { path './src/service_a/' }
  }
  package ServiceB {
    metadata { path './src/service_b/' }
  }
}`,
      },
      files: {
        'src/service_a/sub/module.py': `from .. import helper`,
        'src/service_a/helper.py': `pass`,
        'src/service_b/api.py': `pass`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/service_a/sub/module.py', 'src/service_a/helper.py'],
      });
      // .. import 解析到同一元素 → self-import 忽略
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });

  it('Python 标准库 import 应被过滤', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
}`,
      },
      files: {
        'src/core/main.py': `from os import path\nfrom sys import argv\nimport json`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/main.py'],
      });
      expect(result.violations.length).toBe(0);
      const unmappedWarnings = result.warnings.filter((w) =>
        w.includes('not mapped to any model element'),
      );
      expect(unmappedWarnings.length).toBe(0);
    } finally {
      cleanup();
    }
  });

  it('应正确解析非标准库的 Python 导入', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package ServiceA {
    metadata { path './src/service_a/' }
  }
  package ServiceB {
    metadata { path './src/service_b/' }
  }
}`,
      },
      files: {
        'src/service_a/main.py': `from service_b.api import get`,
        'src/service_b/api.py': `def get(): pass`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/service_a/main.py'],
      });
      // 非标准库导入被解析, target 'service_b' 可能被匹配或产生 warning
      // 主要验证不 crash, 以及 os/sys 等不被计入
      expect(result.unmatched_files).not.toContain('src/service_a/main.py');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — 文件不存在
// ===========================================================================

describe('runCrossRefCheck — missing source files', () => {
  it('变更文件列表中包含不存在的文件时应跳过不报错', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
}`,
      },
      files: {
        'src/core/real.ts': `export const x = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/real.ts', 'src/core/ghost.ts'],
      });
      // ghost.ts 不存在, parseImports 返回 [] → 无 import 无 violation
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — 多元素多文件复杂场景
// ===========================================================================

describe('runCrossRefCheck — complex scenarios', () => {
  it('混合 violation + warning + clean 的场景', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  package Database {
    metadata { path './src/database/' }
  }
  // 只声明 Frontend -> Backend, 不声明 Backend -> Database
  Frontend -> Backend "calls API"
}`,
      },
      files: {
        'src/frontend/app.ts': `import { api } from '../backend/api';`,
        'src/backend/api.ts': `import { query } from '../database/query';`,
        'src/database/query.ts': `export const query = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/frontend/app.ts', 'src/backend/api.ts', 'src/database/query.ts'],
      });
      // Frontend → Backend is declared → OK
      // Backend → Database is NOT declared → violation
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].source).toBe('Backend');
      expect(result.violations[0].target).toBe('Database');
      expect(result.status).toBe('violations_found');
    } finally {
      cleanup();
    }
  });

  it('多个元素, 每个包含多个文件的路径', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path ['./src/core/', './src/shared/'] }
  }
  package Plugin {
    metadata { path './src/plugin/' }
  }
  Core -> Plugin "loads"
}`,
      },
      files: {
        'src/core/index.ts': `import { plugin } from '../plugin/loader';`,
        'src/shared/util.ts': `export const util = () => {};`,
        'src/plugin/loader.ts': `import { util } from '../shared/util';`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/index.ts', 'src/shared/util.ts', 'src/plugin/loader.ts'],
      });
      // Core index → Plugin loader → declared → OK
      // Plugin loader → shared/util → shared is under Core (both in Core's paths)
      // Plugin → Core is NOT declared → violation
      expect(result.violations.length).toBe(1);
      expect(result.violations[0].source).toBe('Plugin');
      expect(result.violations[0].target).toBe('Core');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — extension matching
// ===========================================================================

describe('runCrossRefCheck — extension matching', () => {
  it('path_to_element 中的路径带扩展名时应正确匹配', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/index.ts' }
  }
  package Utils {
    metadata { path './src/utils/helper.ts' }
  }
  Core -> Utils "uses"
}`,
      },
      files: {
        'src/core/index.ts': `import { helper } from '../utils/helper';`,
        'src/utils/helper.ts': `export const helper = () => {};`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/index.ts', 'src/utils/helper.ts'],
      });
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — staged 模式 (不传 files, 用 git diff --cached)
// ===========================================================================

describe('runCrossRefCheck — staged mode', () => {
  it('staged=false 且无 files 时 resolveCheckFiles 返回空数组 → no_changes', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/' }
  }
}`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, { staged: false });
      expect(result.status).toBe('no_changes');
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — 空 model (只有 spec 没有 model 内容)
// ===========================================================================

describe('runCrossRefCheck — spec-only model', () => {
  it('只有 specification 没有模型元素时, 文件应全部 unmatched → clean', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
  element domain
}
model {
}`,
      },
      files: {
        'src/lib/util.ts': `export const x = 1;`,
      },
    });
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/lib/util.ts'],
      });
      expect(result.status).toBe('clean');
      expect(result.unmatched_files).toContain('src/lib/util.ts');
      expect(result.matched).toEqual([]);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// runCrossRefCheck — 清单模式（change → files.written 直通）与 staged 废弃 (AC-9)
// ===========================================================================

describe('runCrossRefCheck — 清单模式与 staged 废弃 (AC-9)', () => {
  /** 在临时项目内写 change 的 workflow.json（files 为 undefined 时模拟机制前旧 change）。 */
  function writeChangeInventory(
    root: string,
    name: string,
    files: { written: string[]; deleted: string[] } | undefined,
  ): void {
    const changeDir = path.join(root, 'openspec', 'changes', name);
    fs.mkdirSync(changeDir, { recursive: true });
    const doc: Record<string, unknown> =
      files === undefined
        ? { workflow_type: 'requirement', created: '2026-09-17' }
        : {
            workflow_type: 'requirement',
            created: '2026-09-17',
            file_log: [
              ...files.written.map((p) => ({
                op: 'write',
                scope: 'workflow',
                path: p,
                at: '2026-09-17T00:00:00.000Z',
              })),
              ...files.deleted.map((p) => ({
                op: 'delete',
                scope: 'workflow',
                path: p,
                at: '2026-09-17T00:00:00.000Z',
              })),
            ],
          };
    fs.writeFileSync(path.join(changeDir, 'workflow.json'), JSON.stringify(doc), 'utf-8');
  }

  /** 带模型与清单直通文件的标准 fixture。 */
  function createInventoryProject(options: {
    written?: string[];
    changeName?: string;
    omitInventory?: boolean;
  }): [string, () => void] {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Core {
    metadata { path './src/core/' }
  }
}`,
      },
      files: {
        'src/core/a.ts': `export const a = 1;`,
        'src/wild/unmapped.ts': `export const y = 2;`,
      },
    });
    if (!options.omitInventory) {
      writeChangeInventory(root, options.changeName ?? 'inv', {
        written: options.written ?? ['src/core/a.ts', 'src/wild/unmapped.ts'],
        deleted: [],
      });
    }
    return [root, cleanup];
  }

  it('传 change → 被查文件集 = files.written 直通（不做 test config 过滤），matched / unmatched_files 按模型映射产出 (AC-9)', async () => {
    const [root, cleanup] = createInventoryProject({});
    try {
      const result = await runCrossRefCheck(root, { change: 'inv' });

      // 直通集合：测试文件等条目也不被过滤——这里 written 同时含已建模与未建模文件
      expect(result.matched).toEqual([{ element_id: 'Core', files: ['src/core/a.ts'] }]);
      expect(result.unmatched_files).toEqual(['src/wild/unmapped.ts']);
      expect(result.status).toBe('clean');
    } finally {
      cleanup();
    }
  });

  it('显式 files 与 change 同传 → files 优先（优先级契约：清单不被读取）', async () => {
    // change 指向机制前旧 change（若被读取会硬报错），files 优先时不受影响
    const [root, cleanup] = createInventoryProject({ omitInventory: false });
    writeChangeInventory(root, 'legacy', undefined);
    try {
      const result = await runCrossRefCheck(root, {
        files: ['src/core/a.ts'],
        change: 'legacy',
      });

      expect(result.matched).toEqual([{ element_id: 'Core', files: ['src/core/a.ts'] }]);
      expect(result.unmatched_files).toEqual([]);
    } finally {
      cleanup();
    }
  });

  it('staged: true → 直接抛错（staged 模式已由清单模式替代），且不产生任何 git 子进程调用 (AC-9)', async () => {
    const [root, cleanup] = createInventoryProject({});
    try {
      await expect(runCrossRefCheck(root, { staged: true })).rejects.toThrow(
        /staged 模式已由清单模式替代/,
      );
      expect(vi.mocked(spiedExecSync)).not.toHaveBeenCalled();
    } finally {
      cleanup();
    }
  });

  it('change 清单模式 + workflow.json 无 files → 硬报错指引重建（非静默降级，AC-9）', async () => {
    const [root, cleanup] = createInventoryProject({ omitInventory: false });
    writeChangeInventory(root, 'legacy', undefined);
    try {
      await expect(runCrossRefCheck(root, { change: 'legacy' })).rejects.toThrow(
        /该 change 创建于文件清单机制之前，请重建/,
      );
    } finally {
      cleanup();
    }
  });

  it('change 不存在 → 报错（workflow.json 不存在 + change_create 指引）', async () => {
    const [root, cleanup] = createInventoryProject({});
    try {
      await expect(runCrossRefCheck(root, { change: 'ghost' })).rejects.toThrow(
        /workflow\.json 不存在/,
      );
    } finally {
      cleanup();
    }
  });

  it('files.written 为空数组 → status no_changes (AC-9)', async () => {
    const [root, cleanup] = createInventoryProject({ written: [] });
    try {
      const result = await runCrossRefCheck(root, { change: 'inv' });
      expect(result.status).toBe('no_changes');
      expect(result.violations).toEqual([]);
      expect(result.matched).toEqual([]);
    } finally {
      cleanup();
    }
  });

  it('清单中文件无对应模型元素 → unmatched_files 含之（直通不过滤的可见后果）；模型文件不进被查文件集', async () => {
    const [root, cleanup] = createInventoryProject({
      written: ['src/wild/unmapped.ts', 'src/notes.txt'],
    });
    try {
      const result = await runCrossRefCheck(root, { change: 'inv' });

      expect(result.unmatched_files).toEqual(['src/wild/unmapped.ts', 'src/notes.txt']);
      // openspec/architecture/** 模型文件仅作参照系，绝不出现在被查文件集中
      expect(result.unmatched_files.some((f) => f.includes('openspec/architecture'))).toBe(false);
      expect(result.matched.some((m) => m.files.some((f) => f.includes('openspec')))).toBe(false);
    } finally {
      cleanup();
    }
  });

  it('清单直通文件间的 import 关系参与交叉引用：未建模依赖 → violations_found (AC-9)', async () => {
    const [root, cleanup] = createProject({
      models: {
        'spec.c4': `specification {
  element package
}
model {
  package Frontend {
    metadata { path './src/frontend/' }
  }
  package Backend {
    metadata { path './src/backend/' }
  }
  // 故意不声明 Frontend -> Backend
}`,
      },
      files: {
        'src/frontend/app.ts': `import { api } from '../backend/api';`,
        'src/backend/api.ts': `export const api = () => {};`,
      },
    });
    writeChangeInventory(root, 'inv', {
      written: ['src/frontend/app.ts', 'src/backend/api.ts'],
      deleted: [],
    });
    try {
      const result = await runCrossRefCheck(root, { change: 'inv' });
      expect(result.status).toBe('violations_found');
      expect(result.violations[0].type).toBe('unmodeled_dependency');
      expect(result.violations[0].source).toBe('Frontend');
      expect(result.violations[0].target).toBe('Backend');
    } finally {
      cleanup();
    }
  });
});
