/**
 * 集成测试: MCP 门控事件序列 → session 绑定/门②③ → file_log 归账与派生净状态
 *
 * 真实链路（模块全为真实实现，仅 mock 进程边界）:
 * - `bin/src/commands/change-create.ts` — workflow.json 创建方（fixture）
 * - `bin/src/commands/phase-start.ts` / `bin/src/commands/phase-log.ts` — 门②开关
 *   （写/清 active_phase 运行态，真盘读写）
 * - `bin/src/hooks.ts` — PostToolUse 记录器（runRecordFiles: 事件解析 / 绑定刷新 /
 *   门①②③判定 / scope 解析 / file_log 落盘）
 * - `bin/src/lib/session-registry.ts` — session 绑定注册表（落盘于隔离 tmpdir）
 * - `bin/src/modules/workflow/files/*` — file_log 覆盖/追加与派生净状态（真盘读写）
 *
 * 只 mock 边界: stdin（node:fs 与 fs 两种拼写的 readFileSync 的 fd 0 分支）、
 * getProjectDir（指向临时项目根，不用 process.chdir）、os.tmpdir（隔离注册表落盘）。
 *
 * 覆盖 test-design「集成测试」场景: 门控事件序列回放（phase_next 绑定 → phase_start
 * 开门 → 归账 → phase_log 关门）/ 重绑竞态回归（AC-5）/ done 后 backtrack →
 * phase_start 重开自愈 / 写方与读方派生等价。关联 AC: AC-4, AC-5, AC-6, AC-9。
 */

import * as fs from 'fs';
import { createHash } from 'node:crypto';
import type * as NodeFs from 'node:fs';
import type * as NodeOs from 'node:os';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runChangeCreate } from '../../src/commands/change-create';
import { runPhaseLog } from '../../src/commands/phase-log';
import { runPhaseStart } from '../../src/commands/phase-start';
import type * as ProjectRoot from '../../src/lib/project-root';

// ---------------------------------------------------------------------------
// 可控 mock（vi.hoisted 确保 mock 对象先于 vi.mock 工厂初始化）
// ---------------------------------------------------------------------------

/** phase 入参类型（phaseIdSchema 的 9 值枚举）。 */
type PhaseId = Parameters<typeof runPhaseStart>[0]['phase'];

const h = vi.hoisted(() => ({
  /** 待注入的 stdin 事件 JSON 队列（readFileSync 的 fd 0 调用按序弹出） */
  queue: [] as string[],
  /** mock os.tmpdir() 返回的隔离目录（session 注册表落盘处，随用例清理） */
  tmpRoot: '',
  /** 真实 readFileSync（非 fd 0 的调用原样委托回去，保证真盘读写不受影响） */
  actualReadFileSync: null as unknown as typeof NodeFs.readFileSync,
  mockReadFileSync: vi.fn(),
  mockGetProjectDir: vi.fn<() => string>(),
}));

// node:fs 与 fs 两种拼写分别 mock（hooks.ts 用 'node:fs'，change-create / file-inventory
// 用 'fs'）：仅替换 readFileSync，其余成员（existsSync / writeFileSync / mkdirSync …）保持真实。
vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeFs>();
  h.actualReadFileSync = actual.readFileSync;
  return { ...actual, readFileSync: h.mockReadFileSync };
});

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeFs>();
  h.actualReadFileSync = actual.readFileSync;
  return { ...actual, readFileSync: h.mockReadFileSync };
});

// session-registry 的注册表落盘于 `<tmpdir>/dev-team-hooks/<sha256 前 16 位>.json`，
// 把 tmpdir 重定向到每用例的隔离目录，注册表随临时目录一起清理。
vi.mock('node:os', async (importOriginal) => {
  const actual = await importOriginal<typeof NodeOs>();
  return { ...actual, tmpdir: () => (h.tmpRoot ? h.tmpRoot : actual.tmpdir()) };
});

// hooks.ts 以 './lib/project-root' 引用，vitest 按解析后的绝对路径归一，mock 生效。
vi.mock('../../src/lib/project-root', async (importOriginal) => {
  const actual = await importOriginal<typeof ProjectRoot>();
  return { ...actual, getProjectDir: h.mockGetProjectDir };
});

