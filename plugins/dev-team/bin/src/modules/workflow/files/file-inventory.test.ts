/**
 * 单元测试: modules/workflow/files/file-inventory.ts — 日志式 change 文件清单
 * （workflow.json.file_log）共享库
 *
 * 覆盖范围:
 * - openspec/changes/phase-lifecycle-file-log/proposal.md:
 *   AC-6 (scope 命名空间, attempt?, path) 键控覆盖/追加、readNetState 派生净状态
 *   （与现行 foldFileOps 折叠规则等价）；AC-7 缺失 file_log 硬报错含重建指引、不回退 git；
 *   AC-8 appendWorkflowFiles workflow scope 按 path upsert（phase 审计记录保留）、
 *   setWorkflowFiles 删涉及 path 全部记录 + 末尾追加 workflow 记录
 * - 读写纪律与 doc-io 一致：文件必须已存在、未知键保留、2 空格缩进 + 尾换行
 * - 本文件不接 gitignore 过滤（记录器管线的结构性保障）
 *
 * 文件系统不 mock：mkdtempSync 临时目录 + 真实读写（同 change-create.test.ts 模式）。
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import {
  appendLogEntries,
  appendWorkflowFiles,
  type FileLogEntry,
  type FileOp,
  type NetFileState,
  readNetState,
  setWorkflowFiles,
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

/** 直接从磁盘读 file_log 原始条目（断言真实落盘形态，不经被测读通道）。 */
function readLogFromDisk(changeDir: string): FileLogEntry[] {
  return (JSON.parse(readWorkflowJsonRaw(changeDir)) as { file_log: FileLogEntry[] }).file_log;
}

/** 将 log 落盘后经读通道 readNetState 取派生净状态（派生规则的读通路形态）。 */
function netStateOf(log: FileLogEntry[]): NetFileState {
  const project = createTempProject();
  try {
    const changeDir = createChangeDir(project);
    writeWorkflowJson(changeDir, validWorkflowDoc(log));
    return readNetState(changeDir);
  } finally {
    project.cleanup();
  }
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

/** 合法 workflow.json 文档（含 eval / 未知键 / file_log）。 */
function validWorkflowDoc(
  fileLog: unknown,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-17',
    eval: [evalEntry('proposal')],
    unknown_key: { keep: true },
    ...extra,
    file_log: fileLog,
  };
}

/** 构造一条合法 file_log 条目（scope 缺省为 workflow 命名空间）。 */
function logEntry(
  op: FileOp['op'],
  path: string,
  scope: string = 'workflow',
  attempt?: number,
): FileLogEntry {
  return attempt === undefined
    ? { op, scope, path, at: '2026-09-18T00:00:00.000Z' }
    : { op, scope, attempt, path, at: '2026-09-18T00:00:00.000Z' };
}

// ===========================================================================
// readNetState — 正向
// ===========================================================================

