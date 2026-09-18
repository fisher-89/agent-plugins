/**
 * 集成测试: change_create → 无 eval 键 → readEvalJson / phase_next
 *
 * `change_create` 若写入 `"eval": []`，会按读优先级挡住遗留 `eval.json`，使进行中
 * change 的历史被静默丢弃；若它补写缺省元数据，又会与「唯一创建者」契约冲突。
 * 本套件在真实项目根调用 `runChangeCreate`，再 `readEvalJson` / `runPhaseNext`，
 * 确认新建 change 无 `eval` 键、无 `eval.json`、读取为 `[]`、下一 phase 为 `proposal`。
 *
 * 涉及模块:
 * - `plugins/dev-team/bin/src/commands/change-create.ts` — 创建方
 * - `plugins/dev-team/bin/src/lib/eval-json.ts` — 读取方
 * - `plugins/dev-team/bin/src/commands/phase-next.ts` — 读取方
 *
 * 关联 AC: AC-6, AC-3, AC-13
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runChangeCreate } from '../../src/commands/change-create';
import { runPhaseNext } from '../../src/commands/phase-next';
import { readEvalJson, type EvalEntry } from '../../src/lib/eval-json';
import { getProjectDir } from '../../src/lib/project-root';

// `getWorkflowType()` 经 `getProjectDir()` 解析工程根，优先级为 call-scoped →
// `CLAUDE_PROJECT_DIR` → `WORKSPACE_FOLDER_PATHS` → `process.cwd()`。这里把
// `getProjectDir` mock 成临时工程根，替代 `process.chdir`：Stryker 的 vitest runner
// 强制 worker 线程池，worker 中 `process.chdir` 会抛
// ERR_WORKER_UNSUPPORTED_OPERATION（与 run-static-analysis.test.ts 相同的处理）。
// 环境变量隔离仍然保留：被测命令之外的真实工程根不得介入。
vi.mock('../../src/lib/project-root');

const CHANGE = 'created-change';
const RUN_ID = 'create-run';

const ENV_KEYS = ['CLAUDE_PROJECT_DIR', 'WORKSPACE_FOLDER_PATHS'] as const;

function saveProjectEnv(): Record<(typeof ENV_KEYS)[number], string | undefined> {
  return {
    CLAUDE_PROJECT_DIR: process.env.CLAUDE_PROJECT_DIR,
    WORKSPACE_FOLDER_PATHS: process.env.WORKSPACE_FOLDER_PATHS,
  };
}

function restoreProjectEnv(saved: ReturnType<typeof saveProjectEnv>): void {
  for (const key of ENV_KEYS) {
    if (saved[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = saved[key];
    }
  }
}

let projectRoot: string;
let savedProjectEnv: ReturnType<typeof saveProjectEnv>;

beforeEach(() => {
  savedProjectEnv = saveProjectEnv();
  delete process.env.CLAUDE_PROJECT_DIR;
  delete process.env.WORKSPACE_FOLDER_PATHS;
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'eval-create-'));
  vi.mocked(getProjectDir).mockReturnValue(projectRoot);
});

afterEach(() => {
  restoreProjectEnv(savedProjectEnv);
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

function changeDir(): string {
  return path.join(projectRoot, 'openspec', 'changes', CHANGE);
}

function workflowPath(): string {
  return path.join(changeDir(), 'workflow.json');
}

function legacyPath(): string {
  return path.join(changeDir(), 'eval.json');
}

function readWorkflow(): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(workflowPath(), 'utf-8')) as Record<string, unknown>;
}

function makeEntry(
  overrides: Partial<EvalEntry> & Pick<EvalEntry, 'phase' | 'verdict'>,
): EvalEntry {
  return {
    attempt: 1,
    timestamp: '2026-09-11T10:00:00.000Z',
    report: 'r',
    checklist: [],
    backtrack_to: null,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// 场景: 新建 change 无评估历史
// ---------------------------------------------------------------------------

describe('新建 change 无评估历史', () => {
  it('创建后无 eval 键、无 eval.json，readEvalJson 为 []（AC-6）', () => {
    const result = runChangeCreate(CHANGE, projectRoot, 'requirement');

    expect(result.path).toBe(changeDir());
    const doc = readWorkflow();
    expect(doc.workflow_type).toBe('requirement');
    expect(doc.created).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(Object.prototype.hasOwnProperty.call(doc, 'eval')).toBe(false);
    expect(Object.keys(doc).sort()).toEqual(['created', 'file_log', 'workflow_type']);
    expect(fs.existsSync(legacyPath())).toBe(false);
    expect(readEvalJson(changeDir())).toEqual([]);
  });

  it('随后 phase_next 为 first run proposal（getWorkflowType 严格读取通过）（AC-13 反证）', () => {
    runChangeCreate(CHANGE, projectRoot, 'requirement');

    const result = runPhaseNext({ change: CHANGE, project_root: projectRoot, run_id: RUN_ID });

    expect(result.next_phase).toBe('proposal');
    expect(result.error).toBeNull();
    expect(result.last_result).toBeNull();
  });

  it('第二次 change_create 同名时抛已存在，不补 eval、不建 eval.json', () => {
    runChangeCreate(CHANGE, projectRoot, 'requirement');
    const before = fs.readFileSync(workflowPath(), 'utf-8');

    expect(() => runChangeCreate(CHANGE, projectRoot, 'requirement')).toThrow(/已存在/);

    expect(fs.readFileSync(workflowPath(), 'utf-8')).toBe(before);
    expect(Object.prototype.hasOwnProperty.call(readWorkflow(), 'eval')).toBe(false);
    expect(fs.existsSync(legacyPath())).toBe(false);
  });

  it('手工在无 eval 键的 workflow.json 旁放遗留 eval.json 时 readEvalJson 返回遗留数组（AC-3）', () => {
    runChangeCreate(CHANGE, projectRoot, 'requirement');
    fs.writeFileSync(
      legacyPath(),
      JSON.stringify([makeEntry({ phase: 'proposal', verdict: 'pass', report: '迁移前历史' })]),
      'utf-8',
    );

    const entries = readEvalJson(changeDir());

    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('迁移前历史');

    // 反证：若 change_create 写了 "eval": []，这里会读到空数组、历史被静默丢弃
    const doc = readWorkflow();
    expect(Object.prototype.hasOwnProperty.call(doc, 'eval')).toBe(false);
  });

  it('4 值枚举逐一创建均无 eval 键、无 eval.json', () => {
    for (const workflowType of ['requirement', 'bug-fix', 'refactor', 'test-only'] as const) {
      const name = `enum-${workflowType}`;
      const result = runChangeCreate(name, projectRoot, workflowType);
      const dir = path.join(projectRoot, 'openspec', 'changes', name);

      expect(result.path).toBe(dir);
      const doc = JSON.parse(fs.readFileSync(path.join(dir, 'workflow.json'), 'utf-8')) as Record<
        string,
        unknown
      >;
      expect(Object.prototype.hasOwnProperty.call(doc, 'eval')).toBe(false);
      expect(fs.existsSync(path.join(dir, 'eval.json'))).toBe(false);
      expect(readEvalJson(dir)).toEqual([]);
    }
  });
});
