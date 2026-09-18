/**
 * 单元测试: modules/workflow/files/record.ts — PostToolUse 归账管线
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-6/AC-4（管线层）: 记录上下文由 scope 决定——phase scope（phase id + attempt）
 *   与 workflow scope（'workflow' 命名空间、无 attempt 字段）盖章落盘
 * - 规范化回归（迁移保留）: 绝对路径 / Windows 反斜杠 → 项目根相对 POSIX 化、越界丢弃
 * - 自污染排除与 gitignore 过滤回归（迁移保留）: openspec/** 与 workflow.json 不入 log、
 *   ignored 路径 write/delete/revert 三态统一过滤、不回溯清洗既有条目、
 *   无 .gitignore 时无 stderr 副作用、fail-open 诊断
 * - revert 事件照常过滤并作为 revert 记录入 log（AC-6）
 * - 空批次 / 全越界 → 不读不写 workflow.json（磁盘逐字节不变）
 * - legacy change（缺 file_log）→ 抛错向上传播，文件不被半写
 * - 落盘纪律: workflow_type / created / eval / active_phase / interrupted / 未知键保留、
 *   2 空格缩进 + 尾换行
 *
 * 文件系统不 mock：mkdtempSync 临时项目 + 真盘读写（同 file-inventory.test.ts 模式）。
 * Mock 面：vi.setSystemTime 固定 at 断言、process.stderr.write spy 捕获诊断。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterAll, beforeAll, describe, expect, it, vi } from 'vite-plus/test';

import { type FileLogEntry, type FileOp } from './file-inventory';
import { recordFileOps } from './record';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'record-test-'): TempProject {
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

/** 构造一条能通过 phaseLogSchema 校验的 eval 条目（workflowFileSchema 要求）。 */
function evalEntry(phase: string): Record<string, unknown> {
  return {
    phase,
    verdict: 'pass',
    attempt: 1,
    timestamp: '2026-09-18T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: true, evidence: 'e' }],
    backtrack_to: null,
  };
}

/** 合法 workflow.json 文档（含 eval / 未知键 / file_log；运行态字段可注入）。 */
function validWorkflowDoc(
  fileLog: unknown,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-18',
    eval: [evalEntry('proposal')],
    unknown_key: { keep: true },
    ...extra,
    file_log: fileLog,
  };
}

/** 捕获 process.stderr.write 的输出行（诊断断言用）。 */
function captureStderrWrite(): { lines: () => string[]; restore: () => void } {
  const spy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
  return {
    lines: () => spy.mock.calls.map((c) => String(c[0])),
    restore: () => spy.mockRestore(),
  };
}

// 系统时钟固定（条目 at 字段断言）
const FIXED_NOW = '2026-09-18T08:30:00.000Z';

beforeAll(() => {
  vi.setSystemTime(new Date(FIXED_NOW));
});

afterAll(() => {
  vi.useRealTimers();
});

// ===========================================================================
// recordFileOps — scope 盖章与管线正序
// ===========================================================================

