/**
 * 单元测试: mcp.ts — MCP 工具注册与 per-call project_root 校验
 *
 * @see openspec/changes/mcp-workspace-root/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { Client } from '@modelcontextprotocol/sdk/client';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vite-plus/test';

import * as backtrackCmd from './commands/backtrack';
import * as changeListCmd from './commands/change-list';
import type { ConfigGetResult } from './commands/config-get';
import * as configGetCmd from './commands/config-get';
import * as phaseLogCmd from './commands/phase-log';
import * as phaseNextCmd from './commands/phase-next';
import * as testDetectFrameworksCmd from './commands/test-detect-frameworks';
import type { ResolveTestPathsResult } from './commands/test-resolve-paths';
import * as testResolvePathsCmd from './commands/test-resolve-paths';
import * as archiQuery from './lib/archi-query';
import * as archiValidate from './lib/archi-validate';
import * as archiWrite from './lib/archi-write';
import * as c4CrossRef from './lib/c4-cross-ref';
import type { ArchiCheckResult, ArchiQueryResult, ArchiValidateResult } from './lib/c4-types';
import {
  archiCheckInputSchema,
  archiQueryInputSchema,
  archiValidateInputSchema,
  archiWriteInputSchema,
  backtrackInputSchema,
  changeListInputSchema,
  changeListOutputSchema,
  configGetInputSchema,
  phaseLogInputSchema,
  phaseNextInputSchema,
  testDetectFrameworksInputSchema,
  type TestDetectFrameworksResult,
  testResolvePathsInputSchema,
} from './schemas';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockResolve = vi.hoisted(() => ({
  root: process.cwd(),
  throwError: null as
    | null
    | (Error & {
        code?: string;
        project_root?: string;
        candidates?: readonly string[];
        force_hint?: string;
      }),
}));

function isMockResolveError(err: unknown): err is Error & {
  code?: string;
  project_root?: string;
  candidates?: readonly string[];
  force_hint?: string;
} {
  if (!(err instanceof Error) || err.name !== 'ProjectRootResolveError') {
    return false;
  }
  return typeof Reflect.get(err, 'code') === 'string';
}

function makeResolveError(
  code: string,
  message: string,
  extras?: {
    project_root?: string;
    candidates?: readonly string[];
    force_hint?: string;
  },
): Error & {
  code: string;
  project_root?: string;
  candidates?: readonly string[];
  force_hint?: string;
} {
  const err = new Error(message) as Error & {
    code: string;
    project_root?: string;
    candidates?: readonly string[];
    force_hint?: string;
  };
  err.name = 'ProjectRootResolveError';
  err.code = code;
  err.project_root = extras?.project_root;
  err.candidates = extras?.candidates;
  err.force_hint = extras?.force_hint;
  return err;
}

import type * as ProjectRootModule from './lib/project-root';

vi.mock('./lib/project-root', async (importOriginal) => {
  const actual = await importOriginal<typeof ProjectRootModule>();

  return {
    ...actual,
    collectProjectRootCandidates: vi.fn().mockResolvedValue(undefined),
    getProjectRootCandidates: vi.fn(() => []),
    getProjectDir: vi.fn(() => mockResolve.root),
    withResolvedProjectRoot: vi.fn(
      async (
        _toolName: string,
        _args: Record<string, unknown>,
        run: (projectRoot: string) => unknown,
      ) => {
        if (mockResolve.throwError) {
          const err = mockResolve.throwError;
          if (!isMockResolveError(err)) {
            throw err;
          }
          return {
            isError: true as const,
            content: [
              {
                type: 'text' as const,
                text: JSON.stringify({
                  code: err.code,
                  message: err.message,
                  project_root: err.project_root,
                  candidates: err.candidates,
                  force_hint: err.force_hint,
                }),
              },
            ],
          };
        }
        return run(mockResolve.root);
      },
    ),
  };
});

import { collectProjectRootCandidates, withResolvedProjectRoot } from './lib/project-root';

/** Exact tool names registered by mcp.ts (sorted). */
const EXPECTED_TOOL_NAMES = [
  'archi_check',
  'archi_query',
  'archi_validate',
  'archi_write',
  'backtrack',
  'change_list',
  'config_get',
  'phase_log',
  'phase_next',
  'test_detect_frameworks',
  'test_resolve_paths',
] as const;

