/**
 * 单元测试: modules/workflow/file-inventory.ts — change 文件清单（workflow.json.files）共享库
 *
 * 覆盖范围（openspec/changes/workflow-file-inventory/test-design.md）:
 * - AC-1: change_create 初始净状态可被 workflowFileSchema 解析
 * - AC-2 / AC-4: readFileInventory 读取与校验、foldFileOps 三条对称折叠规则
 * - AC-6: writeFileInventory 空净状态写回（记录器空折叠共用通道）
 * - AC-13: source 旁挂来源维护（last-writer-wins / 重写清除 / 折叠联动）
 *
 * 文件系统不 mock：mkdtempSync 临时目录 + 真实读写（同 change-create.test.ts 模式）。
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import {
  type FileInventory,
  type FileOp,
  foldFileOps,
  readFileInventory,
  writeFileInventory,
} from './file-inventory';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'file-inventory-test-'): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function createChangeDir(project: TempProject, name = 'my-change'): string {
  const changeDir = path.join(project.root, 'openspec', 'changes', name);
  fs.mkdirSync(changeDir, { recursive: true });
  return changeDir;
}

function writeWorkflowJson(changeDir: string, doc: unknown): void {
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), JSON.stringify(doc), 'utf-8');
}

function readWorkflowJsonRaw(changeDir: string): string {
  return fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');
}

/** 构造一条能通过 phaseLogSchema 校验的 eval 条目（workflowFileSchema 要求）。 */
function evalEntry(phase: string): Record<string, unknown> {
  return {
    phase,
    verdict: 'pass',
    attempt: 1,
    timestamp: '2026-09-17T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: true, evidence: 'e' }],
    backtrack_to: null,
  };
}

/** 合法 workflow.json 文档（含 eval / 未知键 / files）。 */
function validWorkflowDoc(
  files: unknown,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-17',
    eval: [evalEntry('proposal')],
    unknown_key: { keep: true },
    ...extra,
    files,
  };
}

// ===========================================================================
// readFileInventory — 正向
// ===========================================================================