describe('recordFileOps — scope 盖章 (AC-6/AC-4 管线层)', () => {
  it('scope={kind:"phase", phase:"implement", attempt:2} → 落盘条目 scope="implement"、attempt=2、at 为固定 ISO 时刻', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
        projectRoot: project.root,
        scope: { kind: 'phase', phase: 'implement', attempt: 2 },
      });

      expect(readLogFromDisk(changeDir)).toEqual([
        { op: 'write', scope: 'implement', attempt: 2, path: 'src/a.ts', at: FIXED_NOW },
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('scope={kind:"workflow"} → 落盘条目 scope="workflow" 且无 attempt 字段（主 agent 分支的记录形态）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      recordFileOps(changeDir, [{ op: 'write', path: 'src/b.ts' }], {
        projectRoot: project.root,
        scope: { kind: 'workflow' },
      });

      const log = readLogFromDisk(changeDir);
      expect(log).toHaveLength(1);
      expect(log[0].scope).toBe('workflow');
      expect(Object.prototype.hasOwnProperty.call(log[0], 'attempt')).toBe(false);
      expect(log[0].at).toBe(FIXED_NOW);
    } finally {
      project.cleanup();
    }
  });

  it('op:"revert" 事件照常过滤并作为 revert 记录入 log（与 gitignore 过滤叠加：ignored 的 revert 被丢弃）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      recordFileOps(
        changeDir,
        [
          { op: 'revert', path: 'src/c.ts' },
          { op: 'revert', path: '.claude/ignored.md' },
        ],
        { projectRoot: project.root, scope: { kind: 'workflow' } },
      );

      expect(readLogFromDisk(changeDir)).toEqual([
        { op: 'revert', scope: 'workflow', path: 'src/c.ts', at: FIXED_NOW },
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('规范化回归：绝对路径（含 Windows 反斜杠形态）→ 项目根相对 POSIX 化后入 log（迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const ops: FileOp[] = [
        { op: 'write', path: path.join(project.root, 'src', 'deep', 'a.ts') },
        { op: 'write', path: 'src\\backslash.ts' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root, scope: { kind: 'workflow' } });

      expect(readLogFromDisk(changeDir).map((e) => e.path)).toEqual([
        'src/deep/a.ts',
        'src/backslash.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('全部路径越界（../outside.ts）→ 无候选路径，不读不写 workflow.json（磁盘逐字节不变，迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      const before = readWorkflowJsonRaw(changeDir);

      recordFileOps(changeDir, [{ op: 'write', path: '../outside.ts' }], {
        projectRoot: project.root,
        scope: { kind: 'workflow' },
      });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('ops=[] → 不读不写 workflow.json（空批次提前返回，磁盘逐字节不变，迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      const before = readWorkflowJsonRaw(changeDir);

      recordFileOps(changeDir, [], { projectRoot: project.root, scope: { kind: 'workflow' } });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// recordFileOps — 自污染排除与 gitignore 过滤叠加（迁移保留）
// ===========================================================================

describe('recordFileOps — 自污染排除与 gitignore 过滤叠加', () => {
  it('openspec/** 与 workflow.json（含嵌套形态）路径被排除、不入 log 不触发读取（迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      const before = readWorkflowJsonRaw(changeDir);

      const ops: FileOp[] = [
        { op: 'write', path: 'openspec' },
        { op: 'write', path: 'openspec/changes/my-change/proposal.md' },
        { op: 'write', path: 'openspec/config.json' },
        { op: 'write', path: 'workflow.json' },
        { op: 'write', path: path.join(changeDir, 'workflow.json') },
        { op: 'write', path: 'sub/dir/workflow.json' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root, scope: { kind: 'workflow' } });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('项目根 .gitignore 含 .claude/ 时，ignored 路径 write / delete / revert 三态统一过滤丢弃（迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      const ops: FileOp[] = [
        { op: 'write', path: '.claude/x.md' },
        { op: 'delete', path: '.claude/y.md' },
        { op: 'revert', path: '.claude/z.md' },
        { op: 'write', path: 'src/a.ts' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root, scope: { kind: 'workflow' } });

      const log = readLogFromDisk(changeDir);
      expect(log.map((e) => e.path)).toEqual(['src/a.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('ignored 路径（.claude/）与 openspec/（自污染排除）同批出现 → 均不入 log（两类过滤叠加，磁盘逐字节不变）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');
      const before = readWorkflowJsonRaw(changeDir);

      const ops: FileOp[] = [
        { op: 'write', path: '.claude/x.md' },
        { op: 'delete', path: 'openspec/changes/my-change/proposal.md' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root, scope: { kind: 'workflow' } });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('不回溯清洗既有条目：过滤只作用于本次增量 op，log 中既有 ignored 条目原样保留（迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([
          { op: 'write', scope: 'workflow', path: '.claude/old.md', at: FIXED_NOW },
        ]),
      );
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      recordFileOps(changeDir, [{ op: 'write', path: 'src/new.ts' }], {
        projectRoot: project.root,
        scope: { kind: 'workflow' },
      });

      expect(readLogFromDisk(changeDir).map((e) => e.path)).toEqual([
        '.claude/old.md',
        'src/new.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('项目根及祖先链全无 .gitignore → 候选路径全量入 log，过滤器惰性构建不产生副作用（无 stderr 诊断，迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const stderr = captureStderrWrite();
      try {
        const ops: FileOp[] = [
          { op: 'write', path: 'src/a.ts' },
          { op: 'delete', path: 'src/deep/b.ts' },
        ];
        recordFileOps(changeDir, ops, { projectRoot: project.root, scope: { kind: 'workflow' } });
      } finally {
        const lines = stderr.lines();
        stderr.restore();
        expect(lines.some((l) => l.includes('record-files:'))).toBe(false);
      }

      expect(readLogFromDisk(changeDir).map((e) => `${e.op}:${e.path}`)).toEqual([
        'write:src/a.ts',
        'delete:src/deep/b.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('.gitignore 读取失败（fail-open）→ 路径保留入 log，诊断经 process.stderr.write 输出且带 record-files: 前缀，不抛错（迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));
      // 根 .gitignore 为目录 → existsSync 命中、readFileSync 抛错（读取失败形态）
      fs.mkdirSync(path.join(project.root, '.gitignore'), { recursive: true });

      const stderr = captureStderrWrite();
      let lines: string[] = [];
      try {
        expect(() =>
          recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
            projectRoot: project.root,
            scope: { kind: 'workflow' },
          }),
        ).not.toThrow();
      } finally {
        lines = stderr.lines();
        stderr.restore();
      }

      expect(readLogFromDisk(changeDir).map((e) => e.path)).toEqual(['src/a.ts']);
      expect(lines.some((l) => l.includes('record-files:') && l.includes('.gitignore'))).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// recordFileOps — 持久化与异常
// ===========================================================================

describe('recordFileOps — 持久化与异常', () => {
  it('落盘后 workflow_type / created / eval / active_phase / interrupted / 未知键保留、2 空格缩进 + 尾换行（doc-io 纪律）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([], {
          active_phase: { phase: 'implement', attempt: 2, start_at: FIXED_NOW },
          interrupted: [{ phase: 'implement', attempt: 1, start_at: FIXED_NOW, end_at: FIXED_NOW }],
        }),
      );

      recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
        projectRoot: project.root,
        scope: { kind: 'workflow' },
      });

      const raw = readWorkflowJsonRaw(changeDir);
      const doc = JSON.parse(raw) as Record<string, unknown>;
      expect(doc.workflow_type).toBe('requirement');
      expect(doc.created).toBe('2026-09-18');
      expect(doc.eval).toEqual([evalEntry('proposal')]);
      expect(doc.unknown_key).toEqual({ keep: true });
      expect(doc.active_phase).toEqual({ phase: 'implement', attempt: 2, start_at: FIXED_NOW });
      expect(doc.interrupted).toEqual([
        { phase: 'implement', attempt: 1, start_at: FIXED_NOW, end_at: FIXED_NOW },
      ]);
      expect(raw.endsWith('\n')).toBe(true);
      expect(raw).toContain('\n  "workflow_type"');
      expect(raw).toContain('\n      "path": "src/a.ts"');
    } finally {
      project.cleanup();
    }
  });

  it('legacy change（workflow.json 缺 file_log）→ 抛错向上传播（文案为重建 change 指引），文件不被半写', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const legacy = { workflow_type: 'requirement', created: '2026-09-18' };
      writeWorkflowJson(changeDir, legacy);
      const before = readWorkflowJsonRaw(changeDir);

      expect(() =>
        recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
          projectRoot: project.root,
          scope: { kind: 'workflow' },
        }),
      ).toThrow(/该 change 创建于文件清单机制之前，请重建/);
      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });
});
