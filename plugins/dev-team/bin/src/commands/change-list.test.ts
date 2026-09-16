/**
 * Tests for change_list filtering — archive registration and acceptance completion.
 *
 * Covers AC-1~AC-5 from openspec/changes/fix-change-list-archived-filter/test-design.md
 * and AC-7 from openspec/changes/mcp-project-root-lock/test-design.md
 *
 * @see openspec/changes/fix-change-list-archived-filter/test-design.md
 * @see openspec/changes/mcp-project-root-lock/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { type EvalEntry, writeEvalJson } from '../lib/eval-json';
import { getPhaseTable } from '../lib/workflow';
import { changeListInputSchema, changeListOutputSchema } from '../schemas';
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

/**
 * 布置一个 change 目录。
 *
 * 评估历史是 `workflow.json.eval` 的一部分：`evalEntries` 与 `workflowJson`
 * 同时给出时合并进**同一对象**，避免后写的 `workflow.json` 覆盖权威数组。
 * `workflowJson: null` 表示不创建该文件；字符串表示手写原文（非法形状用例）。
 * `invalidEvalJson` 仍写遗留 `eval.json`，用于回退路径的形状用例。
 */
function writeChange(
  changesDir: string,
  name: string,
  options?: {
    artifacts?: Record<string, string>;
    tasksMd?: string;
    evalEntries?: EvalEntry[];
    invalidEvalJson?: string;
    workflowJson?: string | Record<string, unknown> | null;
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

  const workflowPath = path.join(changeDir, 'workflow.json');
  const explicit = options?.workflowJson;
  if (explicit === undefined && options?.evalEntries !== undefined) {
    // 权威存储走真实写入器（写方与读方同源）
    writeFile(workflowPath, JSON.stringify({ workflow_type: 'requirement' }));
    writeEvalJson(changeDir, options.evalEntries);
  } else if (explicit === null) {
    // 显式要求不创建 workflow.json
  } else if (typeof explicit === 'string') {
    writeFile(workflowPath, explicit);
  } else {
    const doc: Record<string, unknown> = { ...(explicit ?? { workflow_type: 'requirement' }) };
    if (options?.evalEntries !== undefined) {
      doc.eval = options.evalEntries;
    }
    writeFile(workflowPath, JSON.stringify(doc));
  }

  if (options?.invalidEvalJson !== undefined) {
    writeFile(path.join(changeDir, 'eval.json'), options.invalidEvalJson);
  }

  return changeDir;
}

/**
 * Build an EvalEntry for every phase in the given phase table with a non-stale pass verdict.
 */
function makeAllPassEntries(workflowType: string): EvalEntry[] {
  return getPhaseTable(workflowType).map((phase) =>
    makeEvalEntry({ phase: phase.id, verdict: 'pass' }),
  );
}

/** Requirement workflow phase table ids (source of truth for phase table assertions). */
function requirementPhaseIds() {
  return getPhaseTable('requirement').map((p) => p.id);
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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);
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
            phase: 'test-execution',
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

      const result = runChangeList(project.root);
      const entry = result.changes.find((c) => c.name === 'latest-phase');

      expect(entry?.latest_phase).toEqual({
        phase: 'test-execution',
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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);
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
      const result = runChangeList(root);

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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);

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

      const result = runChangeList(project.root);

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
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('显式传入 project_root 指向 fixture 根时：result.project_root === 传入值（字节级相等），且可扫描到该根下 change (AC-7)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'rooted-change');

      const result = runChangeList(project.root);

      expect(result.project_root).toBe(project.root);
      expect(changeNames(result)).toContain('rooted-change');
    } finally {
      project.cleanup();
    }
  });

  it('显式 project_root 指向含活跃 change 的 fixture 时，列表结果含该 change；result.project_root !== process.cwd() (AC-7)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'fixture-active', {
        artifacts: { 'proposal.md': '# p' },
      });

      const result = runChangeList(project.root);

      expect(changeNames(result)).toContain('fixture-active');
      expect(result.project_root).toBe(project.root);
      expect(result.project_root).not.toBe(process.cwd());
    } finally {
      project.cleanup();
    }
  });

  it('显式 project_root 指向不存在路径时返回 { changes: [], count: 0 }，且 project_root 仍为传入值（不得改写成 cwd）(AC-7)', () => {
    const missing = path.join(os.tmpdir(), `change-list-missing-${Date.now()}`);

    const result = runChangeList(missing);

    expect(result.changes).toEqual([]);
    expect(result.count).toBe(0);
    expect(result.project_root).toBe(missing);
    expect(result.project_root).not.toBe(process.cwd());
  });

  it('显式超长 project_root（>1000 chars）不崩溃；result.project_root 仍为该超长字符串；count === 0', () => {
    const longMissing = path.join(os.tmpdir(), `${'z'.repeat(1001)}`);

    expect(() => runChangeList(longMissing)).not.toThrow();
    const result = runChangeList(longMissing);
    expect(result.changes).toEqual([]);
    expect(result.count).toBe(0);
    expect(result.project_root).toBe(longMissing);
  });

  it('显式含 emoji 的 project_root 不崩溃；result.project_root 严格等于传入值且 !== cwd', () => {
    const weird = path.join(os.tmpdir(), `change-list-emoji-🚀-${Date.now()}`);

    expect(() => runChangeList(weird)).not.toThrow();
    const result = runChangeList(weird);
    expect(result.project_root).toBe(weird);
    expect(result.project_root).not.toBe(process.cwd());
    expect(result.changes).toEqual([]);
  });

  it('显式根与 process.cwd() 不同时仍以传入 fixture 为准，且 count === changes.length', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'other-root-change');
      writeChange(project.changesDir, 'second-change');

      const result = runChangeList(project.root);

      expect(result.project_root).toBe(project.root);
      expect(result.project_root).not.toBe(process.cwd());
      expect(result.count).toBe(result.changes.length);
      expect(result.count).toBe(2);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeList — CLI vs MCP schema 契约