// stdin 注入实现：fd 0 → 弹出注入队列；其余调用委托真实 readFileSync。
h.mockReadFileSync.mockImplementation(((...args: unknown[]) => {
  if (args[0] === 0) {
    const next = h.queue.shift();
    if (next === undefined) {
      throw new Error('record-files stdin 注入队列已空');
    }
    return next;
  }
  const realReadFileSync = h.actualReadFileSync as unknown as (...a: unknown[]) => unknown;
  return realReadFileSync(args[0], args[1]);
}) as unknown as (...args: unknown[]) => unknown);

// 在 hooks 模块加载前拦截 process，防止模块顶层 main() 自动执行产生副作用
const exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
const stderrMock = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
const stdoutWriteMock = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);

// 动态 import — vi.mock 已生效，process 拦截已就位；argv 指向 record-files 使顶层
// main() 走记录器空读路径（队列为空 → 内部静默返回）后立即还原
const originalArgv = process.argv;
process.argv = ['node', 'hooks', 'record-files'];
const { runRecordFiles } = await import('../../src/hooks');
process.argv = originalArgv;

// ---------------------------------------------------------------------------
// 每用例隔离的临时项目根与注册表目录
// ---------------------------------------------------------------------------

let tmpBase: string;
let projectRoot: string;

beforeEach(() => {
  tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), 'file-inv-rec-'));
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'file-inv-rec-proj-'));
  h.tmpRoot = tmpBase;
  h.queue.length = 0;
  h.mockGetProjectDir.mockReset();
  h.mockGetProjectDir.mockReturnValue(projectRoot);
  stderrMock.mockClear();
  stdoutWriteMock.mockClear();
  exitMock.mockClear();
});

afterEach(() => {
  fs.rmSync(tmpBase, { recursive: true, force: true });
  fs.rmSync(projectRoot, { recursive: true, force: true });
  h.tmpRoot = '';
});

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

function mcpEvent(
  toolSuffix: string,
  sessionId: string,
  toolInput: Record<string, unknown>,
): Record<string, unknown> {
  return {
    tool_name: `mcp__plugin_dev-team_dev-team__${toolSuffix}`,
    tool_input: toolInput,
    session_id: sessionId,
  };
}

function phaseNextEvent(sessionId: string, change: string): Record<string, unknown> {
  return mcpEvent('phase_next', sessionId, { change });
}

function phaseStartEvent(
  sessionId: string,
  change: string,
  phase: string,
): Record<string, unknown> {
  return mcpEvent('phase_start', sessionId, { change, phase });
}

function writeEvent(
  filePath: string,
  sessionId: string,
  agentType?: string,
): Record<string, unknown> {
  const event: Record<string, unknown> = {
    tool_name: 'Write',
    tool_input: { file_path: filePath },
    session_id: sessionId,
  };
  if (agentType !== undefined) {
    event.agent_type = agentType;
  }
  return event;
}

function bashEvent(command: string, sessionId: string): Record<string, unknown> {
  return {
    tool_name: 'Bash',
    tool_input: { command },
    session_id: sessionId,
  };
}

/** Cursor 侧 Edit 等价工具事件（tool_input.file_path，与 Write/Edit 同链路） */
function strReplaceEvent(filePath: string, sessionId: string): Record<string, unknown> {
  return {
    tool_name: 'StrReplace',
    tool_input: { file_path: filePath },
    session_id: sessionId,
  };
}

/** Cursor 侧 shell 工具事件（tool_input.command，与 Bash/PowerShell 同链路） */
function shellEvent(command: string, sessionId: string): Record<string, unknown> {
  return {
    tool_name: 'Shell',
    tool_input: { command },
    session_id: sessionId,
  };
}

/** 注入一个 hook 事件并同步执行一次记录器子命令 */
function recordEvent(event: Record<string, unknown>): void {
  h.queue.push(JSON.stringify(event));
  runRecordFiles();
  expect(h.queue).toHaveLength(0);
}

/** 经真实 runPhaseStart 开门（门②置开），并注入 phase_start hook 事件刷新绑定。 */
function phaseStart(change: string, phase: PhaseId, sessionId = 'S1'): void {
  recordEvent(phaseStartEvent(sessionId, change, phase));
  runPhaseStart({ change, phase, project_root: projectRoot });
}

/** 经真实 runPhaseLog 落盘并清场（门②关闭）；verdict 由 checklist 推导。 */
function phaseLog(
  change: string,
  phase: PhaseId,
  verdict: 'pass' | 'fail' = 'pass',
): { written: boolean; phase: string; attempt: number } {
  return runPhaseLog({
    change,
    phase,
    report: 'ok',
    checklist: [{ item: 'i', pass: verdict === 'pass', evidence: 'e' }],
    project_root: projectRoot,
  });
}