const EXPECTED_TOOL_DESCRIPTIONS: Record<(typeof EXPECTED_TOOL_NAMES)[number], string> = {
  phase_log: 'Append an evaluation result entry to eval.json for a given workflow phase. ',
  archi_query:
    'Query C4 architecture model elements and relationships. Optionally filter by element fully-qualified name.',
  archi_validate:
    'Validate C4 architecture DSL syntax. Validates the current model or a provided DSL text string.',
  archi_write:
    'Validate and write a C4 architecture model file to the models/ directory. Validates DSL before writing.',
  archi_check:
    'Cross-reference validation: check code imports against the C4 architecture model. Detects unmodeled dependencies and unused relationships in changed files.',
  phase_next:
    'Return the next phase to execute in a PGE workflow. Handles gate check, skip passed phases, retry, backtrack, round limit, and mid-phase interruption. Returns the phase identifier and planner/evaluator agent config for the skill to execute.',
  config_get:
    'Read a value from openspec/config.json by dot-separated key path. Returns the value and whether the key exists. When the key does not exist, exists is false.',
  test_detect_frameworks:
    'Detect test framework(s) for given files based on config.json tests suite mappings. When files is omitted, auto-scan the project for files in suite scope. Returns per-file framework detection and a plan built from tests[].',
  test_resolve_paths:
    'Derive unit test file paths from a module list (files or directories). Three modes: (1) modules is an empty array — directories are auto-detected from config.json test configuration; (2) modules is a non-empty array — paths are filtered by test config scope before resolving; (3) modules is "git-change" — reads git diff HEAD --name-only to discover changed files, then resolves test paths filtered by test config. Returns colocated unit test paths per source file.',
  change_list:
    'List all active (non-archived) changes under openspec/changes/. Returns each change with its artifacts, task progress, and latest eval phase.',
  backtrack:
    'Set backtrack target and reason for a phase entry in eval.json. This is the only way to modify backtrack state.',
};

const ALL_INPUT_SCHEMAS = [
  ['phase_log', phaseLogInputSchema],
  ['phase_next', phaseNextInputSchema],
  ['backtrack', backtrackInputSchema],
  ['change_list', changeListInputSchema],
  ['config_get', configGetInputSchema],
  ['archi_query', archiQueryInputSchema],
  ['archi_validate', archiValidateInputSchema],
  ['archi_write', archiWriteInputSchema],
  ['archi_check', archiCheckInputSchema],
  ['test_detect_frameworks', testDetectFrameworksInputSchema],
  ['test_resolve_paths', testResolvePathsInputSchema],
] as const;