// ===========================================================================

describe('runChangeList — CLI vs MCP schema 契约', () => {
  it('changeListInputSchema 必填 project_root；CLI runChangeList(projectRoot) 仍可用（MCP/CLI 契约分离）(AC-2/AC-11)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'schema-contract');
      const shapeKeys = Object.keys(changeListInputSchema.shape);
      expect(shapeKeys).toContain('project_root');
      expect(changeListInputSchema.safeParse({}).success).toBe(false);
      expect(changeListInputSchema.safeParse({ project_root: project.root }).success).toBe(true);

      // CLI/command helper takes a positional root string — not MCP schema args
      const result = runChangeList(project.root);
      expect(result.project_root).toBe(project.root);
      expect(changeNames(result)).toContain('schema-contract');
    } finally {
      project.cleanup();
    }
  });

  it('changeListOutputSchema.safeParse(runChangeList(...)) 成功，且解析后的 project_root 等于命令使用的根 (AC-11)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'output-schema');
      const result = runChangeList(project.root);
      const parsed = changeListOutputSchema.safeParse(result);

      expect(parsed.success).toBe(true);
      if (parsed.success) {
        expect(parsed.data.project_root).toBe(project.root);
        expect(parsed.data.count).toBe(parsed.data.changes.length);
      }
    } finally {
      project.cleanup();
    }
  });

  it("changeListInputSchema.safeParse({ project_root: '/x' }) 成功且保留 project_root（MCP 必填字段，不得 strip）(AC-2)", () => {
    const parsed = changeListInputSchema.safeParse({ project_root: '/x' });
    expect(parsed.success).toBe(true);
    if (parsed.success) {
      expect(parsed.data).toHaveProperty('project_root', '/x');
    }
  });
});

// ===========================================================================
// runChangeList — workflow_done (AC-6)
// ===========================================================================