function changeDir(change: string): string {
  return path.join(projectRoot, 'openspec', 'changes', change);
}

function workflowPath(change: string): string {
  return path.join(changeDir(change), 'workflow.json');
}

interface LogEntry {
  op: string;
  scope: string;
  attempt?: number;
  path: string;
  at: string;
}

function readDoc(change: string): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(workflowPath(change), 'utf-8')) as Record<string, unknown>;
}

function readLog(change: string): LogEntry[] {
  return (readDoc(change).file_log as LogEntry[]) ?? [];
}

/** 按 deriveNetState 语义重放 file_log，得派生净状态（读方视图）。 */
function readNetState(change: string): { written: string[]; deleted: string[] } {
  const written = new Set<string>();
  const deleted = new Set<string>();
  for (const entry of readLog(change)) {
    if (entry.op === 'write') {
      deleted.delete(entry.path);
      written.add(entry.path);
    } else if (entry.op === 'delete') {
      written.delete(entry.path);
      deleted.add(entry.path);
    } else {
      written.delete(entry.path);
      deleted.delete(entry.path);
    }
  }
  return { written: Array.from(written), deleted: Array.from(deleted) };
}

function registryFilePath(): string {
  const hash = createHash('sha256').update(path.resolve(projectRoot)).digest('hex').slice(0, 16);
  return path.join(tmpBase, 'dev-team-hooks', `${hash}.json`);
}

function stderrText(): string {
  return stderrMock.mock.calls.map((call) => String(call[0])).join('');
}

// ============================================================================
// 场景: 门控事件序列回放（phase_next 绑定 → phase_start 开门 → 归账 → phase_log 关门）
// ============================================================================

