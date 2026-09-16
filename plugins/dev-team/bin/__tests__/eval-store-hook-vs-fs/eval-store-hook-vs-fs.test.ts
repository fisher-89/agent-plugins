/**
 * 集成测试: PreToolUse deny → Node fs 写 workflow.json
 *
 * Hook 与 MCP 不共享写入通道：agent 工具调用经 `runProtectFiles`，MCP 经 Node
 * `fs`。本套件对同一相对路径分别走 stdin Write 与 `writeEvalJson` /
 * `runChangeCreate`，证明 deny 不阻止进程内落盘；同时对 `eval.json` 断言
 * 「内置规则放行」与「MCP 侧迁移删除」并存，避免出现「agent 放行、MCP 却不再
 * 清理」的双源残留。
 *
 * 涉及模块:
 * - `plugins/dev-team/bin/src/hooks.ts` — PreToolUse 拦截
 * - `plugins/dev-team/bin/src/lib/eval-json.ts` — MCP 路径：进程内 fs 写
 * - `plugins/dev-team/bin/src/commands/change-create.ts` — MCP 路径：创建
 *
 * 关联 AC: AC-8, AC-9
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { getProjectDir } from '../../src/lib/project-root';

// ---------------------------------------------------------------------------
// hook stdin / stdout harness
// ---------------------------------------------------------------------------

const { stdinRef, mockReadConfig } = vi.hoisted(() => ({
  stdinRef: { current: '' },
  mockReadConfig: vi.fn(),
}));

// 只替换 fd 0（stdin）的读取；路径读取委托给真实实现，保证 eval-json 的落盘不受影响。
vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return {
    ...actual,
    readFileSync: ((...args: unknown[]) => {
      if (args[0] === 0) {
        return stdinRef.current;
      }
      return (actual.readFileSync as (...rest: unknown[]) => unknown)(...args);
    }) as typeof actual.readFileSync,
  };
});

vi.mock('../../src/lib/config', () => ({
  readConfig: mockReadConfig,
}));

// `runProtectFiles` → `loadPatterns(getProjectDir())` 需要工程根。这里把
// `getProjectDir` mock 成临时工程根，替代 `process.chdir`：Stryker 的 vitest runner
// 强制 worker 线程池，worker 中 `process.chdir` 会抛
// ERR_WORKER_UNSUPPORTED_OPERATION（与 run-static-analysis.test.ts 相同的处理）。
vi.mock('../../src/lib/project-root');

// 拦截进程退出，避免模块顶层 main() 因未知子命令终止测试进程
const exitMock = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
vi.spyOn(process.stderr, 'write').mockImplementation(() => true);

const { runProtectFiles } = await import('../../src/hooks');
const { runChangeCreate } = await import('../../src/commands/change-create');
const { readEvalJson, writeEvalJson } = await import('../../src/lib/eval-json');

const stdoutWriteMock = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

let projectRoot: string;
let changeDir: string;

type HookOutput = {
  hookSpecificOutput: { permissionDecision: string; permissionDecisionReason: string };
};

/** 通过 stdin 提交一次工具调用并返回解析后的 hook 输出。 */
function runHook(payload: Record<string, unknown>): HookOutput {
  stdinRef.current = JSON.stringify(payload);
  stdoutWriteMock.mockClear();
  runProtectFiles();
  return JSON.parse(String(stdoutWriteMock.mock.calls[0]?.[0] ?? '')) as HookOutput;
}

beforeEach(() => {
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'hook-vs-fs-'));
  changeDir = path.join(projectRoot, 'openspec', 'changes', 'demo');
  fs.mkdirSync(changeDir, { recursive: true });
  vi.mocked(getProjectDir).mockReturnValue(projectRoot);

  exitMock.mockClear();
  stdoutWriteMock.mockClear();
  mockReadConfig.mockReset().mockReturnValue({ schema: 'spec-driven' });
  stdinRef.current = '';
});

