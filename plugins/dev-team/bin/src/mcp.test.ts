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
import { toJSONSchema } from 'zod/v4';

import * as backtrackCmd from './commands/backtrack';
import * as changeCreateCmd from './commands/change-create';
import * as changeFilesCmd from './commands/change-files';
import * as changeListCmd from './commands/change-list';
import type { ConfigGetResult } from './commands/config-get';
import * as configGetCmd from './commands/config-get';
import * as phaseLogCmd from './commands/phase-log';
import * as phaseNextCmd from './commands/phase-next';
import * as specListCmd from './commands/spec-list';
import * as testDetectFrameworksCmd from './commands/test-detect-frameworks';
import type { ResolveTestPathsResult } from './commands/test-resolve-paths';
import * as testResolvePathsCmd from './commands/test-resolve-paths';
import * as archiQuery from './lib/archi-query';
import * as archiValidate from './lib/archi-validate';
import * as archiWrite from './lib/archi-write';
import * as c4CrossRef from './lib/c4-cross-ref';
import type { ArchiCheckResult, ArchiQueryResult, ArchiValidateResult } from './lib/c4-types';
import { getPhaseTable } from './lib/workflow';
import * as workflowFilesCmd from './modules/workflow';
import {
  archiCheckInputSchema,
  archiDecideInputSchema,
  archiDecideOutputSchema,
  archiQueryInputSchema,
  archiValidateInputSchema,
  archiWriteInputSchema,
  backtrackInputSchema,
  changeCreateInputSchema,
  changeCreateOutputSchema,
  changeFilesInputSchema,
  changeFilesOutputSchema,
  changeListInputSchema,
  changeListOutputSchema,
  configGetInputSchema,
  phaseLogInputSchema,
  phaseNextInputSchema,
  specListInputSchema,
  specListOutputSchema,
  testDetectFrameworksInputSchema,
  type TestDetectFrameworksResult,
  testResolvePathsInputSchema,
  workflowFilesInputSchema,
  workflowFilesOutputSchema,
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
  'archi_decide',
  'archi_query',
  'archi_validate',
  'archi_write',
  'backtrack',
  'change_create',
  'change_files',
  'change_list',
  'config_get',
  'phase_log',
  'phase_next',
  'spec_list',
  'test_detect_frameworks',
  'test_resolve_paths',
  'workflow_files',
] as const;

const EXPECTED_TOOL_DESCRIPTIONS: Record<(typeof EXPECTED_TOOL_NAMES)[number], string> = {
  phase_log:
    'Append an evaluation result entry to the eval field of workflow.json for a given workflow phase. ',
  archi_query:
    'Query C4 architecture model elements and relationships. Optionally filter by element fully-qualified name.',
  archi_validate:
    'Validate C4 architecture DSL syntax. Validates the current model or a provided DSL text string.',
  archi_write:
    'Validate and write a C4 architecture model file to the models/ directory. Validates DSL before writing.',
  archi_check:
    'Cross-reference validation: check code imports against the C4 architecture model. Detects unmodeled dependencies and unused relationships in changed files. The checked file set comes from an explicit files list or the target change file inventory (workflow.json files.written).',
  archi_decide:
    'Create, list, and update Architecture Decision Records (ADRs) under openspec/architecture/decisions/. Use action create, list, or update.',
  phase_next:
    'Return the next phase to execute in a PGE workflow. Handles gate check, skip passed phases, retry, backtrack, round limit, and mid-phase interruption. Returns the phase identifier and planner/evaluator agent config for the skill to execute.',
  config_get:
    'Read a value from openspec/config.json by dot-separated key path. Returns the value and whether the key exists. When the key does not exist, exists is false.',
  test_detect_frameworks:
    'Detect test framework(s) for given files based on config.json tests suite mappings. When files is omitted, auto-scan the project for files in suite scope. Returns per-file framework detection and a plan built from tests[].',
  test_resolve_paths:
    'Derive unit test file paths from a module list (files or directories). Three modes: (1) modules is an empty array — directories are auto-detected from config.json test configuration; (2) modules is a non-empty array — paths are filtered by test config scope before resolving; (3) modules is "change" (with the required `change` argument) — reads the change file inventory (workflow.json `files.written`) to discover changed files, then resolves test paths filtered by test config. Returns colocated unit test paths per source file.',
  change_files:
    'Merge paths into or overwrite the change file inventory (the `files` net state in workflow.json). op="append" folds paths into the net state to record file operations the PostToolUse hook missed (manual fallback for hook-invisible operations); op="set" wholesale-overwrites the provided buckets to explicitly correct the net state (e.g. after restores the hook cannot see). Returns the net state after the operation.',
  change_create:
    'Create a new change directory under openspec/changes/. The sole creator of its workflow.json metadata file (workflow_type + created only), which phase_next / backtrack / phase_log require — they error out when the file is missing. Validates kebab-case name and rejects existing changes.',
  change_list:
    'List all active (non-archived) changes under openspec/changes/. Returns each change with its artifacts, task progress, and latest eval phase.',
  spec_list:
    'Scan openspec/specs/*/spec.md and return a flat list of capabilities with name, path, and description. Replaces the bundled openspec `spec list --json` CLI command.',
  backtrack:
    'Set backtrack target and reason on the latest eval entry of a phase stored in workflow.json. This is the only way to modify backtrack state.',
  workflow_files:
    'Read-only query of the change file inventory (the `files` net state in workflow.json) for a given change. Returns the net `{ written, deleted }` path lists (relative to project root, POSIX style); the `source` audit map is never exposed. Strictly read-only — never modifies workflow.json; to record or correct the inventory use the write channel `change_files` instead. Hard-errors (no git diff fallback, no silent repair) when workflow.json is missing, unparseable, fails schema validation, or lacks the `files` field (a pre-inventory change must be recreated via change_create).',
};