describe('runChangeList — workflow_done (AC-6)', () => {
  it('所有 phase 都有非 stale 的 pass 条目时，workflow_done 为 true (AC-6)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'complete-change', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: makeAllPassEntries('requirement'),
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'complete-change')!;
      expect(change.workflow_done).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('任意 phase 缺少 pass 条目时，workflow_done 为 false (AC-6)', () => {
    const project = createTempProject();
    try {
      const phases = requirementPhaseIds();
      const missing = phases.slice(0, phases.length - 1);
      writeChange(project.changesDir, 'incomplete-change', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: missing.map((phase) => makeEvalEntry({ phase, verdict: 'pass' })),
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'incomplete-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('无 eval.json 的 change，workflow_done 为 false (AC-6)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-eval-change', {
        workflowJson: { workflow_type: 'requirement' },
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'no-eval-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('空 eval.json（[]）时 workflow_done 为 false', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'empty-eval-change', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: [],
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'empty-eval-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 缺失且无任何评估来源时，workflow_done 为 false、latest_phase 为 null，change 仍列出（AC-13 容错面）', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-workflow-change', { workflowJson: null });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'no-workflow-change')!;
      expect(change).toBeDefined();
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 缺失但存在遗留 eval.json 时 workflow_done 为 false，latest_phase 经回退填出（AC-3 容错面）', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-workflow-legacy', {
        workflowJson: null,
        invalidEvalJson: JSON.stringify([
          makeEvalEntry({
            phase: 'proposal',
            verdict: 'pass',
            timestamp: '2026-06-18T09:00:00.000Z',
          }),
        ]),
      });

      const change = runChangeList(project.root).changes.find(
        (c) => c.name === 'no-workflow-legacy',
      )!;
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toEqual({ phase: 'proposal', verdict: 'pass' });
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 内容非法（非 JSON / 非对象 / workflow_type 非字符串 / 空字符串）时 workflow_done 为 false，不崩溃', () => {
    const project = createTempProject();
    try {
      const invalidWorkflows: Record<string, string | Record<string, unknown>> = {
        'bad-json': 'not json {',
        'bad-array': JSON.stringify(['x']),
        'bad-null': JSON.stringify(null),
        'bad-type': JSON.stringify({ workflow_type: 123 }),
        'bad-empty': JSON.stringify({ workflow_type: '' }),
        'bad-missing': JSON.stringify({ other: 'field' }),
      };
      for (const [name, workflowJson] of Object.entries(invalidWorkflows)) {
        writeChange(project.changesDir, name, {
          workflowJson,
          evalEntries: makeAllPassEntries('requirement'),
        });
      }

      const result = runChangeList(project.root);
      for (const name of Object.keys(invalidWorkflows)) {
        const change = result.changes.find((c) => c.name === name)!;
        expect(change.workflow_done).toBe(false);
      }
    } finally {
      project.cleanup();
    }
  });

  it('eval.json 格式非法时，workflow_done 为 false 而非崩溃', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'bad-eval-change', {
        workflowJson: { workflow_type: 'requirement' },
        invalidEvalJson: 'not json [',
      });

      expect(() => runChangeList(project.root)).not.toThrow();
      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'bad-eval-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('eval.json 根元素非数组（对象）时 workflow_done 为 false 而非崩溃', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'obj-eval-change', {
        workflowJson: { workflow_type: 'requirement' },
        invalidEvalJson: JSON.stringify({ phase: 'proposal', verdict: 'pass' }),
      });

      expect(() => runChangeList(project.root)).not.toThrow();
      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'obj-eval-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('所有 phase 的 pass 均为 stale 时，workflow_done 为 false', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'all-stale-change', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: getPhaseTable('requirement').map((phase) =>
          makeEvalEntry({ phase: phase.id, verdict: 'pass', stale: true }),
        ),
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'all-stale-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('仅 acceptance phase 有 pass 但其他 phase 缺失时，workflow_done 为 false', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'only-acceptance-change', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: [makeEvalEntry({ phase: 'acceptance', verdict: 'pass' })],
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'only-acceptance-change')!;
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('skipped 条目视为 pass（skipped: true 且非 stale）归入 workflow_done 计算', () => {
    const project = createTempProject();
    try {
      const phases = requirementPhaseIds();
      const entries = phases.map((phase, i) =>
        i === 0
          ? makeEvalEntry({ phase, verdict: 'pass', skipped: true })
          : makeEvalEntry({ phase, verdict: 'pass' }),
      );
      writeChange(project.changesDir, 'skipped-pass-change', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: entries,
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'skipped-pass-change')!;
      expect(change.workflow_done).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 为 bug-fix 时按 bug-fix phase table（6 阶段）计算 workflow_done', () => {
    const project = createTempProject();
    try {
      const bugFixPhases = getPhaseTable('bug-fix').map((p) => p.id);
      expect(bugFixPhases).toEqual([
        'proposal',
        'dev-design',
        'implement',
        'test-execution',
        'code-review',
        'acceptance',
      ]);

      // bug-fix 全 6 phase 通过 → true
      writeChange(project.changesDir, 'bugfix-complete', {
        workflowJson: { workflow_type: 'bug-fix' },
        evalEntries: bugFixPhases.map((phase) => makeEvalEntry({ phase, verdict: 'pass' })),
      });
      // bug-fix 缺 acceptance → false
      writeChange(project.changesDir, 'bugfix-incomplete', {
        workflowJson: { workflow_type: 'bug-fix' },
        evalEntries: bugFixPhases
          .filter((p) => p !== 'acceptance')
          .map((phase) => makeEvalEntry({ phase, verdict: 'pass' })),
      });

      const result = runChangeList(project.root);
      const complete = result.changes.find((c) => c.name === 'bugfix-complete')!;
      expect(complete.workflow_done).toBe(true);
      const incomplete = result.changes.find((c) => c.name === 'bugfix-incomplete')!;
      expect(incomplete.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 为 test-only 时按 test-only phase table（5 阶段）计算 workflow_done', () => {
    const project = createTempProject();
    try {
      const testOnlyPhases = getPhaseTable('test-only').map((p) => p.id);
      expect(testOnlyPhases).toEqual([
        'proposal',
        'code-analyze',
        'test-design',
        'test-gen',
        'test-execution',
      ]);

      writeChange(project.changesDir, 'testonly-complete', {
        workflowJson: { workflow_type: 'test-only' },
        evalEntries: testOnlyPhases.map((phase) => makeEvalEntry({ phase, verdict: 'pass' })),
      });
      writeChange(project.changesDir, 'testonly-incomplete', {
        workflowJson: { workflow_type: 'test-only' },
        evalEntries: testOnlyPhases
          .filter((p) => p !== 'test-execution')
          .map((phase) => makeEvalEntry({ phase, verdict: 'pass' })),
      });

      const result = runChangeList(project.root);
      const complete = result.changes.find((c) => c.name === 'testonly-complete')!;
      expect(complete.workflow_done).toBe(true);
      const incomplete = result.changes.find((c) => c.name === 'testonly-incomplete')!;
      expect(incomplete.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 为 refactor 时与 requirement 共用同一 phase table（8 阶段）计算 workflow_done', () => {
    const project = createTempProject();
    try {
      expect(getPhaseTable('refactor').map((p) => p.id)).toEqual(requirementPhaseIds());
      writeChange(project.changesDir, 'refactor-complete', {
        workflowJson: { workflow_type: 'refactor' },
        evalEntries: makeAllPassEntries('refactor'),
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'refactor-complete')!;
      expect(change.workflow_done).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 非 4 值枚举（mystery-flow）时 safeParse 失败 → workflow_done 为 false、latest_phase 为 null，change 仍列出', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'unknown-type-complete', {
        workflowJson: { workflow_type: 'mystery-flow' },
        evalEntries: makeAllPassEntries('requirement'),
      });
      writeChange(project.changesDir, 'unknown-type-incomplete', {
        workflowJson: { workflow_type: 'mystery-flow' },
        evalEntries: [makeEvalEntry({ phase: 'proposal', verdict: 'pass' })],
      });

      const result = runChangeList(project.root);
      for (const name of ['unknown-type-complete', 'unknown-type-incomplete']) {
        const change = result.changes.find((c) => c.name === name)!;
        expect(change).toBeDefined();
        expect(change.workflow_done).toBe(false);
      }
    } finally {
      project.cleanup();
    }
  });

  it('created 非 YYYY-MM-DD 时校验失败 → workflow_done 为 false，不崩溃（AC-14 容错面）', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'bad-created', {
        workflowJson: { workflow_type: 'requirement', created: '2026/09/11' },
        evalEntries: makeAllPassEntries('requirement'),
      });

      expect(() => runChangeList(project.root)).not.toThrow();
      const change = runChangeList(project.root).changes.find((c) => c.name === 'bad-created')!;
      expect(change).toBeDefined();
      expect(change.workflow_done).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json 无法解析（非法 JSON / 根非对象 / 根为 null）时 latest_phase 为 null 且不崩溃', () => {
    const project = createTempProject();
    try {
      const unreadable: Record<string, string> = {
        'unreadable-json': 'not json {',
        'unreadable-array': JSON.stringify(['x']),
        'unreadable-null': JSON.stringify(null),
      };
      for (const [name, raw] of Object.entries(unreadable)) {
        writeChange(project.changesDir, name, {
          workflowJson: raw,
          invalidEvalJson: JSON.stringify(makeAllPassEntries('requirement')),
        });
      }

      expect(() => runChangeList(project.root)).not.toThrow();
      const listed = runChangeList(project.root).changes;
      for (const name of Object.keys(unreadable)) {
        const change = listed.find((c) => c.name === name)!;
        expect(change).toBeDefined();
        expect(change.workflow_done).toBe(false);
        expect(change.latest_phase).toBeNull();
      }
    } finally {
      project.cleanup();
    }
  });

  it('workflow.json.eval 为对象 {} 时不崩溃，workflow_done 为 false、latest_phase 为 null（AC-12）', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'object-eval', {
        workflowJson: { workflow_type: 'requirement', eval: {} },
      });

      expect(() => runChangeList(project.root)).not.toThrow();
      const change = runChangeList(project.root).changes.find((c) => c.name === 'object-eval')!;
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('仅有 "eval": [] 时 workflow_done 为 false、latest_phase 为 null', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'empty-authoritative', {
        workflowJson: { workflow_type: 'requirement', eval: [] },
      });

      const change = runChangeList(project.root).changes.find(
        (c) => c.name === 'empty-authoritative',
      )!;
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('无 eval 键且无遗留文件时 workflow_done 为 false、latest_phase 为 null', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-eval-key', {
        workflowJson: { workflow_type: 'requirement' },
      });

      const change = runChangeList(project.root).changes.find((c) => c.name === 'no-eval-key')!;
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('created 缺失（Optional None）+ 未知键时正常计算 workflow_done', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'no-created', {
        workflowJson: { workflow_type: 'requirement', note: 'x' },
        evalEntries: makeAllPassEntries('requirement'),
      });

      const change = runChangeList(project.root).changes.find((c) => c.name === 'no-created')!;
      expect(change.workflow_done).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('遗留 eval.json 为对象时吞错（不崩溃），workflow_done 为 false', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'legacy-object-root', {
        workflowJson: { workflow_type: 'requirement' },
        invalidEvalJson: JSON.stringify({ phase: 'proposal', verdict: 'pass' }),
      });

      expect(() => runChangeList(project.root)).not.toThrow();
      const change = runChangeList(project.root).changes.find(
        (c) => c.name === 'legacy-object-root',
      )!;
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });
  it('workflow_done 字段类型严格为 boolean，changeListOutputSchema.safeParse 验证通过 (AC-6)', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'done-true', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: makeAllPassEntries('requirement'),
      });
      writeChange(project.changesDir, 'done-false', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: [makeEvalEntry({ phase: 'proposal', verdict: 'pass' })],
      });
      writeChange(project.changesDir, 'done-no-workflow', {
        workflowJson: null,
      });

      const result = runChangeList(project.root);
      const parsed = changeListOutputSchema.safeParse(result);
      expect(parsed.success).toBe(true);
      if (parsed.success) {
        for (const change of parsed.data.changes) {
          expect(typeof change.workflow_done).toBe('boolean');
        }
        const doneTrue = parsed.data.changes.find((c) => c.name === 'done-true')!;
        const doneFalse = parsed.data.changes.find((c) => c.name === 'done-false')!;
        const doneNoWorkflow = parsed.data.changes.find((c) => c.name === 'done-no-workflow')!;
        expect(doneTrue.workflow_done).toBe(true);
        expect(doneFalse.workflow_done).toBe(false);
        expect(doneNoWorkflow.workflow_done).toBe(false);
      }
    } finally {
      project.cleanup();
    }
  });

  it('workflow_done 为 false 时不影响 latest_phase 的 stale 标记（stale pass 仍反映为 stale: true）', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'stale-latest', {
        workflowJson: { workflow_type: 'requirement' },
        evalEntries: [
          makeEvalEntry({ phase: 'acceptance', verdict: 'pass', stale: true }),
          makeEvalEntry({ phase: 'proposal', verdict: 'pass' }),
        ],
      });

      const result = runChangeList(project.root);
      const change = result.changes.find((c) => c.name === 'stale-latest')!;
      expect(change.workflow_done).toBe(false);
      expect(change.latest_phase).toEqual({
        phase: 'acceptance',
        verdict: 'pass',
        stale: true,
      });
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runChangeList — artifacts / latest_phase 来源 (AC-7)
// ===========================================================================

