/**
 * 单元测试: commands/change-files.ts — change_files 核心逻辑（append / set）
 *
 * 覆盖范围（openspec/changes/workflow-file-inventory/test-design.md AC-10、AC-13）:
 * - append：按折叠规则逐路径合并（去重、保留既有来源、新路径无来源）
 * - set：整体覆写指定桶并清理被覆写路径的 source 条目
 * - 前置校验：change 存在且 workflow.json 合法且含 files（缺失硬报错指引重建）
 * - changeFilesInputSchema：op 枚举、相对项目根 POSIX 路径、written/deleted 至少其一
 *
 * 文件系统不 mock：临时项目内 runChangeCreate 创建 fixture change（真盘模式）。
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { changeFilesInputSchema } from '../schemas';
import { runChangeCreate } from './change-create';
import { runChangeFiles, type ChangeFilesOptions } from './change-files';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'change-files-test-'));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

interface Fixture {
  root: string;
  changeDir: string;
  workflowPath: string;
}

/** 创建项目 + change，并把 workflow.json.files 置为指定净状态。 */
function createFixture(
  files: Record<string, unknown>,
  extraWorkflow: Record<string, unknown> = {},
): Fixture & { cleanup: () => void } {
  const project = createTempProject();
  const created = runChangeCreate('my-change', project.root, 'requirement');
  const workflowPath = path.join(created.path, 'workflow.json');
  fs.writeFileSync(
    workflowPath,
    JSON.stringify({
      workflow_type: 'requirement',
      created: '2026-09-17',
      unknown_key: { keep: true },
      ...extraWorkflow,
      files,
    }),
    'utf-8',
  );
  return {
    root: project.root,
    changeDir: created.path,
    workflowPath,
    cleanup: () => project.cleanup(),
  };
}

function readWorkflow(fx: Fixture): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(fx.workflowPath, 'utf-8')) as Record<string, unknown>;
}

function appendOptions(
  root: string,
  overrides: Partial<ChangeFilesOptions> = {},
): ChangeFilesOptions {
  return { change: 'my-change', op: 'append', project_root: root, ...overrides };
}

// ===========================================================================
// runChangeFiles — append
// ===========================================================================

describe('runChangeFiles — append', () => {
  it('append written 新路径 → 返回净状态含之且 workflow.json 落盘 (AC-10)', () => {
    const fx = createFixture({ written: [], deleted: [] });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { written: ['src/foo.ts'] }));

      expect(result).toEqual({ written: ['src/foo.ts'], deleted: [] });
      const doc = readWorkflow(fx);
      expect(doc.files).toEqual({ written: ['src/foo.ts'], deleted: [] });
      // 其他键保留（持久化走 writeFileInventory 通道）
      expect(doc.workflow_type).toBe('requirement');
      expect(doc.unknown_key).toEqual({ keep: true });
    } finally {
      fx.cleanup();
    }
  });

  it('append 已存在路径 → 去重不重复且既有 source 保留 (AC-13)', () => {
    const fx = createFixture({
      written: ['src/foo.ts'],
      deleted: [],
      source: { 'src/foo.ts': 'dev-team:implementation-generator' },
    });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { written: ['src/foo.ts'] }));

      expect(result.written).toEqual(['src/foo.ts']);
      expect(result.written).toHaveLength(1);
      const files = readWorkflow(fx).files as Record<string, unknown>;
      expect(files.source).toEqual({ 'src/foo.ts': 'dev-team:implementation-generator' });
    } finally {
      fx.cleanup();
    }
  });

  it('append deleted 路径 → written 移除 + deleted 加入（折叠语义合并）(AC-10)', () => {
    const fx = createFixture({ written: ['src/foo.ts'], deleted: [] });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { deleted: ['src/foo.ts'] }));

      expect(result).toEqual({ written: [], deleted: ['src/foo.ts'] });
    } finally {
      fx.cleanup();
    }
  });

  it('append deleted 路径把该路径从 written 折叠进 deleted，同批 written 新路径不写 source 键 (AC-13)', () => {
    const fx = createFixture({ written: [], deleted: [] });
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { written: ['src/new.ts'], deleted: ['src/gone.ts'] }),
      );

      expect(result).toEqual({ written: ['src/new.ts'], deleted: ['src/gone.ts'] });
      const files = readWorkflow(fx).files as Record<string, unknown>;
      // 手动补录无来源：不新增 source 键
      expect(Object.prototype.hasOwnProperty.call(files, 'source')).toBe(false);
    } finally {
      fx.cleanup();
    }
  });

  it('append 同批重复路径去重（dedupe 稳定）', () => {
    const fx = createFixture({ written: [], deleted: [] });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { written: ['src/a.ts', 'src/a.ts'] }));

      expect(result.written).toEqual(['src/a.ts']);
    } finally {
      fx.cleanup();
    }
  });
});

