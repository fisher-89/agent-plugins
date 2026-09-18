/**
 * 单元测试: commands/change-files.ts — change_files 核心逻辑（append / set）
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-8: append → workflow scope 按 path upsert（phase 审计记录保留）；set →
 *   删除涉及 path 全部记录 + 追加 workflow 记录，未涉及 path 不动
 * - ignored 路径不过滤（人工补录通道契约回归，迁移保留）；同批重复路径去重
 * - 异常: change 不存在 / workflow.json 非法 / 缺 file_log → 硬报错含「重建 change」指引
 * - 边界: 输出仅含 written/deleted 两字段（形状契约回归）；written 与 deleted
 *   均缺省 → schema 拒绝
 *
 * 文件系统不 mock：mkdtempSync 临时项目 + 含 file_log（可含 phase 条目）的
 * workflow.json fixture 真盘（同 file-inventory.test.ts 模式）。
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
  cleanup: () => void;
}

/** 构造一条 file_log 条目（workflow scope 缺省无 attempt）。 */
function logEntry(
  op: 'write' | 'delete' | 'revert',
  target: string,
  scope: string = 'workflow',
  attempt?: number,
): Record<string, unknown> {
  return attempt === undefined
    ? { op, scope, path: target, at: '2026-09-18T00:00:00.000Z' }
    : { op, scope, attempt, path: target, at: '2026-09-18T00:00:00.000Z' };
}

/** 创建项目 + change，并把 workflow.json 的 file_log 置为指定条目列表。 */
function createFixture(fileLog: unknown[], extraWorkflow: Record<string, unknown> = {}): Fixture {
  const project = createTempProject();
  const created = runChangeCreate('my-change', project.root, 'requirement');
  const workflowPath = path.join(created.path, 'workflow.json');
  fs.writeFileSync(
    workflowPath,
    `${JSON.stringify(
      {
        workflow_type: 'requirement',
        created: '2026-09-17',
        unknown_key: { keep: true },
        ...extraWorkflow,
        file_log: fileLog,
      },
      null,
      2,
    )}\n`,
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
// runChangeFiles — append (AC-8)
// ===========================================================================

describe('runChangeFiles — append (AC-8)', () => {
  it('append written/deleted → 追加 workflow scope 记录，返回派生净状态，与磁盘一致 (AC-8)', () => {
    const fx = createFixture([]);
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { written: ['src/foo.ts'], deleted: ['src/gone.ts'] }),
      );

      expect(result).toEqual({ written: ['src/foo.ts'], deleted: ['src/gone.ts'] });
      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      expect(log).toHaveLength(2);
      expect(log[0]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/foo.ts' });
      expect(log[1]).toMatchObject({ op: 'delete', scope: 'workflow', path: 'src/gone.ts' });
      // workflow 条目无 attempt 字段
      for (const entry of log) {
        expect(Object.prototype.hasOwnProperty.call(entry, 'attempt')).toBe(false);
      }
    } finally {
      fx.cleanup();
    }
  });

  it('已有 phase scope 审计记录同 path → upsert 仅覆盖 workflow 命名空间，phase 记录保留 (AC-8)', () => {
    const fx = createFixture([
      logEntry('write', 'src/foo.ts', 'implement', 1),
      logEntry('write', 'src/foo.ts'),
    ]);
    try {
      const result = runChangeFiles(appendOptions(fx.root, { deleted: ['src/foo.ts'] }));

      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      expect(log).toHaveLength(2);
      // phase 审计记录逐字保留
      expect(log[0]).toEqual(logEntry('write', 'src/foo.ts', 'implement', 1));
      // workflow 命名空间原位覆盖
      expect(log[1]).toMatchObject({ op: 'delete', scope: 'workflow', path: 'src/foo.ts' });
      expect(result).toEqual({ written: [], deleted: ['src/foo.ts'] });
    } finally {
      fx.cleanup();
    }
  });

  it('ignored 路径不过滤照常入 log（人工补录通道契约回归）；同批重复路径去重（迁移保留）', () => {
    const fx = createFixture([]);
    fs.writeFileSync(path.join(fx.root, '.gitignore'), '.claude/\n', 'utf-8');
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { written: ['.claude/memory.md', 'src/a.ts', 'src/a.ts'] }),
      );

      expect(result.written).toEqual(['.claude/memory.md', 'src/a.ts']);
      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      expect(log).toHaveLength(2);
      expect(log.map((e) => e.path)).toEqual(['.claude/memory.md', 'src/a.ts']);
    } finally {
      fx.cleanup();
    }
  });
});

// ===========================================================================
// runChangeFiles — set (AC-8)
// ===========================================================================

