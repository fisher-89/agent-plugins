/**
 * 集成测试: phase_next 事件 → session 注册表 → files 清单归账
 *
 * 真实链路（三个模块全为真实实现，不 mock）:
 * - `bin/src/commands/change-create.ts` — workflow.json 创建方（fixture）
 * - `bin/src/hooks.ts` — PostToolUse 记录器（runRecordFiles: 事件解析 / 归一化 / 自污染过滤 / 折叠编排）
 * - `bin/src/lib/session-registry.ts` — session 绑定注册表（落盘于隔离 tmpdir）
 * - `bin/src/lib/file-inventory.ts` — 清单读取 / foldFileOps / 保留键写回（真盘读写）
 *
 * 只 mock 边界: stdin（node:fs 与 fs 两种拼写的 readFileSync 的 fd 0 分支）、
 * getProjectDir（指向临时项目根，不用 process.chdir）、os.tmpdir（隔离注册表落盘）。
 *
 * 覆盖 test-design「集成测试」场景: 工作流事件序列回放净状态 / session 隔离与换绑 /
 * 条目来源审计（agent_type）。关联 AC: AC-2, AC-3, AC-13。
 */

import * as fs from 'fs';
import { createHash } from 'node:crypto';
import type * as NodeFs from 'node:fs';
import type * as NodeOs from 'node:os';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runChangeCreate } from '../../src/commands/change-create';
import type * as ProjectRoot from '../../src/lib/project-root';

// ---------------------------------------------------------------------------
// 可控 mock（vi.hoisted 确保 mock 对象先于 vi.mock 工厂初始化）
// ---------------------------------------------------------------------------

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

function phaseNextEvent(sessionId: string, change: string): Record<string, unknown> {
  return {
    tool_name: 'mcp__plugin_dev-team_dev-team__phase_next',
    tool_input: { change },
    session_id: sessionId,
  };
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

function changeDir(change: string): string {
  return path.join(projectRoot, 'openspec', 'changes', change);
}

function workflowPath(change: string): string {
  return path.join(changeDir(change), 'workflow.json');
}

interface FilesShape {
  written: string[];
  deleted: string[];
  source?: Record<string, string>;
}

function readDoc(change: string): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(workflowPath(change), 'utf-8')) as Record<string, unknown>;
}

function readFiles(change: string): FilesShape {
  return readDoc(change).files as FilesShape;
}

function registryFilePath(): string {
  const hash = createHash('sha256').update(path.resolve(projectRoot)).digest('hex').slice(0, 16);
  return path.join(tmpBase, 'dev-team-hooks', `${hash}.json`);
}

// ============================================================================
// 场景: 工作流事件序列回放净状态
// ============================================================================