describe('readNetState — 正向', () => {
  it('合法 workflow.json（workflow_type + created + eval + 未知键 + 混合 phase/workflow 条目）→ 派生净状态 (AC-6)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const log = [logEntry('write', 'src/a.ts', 'implement', 1), logEntry('delete', 'src/old.ts')];
      writeWorkflowJson(changeDir, validWorkflowDoc(log));

      expect(readNetState(changeDir)).toEqual({ written: ['src/a.ts'], deleted: ['src/old.ts'] });
    } finally {
      project.cleanup();
    }
  });

  it('file_log: [] 空日志合法返回双空净状态（change_create 初始形态）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      expect(readNetState(changeDir)).toEqual({ written: [], deleted: [] });
    } finally {
      project.cleanup();
    }
  });

  it('file_log 条目缺 attempt（workflow scope）与带 attempt（phase scope）均合法读取并参与派生', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const log = [logEntry('write', 'src/w.ts'), logEntry('write', 'src/p.ts', 'implement', 2)];
      writeWorkflowJson(changeDir, validWorkflowDoc(log));

      expect(readNetState(changeDir)).toEqual({ written: ['src/w.ts', 'src/p.ts'], deleted: [] });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// readNetState — 异常
// ===========================================================================

describe('readNetState — 异常', () => {
  it('changeDir 下无 workflow.json → 抛错含 change_create 指引，且不创建任何文件或目录', () => {
    const project = createTempProject();
    try {
      const changeDir = path.join(project.root, 'openspec', 'changes', 'missing');

      expect(() => readNetState(changeDir)).toThrow(/workflow\.json 不存在/);
      expect(() => readNetState(changeDir)).toThrow(/change_create/);
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

      expect(() => readNetState(changeDir)).toThrow(/解析失败/);
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
        expect(() => readNetState(changeDir)).toThrow(/根元素必须是对象/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('合法 workflow.json 但无 file_log 字段（机制前旧 change）→ 抛错文案含「该 change 创建于文件清单机制之前，请重建」，不回退 git (AC-7)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, { workflow_type: 'requirement', created: '2026-01-01' });

      expect(() => readNetState(changeDir)).toThrow(/该 change 创建于文件清单机制之前，请重建/);
    } finally {
      project.cleanup();
    }
  });

  it('file_log 为字符串 / 对象等非数组，或条目缺字段 / op 非法枚举 → schema 校验抛「格式非法」', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const invalidLogs: unknown[] = [
        'not-an-array',
        { op: 'write' },
        [{ op: 'copy', scope: 'workflow', path: 'src/a.ts', at: '2026-09-18T00:00:00.000Z' }],
        [{ scope: 'workflow', path: 'src/a.ts', at: '2026-09-18T00:00:00.000Z' }],
      ];
      for (const fileLog of invalidLogs) {
        writeWorkflowJson(changeDir, validWorkflowDoc(fileLog));
        expect(() => readNetState(changeDir)).toThrow(/格式非法/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 为非法枚举时 schema 校验抛错（整文件校验先于 file_log 判定）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([], { workflow_type: 'unknown-flow' }));

      expect(() => readNetState(changeDir)).toThrow(/格式非法/);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// readNetState — 派生规则（与现行 foldFileOps 等价，AC-6）
// ===========================================================================

describe('readNetState — 派生规则 (AC-6)', () => {
  it('空 log → 双桶均空', () => {
    expect(netStateOf([])).toEqual({ written: [], deleted: [] });
  });

  it('读取为纯读：readNetState 前后 workflow.json 逐字节不变（读通道无副作用）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([
          logEntry('write', 'src/a.ts', 'implement', 1),
          logEntry('delete', 'src/a.ts'),
          logEntry('revert', 'src/b.ts'),
        ]),
      );
      const before = readWorkflowJsonRaw(changeDir);

      readNetState(changeDir);

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('[write(P)] → written=[P]、deleted=[]', () => {
    expect(netStateOf([logEntry('write', 'src/foo.ts')])).toEqual({
      written: ['src/foo.ts'],
      deleted: [],
    });
  });

  it('[delete(P)] → deleted=[P]、written=[]', () => {
    expect(netStateOf([logEntry('delete', 'src/foo.ts')])).toEqual({
      written: [],
      deleted: ['src/foo.ts'],
    });
  });

  it('[write(P), revert(P)] → 双桶均无 P（write→revert 净 untouched）(AC-6)', () => {
    expect(netStateOf([logEntry('write', 'src/foo.ts'), logEntry('revert', 'src/foo.ts')])).toEqual(
      { written: [], deleted: [] },
    );
  });

  it('[delete(P), write(P)] → P 移出 deleted 进入 written（单桶不变量）(AC-6)', () => {
    expect(netStateOf([logEntry('delete', 'src/foo.ts'), logEntry('write', 'src/foo.ts')])).toEqual(
      { written: ['src/foo.ts'], deleted: [] },
    );
  });

  it('[delete(old), write(new)]（mv 双条目）→ old 在 deleted、new 在 written (AC-6)', () => {
    expect(netStateOf([logEntry('delete', 'src/a.ts'), logEntry('write', 'src/b.ts')])).toEqual({
      written: ['src/b.ts'],
      deleted: ['src/a.ts'],
    });
  });

  it('同 path 后条胜：phase 记录之后再记 workflow → workflow 覆盖（scope 翻转派生语义）', () => {
    const log = [logEntry('write', 'src/a.ts', 'implement', 1), logEntry('delete', 'src/a.ts')];
    expect(netStateOf(log)).toEqual({ written: [], deleted: ['src/a.ts'] });
  });

  it('attempt 不参与净状态：同 path 跨 attempt 后条胜', () => {
    const log = [
      logEntry('write', 'src/a.ts', 'implement', 1),
      logEntry('revert', 'src/a.ts', 'implement', 2),
    ];
    expect(netStateOf(log)).toEqual({ written: [], deleted: [] });
  });

  it('write→delete→write→revert 等 10+ 步长序列派生：终态与逐步语义一致（顺序确定性）', () => {
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
    const log = ops.map((op) => logEntry(op.op, op.path));
    // 逐步手算：p1 净 untouched；p2 write→write→delete→write 净 written；
    // p3 delete→write→revert 净 untouched；p4 written。
    expect(netStateOf(log)).toEqual({ written: ['p2', 'p4'], deleted: [] });
  });

  it('派生结果中任意路径不同时存在于 written 与 deleted（单桶不变量扫描断言）', () => {
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
      const result = netStateOf(ops.map((op) => logEntry(op.op, op.path)));
      const overlap = result.written.filter((p: string) => result.deleted.includes(p));
      expect(overlap).toEqual([]);
    }
  });
});

// ===========================================================================
// appendLogEntries — 键控覆盖/追加与落盘纪律
// ===========================================================================

describe('appendLogEntries — 键控覆盖/追加', () => {
  it('同 key（phase, attempt, path）再次 append → 原位覆盖 op/at，数组位置保留 (AC-6)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const initial = [
        logEntry('delete', 'src/keep.ts'),
        logEntry('write', 'src/a.ts', 'implement', 1),
      ];
      writeWorkflowJson(changeDir, validWorkflowDoc(initial));

      const next = appendLogEntries(changeDir, [
        {
          op: 'delete',
          scope: 'implement',
          attempt: 1,
          path: 'src/a.ts',
          at: '2026-09-18T09:00:00.000Z',
        },
      ]);

      expect(next).toHaveLength(2);
      expect(next[0]).toEqual(logEntry('delete', 'src/keep.ts'));
      expect(next[1]).toEqual({
        op: 'delete',
        scope: 'implement',
        attempt: 1,
        path: 'src/a.ts',
        at: '2026-09-18T09:00:00.000Z',
      });
      // 落盘内容与返回一致
      expect(readLogFromDisk(changeDir)).toEqual(next);
    } finally {
      project.cleanup();
    }
  });

  it('跨 attempt / 跨 phase / 跨 scope 同 path → 追加末尾（审计轨迹保留），派生后条胜', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([logEntry('write', 'src/a.ts', 'implement', 1)]),
      );

      const next = appendLogEntries(changeDir, [
        logEntry('delete', 'src/a.ts', 'implement', 2),
        logEntry('write', 'src/a.ts'),
      ]);

      expect(next).toHaveLength(3);
      expect(readNetState(changeDir)).toEqual({ written: ['src/a.ts'], deleted: [] });
    } finally {
      project.cleanup();
    }
  });

  it('workflow 命名空间按 path 去重：同 path 二次 append → 单条目原位覆盖', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const next = appendLogEntries(changeDir, [
        logEntry('write', 'src/a.ts'),
        logEntry('delete', 'src/a.ts'),
      ]);

      expect(next).toHaveLength(1);
      expect(next[0].op).toBe('delete');
    } finally {
      project.cleanup();
    }
  });

  it('entries=[] → 不写盘（磁盘逐字节不变）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const initial = validWorkflowDoc([logEntry('write', 'src/keep.ts')]);
      writeWorkflowJson(changeDir, initial);
      const before = readWorkflowJsonRaw(changeDir);

      appendLogEntries(changeDir, []);

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('落盘后 workflow_type / created / eval / 未知键保留、2 空格缩进 + 尾换行（doc-io 纪律）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      appendLogEntries(changeDir, [logEntry('write', 'src/a.ts')]);

      const raw = readWorkflowJsonRaw(changeDir);
      const doc = JSON.parse(raw) as Record<string, unknown>;
      expect(doc.workflow_type).toBe('requirement');
      expect(doc.created).toBe('2026-09-17');
      expect(doc.eval).toEqual([evalEntry('proposal')]);
      expect(doc.unknown_key).toEqual({ keep: true });
      expect(raw.endsWith('\n')).toBe(true);
      expect(raw).toContain('\n  "workflow_type"');
      expect(raw).toContain('\n      "path": "src/a.ts"');
    } finally {
      project.cleanup();
    }
  });

  it('legacy change（无 file_log）→ 抛错向上传播（命令层 catch-all 兜底），文件不被半写 (AC-7)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const legacy = { workflow_type: 'requirement', created: '2026-09-18' };
      writeWorkflowJson(changeDir, legacy);
      const before = readWorkflowJsonRaw(changeDir);

      expect(() => appendLogEntries(changeDir, [logEntry('write', 'src/a.ts')])).toThrow(
        /该 change 创建于文件清单机制之前，请重建/,
      );
      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('100+ 路径批量 append（workflow scope）→ 全部入 log 且按 path 去重稳定', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      const paths = Array.from({ length: 150 }, (_, i) => `src/batch/file${i}.ts`);
      const withDupes = [
        ...paths.map((p) => logEntry('write', p)),
        logEntry('write', paths[0]),
        logEntry('write', paths[7]),
      ];

      const next = appendLogEntries(changeDir, withDupes);

      expect(next).toHaveLength(150);
      expect(new Set(next.map((e) => e.path)).size).toBe(150);
      expect(readLogFromDisk(changeDir)).toHaveLength(150);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// appendWorkflowFiles — append 读改写（workflow scope upsert）
// ===========================================================================

describe('appendWorkflowFiles — append 读改写', () => {
  it('空 log append written 新路径 → 追加 workflow 条目，返回派生净状态与磁盘一致 (AC-8)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const result = appendWorkflowFiles(changeDir, { written: ['src/foo.ts'] });

      expect(result).toEqual({ written: ['src/foo.ts'], deleted: [] });
      // 落盘条目：workflow scope、at 为当次真实时刻（ISO），按 path upsert 语义落盘
      expect(readLogFromDisk(changeDir)).toEqual([
        expect.objectContaining({ op: 'write', scope: 'workflow', path: 'src/foo.ts' }),
      ]);
      expect(readLogFromDisk(changeDir)[0].at).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    } finally {
      project.cleanup();
    }
  });

  it('workflow scope 已有同 path 记录再次 append → upsert 原位覆盖；phase 审计记录原样保留 (AC-8)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const phaseEntry = logEntry('write', 'src/foo.ts', 'implement', 1);
      writeWorkflowJson(changeDir, validWorkflowDoc([phaseEntry, logEntry('write', 'src/foo.ts')]));

      const result = appendWorkflowFiles(changeDir, { deleted: ['src/foo.ts'] });

      const log = readLogFromDisk(changeDir);
      expect(log).toHaveLength(2);
      expect(log[0]).toEqual(phaseEntry);
      expect(log[1].op).toBe('delete');
      expect(log[1].scope).toBe('workflow');
      expect(result).toEqual({ written: [], deleted: ['src/foo.ts'] });
    } finally {
      project.cleanup();
    }
  });

  it('append deleted → 派生净状态中该路径移出 written 进入 deleted（追加后条胜）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([logEntry('write', 'src/foo.ts')]));

      const result = appendWorkflowFiles(changeDir, { deleted: ['src/foo.ts'] });

      expect(result).toEqual({ written: [], deleted: ['src/foo.ts'] });
    } finally {
      project.cleanup();
    }
  });

  it('同 path 同批出现在 written 与 deleted → deleted 胜（追加顺序在后，同 key 覆盖）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const result = appendWorkflowFiles(changeDir, {
        written: ['src/a.ts', 'src/n.ts'],
        deleted: ['src/a.ts'],
      });

      expect(result.written).toEqual(['src/n.ts']);
      expect(result.deleted).toEqual(['src/a.ts']);
      const log = readLogFromDisk(changeDir);
      expect(log.filter((e) => e.path === 'src/a.ts')).toHaveLength(1);
      expect(log.find((e) => e.path === 'src/a.ts')?.op).toBe('delete');
    } finally {
      project.cleanup();
    }
  });

  it('ignored 路径（项目根 .gitignore 命中）→ 不过滤照常入 log（人工通道契约，本文件不做 gitignore 过滤）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      const result = appendWorkflowFiles(changeDir, { written: ['.claude/memory.md'] });

      expect(result).toEqual({ written: ['.claude/memory.md'], deleted: [] });
      expect(readLogFromDisk(changeDir)).toEqual([
        expect.objectContaining({ op: 'write', scope: 'workflow', path: '.claude/memory.md' }),
      ]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// setWorkflowFiles — set 读改写（删涉及记录 + 末尾追加）
// ===========================================================================

describe('setWorkflowFiles — set 读改写', () => {
  it('删除涉及 provided path 的全部记录（任意 scope/attempt/op），未涉及 path 的记录（含 phase 审计）逐字保留 (AC-8)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const untouchedPhase = logEntry('write', 'src/keep.ts', 'implement', 3);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([
          logEntry('write', 'src/a.ts', 'implement', 1),
          logEntry('delete', 'src/a.ts', 'implement', 2),
          logEntry('delete', 'src/a.ts'),
          untouchedPhase,
        ]),
      );

      const result = setWorkflowFiles(changeDir, { written: ['src/a.ts'] });

      const log = readLogFromDisk(changeDir);
      expect(log).toHaveLength(2);
      expect(log[0]).toEqual(untouchedPhase);
      expect(log[1]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/a.ts' });
      // 派生净状态包含未涉及 path 的既有记录（src/keep.ts 的 phase 审计条目）
      expect(result).toEqual({ written: ['src/keep.ts', 'src/a.ts'], deleted: [] });
    } finally {
      project.cleanup();
    }
  });

  it('written 与 deleted 同批提供不同 path → 未涉及 path 的既有记录保留在前，追加顺序 written 先 deleted 后', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([logEntry('write', 'src/a.ts'), logEntry('write', 'src/b.ts')]),
      );

      const result = setWorkflowFiles(changeDir, { written: ['src/n.ts'], deleted: ['src/m.ts'] });

      expect(result).toEqual({
        written: ['src/a.ts', 'src/b.ts', 'src/n.ts'],
        deleted: ['src/m.ts'],
      });
      const log = readLogFromDisk(changeDir);
      expect(log.map((e) => `${e.scope}:${e.op}:${e.path}`)).toEqual([
        'workflow:write:src/a.ts',
        'workflow:write:src/b.ts',
        'workflow:write:src/n.ts',
        'workflow:delete:src/m.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('同批 written+deleted 提供同一路径 → 该 path 净状态为 deleted', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const result = setWorkflowFiles(changeDir, { written: ['src/a.ts'], deleted: ['src/a.ts'] });

      expect(result).toEqual({ written: [], deleted: ['src/a.ts'] });
    } finally {
      project.cleanup();
    }
  });

  it('changeDir 无 workflow.json / 文件非法 / 缺 file_log → 抛错且磁盘内容不变（复用 updateLog 读前置）', () => {
    const project = createTempProject();
    try {
      // 无 workflow.json
      const missingDir = path.join(project.root, 'openspec', 'changes', 'missing');
      expect(() => setWorkflowFiles(missingDir, { written: ['src/a.ts'] })).toThrow(
        /workflow\.json 不存在/,
      );
      expect(fs.existsSync(missingDir)).toBe(false);

      // 文件非法（截断 JSON）
      const changeDir = createChangeDir(project);
      const broken = '{"workflow_type": "requirement",';
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), broken, 'utf-8');
      expect(() => setWorkflowFiles(changeDir, { written: ['src/a.ts'] })).toThrow(/解析失败/);
      expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(broken);

      // 缺 file_log（机制前旧 change）
      fs.writeFileSync(
        path.join(changeDir, 'workflow.json'),
        JSON.stringify({ workflow_type: 'requirement', created: '2026-09-18' }),
        'utf-8',
      );
      const before = fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');
      expect(() => setWorkflowFiles(changeDir, { written: ['src/a.ts'] })).toThrow(
        /该 change 创建于文件清单机制之前，请重建/,
      );
      expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(before);
    } finally {
      project.cleanup();
    }
  });
});