// ===========================================================================
// runChangeFiles — set
// ===========================================================================

describe('runChangeFiles — set', () => {
  it('set written 覆写 → 指定桶整体替换、被移除路径的 source 条目清理 (AC-10)', () => {
    const fx = createFixture({
      written: ['src/a.ts', 'src/b.ts'],
      deleted: [],
      source: { 'src/a.ts': 'dev-team:x', 'src/b.ts': 'dev-team:y' },
    });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { op: 'set', written: ['src/a.ts'] }));

      expect(result).toEqual({ written: ['src/a.ts'], deleted: [] });
      const files = readWorkflow(fx).files as Record<string, unknown>;
      // src/b.ts 不再在净状态中，其 source 条目被清理
      expect(files.source).toBeUndefined();
    } finally {
      fx.cleanup();
    }
  });

  it('set deleted 覆写 → written 不动、deleted 整体替换、未被提供的路径 source 保留 (AC-10)', () => {
    const fx = createFixture({
      written: ['src/a.ts', 'src/b.ts'],
      deleted: ['src/old1.ts'],
      source: { 'src/a.ts': 'dev-team:x' },
    });
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { op: 'set', deleted: ['src/old2.ts', 'src/old2.ts'] }),
      );

      expect(result).toEqual({
        written: ['src/a.ts', 'src/b.ts'],
        deleted: ['src/old2.ts'],
      });
      const files = readWorkflow(fx).files as Record<string, unknown>;
      // src/a.ts 仍在净状态且未被提供 → source 保留
      expect(files.source).toEqual({ 'src/a.ts': 'dev-team:x' });
    } finally {
      fx.cleanup();
    }
  });

  it('set written: [] → 清空该桶（净状态显式修正通道）', () => {
    const fx = createFixture({ written: ['src/a.ts'], deleted: ['src/b.ts'] });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { op: 'set', written: [] }));

      expect(result).toEqual({ written: [], deleted: ['src/b.ts'] });
    } finally {
      fx.cleanup();
    }
  });

  it('set 只提供 deleted 时 written 桶保持原状（未提供的桶不动）', () => {
    const fx = createFixture({ written: ['src/keep.ts'], deleted: ['src/gone.ts'] });
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { op: 'set', deleted: ['src/other.ts'] }),
      );

      expect(result).toEqual({ written: ['src/keep.ts'], deleted: ['src/other.ts'] });
    } finally {
      fx.cleanup();
    }
  });
});

// ===========================================================================
// runChangeFiles — 异常
// ===========================================================================

