/**
 * Tests for change_list filtering — archive registration and acceptance completion.
 *
 * Covers AC-1~AC-5 from openspec/changes/fix-change-list-archived-filter/test-design.md
 *
 * @see openspec/changes/fix-change-list-archived-filter/test-design.md
 * @see openspec/changes/fix-change-list-archived-filter/design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import { type EvalEntry, writeEvalJson } from '../lib/eval-json';
import { runChangeList } from './change-list';

// ---------------------------------------------------------------------------
// Fixture helpers — mkdtemp project root with openspec/changes/ structure
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  changesDir: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'change-list-test-'));
  const changesDir = path.join(root, 'openspec', 'changes');
  fs.mkdirSync(changesDir, { recursive: true });
  return {
    root,
    changesDir,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeDir(dirPath: string): void {
  fs.mkdirSync(dirPath, { recursive: true });
}

function writeFile(filePath: string, content: string): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf-8');
}

function makeEvalEntry(
  overrides: Partial<EvalEntry> & Pick<EvalEntry, 'phase' | 'verdict'>,
): EvalEntry {
  return {
    attempt: 1,
    timestamp: '2026-06-18T12:00:00.000Z',
    report: 'test report',
    checklist: [{ item: '检查项', pass: true, evidence: 'ok' }],
    backtrack_to: null,
    ...overrides,
  };
}

function writeChange(
  changesDir: string,
  name: string,
  options?: {
    artifacts?: Record<string, string>;
    tasksMd?: string;
    evalEntries?: EvalEntry[];
    invalidEvalJson?: string;
  },
): string {
  const changeDir = path.join(changesDir, name);
  writeDir(changeDir);

  if (options?.artifacts) {
    for (const [file, content] of Object.entries(options.artifacts)) {
      writeFile(path.join(changeDir, file), content);
    }
  }

  if (options?.tasksMd !== undefined) {
    writeFile(path.join(changeDir, 'tasks.md'), options.tasksMd);
  }

  if (options?.evalEntries !== undefined) {
    writeEvalJson(changeDir, options.evalEntries);
  }

  if (options?.invalidEvalJson !== undefined) {
    writeFile(path.join(changeDir, 'eval.json'), options.invalidEvalJson);
  }

  return changeDir;
}

function writeArchiveDir(changesDir: string, archiveDirName: string): string {
  const archiveRoot = path.join(changesDir, 'archive');
  const dir = path.join(archiveRoot, archiveDirName);
  writeDir(dir);
  return dir;
}

function changeNames(result: ReturnType<typeof runChangeList>): string[] {
  return result.changes.map((c) => c.name);
}

// ===========================================================================
// runChangeList -- 活跃 change 聚合 (AC-3)
// ===========================================================================

describe('runChangeList -- 活跃 change 聚合', () => {
  it('进行中 change 应出现在结果中且 artifacts、tasks、latest_phase 字段正确 (AC-3)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'active-change', {
        artifacts: {
          'proposal.md': '# proposal',
          'design.md': '# design',
        },
        tasksMd: '- [ ] task one\n- [x] task two\n',
        evalEntries: [
          makeEvalEntry({
            phase: 'dev-design',
            verdict: 'pass',
            timestamp: '2026-06-18T09:00:00.000Z',
          }),
        ],
      });

      const result = runChangeList({ project_root: project.root });

      expect(result.changes).toHaveLength(1);
      const entry = result.changes[0];
      expect(entry.name).toBe('active-change');
      expect(entry.artifacts).toContain('proposal.md');
      expect(entry.artifacts).toContain('design.md');
      expect(entry.tasks).toEqual({ total: 2, done: 1 });
      expect(entry.latest_phase).toEqual({
        phase: 'dev-design',
        verdict: 'pass',
      });
    } finally {
      project.cleanup();
    }
  });

  it('tasks.md 含 - [ ] / - [x] 时 tasks.total / tasks.done 计数应正确', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'task-count', {
        tasksMd: '- [ ] a\n- [x] b\n- [X] c\n  - [ ] indented ignored\n',
      });

      const result = runChangeList({ project_root: project.root });
      const entry = result.changes.find((c) => c.name === 'task-count');

      expect(entry?.tasks).toEqual({ total: 4, done: 2 });
    } finally {
      project.cleanup();
    }
  });

  it('latest_phase 应取全部 eval 条目（不限 phase）按 timestamp 降序最新一条', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'latest-phase', {
        evalEntries: [
          makeEvalEntry({
            phase: 'proposal',
            verdict: 'pass',
            timestamp: '2026-06-17T10:00:00.000Z',
          }),
          makeEvalEntry({
            phase: 'unit-test',
            verdict: 'fail',
            timestamp: '2026-06-18T11:00:00.000Z',
          }),
          makeEvalEntry({
            phase: 'acceptance',
            verdict: 'fail',
            timestamp: '2026-06-18T10:00:00.000Z',
          }),
        ],
      });

      const result = runChangeList({ project_root: project.root });
      const entry = result.changes.find((c) => c.name === 'latest-phase');

      expect(entry?.latest_phase).toEqual({
        phase: 'unit-test',
        verdict: 'fail',
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeList -- 缺失 eval / 无 acceptance 条目 (AC-4)
// ===========================================================================

describe('runChangeList -- 缺失 eval / 无 acceptance 条目', () => {
  it('无 eval.json 的 change 仍应出现在结果中 (AC-4)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-eval-active');

      const result = runChangeList({ project_root: project.root });

      expect(changeNames(result)).toContain('no-eval-active');
    } finally {
      project.cleanup();
    }
  });

  it('有 eval.json 但无 acceptance 条目的 change 仍应出现在结果中 (AC-4)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-acceptance', {
        evalEntries: [
          makeEvalEntry({
            phase: 'implement',
            verdict: 'pass',
            timestamp: '2026-06-18T10:00:00.000Z',
          }),
        ],
      });

      const result = runChangeList({ project_root: project.root });

      expect(changeNames(result)).toContain('no-acceptance');
    } finally {
      project.cleanup();
    }
  });

  it('eval.json 解析失败时 change 仍视为活跃且 latest_phase 为 null', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'bad-eval-active', {
        invalidEvalJson: 'not-json',
      });

      const result = runChangeList({ project_root: project.root });
      const entry = result.changes.find((c) => c.name === 'bad-eval-active');

      expect(entry).toBeDefined();
      expect(entry?.latest_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeList -- 目录扫描与排序
// ===========================================================================

describe('runChangeList -- 目录扫描与排序', () => {
  it('openspec/changes/ 不存在时应返回 { changes: [], count: 0 }', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'change-list-no-changes-'));
    try {
      const result = runChangeList({ project_root: root });

      expect(result.changes).toEqual([]);
      expect(result.count).toBe(0);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  it('无活跃 change 时应返回 { changes: [], count: 0 }', () => {
    const project = createTempProject();
    try {
      writeArchiveDir(project.changesDir, '2026-06-18-only-archived');

      const result = runChangeList({ project_root: project.root });

      expect(result.changes).toEqual([]);
      expect(result.count).toBe(0);
    } finally {
      project.cleanup();
    }
  });

  it('永不将 name === "archive" 的目录列入 changes', () => {
    const project = createTempProject();
    try {
      writeDir(path.join(project.changesDir, 'archive'));
      writeChange(project.changesDir, 'real-change');

      const result = runChangeList({ project_root: project.root });

      expect(changeNames(result)).not.toContain('archive');
      expect(changeNames(result)).toContain('real-change');
    } finally {
      project.cleanup();
    }
  });

  it('多个活跃 change 应按 name 字典序排序', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'zebra');
      writeChange(project.changesDir, 'alpha');
      writeChange(project.changesDir, 'middle');

      const result = runChangeList({ project_root: project.root });

      expect(changeNames(result)).toEqual(['alpha', 'middle', 'zebra']);
    } finally {
      project.cleanup();
    }
  });

  it('openspec/changes/ 下非目录条目（文件）应被跳过', () => {
    const project = createTempProject();
    try {
      writeFile(path.join(project.changesDir, 'README.md'), '# changes');
      writeChange(project.changesDir, 'valid-dir');

      const result = runChangeList({ project_root: project.root });

      expect(changeNames(result)).toEqual(['valid-dir']);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeList -- 组合过滤
// ===========================================================================

describe('runChangeList -- 组合过滤', () => {
  it('除archive 已登记外，其他都返回', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'active-only');
      writeArchiveDir(project.changesDir, '2026-06-18-archived-same-name');
      writeChange(project.changesDir, 'archived-same-name');
      writeChange(project.changesDir, 'passed-filtered', {
        evalEntries: [
          makeEvalEntry({
            phase: 'acceptance',
            verdict: 'pass',
            timestamp: '2026-06-18T12:00:00.000Z',
          }),
        ],
      });
      writeChange(project.changesDir, 'fail-acceptance-active', {
        evalEntries: [
          makeEvalEntry({
            phase: 'acceptance',
            verdict: 'fail',
            timestamp: '2026-06-18T12:00:00.000Z',
          }),
        ],
      });

      const result = runChangeList({ project_root: project.root });

      expect(changeNames(result)).toContain('active-only');
      expect(changeNames(result)).toContain('archived-same-name');
      expect(changeNames(result)).toContain('fail-acceptance-active');
      expect(changeNames(result)).toContain('passed-filtered');
      expect(result.count).toBe(4);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeList -- project_root
// ===========================================================================

describe('runChangeList -- project_root', () => {
  it('显式传入 project_root 指向 fixture 根目录时应正确解析路径', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'rooted-change');

      const result = runChangeList({ project_root: project.root });

      expect(result.project_root).toBe(project.root);
      expect(changeNames(result)).toContain('rooted-change');
    } finally {
      project.cleanup();
    }
  });

  it('project_root: null 时行为应与省略 project_root 一致', () => {
    const withNull = runChangeList({ project_root: null });
    const omitted = runChangeList({});

    expect(withNull.project_root).toBe(omitted.project_root);
    expect(withNull.changes).toEqual(omitted.changes);
    expect(withNull.count).toBe(omitted.count);
  });
});