describe('场景: 工作流事件序列回放净状态', () => {
  it('事件序列回放后 files 净状态正确，且 workflow_type/created/未知键保留（AC-2、AC-4）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    const docPath = workflowPath('change-a');
    const doc = JSON.parse(fs.readFileSync(docPath, 'utf-8')) as Record<string, unknown>;
    doc.unknown_key = { keep: true };
    fs.writeFileSync(docPath, `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');

    recordEvent(phaseNextEvent('S1', 'change-a'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'foo.ts'), 'S1'));
    recordEvent(bashEvent('echo x > src/bar.ts', 'S1'));
    recordEvent(bashEvent('rm src/foo.ts', 'S1'));
    recordEvent(bashEvent('git restore src/bar.ts', 'S1'));
    recordEvent(bashEvent('mv src/a.ts src/b.ts', 'S1'));

    // foo: write→delete 净 deleted；bar: write→revert 净 untouched；a→b: mv 双条目
    const files = readFiles('change-a');
    expect(files.written).toEqual(['src/b.ts']);
    expect([...files.deleted].sort()).toEqual(['src/a.ts', 'src/foo.ts']);

    const after = readDoc('change-a');
    expect(after.workflow_type).toBe('requirement');
    expect(after.created).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(after.unknown_key).toEqual({ keep: true });
  });

  it('openspec/** 与 workflow.json 自身的写事件不入清单（自污染排除，AC-2）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));

    recordEvent(
      writeEvent(path.join(projectRoot, 'openspec', 'changes', 'change-a', 'proposal.md'), 'S1'),
    );
    recordEvent(writeEvent(workflowPath('change-a'), 'S1'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'ok.ts'), 'S1'));

    const files = readFiles('change-a');
    expect(files.written).toEqual(['src/ok.ts']);
    expect(files.deleted).toEqual([]);
    expect(files.source).toBeUndefined();
  });

  it('注册表文件被外部删除后写事件静默丢弃：stderr 诊断、exit 0、workflow.json 逐字节不变', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'foo.ts'), 'S1'));
    const docPath = workflowPath('change-a');
    const before = fs.readFileSync(docPath, 'utf-8');
    expect(readFiles('change-a').written).toEqual(['src/foo.ts']);

    const registryFile = registryFilePath();
    expect(fs.existsSync(registryFile)).toBe(true);
    fs.rmSync(registryFile, { force: true });

    expect(() =>
      recordEvent(writeEvent(path.join(projectRoot, 'src', 'second.ts'), 'S1')),
    ).not.toThrow();

    expect(fs.readFileSync(docPath, 'utf-8')).toBe(before);
    expect(readFiles('change-a').written).toEqual(['src/foo.ts']);
    const allStderr = stderrMock.mock.calls.map((call) => String(call[0])).join('');
    expect(allStderr).toContain('未绑定');
    expect(exitMock).not.toHaveBeenCalled();
  });
});

// ============================================================================
// 场景: session 隔离与换绑
// ============================================================================

describe('场景: session 隔离与换绑', () => {
  it('S1 与 S2 事件交错注入，两 change 清单各自只含自家路径，互不串账（AC-3）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    runChangeCreate('change-b', projectRoot, 'bug-fix');
    recordEvent(phaseNextEvent('S1', 'change-a'));
    recordEvent(phaseNextEvent('S2', 'change-b'));

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a1.ts'), 'S1'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'b1.ts'), 'S2'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a2.ts'), 'S1'));

    const filesA = readFiles('change-a');
    expect([...filesA.written].sort()).toEqual(['src/a1.ts', 'src/a2.ts']);
    const filesB = readFiles('change-b');
    expect(filesB.written).toEqual(['src/b1.ts']);
    expect(filesB.deleted).toEqual([]);
  });

  it('S1 经新 phase_next 事件换绑 change-b 后事件归 change-b，change-a 清单不再增长（AC-3）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    runChangeCreate('change-b', projectRoot, 'bug-fix');
    recordEvent(phaseNextEvent('S1', 'change-a'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a1.ts'), 'S1'));

    recordEvent(phaseNextEvent('S1', 'change-b'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'a3.ts'), 'S1'));

    const filesA = readFiles('change-a');
    expect(filesA.written).toEqual(['src/a1.ts']);
    expect(filesA.written).not.toContain('src/a3.ts');
    const filesB = readFiles('change-b');
    expect(filesB.written).toEqual(['src/a3.ts']);
  });

  it('首个 phase_next 之前的未绑定事件 → 丢弃且不影响其后建绑与归账（AC-3）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    const docPath = workflowPath('change-a');
    const before = fs.readFileSync(docPath, 'utf-8');

    // 建绑之前的未绑定写事件：静默丢弃（stderr 诊断），清单逐字节不变
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'pre.ts'), 'S1'));
    expect(stderrMock.mock.calls.some((c) => String(c[0]).includes('未绑定'))).toBe(true);
    expect(fs.readFileSync(docPath, 'utf-8')).toBe(before);
    expect(readFiles('change-a').written).toEqual([]);

    // 随后首个 phase_next 建绑照常生效，归账不受此前丢弃影响
    recordEvent(phaseNextEvent('S1', 'change-a'));
    recordEvent(writeEvent(path.join(projectRoot, 'src', 'post.ts'), 'S1'));

    expect(readFiles('change-a').written).toEqual(['src/post.ts']);
    expect(readFiles('change-a').deleted).toEqual([]);
    expect(readFiles('change-a').source).toBeUndefined();
  });
});

// ============================================================================
// 场景: 条目来源审计（agent_type）
// ============================================================================

describe('场景: 条目来源审计（agent_type）', () => {
  it('subagent 写入在 source 旁挂映射中记录 agent_type（AC-13）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));

    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'foo.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );

    const files = readFiles('change-a');
    expect(files.written).toEqual(['src/foo.ts']);
    expect(files.source).toEqual({ 'src/foo.ts': 'dev-team:implementation-generator' });
  });

  it('随后主会话（无 agent_type）重写同路径 → source 键清除，written 仍保留该路径（AC-13）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));
    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'foo.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );

    recordEvent(writeEvent(path.join(projectRoot, 'src', 'foo.ts'), 'S1'));

    const files = readFiles('change-a');
    expect(files.written).toEqual(['src/foo.ts']);
    expect(files.source).toBeUndefined();
  });

  it('revert 折叠移除条目后再次写入，source 仅反映最新写入者（last-writer-wins，AC-13）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));

    recordEvent(
      writeEvent(
        path.join(projectRoot, 'src', 'revert.ts'),
        'S1',
        'dev-team:implementation-generator',
      ),
    );
    recordEvent(bashEvent('git restore src/revert.ts', 'S1'));
    recordEvent(
      writeEvent(path.join(projectRoot, 'src', 'revert.ts'), 'S1', 'dev-team:test-gen-generator'),
    );

    const files = readFiles('change-a');
    expect(files.written).toEqual(['src/revert.ts']);
    expect(files.source).toEqual({ 'src/revert.ts': 'dev-team:test-gen-generator' });
  });
});

// ============================================================================
// 场景: Cursor 平台工具名事件（StrReplace / Shell）
// ============================================================================

describe('场景: Cursor 平台工具名事件（StrReplace / Shell）', () => {
  it('StrReplace 事件按 file_path 归 written，Shell 事件经 extractFileOps 归账（与 Write/Bash 同链路）', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent(phaseNextEvent('S1', 'change-a'));

    recordEvent(strReplaceEvent(path.join(projectRoot, 'src', 'cursor-edit.ts'), 'S1'));
    recordEvent(shellEvent('echo x > src/cursor-shell.ts', 'S1'));
    recordEvent(shellEvent('rm src/cursor-edit.ts', 'S1'));

    const files = readFiles('change-a');
    expect(files.written).toEqual(['src/cursor-shell.ts']);
    expect(files.deleted).toEqual(['src/cursor-edit.ts']);
  });

  it('cursorHome 的 phase_next MCP 全名（mcp__user-dev-team_mcp__phase_next）同样建绑并归账后续事件', () => {
    runChangeCreate('change-a', projectRoot, 'requirement');
    recordEvent({
      tool_name: 'mcp__user-dev-team_mcp__phase_next',
      tool_input: { change: 'change-a' },
      session_id: 'S9',
    });
    recordEvent(strReplaceEvent(path.join(projectRoot, 'src', 'home.ts'), 'S9'));

    expect(readFiles('change-a').written).toEqual(['src/home.ts']);
  });
});