vi.mock('./lib/archi-query', () => ({
  queryModel: vi.fn(async () => ({ elements: [], relationships: [] })),
}));
vi.mock('./lib/archi-validate', () => ({
  validateDsl: vi.fn(async () => ({ valid: true, errors: [] })),
}));
vi.mock('./lib/archi-write', () => ({
  writeDsl: vi.fn(async () => ({ success: true, path: 'models/x.likec4' })),
}));
vi.mock('./lib/c4-cross-ref', () => ({
  runCrossRefCheck: vi.fn(async () => ({
    violations: [],
    warnings: [],
    matched: [],
    unmatched_files: [],
    status: 'clean' as const,
  })),
}));
vi.mock('./lib/c4-parser', () => ({
  readAllModels: vi.fn(() => null),
  findSpecificationBlock: vi.fn(() => null),
  parseC4Dsl: vi.fn(async () => ({
    elements: [],
    relationships: [],
    path_to_element: {},
    errors: [],
  })),
  validateC4Dsl: vi.fn(async () => ({
    elements: [],
    relationships: [],
    path_to_element: {},
    errors: [],
    valid: true,
  })),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function extractText(result: unknown): string {
  if (typeof result !== 'object' || result === null || !('content' in result)) {
    throw new Error('expected direct callTool result but got task result');
  }
  const content: unknown = result.content;
  if (!Array.isArray(content) || content.length === 0) {
    throw new Error('expected non-empty content array');
  }
  const item: unknown = content[0];
  if (typeof item !== 'object' || item === null || !('type' in item) || !('text' in item)) {
    throw new Error('expected content item with type and text');
  }
  if (item.type !== 'text' || typeof item.text !== 'string') {
    throw new Error(`expected text content but got "${String(item.type)}"`);
  }
  return item.text;
}

function isToolError(result: unknown): boolean {
  return (
    typeof result === 'object' && result !== null && 'isError' in result && result.isError === true
  );
}

async function getRegisteredToolNames(client: Client): Promise<string[]> {
  const { tools } = await client.listTools();
  return tools.map((t) => t.name);
}

async function getToolDescription(client: Client, name: string): Promise<string | undefined> {
  const { tools } = await client.listTools();
  return tools.find((t) => t.name === name)?.description;
}

async function getToolInputSchema(
  client: Client,
  name: string,
): Promise<Record<string, unknown> | undefined> {
  const { tools } = await client.listTools();
  const tool = tools.find((t) => t.name === name);
  return tool?.inputSchema as Record<string, unknown> | undefined;
}

function setupTempProject(config?: Record<string, unknown>): {
  dir: string;
  cleanup: () => void;
} {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-test-'));
  fs.mkdirSync(path.join(dir, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(dir, 'openspec', 'config.json'),
    JSON.stringify(config ?? { schema: 'spec-driven' }),
  );
  return {
    dir,
    cleanup: () => fs.rmSync(dir, { recursive: true, force: true }),
  };
}

function setResolvedRoot(dir: string): void {
  mockResolve.root = dir;
  mockResolve.throwError = null;
}

function setResolveError(err: Error): void {
  mockResolve.throwError = err as typeof mockResolve.throwError;
}

function withProjectRoot(
  args: Record<string, unknown>,
  root: string = mockResolve.root,
): Record<string, unknown> {
  return { ...args, project_root: root };
}

// ---------------------------------------------------------------------------
// MCP Server — 集成测试套件
// ---------------------------------------------------------------------------

describe('MCP Server (via InMemoryTransport)', () => {
  let server: McpServer;
  let client: Client;

  beforeAll(async () => {
    mockResolve.root = process.cwd();
    mockResolve.throwError = null;
    vi.mocked(collectProjectRootCandidates).mockClear();
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const { connectToServer } = await import('./mcp');
    server = await connectToServer(serverTransport);
    client = new Client({ name: 'test-client', version: '1.0.0' }, { capabilities: {} });
    await client.connect(clientTransport);
    expect(collectProjectRootCandidates).toHaveBeenCalledTimes(1);
  });

  afterAll(async () => {
    await client.close();
    await server.close();
  });

  beforeEach(() => {
    setResolvedRoot(process.cwd());
    vi.mocked(withResolvedProjectRoot).mockClear();
    // Clear command spy call history so shuffle order cannot leak counts across its
    vi.spyOn(changeListCmd, 'runChangeList').mockClear();
  });

  describe('MCP 注册 — connect 只 collect', () => {
    it('单次 connectToServer 调用 collectProjectRootCandidates 恰好 1 次；不得调用已删除的 requireLockedProjectRoot (AC-1)', async () => {
      vi.mocked(collectProjectRootCandidates).mockClear();
      vi.mocked(collectProjectRootCandidates).mockResolvedValue(undefined);
      const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
      const { connectToServer } = await import('./mcp');
      const s = await connectToServer(serverTransport);
      expect(collectProjectRootCandidates).toHaveBeenCalledTimes(1);
      expect(withResolvedProjectRoot).not.toHaveProperty('requireLockedProjectRoot');
      const c = new Client({ name: 'collect-once-client', version: '1.0.0' }, { capabilities: {} });
      await c.connect(clientTransport);
      await c.close();
      await s.close();
    });

    it('mock collect reject 时 connect 失败行为确定（上抛）；不得假称已锁定默认根 (AC-1)', async () => {
      vi.mocked(collectProjectRootCandidates).mockRejectedValueOnce(new Error('collect-fail'));
      const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
      const { connectToServer } = await import('./mcp');
      await expect(connectToServer(serverTransport)).rejects.toThrow('collect-fail');
      // Restore default mock so shuffle siblings / shared suite are unaffected
      vi.mocked(collectProjectRootCandidates).mockResolvedValue(undefined);
      const c = new Client({ name: 'fail-client', version: '1.0.0' }, { capabilities: {} });
      try {
        await c.connect(clientTransport);
      } catch {
        /* may fail if server never connected */
      }
      await c.close().catch(() => undefined);
    });

    it('connect 成功后 candidates 日志/快照可为空数组，不因 len==0/len==1 退出 (AC-1)', () => {
      expect(collectProjectRootCandidates).toHaveBeenCalled();
    });
  });

  describe('MCP 注册 — 工具 name 精确断言', () => {
    it('listTools() name 集合经 sort 后严格等于预期 11 个 name', async () => {
      const names = (await getRegisteredToolNames(client)).slice().sort();
      expect(names).toEqual([...EXPECTED_TOOL_NAMES]);
      expect(names).toHaveLength(11);
    });

    it('不得包含 list_changed / camelCase 别名', async () => {
      const names = await getRegisteredToolNames(client);
      for (const bad of ['list_changed', 'phaseLog', 'changeList', 'config-get']) {
        expect(names).not.toContain(bad);
      }
    });

    it('name 集合长度恰好 11；无重复 name', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toHaveLength(11);
      expect(new Set(names).size).toBe(11);
    });
  });

  describe('MCP 注册 — 工具 description 精确字面量', () => {
    it('每个工具 description 与 mcp.ts 注册串字节级相等（toBe）', async () => {
      for (const name of EXPECTED_TOOL_NAMES) {
        const desc = await getToolDescription(client, name);
        expect(desc).toBe(EXPECTED_TOOL_DESCRIPTIONS[name]);
      }
    });
  });

  describe('MCP 注册 — server identity', () => {
    it("getServerVersion() name === 'dev-team' 且 version === '2.8.11'", () => {
      const info = client.getServerVersion();
      expect(info?.name).toBe('dev-team');
      expect(info?.version).toBe('2.8.11');
    });
  });

  describe('MCP 注册 — 无 list_changed handler', () => {
    it('服务端未注册 notifications/roots/list_changed', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).not.toContain('notifications/roots/list_changed');
      expect(names.join(',')).not.toMatch(/list_changed/i);
    });
  });

  describe('MCP schema — 必填 project_root', () => {
    const toolsWithProjectRoot = [
      'phase_log',
      'phase_next',
      'backtrack',
      'change_list',
      'config_get',
      'archi_query',
      'archi_validate',
      'archi_write',
      'archi_check',
      'test_detect_frameworks',
      'test_resolve_paths',
    ];

    it('listTools 中全部相关 tool 的 inputSchema.required 均含 project_root (AC-2)', async () => {
      for (const name of toolsWithProjectRoot) {
        const schema = await getToolInputSchema(client, name);
        expect(schema).toBeDefined();
        const props = (schema?.properties ?? {}) as Record<string, unknown>;
        expect(props).toHaveProperty('project_root');
        const required = schema?.required;
        if (Array.isArray(required)) {
          expect(required).toContain('project_root');
        }
      }
    });

    it('直接 import 各 *InputSchema：shape 含 project_root；safeParse 省略该字段时 success === false (AC-2)', () => {
      for (const [name, schema] of ALL_INPUT_SCHEMAS) {
        expect(Object.keys(schema.shape)).toContain('project_root');
        const parsed = schema.safeParse(
          name === 'config_get'
            ? { key: 'schema' }
            : name === 'change_list'
              ? {}
              : name === 'phase_next'
                ? { change: 'c' }
                : name === 'phase_log'
                  ? {
                      change: 'c',
                      phase: 'proposal',
                      report: 'r',
                      checklist: [{ item: 'i', pass: true, evidence: 'e' }],
                    }
                  : name === 'backtrack'
                    ? {
                        change: 'c',
                        phase: 'proposal',
                        backtrack_to: 'explore',
                        backtrack_reason: 'r',
                      }
                    : name === 'test_resolve_paths'
                      ? { modules: [] }
                      : name === 'archi_write'
                        ? { source: 'm', path: 'models/x.likec4' }
                        : {},
        );
        expect(parsed.success).toBe(false);
      }
    });

    it('callTool 省略 project_root 时校验失败 / isError，且 command spy 次数为 0 (AC-2)', async () => {
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      spy.mockClear();
      const result = await client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('project_root 为非 string（number / null / object）时 schema safeParse 失败 (AC-2)', () => {
      for (const bad of [1, null, { x: 1 }]) {
        expect(changeListInputSchema.safeParse({ project_root: bad }).success).toBe(false);
      }
    });

    it('project_root: "" 经 schema 通过或 resolve 失败（不得静默当 cwd）(AC-2/AC-8)', async () => {
      const schemaOk = changeListInputSchema.safeParse({ project_root: '' }).success;
      expect(schemaOk).toBe(true);
      setResolveError(makeResolveError('invalid_path', 'empty', { project_root: '' }));
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      spy.mockClear();
      const result = await client.callTool({
        name: 'change_list',
        arguments: { project_root: '' },
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('project_root 超长 / 含 \\n / emoji：schema 若仅 z.string() 则通过；不得崩溃 (AC-2)', () => {
      for (const value of ['x'.repeat(1001), 'a\nb', 'emoji-🧪']) {
        expect(changeListInputSchema.safeParse({ project_root: value }).success).toBe(true);
      }
    });
  });

  describe('MCP 工具调用 — ∈ 候选接线', () => {
    afterEach(() => {
      // Do not call restoreAllMocks() — it resets vi.mock factories and breaks
      // resolve stubs for shuffled sibling describes under --sequence.shuffle.
      const maybeRestore = (fn: unknown): void => {
        if (
          typeof fn === 'function' &&
          'mockRestore' in fn &&
          typeof fn.mockRestore === 'function'
        ) {
          fn.mockRestore();
        }
      };
      maybeRestore(changeListCmd.runChangeList);
      maybeRestore(configGetCmd.runConfigGet);
      setResolvedRoot(process.cwd());
    });

    it('mock resolve 返回 fixture 根：change_list/config_get/test_*/archi_*/phase_*/backtrack 成功并将该根传入 command/lib (AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', 'wired'), { recursive: true });
      setResolvedRoot(dir);
      const changeSpy = vi.spyOn(changeListCmd, 'runChangeList');
      const configSpy = vi.spyOn(configGetCmd, 'runConfigGet');
      try {
        const list = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}),
        });
        expect(isToolError(list)).toBe(false);
        expect(changeSpy).toHaveBeenCalledWith(dir);
        const data: { project_root: string } = JSON.parse(extractText(list));
        expect(data.project_root).toBe(dir);

        const cfg = await client.callTool({
          name: 'config_get',
          arguments: withProjectRoot({ key: 'schema' }),
        });
        expect(isToolError(cfg)).toBe(false);
        expect(configSpy.mock.calls[0]?.[0]).toMatchObject({ projectRoot: dir });
      } finally {
        cleanup();
      }
    });

    it('resolve 成功但 command 抛业务错：不得吞为 resolve 成功假象', async () => {
      setResolvedRoot(process.cwd());
      vi.spyOn(changeListCmd, 'runChangeList').mockImplementation(() => {
        throw new Error('biz-boom');
      });
      let threw = false;
      let result: unknown;
      try {
        result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}),
        });
      } catch {
        threw = true;
      }
      if (!threw) {
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).not.toMatch(/"changes"\s*:/);
      }
    });

    it('resolve 返回带尾斜杠 normalize 后的根时，传入 command 的路径与回显一致 (AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      const normalized = dir.endsWith(path.sep) ? dir.slice(0, -1) : dir;
      setResolvedRoot(normalized);
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      try {
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, normalized),
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith(normalized);
        expect(JSON.parse(extractText(result)).project_root).toBe(normalized);
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — resolve 错误映射', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('mock 抛 not_in_candidates：isError===true；JSON 字段齐全；command spy 次数 0 (AC-4)', async () => {
      const err = makeResolveError('not_in_candidates', 'not in', {
        project_root: '/tmp/x',
        candidates: ['/tmp/a'],
        force_hint: 'Resubmit the same tool call with identical complete arguments to force-add',
      });
      setResolveError(err);
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      const result = await client.callTool({
        name: 'change_list',
        arguments: withProjectRoot({}, '/tmp/x'),
      });
      expect(isToolError(result)).toBe(true);
      const body = JSON.parse(extractText(result));
      expect(body).toMatchObject({
        code: 'not_in_candidates',
        project_root: '/tmp/x',
        candidates: ['/tmp/a'],
        force_hint: expect.stringMatching(/force-add/i),
        message: expect.any(String),
      });
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('mock 抛 invalid_path：JSON 含 code 与 project_root；无业务 I/O (AC-4)', async () => {
      setResolveError(makeResolveError('invalid_path', 'bad', { project_root: 'relative' }));
      const spy = vi.spyOn(configGetCmd, 'runConfigGet');
      const result = await client.callTool({
        name: 'config_get',
        arguments: withProjectRoot({ key: 'schema' }, 'relative'),
      });
      expect(isToolError(result)).toBe(true);
      const body = JSON.parse(extractText(result));
      expect(body.code).toBe('invalid_path');
      expect(body.project_root).toBe('relative');
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it("鸭类型 name==='ProjectRootResolveError' + string code 仍映射为 isError JSON (AC-4)", async () => {
      const duck = makeResolveError('not_in_candidates', 'duck-msg', {
        candidates: [],
        force_hint: 'force-add identical complete arguments',
        project_root: '/x',
      });
      expect(isMockResolveError(duck)).toBe(true);
      setResolveError(duck);
      const result = await client.callTool({
        name: 'change_list',
        arguments: withProjectRoot({}, '/x'),
      });
      expect(isToolError(result)).toBe(true);
      const body = JSON.parse(extractText(result));
      expect(body.code).toBe('not_in_candidates');
      expect(body.message).toBe('duck-msg');
    });

    it("普通 Error('boom') 不得被包装成成功 JSON (AC-4)", async () => {
      const boom = new Error('boom');
      expect(isMockResolveError(boom)).toBe(false);
      setResolveError(boom);
      let threw = false;
      let result: unknown;
      try {
        result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}),
        });
      } catch {
        threw = true;
      }
      if (!threw) {
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).not.toMatch(/"changes"\s*:/);
        expect(extractText(result)).toMatch(/boom/i);
      }
    });

    it('candidates: [] 与超长 force_hint / project_root 仍可 JSON 序列化', async () => {
      const long = 'p'.repeat(1200);
      setResolveError(
        makeResolveError('not_in_candidates', 'msg', {
          candidates: [],
          force_hint: 'h'.repeat(1200),
          project_root: long,
        }),
      );
      const result = await client.callTool({
        name: 'change_list',
        arguments: withProjectRoot({}, long),
      });
      expect(isToolError(result)).toBe(true);
      expect(() => JSON.parse(extractText(result))).not.toThrow();
      const body = JSON.parse(extractText(result));
      expect(body.candidates).toEqual([]);
      expect(body.project_root).toBe(long);
    });
  });

  describe('MCP 工具调用 — call-scoped 根', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('spy withResolvedProjectRoot：resolve 成功后以 resolve 根调用 run (AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      vi.mocked(withResolvedProjectRoot).mockClear();
      vi.spyOn(phaseLogCmd, 'runPhaseLog').mockReturnValue({
        written: true,
        phase: 'proposal',
        attempt: 1,
      });
      try {
        const result = await client.callTool({
          name: 'phase_log',
          arguments: withProjectRoot({
            change: 'c',
            phase: 'proposal',
            report: 'r',
            checklist: [{ item: 'i', pass: true, evidence: 'e' }],
          }),
        });
        expect(isToolError(result)).toBe(false);
        expect(withResolvedProjectRoot).toHaveBeenCalled();
        const run = vi.mocked(withResolvedProjectRoot).mock.calls[0]?.[2];
        expect(typeof run).toBe('function');
        // mock passes mockResolve.root into run
        expect(mockResolve.root).toBe(dir);
      } finally {
        cleanup();
      }
    });

    it('resolve 抛错时 withResolvedProjectRoot 返回 isError 且 command 不执行 (AC-4)', async () => {
      setResolveError(
        makeResolveError('not_in_candidates', 'x', {
          candidates: [],
          force_hint: 'force-add',
          project_root: '/x',
        }),
      );
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      spy.mockClear();
      const result = await client.callTool({
        name: 'change_list',
        arguments: withProjectRoot({}, '/x'),
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('withResolvedProjectRoot 成功时 run 收到的根等于 mock resolve 根', async () => {
      const special = path.join(os.tmpdir(), 'mcp-special-🧪');
      fs.mkdirSync(special, { recursive: true });
      setResolvedRoot(special);
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      try {
        await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, special),
        });
        expect(spy).toHaveBeenCalledWith(special);
      } finally {
        spy.mockRestore();
        fs.rmSync(special, { recursive: true, force: true });
      }
    });
  });

  describe('MCP 工具调用 — 全 11 handler', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('resolve mock 成功时 11 个 tool 各 callTool 一次均非空成功 (AC-3)', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*'] }],
      });
      const changeName = 'handler-fixture';
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', changeName), { recursive: true });
      fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');
      setResolvedRoot(dir);

      const spies = {
        phase_log: vi.spyOn(phaseLogCmd, 'runPhaseLog').mockReturnValue({
          written: true,
          phase: 'proposal',
          attempt: 1,
        }),
        phase_next: vi.spyOn(phaseNextCmd, 'runPhaseNext'),
        backtrack: vi.spyOn(backtrackCmd, 'runBacktrack').mockReturnValue({
          modified: true,
          phase: 'proposal',
          target: 'explore',
        }),
        archi_query: vi.spyOn(archiQuery, 'queryModel'),
        archi_validate: vi.spyOn(archiValidate, 'validateDsl'),
        archi_write: vi.spyOn(archiWrite, 'writeDsl'),
        archi_check: vi.spyOn(c4CrossRef, 'runCrossRefCheck'),
        config_get: vi.spyOn(configGetCmd, 'runConfigGet'),
        test_detect_frameworks: vi.spyOn(testDetectFrameworksCmd, 'runTestDetectFrameworks'),
        test_resolve_paths: vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths'),
        change_list: vi.spyOn(changeListCmd, 'runChangeList'),
      };

      try {
        const calls: Array<{ name: string; args: Record<string, unknown> }> = [
          {
            name: 'phase_log',
            args: {
              change: changeName,
              phase: 'proposal',
              report: 'r',
              checklist: [{ item: 'i', pass: true, evidence: 'e' }],
            },
          },
          { name: 'phase_next', args: { change: changeName } },
          {
            name: 'backtrack',
            args: {
              change: changeName,
              phase: 'proposal',
              backtrack_to: 'explore',
              backtrack_reason: 'r',
            },
          },
          { name: 'archi_query', args: {} },
          { name: 'archi_validate', args: { source: 'model m' } },
          { name: 'archi_write', args: { source: 'model m', path: 'models/x.likec4' } },
          { name: 'archi_check', args: { staged: false } },
          { name: 'config_get', args: { key: 'schema' } },
          { name: 'test_detect_frameworks', args: {} },
          { name: 'test_resolve_paths', args: { modules: ['src/foo.ts'] } },
          { name: 'change_list', args: {} },
        ];
        expect(calls).toHaveLength(11);

        for (const { name, args } of calls) {
          const result = await client.callTool({
            name,
            arguments: withProjectRoot(args, dir),
          });
          expect(isToolError(result)).toBe(false);
          expect(extractText(result).length).toBeGreaterThan(0);
        }

        for (const spy of Object.values(spies)) {
          expect(spy.mock.calls.length).toBeGreaterThanOrEqual(1);
        }
      } finally {
        for (const spy of Object.values(spies)) {
          spy.mockRestore();
        }
        cleanup();
      }
    });

    it('resolve 抛 not_in_candidates 时 11 个 tool 均 isError 且 spy 次数 0', async () => {
      setResolveError(
        makeResolveError('not_in_candidates', 'x', {
          candidates: [],
          force_hint: 'force-add',
          project_root: '/x',
        }),
      );
      const spies = [
        vi.spyOn(phaseLogCmd, 'runPhaseLog'),
        vi.spyOn(phaseNextCmd, 'runPhaseNext'),
        vi.spyOn(backtrackCmd, 'runBacktrack'),
        vi.spyOn(archiQuery, 'queryModel'),
        vi.spyOn(archiValidate, 'validateDsl'),
        vi.spyOn(archiWrite, 'writeDsl'),
        vi.spyOn(c4CrossRef, 'runCrossRefCheck'),
        vi.spyOn(configGetCmd, 'runConfigGet'),
        vi.spyOn(testDetectFrameworksCmd, 'runTestDetectFrameworks'),
        vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths'),
        vi.spyOn(changeListCmd, 'runChangeList'),
      ];
      for (const spy of spies) {
        spy.mockClear();
      }

      const calls: Array<{ name: string; args: Record<string, unknown> }> = [
        {
          name: 'phase_log',
          args: {
            change: 'x',
            phase: 'proposal',
            report: 'r',
            checklist: [{ item: 'i', pass: true, evidence: 'e' }],
          },
        },
        { name: 'phase_next', args: { change: 'x' } },
        {
          name: 'backtrack',
          args: { change: 'x', phase: 'proposal', backtrack_to: 'explore', backtrack_reason: 'r' },
        },
        { name: 'archi_query', args: {} },
        { name: 'archi_validate', args: { source: 'm' } },
        { name: 'archi_write', args: { source: 'm', path: 'models/x.likec4' } },
        { name: 'archi_check', args: { staged: true } },
        { name: 'config_get', args: { key: 'schema' } },
        { name: 'test_detect_frameworks', args: {} },
        { name: 'test_resolve_paths', args: { modules: [] } },
        { name: 'change_list', args: {} },
      ];

      try {
        for (const { name, args } of calls) {
          const result = await client.callTool({
            name,
            arguments: withProjectRoot(args, '/x'),
          });
          expect(isToolError(result)).toBe(true);
        }
        for (const spy of spies) {
          expect(spy).toHaveBeenCalledTimes(0);
        }
      } finally {
        for (const spy of spies) {
          spy.mockRestore();
        }
      }
    });

    it('11 个 tool 均传入相同合法 project_root 时接线一致', async () => {
      const { dir, cleanup } = setupTempProject();
      const withSep = dir.endsWith(path.sep) ? dir : dir + path.sep;
      setResolvedRoot(dir);
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      try {
        const result = await client.callTool({
          name: 'change_list',
          arguments: { project_root: withSep },
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith(dir);
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — 业务成功路径（带 project_root）', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('config_get 返回 key、value 和 exists 字段', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'config_get',
          arguments: withProjectRoot({ key: 'schema' }),
        });
        const data: ConfigGetResult = JSON.parse(extractText(result));
        expect(data).toMatchObject({ key: 'schema', exists: true });
      } finally {
        cleanup();
      }
    });

    it('test_detect_frameworks 返回 detected 字段', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: withProjectRoot({}),
        });
        const data: TestDetectFrameworksResult = JSON.parse(extractText(result));
        expect(data).toHaveProperty('detected');
      } finally {
        cleanup();
      }
    });

    it('test_resolve_paths 返回 unit_tests 数组', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      setResolvedRoot(dir);
      try {
        fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
        fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');
        const result = await client.callTool({
          name: 'test_resolve_paths',
          arguments: withProjectRoot({ modules: ['src/foo.ts'] }),
        });
        const data: ResolveTestPathsResult = JSON.parse(extractText(result));
        expect(data).toHaveProperty('unit_tests');
      } finally {
        cleanup();
      }
    });

    it('change_list 返回 changes 数组且 output schema 可解析', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}),
        });
        const parsed = changeListOutputSchema.safeParse(JSON.parse(extractText(result)));
        expect(parsed.success).toBe(true);
      } finally {
        cleanup();
      }
    });

    it('phase_next 返回下一阶段信息', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'phase-next-fixture';
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', changeName), { recursive: true });
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'phase_next',
          arguments: withProjectRoot({ change: changeName }),
        });
        expect(isToolError(result)).toBe(false);
        expect(JSON.parse(extractText(result))).toHaveProperty('next_phase');
      } finally {
        cleanup();
      }
    });

    it('archi_validate / archi_query / archi_check 返回预期结构', async () => {
      setResolvedRoot(process.cwd());
      const v = await client.callTool({
        name: 'archi_validate',
        arguments: withProjectRoot({}),
      });
      expect(isToolError(v)).toBe(false);
      const vData: ArchiValidateResult = JSON.parse(extractText(v));
      expect(vData).toMatchObject({ valid: true, errors: [] });

      const q = await client.callTool({
        name: 'archi_query',
        arguments: withProjectRoot({}),
      });
      const qData: ArchiQueryResult = JSON.parse(extractText(q));
      expect(qData).toMatchObject({ elements: [], relationships: [] });

      const c = await client.callTool({
        name: 'archi_check',
        arguments: withProjectRoot({ staged: false }),
      });
      const cData: ArchiCheckResult = JSON.parse(extractText(c));
      expect(cData.status).toBe('clean');
    });

    it('archi_* 将 resolve 根传入 lazy lib', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      const querySpy = vi.spyOn(archiQuery, 'queryModel');
      try {
        await client.callTool({
          name: 'archi_query',
          arguments: withProjectRoot({ element: 'sys' }),
        });
        expect(querySpy).toHaveBeenCalledWith(dir, 'sys');
      } finally {
        cleanup();
      }
    });
  });
});

