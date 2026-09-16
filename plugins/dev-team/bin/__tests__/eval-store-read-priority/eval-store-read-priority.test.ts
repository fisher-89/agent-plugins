/**
 * 集成测试: readEvalJson 优先级 → phase_next / change_list
 *
 * `phase_next` 与 `change_list` 都只通过 `readEvalJson` 取条目，自身不得迁移或
 * 删除文件。本套件用真实磁盘组合「仅遗留文件 / 权威空数组 + 遗留非空 / 权威非数组 /
 * 两文件皆无」验证只读行为与列表字段。
 *
 * 涉及模块:
 * - `plugins/dev-team/bin/src/lib/eval-json.ts` — 读取优先级与校验
 * - `plugins/dev-team/bin/src/commands/phase-next.ts` — 只读消费者
 * - `plugins/dev-team/bin/src/commands/change-list.ts` — 只读消费者
 *
 * 关联 AC: AC-3, AC-5, AC-7, AC-12
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runChangeList } from '../../src/commands/change-list';
import { runPhaseNext } from '../../src/commands/phase-next';
import { type EvalEntry } from '../../src/lib/eval-json';
import { getProjectDir } from '../../src/lib/project-root';

// `getWorkflowType()` 经 `getProjectDir()` 解析工程根，优先级为 call-scoped →
// `CLAUDE_PROJECT_DIR` → `WORKSPACE_FOLDER_PATHS` → `process.cwd()`。这里把
// `getProjectDir` mock 成临时工程根，替代 `process.chdir`：Stryker 的 vitest runner
// 强制 worker 线程池，worker 中 `process.chdir` 会抛
// ERR_WORKER_UNSUPPORTED_OPERATION（与 run-static-analysis.test.ts 相同的处理）。
// 环境变量隔离仍然保留：被测命令之外的真实工程根不得介入。
vi.mock('../../src/lib/project-root');

const CHANGE = 'read-priority';
const RUN_ID = 'priority-run';

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
let changeDir: string;
let savedProjectEnv: ReturnType<typeof saveProjectEnv>;

beforeEach(() => {
  savedProjectEnv = saveProjectEnv();
  delete process.env.CLAUDE_PROJECT_DIR;
  delete process.env.WORKSPACE_FOLDER_PATHS;
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'eval-priority-'));
  changeDir = path.join(projectRoot, 'openspec', 'changes', CHANGE);
  fs.mkdirSync(changeDir, { recursive: true });
  vi.mocked(getProjectDir).mockReturnValue(projectRoot);
});

afterEach(() => {
  restoreProjectEnv(savedProjectEnv);
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

function writeWorkflow(doc: unknown): void {
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), JSON.stringify(doc, null, 2), 'utf-8');
}

function writeLegacy(entries: unknown): void {
  fs.writeFileSync(path.join(changeDir, 'eval.json'), JSON.stringify(entries), 'utf-8');
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

/** 快照 change 目录下所有文件的字节内容，用于断言只读。 */
function snapshotDir(): Record<string, string> {
  const snapshot: Record<string, string> = {};
  for (const name of fs.readdirSync(changeDir).sort()) {
    snapshot[name] = fs.readFileSync(path.join(changeDir, name), 'utf-8');
  }
  return snapshot;
}

function phaseNext(): ReturnType<typeof runPhaseNext> {
  return runPhaseNext({ change: CHANGE, project_root: projectRoot, run_id: RUN_ID });
}

function changeList(): ReturnType<typeof runChangeList> {
  return runChangeList(projectRoot);
}

function changeEntry(): ReturnType<typeof runChangeList>['changes'][number] {
  const entry = changeList().changes.find((c) => c.name === CHANGE);
  expect(entry).toBeDefined();
  return entry!;
}

// ---------------------------------------------------------------------------
// 场景: 仅遗留 eval.json 的只读兼容
// ---------------------------------------------------------------------------