const ALL_INPUT_SCHEMAS = [
  ['phase_log', phaseLogInputSchema],
  ['phase_next', phaseNextInputSchema],
  ['backtrack', backtrackInputSchema],
  ['change_create', changeCreateInputSchema],
  ['change_files', changeFilesInputSchema],
  ['change_list', changeListInputSchema],
  ['spec_list', specListInputSchema],
  ['config_get', configGetInputSchema],
  ['archi_query', archiQueryInputSchema],
  ['archi_validate', archiValidateInputSchema],
  ['archi_write', archiWriteInputSchema],
  ['archi_check', archiCheckInputSchema],
  ['archi_decide', archiDecideInputSchema],
  ['test_detect_frameworks', testDetectFrameworksInputSchema],
  ['test_resolve_paths', testResolvePathsInputSchema],
  ['workflow_files', workflowFilesInputSchema],
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
  runCrossRefCheck: vi.fn(async (_root: string, options?: { staged?: boolean }) => {
    if (options?.staged) {
      throw new Error(
        'staged 模式已由清单模式替代：请传 change（读取文件清单）或 files（显式文件列表）。',
      );
    }
    return {
      violations: [],
      warnings: [],
      matched: [],
      unmatched_files: [],
      status: 'clean' as const,
    };
  }),
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
vi.mock('./lib/archi-decide', () => ({
  createAdr: vi.fn(() => ({
    success: true,
    path: '/mock/decisions/mock.md',
    filename: 'mock.md',
  })),
  listAdrs: vi.fn(() => ({ adrs: [], count: 0 })),
  updateAdr: vi.fn(() => ({
    success: true,
    path: '/mock/decisions/mock.md',
    old_status: 'proposed',
    new_status: 'accepted',
  })),
}));

import * as archiDecide from './lib/archi-decide';

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

/**
 * Create `openspec/changes/<changeName>/` with a valid `workflow.json`.
 * `workflow.json` is the precondition of `phase_next` / `backtrack` /
 * `phase_log` — a directory without it now fails those tools by design.
 */
function setupChangeWithWorkflow(
  dir: string,
  changeName: string,
  workflowType: string = 'requirement',
): string {
  const changeDir = path.join(dir, 'openspec', 'changes', changeName);
  fs.mkdirSync(changeDir, { recursive: true });
  fs.writeFileSync(
    path.join(changeDir, 'workflow.json'),
    JSON.stringify({
      workflow_type: workflowType,
      created: '2026-09-11',
      files: { written: [], deleted: [] },
    }),
    'utf-8',
  );
  return changeDir;
}

/**
 * Create `openspec/changes/<changeName>/workflow.json` with an explicit
 * `files` net state (the general form of `setupChangeWithWorkflow`).
 */
function setupChangeWithFiles(
  dir: string,
  changeName: string,
  files: Record<string, unknown>,
): string {
  const changeDir = path.join(dir, 'openspec', 'changes', changeName);
  fs.mkdirSync(changeDir, { recursive: true });
  fs.writeFileSync(
    path.join(changeDir, 'workflow.json'),
    JSON.stringify({ workflow_type: 'requirement', created: '2026-09-11', files }),
    'utf-8',
  );
  return changeDir;
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

  describe('listTools — archi_decide (AC-01)', () => {
    it('listTools() name 集合经 sort 后严格等于预期 16 个 name（含 workflow_files）(AC-1)', async () => {
      const names = (await getRegisteredToolNames(client)).slice().sort();
      expect(names).toEqual([...EXPECTED_TOOL_NAMES]);
      expect(names).toHaveLength(16);
    });

    it('不得包含 list_changed / camelCase 别名', async () => {
      const names = await getRegisteredToolNames(client);
      for (const bad of ['list_changed', 'phaseLog', 'changeList', 'config-get']) {
        expect(names).not.toContain(bad);
      }
    });

    it('name 集合长度恰好 16；无重复 name', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toHaveLength(16);
      expect(new Set(names).size).toBe(16);
    });

    it('listTools 含 archi_decide 且无斜杠名 archi/decide', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toContain('archi_decide');
      expect(names).not.toContain('archi/decide');
      expect(names).not.toContain('archiDecide');
      expect(names).not.toContain('archi-decide');
    });
  });

  describe('name 边界', () => {
    it('注册名严格等于 archi_decide（禁止 archiDecide / archi-decide）', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toContain('archi_decide');
      expect(names).not.toContain('archiDecide');
      expect(names).not.toContain('archi-decide');
      expect(names).not.toContain('archi/decide');
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
      'change_create',
      'change_files',
      'change_list',
      'spec_list',
      'config_get',
      'archi_query',
      'archi_validate',
      'archi_write',
      'archi_check',
      'archi_decide',
      'test_detect_frameworks',
      'test_resolve_paths',
      'workflow_files',
    ];

    it('listTools 中全部相关 tool 的 inputSchema.required 均含 project_root (AC-2)', async () => {
      for (const name of toolsWithProjectRoot) {
        const schema = await getToolInputSchema(client, name);
        expect(schema).toBeDefined();
        if (name !== 'archi_decide') {
          const props = (schema?.properties ?? {}) as Record<string, unknown>;
          expect(props).toHaveProperty('project_root');
        }
        const required = schema?.required;
        if (Array.isArray(required)) {
          expect(required).toContain('project_root');
        }
      }
    });

    it('直接 import 各 *InputSchema：shape 含 project_root；safeParse 省略该字段时 success === false (AC-2)', () => {
      for (const [name, schema] of ALL_INPUT_SCHEMAS) {
        if ('shape' in schema) {
          expect(Object.keys(schema.shape)).toContain('project_root');
        }
        const parsed = schema.safeParse(
          name === 'config_get'
            ? { key: 'schema' }
            : name === 'change_list'
              ? {}
              : name === 'change_create'
                ? { name: 'my-change' }
                : name === 'change_files'
                  ? { change: 'c', op: 'append', written: ['src/a.ts'] }
                  : name === 'spec_list'
                    ? {}
                    : name === 'phase_next'
                      ? { change: 'c', run_id: 'test-run' }
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
                            : name === 'archi_decide'
                              ? { action: 'list' }
                              : name === 'archi_write'
                                ? { source: 'm', path: 'models/x.likec4' }
                                : name === 'workflow_files'
                                  ? { change: 'c' }
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
      // 该 spy 替换了 runPhaseLog 的实现，必须在本用例内恢复：本文件刻意不调用
      // restoreAllMocks()，模块状态按测试顺序共享，未恢复会泄漏到后续依赖真实
      // 落盘的 phase_log 用例（--sequence.shuffle 下顺序相关抖动）。
      const phaseLogSpy = vi.spyOn(phaseLogCmd, 'runPhaseLog').mockReturnValue({
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
        phaseLogSpy.mockRestore();
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

  describe('MCP 工具调用 — 全 16 handler', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('resolve mock 成功时 16 个 tool 各 callTool 一次均非空成功 (AC-3)', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*'] }],
      });
      const changeName = 'handler-fixture';
      setupChangeWithWorkflow(dir, changeName);
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
        archi_decide: vi.spyOn(archiDecide, 'listAdrs'),
        config_get: vi.spyOn(configGetCmd, 'runConfigGet'),
        test_detect_frameworks: vi.spyOn(testDetectFrameworksCmd, 'runTestDetectFrameworks'),
        test_resolve_paths: vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths'),
        change_create: vi.spyOn(changeCreateCmd, 'runChangeCreate'),
        change_files: vi.spyOn(changeFilesCmd, 'runChangeFiles'),
        change_list: vi.spyOn(changeListCmd, 'runChangeList'),
        spec_list: vi.spyOn(specListCmd, 'runSpecList'),
        workflow_files: vi.spyOn(workflowFilesCmd, 'getChangedFiles'),
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
          { name: 'phase_next', args: { change: changeName, run_id: 'test-run' } },
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
          { name: 'archi_decide', args: { action: 'list' } },
          { name: 'config_get', args: { key: 'schema' } },
          { name: 'test_detect_frameworks', args: {} },
          { name: 'test_resolve_paths', args: { modules: ['src/foo.ts'] } },
          {
            name: 'change_create',
            args: { name: 'handler-new-change', workflow_type: 'requirement' },
          },
          {
            name: 'change_files',
            args: { change: changeName, op: 'append', written: ['src/new.ts'] },
          },
          { name: 'workflow_files', args: { change: changeName } },
          { name: 'change_list', args: {} },
          { name: 'spec_list', args: {} },
        ];
        expect(calls).toHaveLength(16);

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

    it('resolve 抛 not_in_candidates 时 16 个 tool 均 isError 且 spy 次数 0', async () => {
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
        vi.spyOn(archiDecide, 'listAdrs'),
        vi.spyOn(configGetCmd, 'runConfigGet'),
        vi.spyOn(testDetectFrameworksCmd, 'runTestDetectFrameworks'),
        vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths'),
        vi.spyOn(changeCreateCmd, 'runChangeCreate'),
        vi.spyOn(changeFilesCmd, 'runChangeFiles'),
        vi.spyOn(changeListCmd, 'runChangeList'),
        vi.spyOn(specListCmd, 'runSpecList'),
        vi.spyOn(workflowFilesCmd, 'getChangedFiles'),
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
        { name: 'phase_next', args: { change: 'x', run_id: 'test-run' } },
        {
          name: 'backtrack',
          args: { change: 'x', phase: 'proposal', backtrack_to: 'explore', backtrack_reason: 'r' },
        },
        { name: 'archi_query', args: {} },
        { name: 'archi_validate', args: { source: 'm' } },
        { name: 'archi_write', args: { source: 'm', path: 'models/x.likec4' } },
        { name: 'archi_check', args: { staged: true } },
        { name: 'archi_decide', args: { action: 'list' } },
        { name: 'config_get', args: { key: 'schema' } },
        { name: 'test_detect_frameworks', args: {} },
        { name: 'test_resolve_paths', args: { modules: [] } },
        { name: 'change_create', args: { name: 'x' } },
        { name: 'change_files', args: { change: 'x', op: 'append', written: ['a.ts'] } },
        { name: 'workflow_files', args: { change: 'x' } },
        { name: 'change_list', args: {} },
        { name: 'spec_list', args: {} },
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

    it('全部 tool 均传入相同合法 project_root 时接线一致（以 change_list 为代表）', async () => {
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
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'phase_next',
          arguments: withProjectRoot({ change: changeName, run_id: 'test-run' }),
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

  describe('MCP 注册 — change_create (AC-1)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('listTools 返回的 tool names 中包含 change_create，且 EXPECTED_TOOL_NAMES 扩展到 16 (AC-1)', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toContain('change_create');
      expect(names).toHaveLength(16);
    });

    it('change_create 的 inputSchema 包含 name 字段（z.string()）', async () => {
      const schema = await getToolInputSchema(client, 'change_create');
      expect(schema).toBeDefined();
      const props = (schema?.properties ?? {}) as Record<string, unknown>;
      expect(props).toHaveProperty('name');
    });

    it('changeCreateInputSchema：name 必填且 kebab-case；缺 name / 缺 project_root 时 safeParse 失败', () => {
      expect(
        changeCreateInputSchema.safeParse({ name: 'my-change', workflow_type: 'requirement' })
          .success,
      ).toBe(false);
      expect(
        changeCreateInputSchema.safeParse({ project_root: '/tmp/x', workflow_type: 'requirement' })
          .success,
      ).toBe(false);
      expect(
        changeCreateInputSchema.safeParse({
          name: 'my-change',
          project_root: '/tmp/x',
          workflow_type: 'requirement',
        }).success,
      ).toBe(true);
      expect(
        changeCreateInputSchema.safeParse({
          name: 'MyChange',
          project_root: '/tmp/x',
          workflow_type: 'requirement',
        }).success,
      ).toBe(false);
    });

    it('changeCreateInputSchema：workflow_type 必填；非法值 safeParse 失败', () => {
      const parsed = changeCreateInputSchema.safeParse({
        name: 'my-change',
        project_root: '/tmp/x',
        workflow_type: 'requirement',
      });
      expect(parsed.success).toBe(true);
      if (parsed.success) {
        expect(parsed.data.workflow_type).toBe('requirement');
      }
      expect(
        changeCreateInputSchema.safeParse({
          name: 'my-change',
          project_root: '/tmp/x',
          workflow_type: 'test-only',
        }).success,
      ).toBe(true);
      expect(
        changeCreateInputSchema.safeParse({
          name: 'my-change',
          project_root: '/tmp/x',
          workflow_type: 'unknown-type',
        }).success,
      ).toBe(false);
    });

    it('callTool({ name: "change_create", name: "my-change", project_root: dir, workflow_type: "requirement" }) 时 runChangeCreate spy 被调用且参数正确 (AC-1)', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      const spy = vi.spyOn(changeCreateCmd, 'runChangeCreate');
      try {
        const result = await client.callTool({
          name: 'change_create',
          arguments: withProjectRoot({ name: 'my-change', workflow_type: 'requirement' }, dir),
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        expect(spy).toHaveBeenCalledWith('my-change', dir, 'requirement');
        const data = JSON.parse(extractText(result));
        expect(changeCreateOutputSchema.safeParse(data).success).toBe(true);
        expect(data).toMatchObject({ name: 'my-change' });
        expect(data.path).toBe(path.resolve(dir, 'openspec', 'changes', 'my-change'));
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('callTool 省略 name → isError 且 runChangeCreate spy 次数为 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(changeCreateCmd, 'runChangeCreate');
      spy.mockClear();
      const result = await client.callTool({
        name: 'change_create',
        arguments: withProjectRoot({}),
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('callTool name 为空字符串 → isError 且 runChangeCreate spy 次数为 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(changeCreateCmd, 'runChangeCreate');
      spy.mockClear();
      const result = await client.callTool({
        name: 'change_create',
        arguments: withProjectRoot({ name: '' }),
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });
  });

  describe('MCP 注册 — spec_list (AC-5)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('listTools 返回的 tool names 中包含 spec_list (AC-5)', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toContain('spec_list');
    });

    it('spec_list 的 inputSchema 包含 project_root 字段', async () => {
      const schema = await getToolInputSchema(client, 'spec_list');
      expect(schema).toBeDefined();
      const props = (schema?.properties ?? {}) as Record<string, unknown>;
      expect(props).toHaveProperty('project_root');
    });

    it('specListInputSchema：缺 project_root 时 safeParse 失败；带 project_root 通过', () => {
      expect(specListInputSchema.safeParse({}).success).toBe(false);
      expect(specListInputSchema.safeParse({ project_root: '/tmp/x' }).success).toBe(true);
    });

    it('callTool({ name: "spec_list", project_root: dir }) 时 runSpecList spy 被调用且参数正确 (AC-5)', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      const spy = vi.spyOn(specListCmd, 'runSpecList');
      try {
        const result = await client.callTool({
          name: 'spec_list',
          arguments: withProjectRoot({}, dir),
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        expect(spy).toHaveBeenCalledWith(dir);
        const data = JSON.parse(extractText(result));
        expect(specListOutputSchema.safeParse(data).success).toBe(true);
        expect(data).toMatchObject({ project_root: dir, specs: [] });
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('callTool 省略 project_root → isError 且 runSpecList spy 次数为 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(specListCmd, 'runSpecList');
      spy.mockClear();
      const result = await client.callTool({ name: 'spec_list', arguments: {} });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('spec_list 返回真实 capability 列表（fixture 目录中的 spec.md）', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        fs.mkdirSync(path.join(dir, 'openspec', 'specs', 'alpha'), { recursive: true });
        fs.writeFileSync(
          path.join(dir, 'openspec', 'specs', 'alpha', 'spec.md'),
          '## alpha\n\nAlpha capability body.',
          'utf-8',
        );
        const result = await client.callTool({
          name: 'spec_list',
          arguments: withProjectRoot({}, dir),
        });
        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        expect(specListOutputSchema.safeParse(data).success).toBe(true);
        expect(data.specs).toHaveLength(1);
        expect(data.specs[0]).toMatchObject({
          name: 'alpha',
          description: 'Alpha capability body.',
        });
        expect(data.specs[0].path).toBe(path.join(dir, 'openspec', 'specs', 'alpha', 'spec.md'));
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 调用 — 所有 16 个 tool 各 callTool 一次均成功（含 change_create / change_files / workflow_files / spec_list）', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('在同一个 MCP server 上 16 个 tool 各调用一次均非 isError 且返回非空文本 (AC-1/AC-5)', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*'] }],
      });
      const changeName = 'all-14-fixture';
      setupChangeWithWorkflow(dir, changeName);
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
        archi_decide: vi.spyOn(archiDecide, 'listAdrs'),
        config_get: vi.spyOn(configGetCmd, 'runConfigGet'),
        test_detect_frameworks: vi.spyOn(testDetectFrameworksCmd, 'runTestDetectFrameworks'),
        test_resolve_paths: vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths'),
        change_create: vi.spyOn(changeCreateCmd, 'runChangeCreate'),
        change_files: vi.spyOn(changeFilesCmd, 'runChangeFiles'),
        change_list: vi.spyOn(changeListCmd, 'runChangeList'),
        spec_list: vi.spyOn(specListCmd, 'runSpecList'),
        workflow_files: vi.spyOn(workflowFilesCmd, 'getChangedFiles'),
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
          { name: 'phase_next', args: { change: changeName, run_id: 'test-run' } },
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
          { name: 'archi_decide', args: { action: 'list' } },
          { name: 'config_get', args: { key: 'schema' } },
          { name: 'test_detect_frameworks', args: {} },
          { name: 'test_resolve_paths', args: { modules: ['src/foo.ts'] } },
          { name: 'change_create', args: { name: 'new-change-14', workflow_type: 'requirement' } },
          {
            name: 'change_files',
            args: { change: changeName, op: 'append', written: ['src/new.ts'] },
          },
          { name: 'workflow_files', args: { change: changeName } },
          { name: 'change_list', args: {} },
          { name: 'spec_list', args: {} },
        ];
        expect(calls).toHaveLength(16);

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
  });

  describe('MCP 调用 — change_list workflow_done (AC-6)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    /**
     * 布置一个 change 目录。评估历史是 `workflow.json.eval` 的一部分：
     * `evalEntries` 与 `workflowJson` 合并进同一对象，避免后写的 `workflow.json`
     * 覆盖权威数组；`workflowJson` 为字符串时按原文写入（非法形状用例）。
     */
    function writeDoneChange(
      dir: string,
      changeName: string,
      options: { workflowJson?: Record<string, unknown> | string; evalEntries?: unknown[] },
    ): void {
      const changeDir = path.join(dir, 'openspec', 'changes', changeName);
      fs.mkdirSync(changeDir, { recursive: true });

      if (options.workflowJson !== undefined) {
        const content =
          typeof options.workflowJson === 'string'
            ? options.workflowJson
            : JSON.stringify({ ...options.workflowJson, eval: options.evalEntries });
        fs.writeFileSync(path.join(changeDir, 'workflow.json'), content, 'utf-8');
      } else if (options.evalEntries !== undefined) {
        fs.writeFileSync(
          path.join(changeDir, 'workflow.json'),
          JSON.stringify({ workflow_type: 'requirement', eval: options.evalEntries }),
          'utf-8',
        );
      }
    }

    function allPassEvalEntries(): unknown[] {
      return getPhaseTable('requirement').map((phase) => ({
        phase: phase.id,
        verdict: 'pass',
        attempt: 1,
        timestamp: '2026-08-14T12:00:00.000Z',
        report: 'test',
        checklist: [],
        backtrack_to: null,
      }));
    }

    it('所有 phase 有非 stale pass 的 change，workflow_done 为 true (AC-6)', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'done-change', {
          workflowJson: { workflow_type: 'requirement' },
          evalEntries: allPassEvalEntries(),
        });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'done-change');
        expect(entry.workflow_done).toBe(true);
      } finally {
        cleanup();
      }
    });

    it('缺少 pass 的 change，workflow_done 为 false (AC-6)', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'incomplete', {
          workflowJson: { workflow_type: 'requirement' },
          evalEntries: [
            {
              phase: 'proposal',
              verdict: 'pass',
              attempt: 1,
              timestamp: '2026-08-14T12:00:00.000Z',
              report: 'r',
              checklist: [],
              backtrack_to: null,
            },
          ],
        });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'incomplete');
        expect(entry.workflow_done).toBe(false);
      } finally {
        cleanup();
      }
    });

    it('workflow_done 字段类型为 boolean，changeListOutputSchema 解析通过 (AC-6)', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'typed-change', {
          workflowJson: { workflow_type: 'requirement' },
          evalEntries: allPassEvalEntries(),
        });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        const data = JSON.parse(extractText(result));
        const parsed = changeListOutputSchema.safeParse(data);
        expect(parsed.success).toBe(true);
        if (!parsed.success) return;
        const entry = parsed.data.changes.find((c) => c.name === 'typed-change');
        expect(typeof entry?.workflow_done).toBe('boolean');
      } finally {
        cleanup();
      }
    });

    it('无评估条目（仅 workflow_type + created）时 workflow_done 为 false、latest_phase 为 null，不崩溃', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'no-eval', { workflowJson: { workflow_type: 'requirement' } });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'no-eval');
        expect(entry.workflow_done).toBe(false);
        expect(entry.latest_phase).toBeNull();
      } finally {
        cleanup();
      }
    });

    it('空 eval.json（[]）时 workflow_done 为 false', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'empty-eval', {
          workflowJson: { workflow_type: 'requirement' },
          evalEntries: [],
        });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'empty-eval');
        expect(entry.workflow_done).toBe(false);
      } finally {
        cleanup();
      }
    });

    it('无 workflow.json 时 workflow_done 为 false，change 仍列出（AC-13 容错面）', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const changeDir = path.join(dir, 'openspec', 'changes', 'no-workflow');
        fs.mkdirSync(changeDir, { recursive: true });
        fs.writeFileSync(
          path.join(changeDir, 'eval.json'),
          JSON.stringify(allPassEvalEntries()),
          'utf-8',
        );
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'no-workflow');
        expect(entry).toBeDefined();
        expect(entry.workflow_done).toBe(false);
      } finally {
        cleanup();
      }
    });

    it('workflow.json 非法 JSON 时 change_list 不崩溃，workflow_done 为 false、latest_phase 为 null（AC-14 容错面）', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'broken-json', { workflowJson: 'not json {' });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'broken-json');
        expect(entry).toBeDefined();
        expect(entry.workflow_done).toBe(false);
        expect(entry.latest_phase).toBeNull();
      } finally {
        cleanup();
      }
    });

    it('eval 非法（对象）时 change_list 仍返回该 change，workflow_done 为 false（AC-12）', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'object-eval', {
          workflowJson: { workflow_type: 'requirement', eval: {} },
        });
        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'object-eval');
        expect(entry).toBeDefined();
        expect(entry.workflow_done).toBe(false);
        expect(entry.latest_phase).toBeNull();
      } finally {
        cleanup();
      }
    });

    it('全 phase pass 写在 workflow.json.eval 时 workflow_done 为 true，且目录内不产生 eval.json', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        writeDoneChange(dir, 'workflow-store-done', {
          workflowJson: { workflow_type: 'requirement' },
          evalEntries: allPassEvalEntries(),
        });

        const result = await client.callTool({
          name: 'change_list',
          arguments: withProjectRoot({}, dir),
        });
        const data = JSON.parse(extractText(result));
        const entry = data.changes.find((c: { name: string }) => c.name === 'workflow-store-done');
        expect(entry.workflow_done).toBe(true);
        expect(
          fs.existsSync(path.join(dir, 'openspec', 'changes', 'workflow-store-done', 'eval.json')),
        ).toBe(false);
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 调用 — phase_log 落盘到 workflow.json.eval (AC-1, AC-10)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('经 MCP 调用 phase_log（已有 workflow.json）后条目落 workflow.json.eval，目录内不产生 eval.json（AC-1）', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const changeName = 'phase-log-store';
        const changeDir = setupChangeWithWorkflow(dir, changeName);

        const result = await client.callTool({
          name: 'phase_log',
          arguments: withProjectRoot({
            change: changeName,
            phase: 'proposal',
            report: '提案通过',
            checklist: [{ item: '范围明确', pass: true, evidence: 'ok' }],
          }),
        });

        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        expect(data.written).toBe(true);
        expect(data.phase).toBe('proposal');

        const doc = JSON.parse(
          fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8'),
        ) as Record<string, unknown>;
        const entries = doc.eval as Record<string, unknown>[];
        expect(entries).toHaveLength(1);
        expect(entries[0].verdict).toBe('pass');
        expect(doc.workflow_type).toBe('requirement');
        expect(doc.created).toBe('2026-09-11');
        expect(fs.existsSync(path.join(changeDir, 'eval.json'))).toBe(false);
      } finally {
        cleanup();
      }
    });

    it('缺 workflow.json 时 phase_log 报错且不创建任何文件（AC-13）', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const changeName = 'phase-log-missing';
        const changeDir = path.join(dir, 'openspec', 'changes', changeName);
        fs.mkdirSync(changeDir, { recursive: true });

        const result = await client.callTool({
          name: 'phase_log',
          arguments: withProjectRoot({
            change: changeName,
            phase: 'proposal',
            report: 'ok',
            checklist: [{ item: 'x', pass: true, evidence: 'ok' }],
          }),
        });

        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).toContain('change_create');
        expect(fs.existsSync(path.join(changeDir, 'workflow.json'))).toBe(false);
        expect(fs.existsSync(path.join(changeDir, 'eval.json'))).toBe(false);
      } finally {
        cleanup();
      }
    });

    it('phase_log 缺必填字段（checklist）时 Zod 拒参，不写盘', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const changeName = 'phase-log-invalid';
        const changeDir = setupChangeWithWorkflow(dir, changeName);

        const result = await client.callTool({
          name: 'phase_log',
          arguments: withProjectRoot({ change: changeName, phase: 'proposal', report: 'ok' }),
        });

        expect(isToolError(result)).toBe(true);
        const doc = JSON.parse(
          fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8'),
        ) as Record<string, unknown>;
        expect(doc).not.toHaveProperty('eval');
      } finally {
        cleanup();
      }
    });

    it('phase_log 的 tool_input 含 backtrack_to 时被 schema 忽略，写入的条目不含该字段', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const changeName = 'phase-log-strip';
        const changeDir = setupChangeWithWorkflow(dir, changeName);

        const result = await client.callTool({
          name: 'phase_log',
          arguments: withProjectRoot({
            change: changeName,
            phase: 'proposal',
            report: 'ok',
            checklist: [{ item: 'x', pass: true, evidence: 'ok' }],
            backtrack_to: 'dev-design',
          }),
        });

        expect(isToolError(result)).toBe(false);
        const doc = JSON.parse(
          fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8'),
        ) as Record<string, unknown>;
        const entry = (doc.eval as Record<string, unknown>[])[0];
        expect(entry.backtrack_to).toBeUndefined();
      } finally {
        cleanup();
      }
    });

    it('change_create 传入非法枚举 workflow_type 时 Zod 拒参', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'change_create',
          arguments: withProjectRoot({ name: 'bad-enum-change', workflow_type: 'unknown-flow' }),
        });

        expect(isToolError(result)).toBe(true);
        expect(fs.existsSync(path.join(dir, 'openspec', 'changes', 'bad-enum-change'))).toBe(false);
      } finally {
        cleanup();
      }
    });

    it('backtrack 缺必填字段（backtrack_reason）时 Zod 拒参', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'backtrack',
          arguments: withProjectRoot({
            change: 'some-change',
            phase: 'test-execution',
            backtrack_to: 'proposal',
          }),
        });

        expect(isToolError(result)).toBe(true);
      } finally {
        cleanup();
      }
    });

    it('backtrack 描述与注册串字节级相等，且提及 workflow.json 评估条目（AC-10）', async () => {
      const listed = await client.listTools();
      const tool = listed.tools.find((t) => t.name === 'backtrack');
      expect(tool?.description).toBe(EXPECTED_TOOL_DESCRIPTIONS.backtrack);
      expect(tool?.description).toContain('workflow.json');
    });

    it('change_create 描述含「唯一创建者 / 缺文件时读取方报错」契约（AC-10）', async () => {
      const listed = await client.listTools();
      const tool = listed.tools.find((t) => t.name === 'change_create');
      expect(tool?.description).toBe(EXPECTED_TOOL_DESCRIPTIONS.change_create);
      expect(tool?.description).toContain('sole creator');
      expect(tool?.description).toContain('workflow.json');
      expect(tool?.description).toMatch(/error out when the file is missing/);
    });

    it('phase_log 描述非空、含 workflow.json 且不以 eval.json 为写入目标（AC-10）', async () => {
      const listed = await client.listTools();
      const tool = listed.tools.find((t) => t.name === 'phase_log');
      expect(tool?.description).toBe(EXPECTED_TOOL_DESCRIPTIONS.phase_log);
      expect(tool?.description).not.toBe('');
      expect(tool?.description).toContain('workflow.json');
      expect(tool?.description).not.toContain('eval.json');
    });

    it('工具名集合仍含 phase_log / phase_next / backtrack / change_list / change_create，无重命名', async () => {
      const listed = await client.listTools();
      const names = listed.tools.map((t) => t.name);
      for (const name of ['phase_log', 'phase_next', 'backtrack', 'change_list', 'change_create']) {
        expect(names).toContain(name);
      }
    });
  });

  describe('MCP schema — phase_next run_id (AC-1)', () => {
    it('phaseNextInputSchema.shape 含 run_id', () => {
      expect(Object.keys(phaseNextInputSchema.shape)).toContain('run_id');
    });

    it('listTools 中 phase_next.inputSchema.required 含 run_id', async () => {
      const schema = await getToolInputSchema(client, 'phase_next');
      expect(schema).toBeDefined();
      const required = schema?.required;
      if (Array.isArray(required)) {
        expect(required).toContain('run_id');
      }
    });

    it('safeParse 缺 run_id → success === false', () => {
      const parsed = phaseNextInputSchema.safeParse({
        project_root: mockResolve.root,
        change: 'c',
      });
      expect(parsed.success).toBe(false);
    });

    it('safeParse run_id 为空或仅空白 → success === false', () => {
      for (const runId of ['', '   ']) {
        const parsed = phaseNextInputSchema.safeParse({
          project_root: mockResolve.root,
          change: 'c',
          run_id: runId,
        });
        expect(parsed.success).toBe(false);
      }
    });

    it('safeParse run_id 非法类型 → success === false', () => {
      for (const runId of [123, null, {}]) {
        const parsed = phaseNextInputSchema.safeParse({
          project_root: mockResolve.root,
          change: 'c',
          run_id: runId,
        });
        expect(parsed.success).toBe(false);
      }
    });

    it('run_id 超长 / 含 \\n / emoji 且非空 → schema 通过', () => {
      for (const runId of ['x'.repeat(1001), 'a\nb', 'emoji-🧪']) {
        const parsed = phaseNextInputSchema.safeParse({
          project_root: mockResolve.root,
          change: 'c',
          run_id: runId,
        });
        expect(parsed.success).toBe(true);
      }
    });
  });

  describe('MCP 工具调用 — phase_next run_id 接线 (AC-1, AC-7)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('callTool 带非空 run_id 成功且 runPhaseNext spy 收到同一 run_id', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'run-id-wire';
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      const spy = vi.spyOn(phaseNextCmd, 'runPhaseNext');
      try {
        const result = await client.callTool({
          name: 'phase_next',
          arguments: withProjectRoot({ change: changeName, run_id: 'wire-run-1' }),
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith(
          expect.objectContaining({
            change: changeName,
            run_id: 'wire-run-1',
            project_root: dir,
          }),
        );
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('callTool 省略 run_id → isError 且 runPhaseNext spy 次数为 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(phaseNextCmd, 'runPhaseNext');
      spy.mockClear();
      const result = await client.callTool({
        name: 'phase_next',
        arguments: withProjectRoot({ change: 'c' }),
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('callTool run_id 仅空白 → schema 失败且 spy 次数为 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(phaseNextCmd, 'runPhaseNext');
      spy.mockClear();
      const result = await client.callTool({
        name: 'phase_next',
        arguments: withProjectRoot({ change: 'c', run_id: '   ' }),
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('callTool run_id 非法类型 → schema 失败且 spy 次数为 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(phaseNextCmd, 'runPhaseNext');
      spy.mockClear();
      const result = await client.callTool({
        name: 'phase_next',
        arguments: withProjectRoot({ change: 'c', run_id: 123 as unknown as string }),
      });
      expect(isToolError(result)).toBe(true);
      expect(spy).toHaveBeenCalledTimes(0);
      spy.mockRestore();
    });

    it('返回下一阶段信息路径传入空 run_id → 工具失败', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'run-id-empty';
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      const spy = vi.spyOn(phaseNextCmd, 'runPhaseNext');
      try {
        const result = await client.callTool({
          name: 'phase_next',
          arguments: withProjectRoot({ change: changeName, run_id: '' }),
        });
        expect(isToolError(result)).toBe(true);
        expect(spy).toHaveBeenCalledTimes(0);
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('全 14 handler 中 phase_next 故意省略 run_id → 该条 isError', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*'] }],
      });
      const changeName = 'handler-omit-run-id';
      setupChangeWithWorkflow(dir, changeName);
      fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');
      setResolvedRoot(dir);
      const spy = vi.spyOn(phaseNextCmd, 'runPhaseNext');
      spy.mockClear();
      try {
        const result = await client.callTool({
          name: 'phase_next',
          arguments: withProjectRoot({ change: changeName }),
        });
        expect(isToolError(result)).toBe(true);
        expect(spy).toHaveBeenCalledTimes(0);
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });
  });

  describe('listTools — description', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('archi_decide description 与注册字面量逐字节相等', async () => {
      const desc = await getToolDescription(client, 'archi_decide');
      expect(desc).toBe(EXPECTED_TOOL_DESCRIPTIONS.archi_decide);
    });
  });

  describe('inputSchema — project_root (AC-06)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('archi_decide inputSchema.required 含 project_root 与 action', async () => {
      const schema = await getToolInputSchema(client, 'archi_decide');
      expect(schema).toBeDefined();
      const required = schema?.required;
      if (Array.isArray(required)) {
        expect(required).toContain('project_root');
        expect(required).toContain('action');
      }
    });
  });

  describe('inputSchema — action 判别', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('create 分支必填 title/background/decision；list 仅根+action；update 必填 file/status', () => {
      expect(
        archiDecideInputSchema.safeParse({ project_root: mockResolve.root, action: 'list' })
          .success,
      ).toBe(true);
      expect(
        archiDecideInputSchema.safeParse({
          project_root: mockResolve.root,
          action: 'create',
          title: 't',
          background: 'b',
          decision: 'd',
        }).success,
      ).toBe(true);
      expect(
        archiDecideInputSchema.safeParse({
          project_root: mockResolve.root,
          action: 'update',
          file: 'f.md',
          status: 'accepted',
        }).success,
      ).toBe(true);
      expect(
        archiDecideInputSchema.safeParse({
          project_root: mockResolve.root,
          action: 'create',
          background: 'b',
          decision: 'd',
        }).success,
      ).toBe(false);
      expect(
        archiDecideInputSchema.safeParse({
          project_root: mockResolve.root,
          action: 'create',
          file: 'f.md',
          status: 'accepted',
        }).success,
      ).toBe(false);
    });
  });

  describe('inputSchema — discriminatedUnion (MCP SDK)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
    });

    it('archiDecideInputSchema 导出 JSON Schema 含 oneOf 三分支（MCP SDK 兼容）', () => {
      const json = toJSONSchema(archiDecideInputSchema) as Record<string, unknown>;
      const branches = (json.oneOf ?? json.anyOf) as Array<Record<string, unknown>> | undefined;
      expect(branches?.length).toBe(3);
      const actions = branches?.map((branch) => {
        const props = branch.properties as Record<string, unknown> | undefined;
        const action = props?.action as { const?: string; enum?: string[] } | undefined;
        return action?.const ?? action?.enum?.[0];
      });
      expect(actions).toEqual(expect.arrayContaining(['create', 'list', 'update']));
    });

    it('archiDecideOutputSchema 各 action 分支 safeParse 通过', () => {
      expect(
        archiDecideOutputSchema.safeParse({ action: 'list', adrs: [], count: 0 }).success,
      ).toBe(true);
      expect(
        archiDecideOutputSchema.safeParse({
          action: 'create',
          success: true,
          path: '/p',
          filename: 'a.md',
        }).success,
      ).toBe(true);
      expect(
        archiDecideOutputSchema.safeParse({
          action: 'update',
          success: false,
          error: 'superseded_by required',
        }).success,
      ).toBe(true);
    });

    it('update 分支缺 file 时 inputSchema 拒参', () => {
      expect(
        archiDecideInputSchema.safeParse({
          project_root: mockResolve.root,
          action: 'update',
          status: 'accepted',
        }).success,
      ).toBe(false);
    });
  });

  describe('callTool — 分发到 lib', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('action 各值 callTool 一次时对应 lib spy 调用 1 次且入参含解析后的 project root', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      try {
        await client.callTool({
          name: 'archi_decide',
          arguments: withProjectRoot({
            action: 'create',
            title: 'T',
            background: 'B',
            decision: 'D',
          }),
        });
        expect(archiDecide.createAdr).toHaveBeenCalledWith(
          dir,
          expect.objectContaining({ title: 'T', background: 'B', decision: 'D' }),
        );

        await client.callTool({
          name: 'archi_decide',
          arguments: withProjectRoot({ action: 'list' }),
        });
        expect(archiDecide.listAdrs).toHaveBeenCalledWith(dir, undefined);

        await client.callTool({
          name: 'archi_decide',
          arguments: withProjectRoot({
            action: 'update',
            file: 'x.md',
            status: 'accepted',
          }),
        });
        expect(archiDecide.updateAdr).toHaveBeenCalledWith(
          dir,
          expect.objectContaining({ file: 'x.md', status: 'accepted' }),
        );
      } finally {
        cleanup();
      }
    });
  });

  describe('callTool — create 成功结构 (AC-02)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('lib 返回成功时 MCP 非 isError 且 structured 含 success 与 filename/path', async () => {
      vi.mocked(archiDecide.createAdr).mockReturnValueOnce({
        success: true,
        path: '/mock/a.md',
        filename: 'a.md',
      });
      setResolvedRoot(process.cwd());
      const result = await client.callTool({
        name: 'archi_decide',
        arguments: withProjectRoot({
          action: 'create',
          title: 'T',
          background: 'B',
          decision: 'D',
        }),
      });
      expect(isToolError(result)).toBe(false);
      const body = JSON.parse(extractText(result));
      expect(body).toMatchObject({
        action: 'create',
        success: true,
        filename: 'a.md',
        path: '/mock/a.md',
      });
    });
  });

  describe('withResolvedProjectRoot (AC-06)', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('resolve 成功后 withResolvedProjectRoot 被调用，run 收到的根等于 mock resolve 根', async () => {
      const { dir, cleanup } = setupTempProject();
      setResolvedRoot(dir);
      vi.mocked(withResolvedProjectRoot).mockClear();
      try {
        await client.callTool({
          name: 'archi_decide',
          arguments: withProjectRoot({ action: 'list' }),
        });
        expect(withResolvedProjectRoot).toHaveBeenCalled();
        const run = vi.mocked(withResolvedProjectRoot).mock.calls.at(-1)?.[2];
        expect(run).toBeTypeOf('function');
        expect(await run!(dir)).toBeDefined();
      } finally {
        cleanup();
      }
    });

    it('resolve 抛错时返回 isError 且 lib spy 调用次数为 0', async () => {
      setResolveError(makeResolveError('invalid_path', 'bad', { project_root: '' }));
      vi.mocked(archiDecide.createAdr).mockClear();
      const result = await client.callTool({
        name: 'archi_decide',
        arguments: withProjectRoot(
          {
            action: 'create',
            title: 'T',
            background: 'B',
            decision: 'D',
          },
          '',
        ),
      });
      expect(isToolError(result)).toBe(true);
      expect(archiDecide.createAdr).toHaveBeenCalledTimes(0);
    });
  });

  describe('callTool — Zod 拒参', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('缺 project_root / 缺 action / create 缺 title 时校验失败且不调用 lib', async () => {
      vi.mocked(archiDecide.createAdr).mockClear();
      for (const args of [
        { action: 'list' },
        { project_root: mockResolve.root },
        {
          project_root: mockResolve.root,
          action: 'create',
          background: 'b',
          decision: 'd',
        },
      ]) {
        const result = await client.callTool({ name: 'archi_decide', arguments: args });
        expect(isToolError(result)).toBe(true);
      }
      expect(archiDecide.createAdr).toHaveBeenCalledTimes(0);
    });
  });

  describe('callTool — 非法 action', () => {
    afterEach(() => {
      setResolvedRoot(process.cwd());
      vi.mocked(archiDecide.createAdr).mockClear();
      vi.mocked(archiDecide.listAdrs).mockClear();
      vi.mocked(archiDecide.updateAdr).mockClear();
    });

    it('action 为 delete 或空串时校验失败', async () => {
      for (const action of ['delete', '']) {
        const parsed = archiDecideInputSchema.safeParse({
          project_root: mockResolve.root,
          action,
        });
        expect(parsed.success).toBe(false);
      }
    });
  });
});