describe('runChangeFiles — 异常', () => {
  it('change 不存在 → 抛错', () => {
    const project = createTempProject();
    try {
      expect(() => runChangeFiles(appendOptions(project.root, { written: ['src/a.ts'] }))).toThrow(
        /workflow\.json 不存在/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 非法 → 硬报错', () => {
    const project = createTempProject();
    try {
      const changeDir = path.join(project.root, 'openspec', 'changes', 'my-change');
      fs.mkdirSync(changeDir, { recursive: true });
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), '{broken', 'utf-8');

      expect(() => runChangeFiles(appendOptions(project.root, { written: ['src/a.ts'] }))).toThrow(
        /解析失败/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 缺 files 字段 → 硬报错含「该 change 创建于文件清单机制之前，请重建」指引', () => {
    const fx = createFixture(undefined as unknown as Record<string, unknown>);
    // 手工覆盖为无 files 的旧形态
    fs.writeFileSync(
      fx.workflowPath,
      JSON.stringify({ workflow_type: 'requirement', created: '2026-09-17' }),
      'utf-8',
    );
    try {
      expect(() => runChangeFiles(appendOptions(fx.root, { written: ['src/a.ts'] }))).toThrow(
        /该 change 创建于文件清单机制之前，请重建/,
      );
    } finally {
      fx.cleanup();
    }
  });

  it('written 与 deleted 均缺省 → changeFilesInputSchema（至少其一 refine）拒绝', () => {
    expect(
      changeFilesInputSchema.safeParse({
        change: 'my-change',
        op: 'append',
        project_root: '/tmp/x',
      }).success,
    ).toBe(false);
    expect(
      changeFilesInputSchema.safeParse({
        change: 'my-change',
        op: 'set',
        written: [],
        deleted: [],
        project_root: '/tmp/x',
      }).success,
    ).toBe(false);
  });

  it('路径命中受保护范围（workflow.json 自身 / openspec/config.json / 自污染排除范围）→ 现状实现不拒绝、照常入清单（AC-10 偏差记录：本命令与 schema 均无受保护范围判断，仅校验路径形态；写入拦截由 PreToolUse 保护层、openspec 排除由记录器 hook 层承载）', () => {
    const protectedPaths = [
      'openspec/config.json',
      'openspec/changes/my-change/workflow.json',
      'openspec/changes/my-change/design.md',
    ];
    const fx = createFixture({ written: [], deleted: [] });
    try {
      // schema 层：均为相对项目根 POSIX 形态 → 形态校验通过，不做受保护范围判断
      for (const p of protectedPaths) {
        expect(
          changeFilesInputSchema.safeParse(appendOptions(fx.root, { written: [p] })).success,
        ).toBe(true);
      }

      // 命令层：同样不拒绝，照常折叠入净状态并落盘（按实际行为断言）
      const result = runChangeFiles(appendOptions(fx.root, { written: protectedPaths }));

      expect(result.written).toEqual(protectedPaths);
      const files = readWorkflow(fx).files as { written: string[] };
      expect(files.written).toEqual(protectedPaths);
    } finally {
      fx.cleanup();
    }
  });
});

// ===========================================================================
// runChangeFiles — 边界（changeFilesInputSchema 校验行为）
// ===========================================================================

describe('runChangeFiles — 边界（schema 校验行为）', () => {
  it('op 为非法枚举值（如 merge）→ schema 拒绝', () => {
    expect(
      changeFilesInputSchema.safeParse({
        change: 'my-change',
        op: 'merge',
        written: ['src/a.ts'],
        project_root: '/tmp/x',
      }).success,
    ).toBe(false);
  });

  it('路径为空字符串 / 绝对路径 / 反斜杠 / .. 穿越形态 → schema 拒绝（相对项目根 POSIX 风格契约，受保护与自污染范围由此挡在清单外）', () => {
    const base = { change: 'my-change', op: 'append' as const, project_root: '/tmp/x' };
    for (const bad of [
      '',
      '/abs/path.ts',
      'C:\\abs\\path.ts',
      'src\\backslash.ts',
      '../outside.ts',
      'src/../outside.ts',
      'openspec/changes/x/../../../etc.ts',
    ]) {
      expect(
        changeFilesInputSchema.safeParse({ ...base, written: ['src/ok.ts', bad] }).success,
      ).toBe(false);
    }
    // 合法 POSIX 相对路径通过
    expect(changeFilesInputSchema.safeParse({ ...base, written: ['src/ok.ts'] }).success).toBe(
      true,
    );
  });

  it('批量 100+ 路径一次性 append → 全部入净状态且去重稳定（超大列表）', () => {
    const fx = createFixture({ written: [], deleted: [] });
    try {
      const paths = Array.from({ length: 150 }, (_, i) => `src/generated/file${i}.ts`);
      // 注入重复项验证去重
      const withDupes = [...paths, paths[0], paths[7]];

      const result = runChangeFiles(appendOptions(fx.root, { written: withDupes }));

      expect(result.written).toHaveLength(150);
      expect(new Set(result.written).size).toBe(150);
      expect(result.written).toEqual(expect.arrayContaining(paths));
    } finally {
      fx.cleanup();
    }
  });

  it('append / set 输出仅含 written 与 deleted 两个字段（changeFilesOutput 形状契约）', () => {
    const fx = createFixture({
      written: [],
      deleted: [],
      source: { 'src/a.ts': 'dev-team:x' },
    });
    try {
      const result = runChangeFiles(appendOptions(fx.root, { written: ['src/a.ts'] }));
      expect(Object.keys(result).sort()).toEqual(['deleted', 'written']);
    } finally {
      fx.cleanup();
    }
  });
});