describe('仅遗留 eval.json 的只读兼容', () => {
  beforeEach(() => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11' });
  });

  it('phase_next 返回 dev-design 且不写盘（AC-3）', () => {
    writeLegacy([
      makeEntry({
        phase: 'proposal',
        verdict: 'pass',
        report: '提案通过',
        timestamp: '2026-09-11T09:00:00.000Z',
      }),
      makeEntry({
        phase: 'dev-design',
        verdict: 'fail',
        report: '设计待改进',
        timestamp: '2026-09-11T11:00:00.000Z',
      }),
    ]);
    const before = snapshotDir();

    const result = phaseNext();

    expect(result.next_phase).toBe('dev-design');
    expect(result.error).toBeNull();
    expect(result.last_result!.phase).toBe('dev-design');
    expect(snapshotDir()).toEqual(before);
  });

  it('change_list 填出 latest_phase 且 artifacts 无 eval.json（AC-7）', () => {
    writeLegacy([makeEntry({ phase: 'proposal', verdict: 'pass', report: '提案通过' })]);
    fs.writeFileSync(path.join(changeDir, 'proposal.md'), '# proposal', 'utf-8');
    const before = snapshotDir();

    const entry = changeEntry();

    expect(entry.latest_phase).toEqual({ phase: 'proposal', verdict: 'pass' });
    expect(entry.artifacts).toContain('proposal.md');
    expect(entry.artifacts).not.toContain('eval.json');
    expect(snapshotDir()).toEqual(before);
  });

  it('遗留根为对象时 phase_next 抛 Failed to read workflow.json（包装根类型错误），change_list 不抛且 workflow_done 为 false', () => {
    fs.writeFileSync(
      path.join(changeDir, 'eval.json'),
      JSON.stringify({ phase: 'proposal', verdict: 'pass' }),
      'utf-8',
    );

    expect(() => phaseNext()).toThrow(
      /Failed to read workflow\.json: .*eval\.json 根元素必须是数组/,
    );

    expect(() => changeList()).not.toThrow();
    expect(changeEntry().workflow_done).toBe(false);
  });

  it('两文件皆无评估来源时 phase_next 返回 first run proposal，change_list.latest_phase 为 null（AC-3）', () => {
    const result = phaseNext();

    expect(result.next_phase).toBe('proposal');
    expect(result.last_result).toBeNull();
    expect(changeEntry().latest_phase).toBeNull();
    expect(changeEntry().workflow_done).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 场景: 权威空数组忽略遗留文件
// ---------------------------------------------------------------------------

describe('权威空数组忽略遗留文件', () => {
  it('eval===[] 挡住遗留非空数组：phase_next 回到 first run（AC-5）', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', eval: [] });
    writeLegacy([
      makeEntry({ phase: 'proposal', verdict: 'pass' }),
      makeEntry({ phase: 'dev-design', verdict: 'pass' }),
      makeEntry({ phase: 'test-design', verdict: 'pass' }),
    ]);

    const result = phaseNext();

    expect(result.next_phase).toBe('proposal');
    expect(result.last_result).toBeNull();
    expect(changeEntry().latest_phase).toBeNull();
    expect(changeEntry().workflow_done).toBe(false);
  });

  it('phase_next 不 unlink 遗留 eval.json，两文件字节不变（AC-3）', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', eval: [] });
    writeLegacy([makeEntry({ phase: 'proposal', verdict: 'pass' })]);
    const before = snapshotDir();

    phaseNext();
    changeList();

    expect(snapshotDir()).toEqual(before);
    expect(fs.existsSync(path.join(changeDir, 'eval.json'))).toBe(true);
  });

  it('eval 为对象时 phase_next 抛错，change_list 该 change 仍在列表且 workflow_done 为 false（AC-12）', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', eval: {} });

    // 整文件校验（getWorkflowType）先于条目读取失败
    expect(() => phaseNext()).toThrow(/workflow\.json 格式非法/);

    expect(() => changeList()).not.toThrow();
    const entry = changeEntry();
    expect(entry.workflow_done).toBe(false);
    expect(entry.latest_phase).toBeNull();
  });

  it('eval===[] 且无遗留文件时与「无评估条目」相同', () => {
    writeWorkflow({ workflow_type: 'requirement', created: '2026-09-11', eval: [] });

    const result = phaseNext();

    expect(result.next_phase).toBe('proposal');
    expect(changeEntry().workflow_done).toBe(false);
    expect(changeEntry().latest_phase).toBeNull();
  });
});