describe('MCP 注册 — registerTool spy 精确字面量', () => {
  const REGISTER_ORDER = [
    'phase_log',
    'archi_query',
    'archi_validate',
    'archi_write',
    'archi_check',
    'archi_decide',
    'phase_next',
    'config_get',
    'test_detect_frameworks',
    'test_resolve_paths',
    'change_list',
    'change_create',
    'change_files',
    'workflow_files',
    'spec_list',
    'backtrack',
  ] as const;

  it('按注册顺序对每次调用断言 name/description；调用次数恰好 16', async () => {
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

      expect(registerSpy).toHaveBeenCalledTimes(16);
      const names: string[] = [];
      for (let i = 0; i < 16; i++) {
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

  it('test_resolve_paths 描述不再含 git diff 圈定语义；archi_check 描述指向清单来源（schema description 驱动 MCP 元数据）', async () => {
    mockResolve.root = process.cwd();
    mockResolve.throwError = null;
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const { connectToServer } = await import('./mcp');
    const s = await connectToServer(serverTransport);
    const c = new Client({ name: 'desc-client', version: '1.0.0' }, { capabilities: {} });
    await c.connect(clientTransport);
    try {
      const resolveDesc = await getToolDescription(c, 'test_resolve_paths');
      expect(resolveDesc).not.toContain('git diff');
      expect(resolveDesc).not.toContain('git-change');
      expect(resolveDesc).toContain('files.written');

      const checkDesc = await getToolDescription(c, 'archi_check');
      expect(checkDesc).toContain('change file inventory');
    } finally {
      await c.close();
      await s.close();
    }
  });
});

// ===========================================================================
// MCP 注册 — change_files / archi_check change / test_resolve_paths 清单模式
// (AC-8, AC-9, AC-10)
// ===========================================================================

describe('MCP 注册 — change_files 与清单模式接线 (AC-8, AC-9, AC-10)', () => {
  let server: McpServer;
  let client: Client;

  beforeAll(async () => {
    mockResolve.root = process.cwd();
    mockResolve.throwError = null;
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const { connectToServer } = await import('./mcp');
    server = await connectToServer(serverTransport);
    client = new Client({ name: 'inventory-client', version: '1.0.0' }, { capabilities: {} });
    await client.connect(clientTransport);
  });

  afterAll(async () => {
    await client.close();
    await server.close();
  });

  afterEach(() => {
    setResolvedRoot(process.cwd());
  });

  describe('change_files 注册 (AC-10)', () => {
    it('tools/list 含 change_files；inputSchema 与 changeFilesInputSchema 的 JSON Schema 形状一致（change 必填、op 枚举 append/set）', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toContain('change_files');

      const schema = await getToolInputSchema(client, 'change_files');
      expect(schema).toBeDefined();
      const props = (schema?.properties ?? {}) as Record<
        string,
        { type?: string; enum?: string[] }
      >;
      expect(props).toHaveProperty('change');
      expect(props).toHaveProperty('op');
      expect(props).toHaveProperty('written');
      expect(props).toHaveProperty('deleted');
      expect(props.op?.enum).toEqual(['append', 'set']);
      expect(schema?.required).toContain('change');
      expect(schema?.required).toContain('project_root');
    });

    it('changeFilesInputSchema：written 与 deleted 均缺省 → refine 拒绝；至少其一通过', () => {
      const base = { change: 'c', op: 'append' as const, project_root: '/tmp/x' };
      expect(changeFilesInputSchema.safeParse(base).success).toBe(false);
      expect(changeFilesInputSchema.safeParse({ ...base, written: ['src/a.ts'] }).success).toBe(
        true,
      );
      expect(
        changeFilesInputSchema.safeParse({ ...base, op: 'set', deleted: ['src/a.ts'] }).success,
      ).toBe(true);
    });

    it('tools/call append → 委托 runChangeFiles（注入 project_root）并返回操作后净状态 { written, deleted } (AC-10)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'cf-append';
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      const spy = vi.spyOn(changeFilesCmd, 'runChangeFiles');
      try {
        const result = await client.callTool({
          name: 'change_files',
          arguments: withProjectRoot(
            { change: changeName, op: 'append', written: ['src/foo.ts'] },
            dir,
          ),
        });

        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith({
          change: changeName,
          op: 'append',
          written: ['src/foo.ts'],
          project_root: dir,
        });
        const data = JSON.parse(extractText(result));
        expect(changeFilesOutputSchema.safeParse(data).success).toBe(true);
        expect(data).toEqual({ written: ['src/foo.ts'], deleted: [] });
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('tools/call set → 覆写指定桶并返回净状态 (AC-10)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'cf-set';
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      const spy = vi.spyOn(changeFilesCmd, 'runChangeFiles');
      try {
        const result = await client.callTool({
          name: 'change_files',
          arguments: withProjectRoot(
            { change: changeName, op: 'set', written: [], deleted: ['src/gone.ts'] },
            dir,
          ),
        });

        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith({
          change: changeName,
          op: 'set',
          written: [],
          deleted: ['src/gone.ts'],
          project_root: dir,
        });
        expect(JSON.parse(extractText(result))).toEqual({
          written: [],
          deleted: ['src/gone.ts'],
        });
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('非法输入（缺 change / op 非法 / written 与 deleted 均缺省）→ 工具层参数错误且 runChangeFiles 次数 0', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(changeFilesCmd, 'runChangeFiles');
      spy.mockClear();
      try {
        for (const args of [
          { op: 'append', written: ['src/a.ts'] },
          { change: 'c', op: 'merge', written: ['src/a.ts'] },
          { change: 'c', op: 'append' },
        ]) {
          const result = await client.callTool({
            name: 'change_files',
            arguments: withProjectRoot(args as Record<string, unknown>),
          });
          expect(isToolError(result)).toBe(true);
        }
        expect(spy).toHaveBeenCalledTimes(0);
      } finally {
        spy.mockRestore();
      }
    });

    it('对穿越 / 绝对路径 / 反斜杠 / 空串路径拒绝入清单（schema 校验层拦截，AC-10）', () => {
      const base = { change: 'c', op: 'append' as const, project_root: '/tmp/x' };
      for (const bad of [['../outside.ts'], ['/abs/x.ts'], ['src\\x.ts'], ['']]) {
        expect(changeFilesInputSchema.safeParse({ ...base, written: bad }).success).toBe(false);
      }
      // 合法 POSIX 相对路径通过
      expect(changeFilesInputSchema.safeParse({ ...base, written: ['src/ok.ts'] }).success).toBe(
        true,
      );
    });
  });

  describe('workflow_files 注册 (AC-1)', () => {
    it('tools/list 含 workflow_files；inputSchema.properties 含 change 与 project_root；required 含 change 与 project_root (AC-1)', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toContain('workflow_files');

      const schema = await getToolInputSchema(client, 'workflow_files');
      expect(schema).toBeDefined();
      const props = (schema?.properties ?? {}) as Record<string, unknown>;
      expect(props).toHaveProperty('change');
      expect(props).toHaveProperty('project_root');
      expect(schema?.required).toContain('change');
      expect(schema?.required).toContain('project_root');
    });

    it('workflowFilesInputSchema：缺 change / change 为空串 / change 为非 string → safeParse 失败；{ change: "c" } + project_root 通过 (AC-1)', () => {
      const base = { project_root: '/tmp/x' };
      expect(workflowFilesInputSchema.safeParse({}).success).toBe(false);
      expect(workflowFilesInputSchema.safeParse(base).success).toBe(false);
      expect(workflowFilesInputSchema.safeParse({ ...base, change: '' }).success).toBe(false);
      expect(
        workflowFilesInputSchema.safeParse({ ...base, change: 42 as unknown as string }).success,
      ).toBe(false);
      expect(workflowFilesInputSchema.safeParse({ ...base, change: 'c' }).success).toBe(true);
    });

    it('workflow_files description 与注册字面量字节级相等，且含只读查询 / 从不修改 workflow.json / 不含 source 审计映射 / 缺失硬报错不回退 git diff / 补录修正请用 change_files 要点 (AC-1)', async () => {
      const desc = await getToolDescription(client, 'workflow_files');
      expect(desc).toBe(EXPECTED_TOOL_DESCRIPTIONS.workflow_files);
      expect(desc).toContain('Read-only query');
      expect(desc).toContain('never modifies workflow.json');
      expect(desc).toContain('`source` audit map is never exposed');
      expect(desc).toContain('no git diff fallback');
      expect(desc).toContain('`change_files`');
    });
  });

  describe('workflow_files 查询链路 — 正向端到端 (AC-1, AC-2)', () => {
    it('真实 fixture（files 含 source）callTool workflow_files → 非 isError，返回 { written, deleted }，序列化文本不含 "source"；查询前后 workflow.json 原始文本逐字节一致 (AC-1, AC-2)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-e2e';
      const changeDir = setupChangeWithFiles(dir, changeName, {
        written: ['src/a.ts', 'src/b.ts'],
        deleted: ['src/old.ts'],
        source: { 'src/a.ts': 'dev-team:implementation-generator' },
      });
      setResolvedRoot(dir);
      const rawBefore = fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        expect(workflowFilesOutputSchema.safeParse(data).success).toBe(true);
        expect(data).toEqual({
          written: ['src/a.ts', 'src/b.ts'],
          deleted: ['src/old.ts'],
        });
        expect(extractText(result)).not.toContain('"source"');
        const structured = (result as { structuredContent?: unknown }).structuredContent;
        expect(structured).toEqual({
          written: ['src/a.ts', 'src/b.ts'],
          deleted: ['src/old.ts'],
        });
        expect(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')).toBe(rawBefore);
      } finally {
        cleanup();
      }
    });

    it('change_create 初始空净状态 change → 查询返回 { written: [], deleted: [] }（链路对最小净状态成立）(AC-2)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-empty';
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(result)).toBe(false);
        expect(JSON.parse(extractText(result))).toEqual({ written: [], deleted: [] });
      } finally {
        cleanup();
      }
    });

    it('省略 change（schema 必填缺失）→ isError 且 getChangedFiles spy 次数 0（校验层拦截，不到查询模块）(AC-1)', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(workflowFilesCmd, 'getChangedFiles');
      spy.mockClear();
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({}, process.cwd()),
        });
        expect(isToolError(result)).toBe(true);
        expect(spy).toHaveBeenCalledTimes(0);
      } finally {
        spy.mockRestore();
      }
    });

    it('resolve 错误映射：mock 抛 not_in_candidates → isError JSON（code/project_root/candidates/force_hint 齐全）且 getChangedFiles spy 次数 0', async () => {
      setResolveError(
        makeResolveError('not_in_candidates', 'not in', {
          project_root: '/tmp/x',
          candidates: ['/tmp/a'],
          force_hint: 'Resubmit the same tool call with identical complete arguments to force-add',
        }),
      );
      const spy = vi.spyOn(workflowFilesCmd, 'getChangedFiles');
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: 'c' }, '/tmp/x'),
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
      } finally {
        spy.mockRestore();
      }
    });
  });

  describe('workflow_files 四态硬报错经 MCP 层映射为 isError（不回退 git diff）(AC-3)', () => {
    it('态① workflow.json 不存在 → isError 且文案含「不存在」与 change_create 指引 (AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-state1-missing';
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', changeName), { recursive: true });
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).toContain('不存在');
        expect(extractText(result)).toContain('change_create');
      } finally {
        cleanup();
      }
    });

    it('态② JSON 截断 → isError 且文案含「解析失败」(AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-state2-broken';
      const changeDir = setupChangeWithWorkflow(dir, changeName);
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), '{"workflow_type": "requ', 'utf-8');
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).toContain('解析失败');
      } finally {
        cleanup();
      }
    });

    it('态③ files 类型非法（schema 不通过）→ isError 且文案含「格式非法」(AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-state3-invalid';
      setupChangeWithFiles(dir, changeName, { written: 'src/a.ts', deleted: [] });
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).toContain('格式非法');
      } finally {
        cleanup();
      }
    });

    it('态④ 缺 files 字段 → isError 且文案含「创建于文件清单机制之前，请重建」(AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-state4-legacy';
      const changeDir = path.join(dir, 'openspec', 'changes', changeName);
      fs.mkdirSync(changeDir, { recursive: true });
      fs.writeFileSync(
        path.join(changeDir, 'workflow.json'),
        JSON.stringify({ workflow_type: 'requirement', created: '2026-01-01' }),
        'utf-8',
      );
      setResolvedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).toContain('创建于文件清单机制之前，请重建');
      } finally {
        cleanup();
      }
    });

    it('四态错误均不修改盘上 workflow.json、不创建文件；文案均不含 git diff 字样（无回退语义）(AC-3)', async () => {
      const { dir, cleanup } = setupTempProject();

      // 态①：change 目录存在但无 workflow.json
      const s1 = path.join(dir, 'openspec', 'changes', 'wf-ro-1');
      fs.mkdirSync(s1, { recursive: true });

      // 态②：JSON 截断
      const s2 = setupChangeWithWorkflow(dir, 'wf-ro-2');
      fs.writeFileSync(path.join(s2, 'workflow.json'), '{"workflow_type": "requ', 'utf-8');

      // 态③：files.written 非数组
      const s3 = setupChangeWithFiles(dir, 'wf-ro-3', { written: 42, deleted: [] });

      // 态④：缺 files 字段
      const s4 = path.join(dir, 'openspec', 'changes', 'wf-ro-4');
      fs.mkdirSync(s4, { recursive: true });
      fs.writeFileSync(
        path.join(s4, 'workflow.json'),
        JSON.stringify({ workflow_type: 'requirement' }),
        'utf-8',
      );

      const before = new Map(
        [s1, s2, s3, s4].map((changeDir) => [
          changeDir,
          {
            raw: fs.existsSync(path.join(changeDir, 'workflow.json'))
              ? (fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8') as string | null)
              : null,
            entries: fs.readdirSync(changeDir).sort(),
          },
        ]),
      );

      setResolvedRoot(dir);
      try {
        for (const changeName of ['wf-ro-1', 'wf-ro-2', 'wf-ro-3', 'wf-ro-4']) {
          const result = await client.callTool({
            name: 'workflow_files',
            arguments: withProjectRoot({ change: changeName }, dir),
          });
          expect(isToolError(result)).toBe(true);
          expect(extractText(result)).not.toMatch(/git diff|diff --git/i);
        }

        for (const [changeDir, snapshot] of before) {
          const filePath = path.join(changeDir, 'workflow.json');
          if (snapshot.raw === null) {
            expect(fs.existsSync(filePath)).toBe(false);
          } else {
            expect(fs.readFileSync(filePath, 'utf-8')).toBe(snapshot.raw);
          }
          expect(fs.readdirSync(changeDir).sort()).toEqual(snapshot.entries);
        }
      } finally {
        cleanup();
      }
    });
  });

  describe('workflow_files × change_files 读写闭环 (AC-2, AC-4)', () => {
    it('change_files append 后 callTool workflow_files → 净状态含新写入路径，与写工具返回的净状态一致 (AC-2, AC-4)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-loop-append';
      setupChangeWithFiles(dir, changeName, {
        written: ['src/a.ts'],
        deleted: [],
        source: { 'src/a.ts': 'dev-team:implementation-generator' },
      });
      setResolvedRoot(dir);
      try {
        const writeResult = await client.callTool({
          name: 'change_files',
          arguments: withProjectRoot(
            { change: changeName, op: 'append', written: ['src/new.ts'] },
            dir,
          ),
        });
        expect(isToolError(writeResult)).toBe(false);
        const writtenState = JSON.parse(extractText(writeResult));
        expect(writtenState.written).toEqual(expect.arrayContaining(['src/a.ts', 'src/new.ts']));

        const readResult = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(readResult)).toBe(false);
        const readState = JSON.parse(extractText(readResult));
        expect(readState).toEqual(writtenState);
        expect(readState.written).toEqual(expect.arrayContaining(['src/a.ts', 'src/new.ts']));
      } finally {
        cleanup();
      }
    });

    it('预置 source 的 fixture：append 后查询输出仍无 source，而盘上 files.source 原样保留（写读两端对 source 的契约分工正确）(AC-2)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-loop-source';
      const changeDir = setupChangeWithFiles(dir, changeName, {
        written: ['src/a.ts'],
        deleted: [],
        source: { 'src/a.ts': 'dev-team:implementation-generator' },
      });
      setResolvedRoot(dir);
      try {
        const writeResult = await client.callTool({
          name: 'change_files',
          arguments: withProjectRoot(
            { change: changeName, op: 'append', written: ['src/new.ts'] },
            dir,
          ),
        });
        expect(isToolError(writeResult)).toBe(false);

        const readResult = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(readResult)).toBe(false);
        expect(extractText(readResult)).not.toContain('"source"');

        const doc = JSON.parse(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8')) as {
          files: { source?: Record<string, string> };
        };
        expect(doc.files.source).toEqual({
          'src/a.ts': 'dev-team:implementation-generator',
        });
      } finally {
        cleanup();
      }
    });

    it('set 覆写后查询可见覆写结果（写通道两种 op 与读通道均闭环）(AC-2, AC-4)', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'wf-loop-set';
      setupChangeWithWorkflow(dir, changeName);
      setResolvedRoot(dir);
      try {
        const writeResult = await client.callTool({
          name: 'change_files',
          arguments: withProjectRoot(
            {
              change: changeName,
              op: 'set',
              written: ['src/rewrite.ts'],
              deleted: ['src/gone.ts'],
            },
            dir,
          ),
        });
        expect(isToolError(writeResult)).toBe(false);

        const readResult = await client.callTool({
          name: 'workflow_files',
          arguments: withProjectRoot({ change: changeName }, dir),
        });
        expect(isToolError(readResult)).toBe(false);
        expect(JSON.parse(extractText(readResult))).toEqual({
          written: ['src/rewrite.ts'],
          deleted: ['src/gone.ts'],
        });
      } finally {
        cleanup();
      }
    });
  });

  describe('archi_check — change 传参与 staged 显式报错 (AC-9)', () => {
    it('tools/call 传 change → runCrossRefCheck 收到 change 传参 (AC-9)', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(c4CrossRef, 'runCrossRefCheck');
      try {
        const result = await client.callTool({
          name: 'archi_check',
          arguments: withProjectRoot({ change: 'my-change' }),
        });

        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith(process.cwd(), {
          staged: undefined,
          files: undefined,
          change: 'my-change',
        });
        expect(JSON.parse(extractText(result)).status).toBe('clean');
      } finally {
        spy.mockRestore();
      }
    });

    it('tools/call 传 files → 拆分逗号分隔列表并透传 (AC-9)', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(c4CrossRef, 'runCrossRefCheck');
      try {
        await client.callTool({
          name: 'archi_check',
          arguments: withProjectRoot({ files: ' src/a.ts , src/b.ts ,' }),
        });

        expect(spy).toHaveBeenCalledWith(process.cwd(), {
          staged: undefined,
          files: ['src/a.ts', 'src/b.ts'],
          change: undefined,
        });
      } finally {
        spy.mockRestore();
      }
    });

    it('staged: true → 工具调用返回错误（显式废弃别名，AC-9）', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(c4CrossRef, 'runCrossRefCheck');
      spy.mockClear();
      try {
        const result = await client.callTool({
          name: 'archi_check',
          arguments: withProjectRoot({ staged: true }),
        });

        expect(isToolError(result)).toBe(true);
        expect(spy).toHaveBeenCalledTimes(1);
        expect(extractText(result)).toContain('staged 模式已由清单模式替代');
      } finally {
        spy.mockRestore();
      }
    });
  });

  describe('test_resolve_paths — 清单模式接线 (AC-8)', () => {
    it('tools/call modules="change" + change → runTestResolvePaths 收到新入参形态 (AC-8)', async () => {
      const { dir, cleanup } = setupTempProject();
      setupChangeWithWorkflow(dir, 'tr-change');
      setResolvedRoot(dir);
      const spy = vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths');
      try {
        const result = await client.callTool({
          name: 'test_resolve_paths',
          arguments: withProjectRoot({ modules: 'change', change: 'tr-change' }, dir),
        });

        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledWith({
          modules: 'change',
          change: 'tr-change',
          project_root: dir,
        });
        const data = JSON.parse(extractText(result));
        expect(data).toHaveProperty('unit_tests');
        expect(data).toHaveProperty('errors');
      } finally {
        spy.mockRestore();
        cleanup();
      }
    });

    it('modules="change" 缺 change → schema refine 校验拒绝（AC-8）', async () => {
      setResolvedRoot(process.cwd());
      const spy = vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths');
      spy.mockClear();
      try {
        const result = await client.callTool({
          name: 'test_resolve_paths',
          arguments: withProjectRoot({ modules: 'change' }),
        });

        expect(isToolError(result)).toBe(true);
        expect(spy).toHaveBeenCalledTimes(0);
      } finally {
        spy.mockRestore();
      }
    });
  });

  describe('工具清单回归 — change_files 为纯新增', () => {
    it('既有工具注册名称不变且总数为 16（workflow_files 为纯新增，change_files 注册 / description 不变）', async () => {
      const names = await getRegisteredToolNames(client);
      for (const name of [
        'phase_log',
        'phase_next',
        'backtrack',
        'change_create',
        'change_list',
        'spec_list',
        'config_get',
        'archi_check',
        'archi_query',
        'archi_validate',
        'archi_write',
        'archi_decide',
        'test_detect_frameworks',
        'test_resolve_paths',
      ]) {
        expect(names).toContain(name);
      }
      expect(names).toContain('change_files');
      expect(names).toHaveLength(16);
    });
  });
});