describe('runChangeFiles — set (AC-8)', () => {
  it('set → log 中涉及 provided path 的全部记录（任意 scope/attempt/op）删除 + 末尾追加 workflow 记录；未涉及 path 不动 (AC-8)', () => {
    const fx = createFixture([
      logEntry('write', 'src/a.ts', 'implement', 1),
      logEntry('delete', 'src/a.ts', 'implement', 2),
      logEntry('delete', 'src/a.ts'),
      logEntry('write', 'src/keep.ts', 'implement', 3),
    ]);
    try {
      const result = runChangeFiles(appendOptions(fx.root, { op: 'set', written: ['src/a.ts'] }));

      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      expect(log).toHaveLength(2);
      // 未涉及 path 的 phase 审计记录逐字保留、顺序不变
      expect(log[0]).toEqual(logEntry('write', 'src/keep.ts', 'implement', 3));
      // 末尾追加 workflow 记录
      expect(log[1]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/a.ts' });
      expect(result).toEqual({ written: ['src/keep.ts', 'src/a.ts'], deleted: [] });
    } finally {
      fx.cleanup();
    }
  });

  it('同 path 出现在 written 与 deleted 两桶 → deleted 胜（追加顺序 written 先、deleted 后）', () => {
    const fx = createFixture([logEntry('write', 'src/keep.ts')]);
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { op: 'set', written: ['src/a.ts'], deleted: ['src/a.ts'] }),
      );

      expect(result).toEqual({ written: ['src/keep.ts'], deleted: ['src/a.ts'] });
      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      // set 追加顺序 written 先、deleted 后（不去重）→ 派生后条胜 deleted
      expect(log).toHaveLength(3);
      expect(log[0]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/keep.ts' });
      expect(log[1]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/a.ts' });
      expect(log[2]).toMatchObject({ op: 'delete', scope: 'workflow', path: 'src/a.ts' });
    } finally {
      fx.cleanup();
    }
  });

  it('set 语义修正通道：先前 deleted 的路径 set 回 written 后派生净状态移出 deleted', () => {
    const fx = createFixture([logEntry('delete', 'src/gone.ts')]);
    try {
      const result = runChangeFiles(
        appendOptions(fx.root, { op: 'set', written: ['src/gone.ts'] }),
      );

      expect(result).toEqual({ written: ['src/gone.ts'], deleted: [] });
      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      expect(log).toHaveLength(1);
      expect(log[0]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/gone.ts' });
    } finally {
      fx.cleanup();
    }
  });
});

// ===========================================================================
// runChangeFiles — 异常（硬报错，不回退 git）
// ===========================================================================

describe('runChangeFiles — 异常', () => {
  it('change 不存在 → 抛「workflow.json 不存在」', () => {
    const project = createTempProject();
    try {
      expect(() => runChangeFiles(appendOptions(project.root, { written: ['src/a.ts'] }))).toThrow(
        /workflow\.json 不存在/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 非法 → 硬报错「解析失败」', () => {
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

  it('workflow.json 缺 file_log 字段 → 硬报错含「该 change 创建于文件清单机制之前，请重建」指引', () => {
    const project = createTempProject();
    try {
      const changeDir = path.join(project.root, 'openspec', 'changes', 'my-change');
      fs.mkdirSync(changeDir, { recursive: true });
      fs.writeFileSync(
        path.join(changeDir, 'workflow.json'),
        JSON.stringify({ workflow_type: 'requirement', created: '2026-09-17' }),
        'utf-8',
      );

      expect(() => runChangeFiles(appendOptions(project.root, { written: ['src/a.ts'] }))).toThrow(
        /该 change 创建于文件清单机制之前，请重建/,
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeFiles — 边界（形状契约与 schema 校验行为）
// ===========================================================================

describe('runChangeFiles — 边界（形状契约与 schema 校验行为）', () => {
  it('append / set 输出仅含 written 与 deleted 两个字段（changeFilesOutput 形状契约回归）', () => {
    const fx = createFixture([]);
    try {
      const appended = runChangeFiles(appendOptions(fx.root, { written: ['src/a.ts'] }));
      expect(Object.keys(appended).sort()).toEqual(['deleted', 'written']);

      const set = runChangeFiles(appendOptions(fx.root, { op: 'set', written: ['src/b.ts'] }));
      expect(Object.keys(set).sort()).toEqual(['deleted', 'written']);
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

  it('op 为非法枚举值（如 merge）→ schema 拒绝（迁移保留）', () => {
    expect(
      changeFilesInputSchema.safeParse({
        change: 'my-change',
        op: 'merge',
        written: ['src/a.ts'],
        project_root: '/tmp/x',
      }).success,
    ).toBe(false);
  });

  it('路径为空字符串 / 绝对路径 / 反斜杠 / .. 穿越形态 → schema 拒绝（迁移保留）', () => {
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
    expect(changeFilesInputSchema.safeParse({ ...base, written: ['src/ok.ts'] }).success).toBe(
      true,
    );
  });

  it('批量 100+ 路径一次性 append → 全部入 log 且去重稳定（超大列表，迁移保留）', () => {
    const fx = createFixture([]);
    try {
      const paths = Array.from({ length: 150 }, (_, i) => `src/generated/file${i}.ts`);
      const withDupes = [...paths, paths[0], paths[7]];

      const result = runChangeFiles(appendOptions(fx.root, { written: withDupes }));

      expect(result.written).toHaveLength(150);
      expect(new Set(result.written).size).toBe(150);
      expect(result.written).toEqual(expect.arrayContaining(paths));
    } finally {
      fx.cleanup();
    }
  });

  it('路径命中 openspec/** / workflow.json（自污染范围）→ 现状实现不拒绝、照常入 log（schema 与命令层均无受保护范围判断，迁移保留）', () => {
    const protectedPaths = ['openspec/config.json', 'openspec/changes/my-change/workflow.json'];
    const fx = createFixture([]);
    try {
      for (const p of protectedPaths) {
        expect(
          changeFilesInputSchema.safeParse(appendOptions(fx.root, { written: [p] })).success,
        ).toBe(true);
      }

      const result = runChangeFiles(appendOptions(fx.root, { written: protectedPaths }));

      expect(result.written).toEqual(protectedPaths);
      const log = readWorkflow(fx).file_log as Array<Record<string, unknown>>;
      expect(log.map((e) => e.path)).toEqual(protectedPaths);
    } finally {
      fx.cleanup();
    }
  });
});