afterEach(() => {
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

// ---------------------------------------------------------------------------
// 场景: 工具 deny 与 fs 成功并存
// ---------------------------------------------------------------------------

describe('工具 deny 与 fs 成功并存', () => {
  it('Write workflow.json 时 hook deny，同时 writeEvalJson 在临时 change 目录写成功（AC-8）', () => {
    fs.writeFileSync(
      path.join(changeDir, 'workflow.json'),
      JSON.stringify({ workflow_type: 'requirement', created: '2026-09-11' }),
      'utf-8',
    );

    const hookOutput = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/demo/workflow.json' },
    });
    expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(hookOutput.hookSpecificOutput.permissionDecisionReason).toContain('workflow.json');

    // 进程内 fs 不受 PreToolUse 限制
    writeEvalJson(changeDir, [
      {
        phase: 'proposal',
        verdict: 'pass',
        attempt: 1,
        timestamp: '2026-09-11T10:00:00.000Z',
        report: 'ok',
        checklist: [],
        backtrack_to: null,
      },
    ]);

    expect(readEvalJson(changeDir)).toHaveLength(1);
  });

  it('runChangeCreate 仍能写 workflow.json（进程内 fs 不受 hook 限制）（AC-8）', () => {
    const hookOutput = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/brand-new/workflow.json' },
    });
    expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('deny');

    const result = runChangeCreate('brand-new', projectRoot, 'test-only');

    const doc = JSON.parse(
      fs.readFileSync(path.join(result.path, 'workflow.json'), 'utf-8'),
    ) as Record<string, unknown>;
    expect(doc.workflow_type).toBe('test-only');
    expect(readEvalJson(result.path)).toEqual([]);
  });

  it('Shell 重定向写 workflow.json 时 deny，且不影响随后的 writeEvalJson', () => {
    fs.writeFileSync(
      path.join(changeDir, 'workflow.json'),
      JSON.stringify({ workflow_type: 'requirement', created: '2026-09-11' }),
      'utf-8',
    );

    const hookOutput = runHook({
      tool_name: 'Bash',
      tool_input: { command: 'echo "{}" > openspec/changes/demo/workflow.json' },
    });
    expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('deny');

    expect(() =>
      writeEvalJson(changeDir, [
        {
          phase: 'dev-design',
          verdict: 'pass',
          attempt: 1,
          timestamp: '2026-09-11T10:00:00.000Z',
          report: 'ok',
          checklist: [],
          backtrack_to: null,
        },
      ]),
    ).not.toThrow();
    expect(readEvalJson(changeDir)).toHaveLength(1);
  });

  it('StrReplace workflow.json 时 deny，Edit 对照同样 deny', () => {
    for (const toolName of ['StrReplace', 'Edit']) {
      const hookOutput = runHook({
        tool_name: toolName,
        tool_input: { file_path: 'openspec/changes/demo/workflow.json' },
      });

      expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('deny');
      expect(hookOutput.hookSpecificOutput.permissionDecisionReason).toContain(toolName);
    }
  });

  it('config.json 同批仍 deny（内置集合未整体放宽）（AC-8）', () => {
    const hookOutput = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/config.json' },
    });

    expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(hookOutput.hookSpecificOutput.permissionDecisionReason).toContain(
      'openspec/config.json',
    );
  });
});

// ---------------------------------------------------------------------------
// 场景: eval.json 内置豁免与 MCP 迁移删除并存
// ---------------------------------------------------------------------------

describe('eval.json 内置豁免与 MCP 迁移删除并存', () => {
  beforeEach(() => {
    fs.writeFileSync(
      path.join(changeDir, 'workflow.json'),
      JSON.stringify({ workflow_type: 'requirement', created: '2026-09-11' }),
      'utf-8',
    );
  });

  it('无 write_protection 时 Write eval.json 不产生内置 deny（AC-9）', () => {
    const hookOutput = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/demo/eval.json' },
    });

    expect(hookOutput.hookSpecificOutput.permissionDecision).not.toBe('deny');
    expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('allow');
  });

  it('同一批次再 writeEvalJson 时遗留 eval.json 被删除，条目落在 workflow.json.eval（AC-4、AC-9）', () => {
    fs.writeFileSync(
      path.join(changeDir, 'eval.json'),
      JSON.stringify([
        {
          phase: 'proposal',
          verdict: 'pass',
          attempt: 1,
          timestamp: '2026-09-11T09:00:00.000Z',
          report: '遗留条目',
          checklist: [],
          backtrack_to: null,
        },
      ]),
      'utf-8',
    );

    const hookOutput = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/demo/eval.json' },
    });
    expect(hookOutput.hookSpecificOutput.permissionDecision).not.toBe('deny');

    writeEvalJson(changeDir, [
      {
        phase: 'proposal',
        verdict: 'pass',
        attempt: 1,
        timestamp: '2026-09-11T10:00:00.000Z',
        report: '迁移后条目',
        checklist: [],
        backtrack_to: null,
      },
    ]);

    expect(fs.existsSync(path.join(changeDir, 'eval.json'))).toBe(false);
    const entries = readEvalJson(changeDir);
    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('迁移后条目');
  });

  it('write_protection.files 显式加入 eval.json glob 后同一 stdin 变为 deny', () => {
    mockReadConfig.mockReturnValue({
      schema: 'spec-driven',
      write_protection: {
        files: [{ glob: 'openspec/changes/*/eval.json', reason: '用户保护：%s（%t）' }],
      },
    });

    const hookOutput = runHook({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/demo/eval.json' },
    });

    expect(hookOutput.hookSpecificOutput.permissionDecision).toBe('deny');
    expect(hookOutput.hookSpecificOutput.permissionDecisionReason).toContain('用户保护');
  });
});