describe('场景: 门控事件序列回放', () => {
  it('正向: phase_next(绑定) → phase_start(开门) → executor 写 → 主 agent 写 → log 含 phase 条目与 workflow 条目各一条，workflow_type/created/未知键保留', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    const docPath = workflowPath('change-a');
    const doc = JSON.parse(fs.readFileSync(docPath, 'utf-8')) as Record<string, unknown>;
    doc.unknown_key = { keep: true };
    fs.writeFileSync(docPath, `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');

    recordEvent(phaseNextEvent('S1', 'change-a'));
    phaseStart('change-a', 'implement');

    // executor 写事件（agent_type 归一匹配 implement executor）→ phase scope
    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'executor.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );
    // 主 agent 写事件（无 agent_type）→ workflow scope
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'main.ts'), 'S1'));

    const log = readLog('change-a');
    expect(log).toHaveLength(2);
    expect(log[0]).toMatchObject({
      op: 'write',
      scope: 'implement',
      attempt: 1,
      path: 'src/executor.ts',
    });
    expect(log[1]).toMatchObject({ op: 'write', scope: 'workflow', path: 'src/main.ts' });
    expect(Object.prototype.hasOwnProperty.call(log[1], 'attempt')).toBe(false);

    const after = readDoc('change-a');
    expect(after.workflow_type).toBe('requirement');
    expect(after.created).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(after.unknown_key).toEqual({ keep: true });
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('正向: phase_log 清场后同 session 再写 → log 不增长，stderr 含门②诊断，exit 0', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'first.ts'), 'S1'));
    expect(readLog('change-a')).toHaveLength(1);

    // phase_log(pass) 落盘并清场
    phaseLog('change-a', 'implement');
    expect(readDoc('change-a').active_phase ?? null).toBeNull();

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'after.ts'), 'S1'));

    expect(readLog('change-a')).toHaveLength(1);
    expect(stderrText()).toContain('无 active_phase');
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('异常: 绑定后、phase_start 前的写事件 → 门②丢弃 + 诊断（前置顺序敏感）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'early.ts'), 'S1'));

    expect(readLog('change-a')).toEqual([]);
    expect(stderrText()).toContain('无 active_phase');
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('边界: executor: null 的 phase（code-review）开门后写事件 → workflow scope（门③降级不丢事件）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'code-review');

    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'reviewed.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );

    const log = readLog('change-a');
    expect(log).toHaveLength(1);
    expect(log[0]).toMatchObject({ scope: 'workflow', path: 'src/reviewed.ts' });
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('边界: dev-team_<id>（cursor-home 前缀）与 dev-team:<id>（claude 前缀）分别匹配同一 executor token → 均 phase scope', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'claude.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );
    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'cursor.ts'),
        'S1',
        'dev-team_implementation-generator',
      ),
    );

    const log = readLog('change-a');
    expect(log).toHaveLength(2);
    expect(log[0]).toMatchObject({ scope: 'implement', path: 'src/claude.ts' });
    expect(log[1]).toMatchObject({ scope: 'implement', path: 'src/cursor.ts' });
  });
});

// ============================================================================
// 场景: 重绑竞态回归（AC-5）— Verdict 的 phase_next 仅刷新绑定，不复活归账
// ============================================================================

describe('场景: 重绑竞态回归（AC-5）', () => {
  it('正向: 最终 pass 后 phase_next 事件 → 绑定刷新；后续写事件被门②丢弃，归账不复活（AC-5）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');
    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'during.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );
    expect(readLog('change-a')).toHaveLength(1);

    // 最终 pass：phase_log 落盘清场（门②关闭）
    phaseLog('change-a', 'implement', 'pass');

    // Verdict 的 phase_next 事件：绑定注册表确已刷新（非「未绑定」路径）
    recordEvent(phaseNextEvent('S1', 'change-a'));
    expect(fs.existsSync(registryFilePath())).toBe(true);

    // 随后写事件仍被门②丢弃：file_log 逐字节不变、归账不复活
    const before = fs.readFileSync(workflowPath('change-a'), 'utf-8');
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'revived.ts'), 'S1'));

    expect(fs.readFileSync(workflowPath('change-a'), 'utf-8')).toBe(before);
    expect(readLog('change-a')).toHaveLength(1);
    expect(stderrText()).toContain('无 active_phase');
    expect(stderrText()).not.toContain('未绑定');
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ============================================================================
// 场景: done 后 backtrack → phase_start 重开自愈
// ============================================================================

describe('场景: done 后 backtrack → phase_start 重开自愈', () => {
  it('正向: 门②关闭后重新 phase_start → active_phase 重新写入，写事件恢复归账（scope=目标 phase）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');
    phaseLog('change-a', 'implement', 'pass');

    // 回溯后的新 turn：重新 phase_start（last-wins 写入新运行态）
    phaseStart('change-a', 'test-gen');

    recordEvent(
      writeEvent(path.join(projectRoot, 'src', 'redo.ts'), 'S1', 'dev-team:test-gen-generator'),
    );

    const log = readLog('change-a');
    const redo = log.find((e) => e.path === 'src/redo.ts');
    expect(redo).toBeDefined();
    expect(redo).toMatchObject({ scope: 'test-gen', attempt: 1 });
    expect((readDoc('change-a').active_phase as Record<string, unknown>).phase).toBe('test-gen');
  });

  it('边界: 错误终态 turn（未 phase_start，如 done/error 间隙）写事件 → 门②丢弃不开门', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'gap.ts'), 'S1'));

    expect(readLog('change-a')).toEqual([]);
    expect(stderrText()).toContain('无 active_phase');
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ============================================================================
// 场景: 写方与读方派生等价（跨模块净状态一致性，AC-6）
// ============================================================================

describe('场景: 写方与读方派生等价', () => {
  it('正向: phase 写 + workflow 重写同 path → 派生 written 含该 path 且无 scope/attempt 泄漏（后条胜）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'shared.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'shared.ts'), 'S1'));

    // 日志保留全部来源轨迹（跨 key 两条）
    expect(readLog('change-a')).toHaveLength(2);
    // 派生净状态：workflow 后条胜，无审计明细键泄漏
    expect(readNetState('change-a')).toEqual({ written: ['src/shared.ts'], deleted: [] });
  });

  it('边界: revert 事件（git restore）→ 派生双桶均不含该 path', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'reverted.ts'), 'S1'));
    recordEvent(bashEvent('git restore src/reverted.ts', 'S1'));

    expect(readNetState('change-a')).toEqual({ written: [], deleted: [] });
    // 两次事件同为 workflow scope（同 key）→ 原位覆盖，log 仅剩 revert 一条
    expect(readLog('change-a').map((e) => e.op)).toEqual(['revert']);
  });
});

// ============================================================================
// 场景: session 隔离与换绑（迁移保留，适配门②）
// ============================================================================

describe('场景: session 隔离与换绑', () => {
  it('S1 与 S2 事件交错注入并分别开门，两 change 的 log 各自只含自家路径，互不串账', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    runChangeCreate('change-b', projectRoot, 'bug-fix');
    phaseStart('change-a', 'implement', 'S1');
    phaseStart('change-b', 'implement', 'S2');

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a1.ts'), 'S1'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'b1.ts'), 'S2'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a2.ts'), 'S1'));

    expect(readNetState('change-a').written.sort()).toEqual(['src/a1.ts', 'src/a2.ts']);
    expect(readNetState('change-b').written).toEqual(['src/b1.ts']);
    expect(readNetState('change-b').deleted).toEqual([]);
  });

  it('S1 经新 phase_next 事件换绑 change-b（并开门）后事件归 change-b，change-a 的 log 不再增长', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    runChangeCreate('change-b', projectRoot, 'bug-fix');
    phaseStart('change-a', 'implement', 'S1');
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a1.ts'), 'S1'));

    recordEvent(phaseNextEvent('S1', 'change-b'));
    phaseStart('change-b', 'implement', 'S1');
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a3.ts'), 'S1'));

    expect(readNetState('change-a').written).toEqual(['src/a1.ts']);
    expect(readNetState('change-a').written).not.toContain('src/a3.ts');
    expect(readNetState('change-b').written).toEqual(['src/a3.ts']);
  });

  it('首个 phase_next 之前的未绑定事件 → 丢弃且不影响其后建绑与归账（门①诊断）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    const docPath = workflowPath('change-a');
    const before = fs.readFileSync(docPath, 'utf-8');

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'pre.ts'), 'S1'));
    expect(stderrText()).toContain('未绑定');
    expect(fs.readFileSync(docPath, 'utf-8')).toBe(before);
    expect(readLog('change-a')).toEqual([]);

    // 建绑 + 开门后归账照常
    phaseStart('change-a', 'implement', 'S1');
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'post.ts'), 'S1'));

    expect(readNetState('change-a').written).toEqual(['src/post.ts']);
    expect(readNetState('change-a').deleted).toEqual([]);
  });
});

// ============================================================================
// 场景: fd 复制重定向不误记账（2>&1）— 迁移保留
// ============================================================================

describe('场景: fd 复制重定向不误记账（2>&1）', () => {
  it('含 2>&1 / 1>&2 / >&2 的命令不产生任何文件条目（回归: written 曾记入 "1"）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(bashEvent('pnpm run check 2>&1 | tail -60', 'S1'));
    recordEvent(bashEvent('node cli.js 1>&2', 'S1'));
    recordEvent(shellEvent('pwsh script.ps1 >&2', 'S1'));

    expect(readNetState('change-a')).toEqual({ written: [], deleted: [] });
  });

  it('同一命令中真实重定向照常归账，仅 fd 复制目标被忽略', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(bashEvent('pnpm run test > test-output.log 2>&1', 'S1'));

    expect(readNetState('change-a').written).toEqual(['test-output.log']);
  });

  it('非纯数字的 >&file 目标仍归账（legacy 重定向不误伤）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(bashEvent('cat a.txt >&outfile', 'S1'));
    recordEvent(bashEvent('cmd >&2.log', 'S1'));

    expect(readNetState('change-a').written.sort()).toEqual(['2.log', 'outfile']);
  });
});

// ============================================================================
// 场景: Cursor 平台工具名事件（StrReplace / Shell）— 迁移保留
// ============================================================================

describe('场景: Cursor 平台工具名事件（StrReplace / Shell）', () => {
  it('StrReplace 事件按 file_path 归账，Shell 事件经 extractFileOps 归账（与 Write/Bash 同链路）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    phaseStart('change-a', 'implement');

    recordEvent(strReplaceEvent(path.join(projectRoot, 'src', 'cursor-edit.ts'), 'S1'));
    recordEvent(shellEvent('echo x > src/cursor-shell.ts', 'S1'));
    recordEvent(shellEvent('rm src/cursor-edit.ts', 'S1'));

    expect(readNetState('change-a').written).toEqual(['src/cursor-shell.ts']);
    expect(readNetState('change-a').deleted).toEqual(['src/cursor-edit.ts']);
  });

  it('cursorHome 的 phase_next MCP 全名（mcp__user-dev-team_mcp__phase_next）同样建绑并归账后续事件', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent({
      tool_name: 'mcp__user-dev-team_mcp__phase_next',
      tool_input: { change: 'change-a' },
      session_id: 'S9',
    });
    phaseStart('change-a', 'implement', 'S9');
    recordEvent(strReplaceEvent(path.join(projectRoot, 'src', 'home.ts'), 'S9'));

    expect(readNetState('change-a').written).toEqual(['src/home.ts']);
  });
});