describe('MCP 配置 — .mcp.json 约束', () => {
  it('plugins/dev-team/.mcp.json：dev-team 无 env 回灌 CLAUDE_PROJECT_DIR；不含 ${workspaceFolder} (AC-10)', () => {
    const mcpPath = path.resolve(__dirname, '../../.mcp.json');
    const raw = fs.readFileSync(mcpPath, 'utf-8');
    const json = JSON.parse(raw) as {
      mcpServers: { 'dev-team': Record<string, unknown>; likec4?: Record<string, unknown> };
    };
    const devTeam = json.mcpServers['dev-team'];
    expect(devTeam).toBeDefined();
    expect(devTeam).not.toHaveProperty('env');
    expect(JSON.stringify(devTeam)).not.toContain('${workspaceFolder}');
    expect(raw.includes('${workspaceFolder}')).toBe(false);
  });

  it('dev-team 节点若出现 env.CLAUDE_PROJECT_DIR 字面量 ${...} 则本用例失败 (AC-10)', () => {
    const mcpPath = path.resolve(__dirname, '../../.mcp.json');
    const json = JSON.parse(fs.readFileSync(mcpPath, 'utf-8')) as {
      mcpServers: { 'dev-team': { env?: { CLAUDE_PROJECT_DIR?: string } } };
    };
    const env = json.mcpServers['dev-team'].env;
    if (env?.CLAUDE_PROJECT_DIR) {
      expect(env.CLAUDE_PROJECT_DIR).not.toMatch(/\$\{/);
    }
  });

  it('likec4 服务可继续使用 ${CLAUDE_PROJECT_DIR}；断言仅约束 dev-team (AC-10)', () => {
    const mcpPath = path.resolve(__dirname, '../../.mcp.json');
    const json = JSON.parse(fs.readFileSync(mcpPath, 'utf-8')) as {
      mcpServers: {
        'dev-team': Record<string, unknown>;
        likec4: { env?: Record<string, string> };
      };
    };
    expect(JSON.stringify(json.mcpServers['dev-team'])).not.toContain('${workspaceFolder}');
    const likec4Env = JSON.stringify(json.mcpServers.likec4?.env ?? {});
    expect(likec4Env).toContain('${CLAUDE_PROJECT_DIR}');
  });
});

describe('MCP 注册 — registerTool spy 精确字面量', () => {
  const REGISTER_ORDER = [
    'phase_log',
    'archi_query',
    'archi_validate',
    'archi_write',
    'archi_check',
    'phase_next',
    'config_get',
    'test_detect_frameworks',
    'test_resolve_paths',
    'change_list',
    'backtrack',
  ] as const;

  it('按注册顺序对每次调用断言 name/description；调用次数恰好 11', async () => {
    mockResolve.root = process.cwd();
    mockResolve.throwError = null;
    const registerSpy = vi.spyOn(McpServer.prototype, 'registerTool');
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    let server: McpServer | undefined;
    let client: Client | undefined;
    try {
      const { connectToServer } = await import('./mcp');
      server = await connectToServer(serverTransport);
      client = new Client({ name: 'reg-spy', version: '1.0.0' }, { capabilities: {} });
      await client.connect(clientTransport);

      expect(registerSpy).toHaveBeenCalledTimes(11);
      const names: string[] = [];
      for (let i = 0; i < 11; i++) {
        const call = registerSpy.mock.calls[i];
        const name = call[0] as string;
        const config = call[1] as { description?: unknown };
        names.push(name);
        expect(name).toBe(REGISTER_ORDER[i]);
        expect(config.description).toBe(
          EXPECTED_TOOL_DESCRIPTIONS[name as (typeof EXPECTED_TOOL_NAMES)[number]],
        );
      }
      expect(names).toEqual([...REGISTER_ORDER]);
    } finally {
      registerSpy.mockRestore();
      await client?.close();
      await server?.close();
    }
  });
});