describe('runChangeList — artifacts 与 latest_phase 来源 (AC-7)', () => {
  it('workflow.json.eval 非空且存在 proposal.md 时 artifacts 含 proposal.md、不含 eval.json，latest_phase 为 timestamp 最新条目', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'artifacts-change', {
        artifacts: { 'proposal.md': '# proposal', 'test-design.md': '# td' },
        evalEntries: [
          makeEvalEntry({
            phase: 'proposal',
            verdict: 'pass',
            timestamp: '2026-06-18T09:00:00.000Z',
          }),
          makeEvalEntry({
            phase: 'dev-design',
            verdict: 'fail',
            timestamp: '2026-06-18T11:00:00.000Z',
          }),
        ],
      });

      const entry = runChangeList(project.root).changes.find((c) => c.name === 'artifacts-change')!;

      expect(entry.artifacts).toContain('proposal.md');
      expect(entry.artifacts).toContain('test-design.md');
      expect(entry.artifacts).not.toContain('eval.json');
      expect(entry.artifacts).not.toContain('workflow.json');
      expect(entry.latest_phase).toEqual({ phase: 'dev-design', verdict: 'fail' });
    } finally {
      project.cleanup();
    }
  });

  it('目录仍有遗留 eval.json 文件时 artifacts 仍不含它；latest_phase 可经回退填出（AC-7）', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'legacy-artifact', {
        artifacts: { 'proposal.md': '# proposal' },
        workflowJson: { workflow_type: 'requirement' },
        invalidEvalJson: JSON.stringify([makeEvalEntry({ phase: 'proposal', verdict: 'pass' })]),
      });

      const entry = runChangeList(project.root).changes.find((c) => c.name === 'legacy-artifact')!;

      expect(entry.artifacts).not.toContain('eval.json');
      expect(entry.artifacts).toEqual(['proposal.md']);
      expect(entry.latest_phase).toEqual({ phase: 'proposal', verdict: 'pass' });
    } finally {
      project.cleanup();
    }
  });

  it('artifacts 仅为已知四文件之子集，workflow.json 自身不列入 artifacts', () => {
    const project = createTempProject();
    try {
      writeChange(project.changesDir, 'subset-artifacts', {
        artifacts: {
          'proposal.md': '# p',
          'design.md': '# d',
          'tasks.md': '# t',
          'test-design.md': '# td',
          'notes.txt': 'ignored',
        },
        workflowJson: { workflow_type: 'requirement' },
      });

      const entry = runChangeList(project.root).changes.find((c) => c.name === 'subset-artifacts')!;

      expect(new Set(entry.artifacts)).toEqual(
        new Set(['proposal.md', 'design.md', 'tasks.md', 'test-design.md']),
      );
      expect(entry.artifacts).not.toContain('notes.txt');
      expect(entry.artifacts).not.toContain('workflow.json');
    } finally {
      project.cleanup();
    }
  });

  it('4 值枚举逐一按各自 phase table 计算 workflow_done', () => {
    const project = createTempProject();
    try {
      for (const workflowType of ['requirement', 'bug-fix', 'refactor', 'test-only']) {
        writeChange(project.changesDir, `enum-${workflowType}`, {
          workflowJson: { workflow_type: workflowType },
          evalEntries: makeAllPassEntries(workflowType),
        });
      }

      const listed = runChangeList(project.root).changes;
      for (const workflowType of ['requirement', 'bug-fix', 'refactor', 'test-only']) {
        const change = listed.find((c) => c.name === `enum-${workflowType}`)!;
        expect(change.workflow_done).toBe(true);
      }
    } finally {
      project.cleanup();
    }
  });

  it('projectRoot 为 undefined / null（None）时抛错而非崩溃进程', () => {
    expect(() => runChangeList(undefined as unknown as string)).toThrow();
    expect(() => runChangeList(null as unknown as string)).toThrow();
  });

  it('projectRoot 为空串时既有 project_root 契约仍成立（按相对路径扫描，count === 0）', () => {
    const result0 = runChangeList('');

    expect(result0.project_root).toBe('');
    expect(result0.count).toBe(result0.changes.length);
  });
});