describe('readFileInventory — 正向', () => {
  it('合法 workflow.json（workflow_type + created + eval 数组 + 未知键 + files 双数组 + source 映射）返回完整 FileInventory，written/deleted/source 透传 (AC-4)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({
          written: ['src/a.ts', 'src/b.ts'],
          deleted: ['src/old.ts'],
          source: { 'src/a.ts': 'dev-team:implementation-generator' },
        }),
      );

      const inventory = readFileInventory(changeDir);
      expect(inventory).toEqual({
        written: ['src/a.ts', 'src/b.ts'],
        deleted: ['src/old.ts'],
        source: { 'src/a.ts': 'dev-team:implementation-generator' },
      });
    } finally {
      project.cleanup();
    }
  });

  it('files 缺 source 键时返回 source 为 undefined（不补默认空对象，保证写回不新增键）(AC-13)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: ['src/a.ts'], deleted: [] }));

      const inventory = readFileInventory(changeDir);
      expect(inventory.written).toEqual(['src/a.ts']);
      expect(inventory.deleted).toEqual([]);
      expect(Object.prototype.hasOwnProperty.call(inventory, 'source')).toBe(false);
      expect(inventory.source).toBeUndefined();
    } finally {
      project.cleanup();
    }
  });

  it('files 内出现未知额外字段时 looseObject 语义不报错（与 workflow.schema 未知键保留策略一致）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({ written: [], deleted: [], future_field: 'reserved' }),
      );

      expect(() => readFileInventory(changeDir)).not.toThrow();
      expect(readFileInventory(changeDir).written).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('files: { written: [], deleted: [] } 空净状态合法返回（change_create 初始形态，AC-1）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      expect(readFileInventory(changeDir)).toEqual({ written: [], deleted: [] });
    } finally {
      project.cleanup();
    }
  });

  it('written 含空字符串路径 / 超长路径（>1000 chars）/ 空格、中文、emoji 路径 → 原样透传不做语义裁剪', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const longPath = `src/${'x'.repeat(1001)}.ts`;
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({
          written: ['', longPath, 'src/含 空格.ts', 'src/中文路径.ts', 'src/emoji-🧪.ts'],
          deleted: [],
        }),
      );

      const inventory = readFileInventory(changeDir);
      expect(inventory.written).toEqual([
        '',
        longPath,
        'src/含 空格.ts',
        'src/中文路径.ts',
        'src/emoji-🧪.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// readFileInventory — 异常
// ===========================================================================

describe('readFileInventory — 异常', () => {
  it('changeDir 下无 workflow.json → 抛错含 change_create 指引，且不创建任何文件或目录', () => {
    const project = createTempProject();
    try {
      const changeDir = path.join(project.root, 'openspec', 'changes', 'missing');

      expect(() => readFileInventory(changeDir)).toThrow(/workflow\.json 不存在/);
      expect(() => readFileInventory(changeDir)).toThrow(/change_create/);
      expect(fs.existsSync(changeDir)).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json JSON 非法（截断）→ 抛解析错误', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), '{"workflow_type": "requ', 'utf-8');

      expect(() => readFileInventory(changeDir)).toThrow(/解析失败/);
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 含注释（非 JSON）→ 抛解析错误', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      fs.writeFileSync(
        path.join(changeDir, 'workflow.json'),
        '{ /* comment */ "workflow_type": "requirement" }',
        'utf-8',
      );

      expect(() => readFileInventory(changeDir)).toThrow(/解析失败/);
    } finally {
      project.cleanup();
    }
  });

  it('根元素为数组 / 字符串 / null → 抛「根元素必须是对象」类错误', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      for (const raw of ['[]', '"just a string"', 'null']) {
        fs.writeFileSync(path.join(changeDir, 'workflow.json'), raw, 'utf-8');
        expect(() => readFileInventory(changeDir)).toThrow(/根元素必须是对象/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('合法 workflow.json 但无 files 字段（机制前旧 change）→ 抛错文案含「该 change 创建于文件清单机制之前，请重建」', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, { workflow_type: 'requirement', created: '2026-01-01' });

      expect(() => readFileInventory(changeDir)).toThrow(
        /该 change 创建于文件清单机制之前，请重建/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('files.written / files.deleted 为字符串 / 对象 / null 等非数组 → schema 校验抛「格式非法」', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const invalidFiles: unknown[] = [
        { written: 'src/a.ts', deleted: [] },
        { written: [], deleted: { 0: 'x' } },
        { written: null, deleted: null },
        {},
      ];
      for (const files of invalidFiles) {
        writeWorkflowJson(changeDir, validWorkflowDoc(files));
        expect(() => readFileInventory(changeDir)).toThrow(/格式非法/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 为非法枚举时 schema 校验抛错（整文件校验先于 files 判定）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({ written: [], deleted: [] }, { workflow_type: 'unknown-flow' }),
      );

      expect(() => readFileInventory(changeDir)).toThrow(/格式非法/);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// foldFileOps — 折叠规则（AC-4）
// ===========================================================================

describe('foldFileOps — 折叠规则 (AC-4)', () => {
  it('空净状态 + [write(P)] → written=[P]、deleted=[]', () => {
    expect(
      foldFileOps({ written: [], deleted: [] }, [{ op: 'write', path: 'src/foo.ts' }]),
    ).toEqual({ written: ['src/foo.ts'], deleted: [] });
  });

  it('空净状态 + [delete(P)] → deleted=[P]、written=[]', () => {
    expect(
      foldFileOps({ written: [], deleted: [] }, [{ op: 'delete', path: 'src/foo.ts' }]),
    ).toEqual({ written: [], deleted: ['src/foo.ts'] });
  });

  it('written=[P] + [revert(P)] → 双桶均无 P（write→revert 净 untouched）(AC-4)', () => {
    expect(
      foldFileOps({ written: ['src/foo.ts'], deleted: [] }, [{ op: 'revert', path: 'src/foo.ts' }]),
    ).toEqual({ written: [], deleted: [] });
  });

  it('deleted=[P] + [write(P)] → P 移出 deleted 进入 written（delete→write 折叠为 written，单桶不变量）(AC-4)', () => {
    expect(
      foldFileOps({ written: [], deleted: ['src/foo.ts'] }, [{ op: 'write', path: 'src/foo.ts' }]),
    ).toEqual({ written: ['src/foo.ts'], deleted: [] });
  });

  it('written=[P] + [delete(P)] → P 移出 written 进入 deleted', () => {
    expect(
      foldFileOps({ written: ['src/foo.ts'], deleted: [] }, [{ op: 'delete', path: 'src/foo.ts' }]),
    ).toEqual({ written: [], deleted: ['src/foo.ts'] });
  });

  it('[delete(old), write(new)]（mv 双条目）→ old 在 deleted、new 在 written (AC-4)', () => {
    const ops: FileOp[] = [
      { op: 'delete', path: 'src/a.ts' },
      { op: 'write', path: 'src/b.ts' },
    ];
    expect(foldFileOps({ written: [], deleted: [] }, ops)).toEqual({
      written: ['src/b.ts'],
      deleted: ['src/a.ts'],
    });
  });

  it('write→delete→write→revert 等 10+ 步长序列折叠：终态与逐步语义一致（顺序确定性）', () => {
    const start: FileInventory = { written: [], deleted: [] };
    const ops: FileOp[] = [
      { op: 'write', path: 'p1' },
      { op: 'delete', path: 'p1' },
      { op: 'write', path: 'p1' },
      { op: 'revert', path: 'p1' },
      { op: 'write', path: 'p2' },
      { op: 'write', path: 'p2' },
      { op: 'delete', path: 'p2' },
      { op: 'write', path: 'p2' },
      { op: 'delete', path: 'p3' },
      { op: 'write', path: 'p3' },
      { op: 'revert', path: 'p3' },
      { op: 'write', path: 'p4' },
    ];
    // 逐步手算：p1 净 untouched；p2 write→write→delete→write 净 written；
    // p3 delete→write→revert 净 untouched；p4 written。
    const expected: FileInventory = { written: ['p2', 'p4'], deleted: [] };
    // 一步到位
    expect(foldFileOps(start, ops)).toEqual(expected);
    // 分批折叠与一次折叠等价（顺序确定性 / 结合性）
    const split = foldFileOps(
      foldFileOps(foldFileOps(start, ops.slice(0, 4)), ops.slice(4, 8)),
      ops.slice(8),
    );
    expect(split).toEqual(expected);
  });

  it('同一 op 重复出现（[write(P), write(P)]）→ written 单条目（去重幂等）', () => {
    const ops: FileOp[] = [
      { op: 'write', path: 'src/foo.ts' },
      { op: 'write', path: 'src/foo.ts' },
    ];
    const result = foldFileOps({ written: [], deleted: [] }, ops);
    expect(result.written).toEqual(['src/foo.ts']);
    expect(result.written).toHaveLength(1);
  });

  it('目录路径（rm -rf src/lib 记录目录路径本身）与文件路径同规则折叠，不做 glob 展开 (AC-4)', () => {
    const ops: FileOp[] = [
      { op: 'delete', path: 'src/lib' },
      { op: 'write', path: 'src/lib' },
    ];
    expect(foldFileOps({ written: [], deleted: [] }, ops)).toEqual({
      written: ['src/lib'],
      deleted: [],
    });

    const deleteOnly = foldFileOps({ written: [], deleted: [] }, [
      { op: 'delete', path: 'src/lib' },
    ]);
    expect(deleteOnly).toEqual({ written: [], deleted: ['src/lib'] });
  });
});

// ===========================================================================
// foldFileOps — source 旁挂来源维护（AC-13）
// ===========================================================================

describe('foldFileOps — source 旁挂来源维护 (AC-13)', () => {
  it('op 携带 agentType → source[path]=agentType；同路径再次 write 携带不同 agentType → 覆写（last-writer-wins）(AC-13)', () => {
    const first = foldFileOps({ written: [], deleted: [] }, [
      { op: 'write', path: 'src/foo.ts', agentType: 'dev-team:implementation-generator' },
    ]);
    expect(first.source).toEqual({ 'src/foo.ts': 'dev-team:implementation-generator' });

    const second = foldFileOps(first, [
      { op: 'write', path: 'src/foo.ts', agentType: 'dev-team:test-gen-generator' },
    ]);
    expect(second.source).toEqual({ 'src/foo.ts': 'dev-team:test-gen-generator' });
  });

  it('已有 source[path] 的路径被不带 agentType 的 op 重写 → source[path] 清除 (AC-13)', () => {
    const inventory: FileInventory = {
      written: ['src/foo.ts'],
      deleted: [],
      source: { 'src/foo.ts': 'dev-team:implementation-generator' },
    };
    const result = foldFileOps(inventory, [{ op: 'write', path: 'src/foo.ts' }]);
    expect(result.written).toEqual(['src/foo.ts']);
    expect(result.source).toBeUndefined();
  });

  it('条目被 revert 折叠移出双桶 → 对应 source 键随之移除 (AC-13)', () => {
    const inventory: FileInventory = {
      written: ['src/foo.ts'],
      deleted: [],
      source: { 'src/foo.ts': 'dev-team:implementation-generator', 'src/kept.ts': 'dev-team:x' },
    };
    const result = foldFileOps(inventory, [{ op: 'revert', path: 'src/foo.ts' }]);
    expect(result.written).toEqual([]);
    expect(result.source).toEqual({ 'src/kept.ts': 'dev-team:x' });
  });

  it('条目经 write→delete 折叠移出 written 桶且 delete 不带 agentType → source 键随之移除 (AC-13)', () => {
    const inventory: FileInventory = {
      written: ['src/foo.ts'],
      deleted: [],
      source: { 'src/foo.ts': 'dev-team:implementation-generator' },
    };
    const result = foldFileOps(inventory, [{ op: 'delete', path: 'src/foo.ts' }]);
    expect(result.deleted).toEqual(['src/foo.ts']);
    expect(result.source).toBeUndefined();
  });

  it('delete op 携带 agentType → deleted 路径的 source 记录删除者（审计语义）', () => {
    const result = foldFileOps({ written: [], deleted: [] }, [
      { op: 'delete', path: 'src/old.ts', agentType: 'dev-team:implementation-generator' },
    ]);
    expect(result.deleted).toEqual(['src/old.ts']);
    expect(result.source).toEqual({ 'src/old.ts': 'dev-team:implementation-generator' });
  });

  it('source 全部清空后结果不含 source 键（空映射从结果中丢弃）', () => {
    const inventory: FileInventory = {
      written: ['src/foo.ts'],
      deleted: [],
      source: { 'src/foo.ts': 'dev-team:implementation-generator' },
    };
    const result = foldFileOps(inventory, [{ op: 'revert', path: 'src/foo.ts' }]);
    expect(Object.prototype.hasOwnProperty.call(result, 'source')).toBe(false);
  });
});

// ===========================================================================
// foldFileOps — 异常与纯函数不变量
// ===========================================================================

describe('foldFileOps — 异常与纯函数不变量', () => {
  it('未知 op 值（如 copy）按运行时兜底语义忽略：净状态不变（switch 无 default，schema 层已约束枚举）', () => {
    const inventory: FileInventory = { written: ['src/a.ts'], deleted: ['src/b.ts'] };
    const unknownOp = { op: 'copy', path: 'src/a.ts' } as unknown as FileOp;

    expect(foldFileOps(inventory, [unknownOp])).toEqual({
      written: ['src/a.ts'],
      deleted: ['src/b.ts'],
    });
  });

  it('ops=[] → 返回与输入等价的净状态深拷贝，且不修改入参 inventory（纯函数不变量）', () => {
    const inventory: FileInventory = {
      written: ['src/a.ts'],
      deleted: ['src/b.ts'],
      source: { 'src/a.ts': 'dev-team:x' },
    };
    const snapshot = JSON.parse(JSON.stringify(inventory)) as FileInventory;

    const result = foldFileOps(inventory, []);

    expect(result).toEqual(inventory);
    expect(result).not.toBe(inventory);
    expect(result.written).not.toBe(inventory.written);
    // 入参未被修改
    expect(inventory).toEqual(snapshot);
  });

  it('折叠结果中任意路径不同时存在于 written 与 deleted（单桶不变量扫描断言）', () => {
    const sequences: FileOp[][] = [
      [
        { op: 'write', path: 'p1' },
        { op: 'delete', path: 'p1' },
        { op: 'write', path: 'p1' },
        { op: 'delete', path: 'p2' },
        { op: 'write', path: 'p2' },
      ],
      [
        { op: 'delete', path: 'p3' },
        { op: 'revert', path: 'p3' },
        { op: 'write', path: 'p3' },
        { op: 'revert', path: 'p3' },
      ],
      [
        { op: 'write', path: 'p4' },
        { op: 'revert', path: 'p4' },
        { op: 'delete', path: 'p4' },
      ],
    ];
    for (const ops of sequences) {
      const result = foldFileOps({ written: [], deleted: [] }, ops);
      const overlap = result.written.filter((p) => result.deleted.includes(p));
      expect(overlap).toEqual([]);
    }
  });
});

// ===========================================================================
// writeFileInventory — 写回纪律
// ===========================================================================

describe('writeFileInventory — 写回纪律', () => {
  it('写回后 files 更新；workflow_type / created / eval / 未知键原样保留 (AC-4)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      writeFileInventory(changeDir, { written: ['src/new.ts'], deleted: ['src/gone.ts'] });

      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      expect(doc.files).toEqual({ written: ['src/new.ts'], deleted: ['src/gone.ts'] });
      expect(doc.workflow_type).toBe('requirement');
      expect(doc.created).toBe('2026-09-17');
      expect(doc.eval).toEqual([evalEntry('proposal')]);
      expect(doc.unknown_key).toEqual({ keep: true });
    } finally {
      project.cleanup();
    }
  });

  it('输出 2 空格缩进 + 末尾换行（与 eval-json 写盘纪律同构）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      writeFileInventory(changeDir, { written: ['src/a.ts'], deleted: [] });

      const raw = readWorkflowJsonRaw(changeDir);
      expect(raw.endsWith('\n')).toBe(true);
      expect(raw).toContain('\n  "workflow_type"');
      expect(raw).toContain('\n      "src/a.ts"');
      // 可被 JSON.parse 还原（写盘后可读）
      expect(() => JSON.parse(raw)).not.toThrow();
    } finally {
      project.cleanup();
    }
  });

  it('source 非空时写为旁挂映射（仅审计字段）；source 为 undefined 时不写 source 键 (AC-13)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      // source 为 undefined：不新增 source 键
      writeFileInventory(changeDir, { written: ['src/a.ts'], deleted: [] });
      let doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      let files = doc.files as Record<string, unknown>;
      expect(Object.prototype.hasOwnProperty.call(files, 'source')).toBe(false);

      // source 非空：写为旁挂映射
      writeFileInventory(changeDir, {
        written: ['src/a.ts'],
        deleted: [],
        source: { 'src/a.ts': 'dev-team:implementation-generator' },
      });
      doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      files = doc.files as Record<string, unknown>;
      expect(files.source).toEqual({ 'src/a.ts': 'dev-team:implementation-generator' });
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 不存在 → 抛错、不创建文件与目录（绝不创建契约）(AC-2)', () => {
    const project = createTempProject();
    try {
      const changeDir = path.join(project.root, 'openspec', 'changes', 'ghost');

      expect(() => writeFileInventory(changeDir, { written: ['src/a.ts'], deleted: [] })).toThrow(
        /workflow\.json 不存在/,
      );
      expect(fs.existsSync(changeDir)).toBe(false);
      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json JSON 非法 → 抛错且磁盘内容逐字节不变', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const broken = '{"workflow_type": "requirement",';
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), broken, 'utf-8');

      expect(() => writeFileInventory(changeDir, { written: ['src/a.ts'], deleted: [] })).toThrow(
        /解析失败/,
      );
      expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(broken);
    } finally {
      project.cleanup();
    }
  });

  it('根元素为数组时抛「根元素必须是对象」且内容不变', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), '[]', 'utf-8');

      expect(() => writeFileInventory(changeDir, { written: [], deleted: [] })).toThrow(
        /根元素必须是对象/,
      );
      expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe('[]');
    } finally {
      project.cleanup();
    }
  });

  it('空净状态写回（written=[] deleted=[]）成功（记录器空折叠共用通道）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({
          written: ['src/a.ts'],
          deleted: ['src/b.ts'],
          source: { 'src/a.ts': 'dev-team:x' },
        }),
      );

      writeFileInventory(changeDir, { written: [], deleted: [] });

      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      expect(doc.files).toEqual({ written: [], deleted: [] });
      expect(doc.eval).toEqual([evalEntry('proposal')]);
    } finally {
      project.cleanup();
    }
  });
});
