/**
 * 单元测试: modules/workflow/files/record.ts — PostToolUse 归账管线
 *
 * 覆盖范围（openspec/changes/move-files-write-into-workflow-module/test-design.md）:
 * - AC-2（管线层）: gitignore 过滤接入（命中路径不入桶、write/delete/revert 统一生效、
 *   不回溯清洗存量、全无 .gitignore 时行为与现状一致）
 * - AC-4（迁移语义回归）: 规范化（项目根相对化、越界丢弃、POSIX 化）、自污染排除
 *   （openspec/** 与 workflow.json）、agentType 盖章 / last-writer-wins / 清除、
 *   持久化保留键纪律、legacy 报错向上传播、fail-open 不阻塞
 *
 * 文件系统不 mock：mkdtempSync 临时项目 + validWorkflowDoc fixture（真盘读写，
 * 同 file-inventory.test.ts 模式）。process.stderr.write 经 vi.spyOn 捕获诊断。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it, vi } from 'vite-plus/test';

import { type FileOp } from './file-inventory';
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

/** 读回 workflow.json 的 files 净状态（真盘观察点）。 */
function readRecordedFiles(changeDir: string): {
  written: string[];
  deleted: string[];
  source?: Record<string, string>;
} {
  const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as {
    files?: { written: string[]; deleted: string[]; source?: Record<string, string> };
  };
  if (!doc.files) throw new Error('workflow.json 无 files 字段');
  return doc.files;
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

/** 合法 workflow.json 文档（含 eval / 未知键 / files）。 */
function validWorkflowDoc(
  files: unknown,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-18',
    eval: [evalEntry('proposal')],
    unknown_key: { keep: true },
    ...extra,
    files,
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

// ===========================================================================
// recordFileOps — 管线正序
// ===========================================================================

describe('recordFileOps — 管线正序', () => {
  it('ops 含绝对路径（含 Windows 反斜杠形态）→ 相对项目根 POSIX 化后入桶（normalizeRecordedPath 迁移语义保持）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      const ops: FileOp[] = [
        { op: 'write', path: path.join(project.root, 'src', 'deep', 'a.ts') },
        { op: 'write', path: 'src\\backslash.ts' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root });

      expect(readRecordedFiles(changeDir).written).toEqual(['src/deep/a.ts', 'src/backslash.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('op 携带 agentType / context 携带 agentType → source[path]=agent_type 盖章，沿用 fold 的 last-writer-wins 与无 agentType 清除语义', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      // op 携带 agentType → 盖章
      recordFileOps(
        changeDir,
        [{ op: 'write', path: 'src/a.ts', agentType: 'dev-team:implementation-generator' }],
        { projectRoot: project.root },
      );
      expect(readRecordedFiles(changeDir).source).toEqual({
        'src/a.ts': 'dev-team:implementation-generator',
      });

      // 主会话重写（op 与 context 均无 agentType）→ source 清除
      recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
        projectRoot: project.root,
      });
      const afterClear = readRecordedFiles(changeDir);
      expect(afterClear.source).toBeUndefined();
      expect(Object.prototype.hasOwnProperty.call(afterClear, 'source')).toBe(false);

      // context.agentType 兜底（op 未携带）→ 以 context 值盖章（last-writer-wins）
      recordFileOps(changeDir, [{ op: 'write', path: 'src/b.ts' }], {
        projectRoot: project.root,
        agentType: 'dev-team:context-agent',
      });
      expect(readRecordedFiles(changeDir).source).toEqual({
        'src/b.ts': 'dev-team:context-agent',
      });
    } finally {
      project.cleanup();
    }
  });

  it('write / delete / revert 三类 op 各按折叠规则入桶并落盘', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({ written: ['src/b.ts', 'src/c.ts'], deleted: ['src/d.ts'] }),
      );

      const ops: FileOp[] = [
        { op: 'write', path: 'src/a.ts' },
        { op: 'delete', path: 'src/b.ts' },
        { op: 'revert', path: 'src/c.ts' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root });

      expect(readRecordedFiles(changeDir)).toEqual({
        written: ['src/a.ts'],
        deleted: ['src/d.ts', 'src/b.ts'],
      });
    } finally {
      project.cleanup();
    }
  });

  it('ops=[] → 不读不写 workflow.json（空批次提前返回，磁盘逐字节不变）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: ['src/keep.ts'], deleted: [] }));
      const before = readWorkflowJsonRaw(changeDir);

      recordFileOps(changeDir, [], { projectRoot: project.root });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('全部路径越界（../outside.ts）→ 无候选路径，不读不写清单（磁盘逐字节不变）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));
      const before = readWorkflowJsonRaw(changeDir);

      recordFileOps(changeDir, [{ op: 'write', path: '../outside.ts' }], {
        projectRoot: project.root,
      });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// recordFileOps — 自污染排除与 gitignore 过滤叠加
// ===========================================================================

describe('recordFileOps — 自污染排除与 gitignore 过滤叠加', () => {
  it('openspec/** 与 workflow.json（含嵌套形态）路径被排除、不入桶不触发读取（迁移语义保持）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));
      const before = readWorkflowJsonRaw(changeDir);

      const ops: FileOp[] = [
        { op: 'write', path: 'openspec' },
        { op: 'write', path: 'openspec/changes/my-change/proposal.md' },
        { op: 'write', path: 'openspec/config.json' },
        { op: 'write', path: 'workflow.json' },
        { op: 'write', path: path.join(changeDir, 'workflow.json') },
        { op: 'write', path: 'sub/dir/workflow.json' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('项目根 .gitignore 含 .claude/ 时，op .claude/x.md 被过滤丢弃、同批 src/a.ts 照常入桶 (AC-2 管线层)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      const ops: FileOp[] = [
        { op: 'write', path: '.claude/x.md' },
        { op: 'write', path: 'src/a.ts' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root });

      const files = readRecordedFiles(changeDir);
      expect(files.written).toEqual(['src/a.ts']);
      expect(files.written).not.toContain('.claude/x.md');
    } finally {
      project.cleanup();
    }
  });

  it('gitignore 过滤对 write / delete / revert 统一生效：被忽略路径的 delete op 不入 deleted 桶', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      const ops: FileOp[] = [
        { op: 'delete', path: '.claude/x.md' },
        { op: 'revert', path: '.claude/y.md' },
        { op: 'write', path: 'src/a.ts' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root });

      const files = readRecordedFiles(changeDir);
      expect(files.deleted).toEqual([]);
      expect(files.written).toEqual(['src/a.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('.claude/（gitignore 命中）与 openspec/（自污染排除）同批出现 → 均不入桶（两类过滤叠加，磁盘逐字节不变）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');
      const before = readWorkflowJsonRaw(changeDir);

      const ops: FileOp[] = [
        { op: 'write', path: '.claude/x.md' },
        { op: 'delete', path: 'openspec/changes/my-change/proposal.md' },
      ];
      recordFileOps(changeDir, ops, { projectRoot: project.root });

      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('净状态中已存在的历史 ignored 路径不被回溯清理：过滤只作用于本次增量 op，既有桶内条目原样保留', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({ written: ['.claude/old.md', 'src/keep.ts'], deleted: [] }),
      );
      fs.writeFileSync(path.join(project.root, '.gitignore'), '.claude/\n', 'utf-8');

      recordFileOps(changeDir, [{ op: 'write', path: 'src/new.ts' }], {
        projectRoot: project.root,
      });

      expect(readRecordedFiles(changeDir).written).toEqual([
        '.claude/old.md',
        'src/keep.ts',
        'src/new.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('项目根及祖先链全无 .gitignore → 行为与现状一致（候选路径全量入桶），过滤器惰性构建不产生副作用（无 stderr 诊断）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      const stderr = captureStderrWrite();
      try {
        const ops: FileOp[] = [
          { op: 'write', path: 'src/a.ts' },
          { op: 'delete', path: 'src/deep/b.ts' },
        ];
        recordFileOps(changeDir, ops, { projectRoot: project.root });
      } finally {
        const lines = stderr.lines();
        stderr.restore();
        expect(lines.some((l) => l.includes('record-files:'))).toBe(false);
      }

      expect(readRecordedFiles(changeDir)).toEqual({
        written: ['src/a.ts'],
        deleted: ['src/deep/b.ts'],
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// recordFileOps — 持久化与异常
// ===========================================================================

describe('recordFileOps — 持久化与异常', () => {
  it('落盘后 workflow_type / created / eval / 未知键保留、2 空格缩进 + 尾换行（经 writeFileInventory 纪律）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
        projectRoot: project.root,
      });

      const raw = readWorkflowJsonRaw(changeDir);
      const doc = JSON.parse(raw) as Record<string, unknown>;
      expect(doc.workflow_type).toBe('requirement');
      expect(doc.created).toBe('2026-09-18');
      expect(doc.eval).toEqual([evalEntry('proposal')]);
      expect(doc.unknown_key).toEqual({ keep: true });
      expect(doc.files).toEqual({ written: ['src/a.ts'], deleted: [] });
      expect(raw.endsWith('\n')).toBe(true);
      expect(raw).toContain('\n  "workflow_type"');
      expect(raw).toContain('\n      "src/a.ts"');
    } finally {
      project.cleanup();
    }
  });

  it('legacy change（workflow.json 无 files）→ 抛错向上传播（由命令层 catch-all 兜底），文件不被半写', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const legacy = { workflow_type: 'requirement', created: '2026-09-18' };
      writeWorkflowJson(changeDir, legacy);
      const before = readWorkflowJsonRaw(changeDir);

      expect(() =>
        recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
          projectRoot: project.root,
        }),
      ).toThrow(/该 change 创建于文件清单机制之前，请重建/);
      expect(readWorkflowJsonRaw(changeDir)).toBe(before);
    } finally {
      project.cleanup();
    }
  });

  it('.gitignore 读取失败（fail-open）→ 路径保留入清单，诊断经 process.stderr.write 输出且带 record-files: 前缀，不抛错', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));
      // 根 .gitignore 为目录 → existsSync 命中、readFileSync 抛错（读取失败形态）
      fs.mkdirSync(path.join(project.root, '.gitignore'), { recursive: true });

      const stderr = captureStderrWrite();
      let lines: string[] = [];
      try {
        expect(() =>
          recordFileOps(changeDir, [{ op: 'write', path: 'src/a.ts' }], {
            projectRoot: project.root,
          }),
        ).not.toThrow();
      } finally {
        lines = stderr.lines();
        stderr.restore();
      }

      expect(readRecordedFiles(changeDir).written).toEqual(['src/a.ts']);
      expect(lines.some((l) => l.includes('record-files:') && l.includes('.gitignore'))).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});
