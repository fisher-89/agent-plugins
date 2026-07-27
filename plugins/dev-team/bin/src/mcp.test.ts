/**
 * 单元测试: mcp.ts — MCP 工具注册与调用验证 (via InMemoryTransport)
 *
 * 使用 McpServer + Client + InMemoryTransport 进行真实 MCP 协议级测试。
 *
 * @see openspec/changes/mcp-project-root-lock/test-design.md
 * @see openspec/changes/write-protection-config/test-design.md
 * @see openspec/changes/remove-integration-test-path-resolution/test-design.md
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
  changeListInputSchema,
  changeListOutputSchema,
  type TestDetectFrameworksResult,
} from './schemas';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockLockedRoot = vi.hoisted(() => ({ value: process.cwd() as string | null }));

vi.mock('./lib/project-root', () => {
  class ProjectRootLockError extends Error {
    readonly code: string;
    constructor(code: string, message: string) {
      super(message);
      this.name = 'ProjectRootLockError';
      this.code = code;
    }
  }

  return {
    initProjectRootFromMcp: vi.fn().mockResolvedValue(undefined),
    getMcpCachedProjectRoot: vi.fn(() => mockLockedRoot.value),
    requireLockedProjectRoot: vi.fn(() => {
      if (!mockLockedRoot.value) {
        throw new ProjectRootLockError(
          'not_locked',
          'Project root is not locked: no unique usable project root found',
        );
      }
      return mockLockedRoot.value;
    }),
    getProjectDir: vi.fn(() => mockLockedRoot.value ?? process.cwd()),
    ProjectRootLockError,
    isProjectRootLockError: vi.fn(
      (err: unknown) =>
        err instanceof Error &&
        err.name === 'ProjectRootLockError' &&
        typeof Reflect.get(err, 'code') === 'string',
    ),
  };
});

import {
  getMcpCachedProjectRoot,
  getProjectDir,
  initProjectRootFromMcp,
  isProjectRootLockError,
  ProjectRootLockError,
  requireLockedProjectRoot,
} from './lib/project-root';

/** Exact tool names registered by mcp.ts (sorted). StringLiteral name mutants must fail. */
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

/** Exact description literals from mcp.ts MCP_TOOLS (byte-level toBe). */
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

const LOCK_ERROR_FIXTURE_MSG =
  'Project root is not locked: fixture-reason. Set CLAUDE_PROJECT_DIR to an existing absolute path, or provide exactly one MCP root / WORKSPACE_FOLDER_PATHS entry.';

function assertLockErrorResult(
  result: Awaited<ReturnType<Client['callTool']>>,
  expectedMessage: string,
): void {
  expect(result).toMatchObject({ isError: true });
  const content = Reflect.get(result, 'content');
  const item = Array.isArray(content) ? content[0] : undefined;
  expect(item).toMatchObject({ type: 'text', text: expectedMessage });
  expect(
    typeof item === 'object' &&
      item !== null &&
      'type' in item &&
      item.type === 'text' &&
      'text' in item &&
      item.text === expectedMessage,
  ).toBe(true);
}

// Lazy-loaded archi modules (mcp.ts dynamic import) — mock at the import boundary
// so connect/callTool never cold-loads @likec4/language-services.
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

// Keep c4-parser mocked as a safety net for any transitive static imports.
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

/**
 * 从 callTool() 返回值中安全提取第一个 text content 的字符串值。
 */
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

/** 通过 client.listTools() 获取所有已注册工具名称 */
async function getRegisteredToolNames(client: Client): Promise<string[]> {
  const { tools } = await client.listTools();
  return tools.map((t) => t.name);
}

/** 通过 client.listTools() 获取指定工具的 description */
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

/** 创建临时目录并初始化 openspec/config.json，返回目录路径和清理函数 */
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

function setLockedRoot(dir: string | null): void {
  mockLockedRoot.value = dir;
  vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
    if (!mockLockedRoot.value) {
      throw new ProjectRootLockError(
        'not_locked',
        'Project root is not locked: no unique usable project root found',
      );
    }
    return mockLockedRoot.value;
  });
  vi.mocked(getMcpCachedProjectRoot).mockReturnValue(dir);
  vi.mocked(getProjectDir).mockReturnValue(dir ?? process.cwd());
}

// ---------------------------------------------------------------------------
// MCP Server — 集成测试套件
// ---------------------------------------------------------------------------

describe('MCP Server (via InMemoryTransport)', () => {
  let server: McpServer;
  let client: Client;

  beforeAll(async () => {
    mockLockedRoot.value = process.cwd();
    vi.mocked(initProjectRootFromMcp).mockClear();
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const { connectToServer } = await import('./mcp');
    server = await connectToServer(serverTransport);
    client = new Client({ name: 'test-client', version: '1.0.0' }, { capabilities: {} });
    await client.connect(clientTransport);
    // Startup lock: connectToServer must init project root after transport connect.
    expect(initProjectRootFromMcp).toHaveBeenCalledTimes(1);
  });

  afterAll(async () => {
    await client.close();
    await server.close();
  });

  beforeEach(() => {
    setLockedRoot(process.cwd());
    vi.mocked(isProjectRootLockError).mockImplementation(
      (err: unknown) =>
        err instanceof Error &&
        err.name === 'ProjectRootLockError' &&
        typeof Reflect.get(err, 'code') === 'string',
    );
  });

  // -------------------------------------------------------------------------
  // MCP 注册 — 工具 name / description 精确断言
  // -------------------------------------------------------------------------

  describe('MCP 注册 — 工具 name 精确断言', () => {
    it('listTools() name 集合经 sort 后严格等于预期 11 个 name（逐元素 ===）', async () => {
      const names = (await getRegisteredToolNames(client)).slice().sort();
      expect(names).toEqual([...EXPECTED_TOOL_NAMES]);
      expect(names).toHaveLength(11);
    });

    it('对每个预期 name 单独 toContain（防止集合宽松匹配绕过）', async () => {
      const names = await getRegisteredToolNames(client);
      for (const toolName of EXPECTED_TOOL_NAMES) {
        expect(names).toContain(toolName);
      }
    });

    it('name 集合不得包含近似变体 phaseLog / Phase_Log / list_changed / changeList / config-get', async () => {
      const names = await getRegisteredToolNames(client);
      for (const bad of [
        'phaseLog',
        'Phase_Log',
        'list_changed',
        'roots',
        'changeList',
        'config-get',
      ]) {
        expect(names).not.toContain(bad);
      }
    });
  });

  describe('MCP 注册 — 工具 description 精确字面量', () => {
    it('每个工具 description 与 mcp.ts 注册串字节级相等（toBe）', async () => {
      for (const name of EXPECTED_TOOL_NAMES) {
        const desc = await getToolDescription(client, name);
        expect(desc).toBe(EXPECTED_TOOL_DESCRIPTIONS[name]);
      }
    });

    it('任一工具 description 不得为空串 / undefined', async () => {
      for (const name of EXPECTED_TOOL_NAMES) {
        const desc = await getToolDescription(client, name);
        expect(desc).toBeDefined();
        expect(desc!.length).toBeGreaterThan(0);
      }
    });
  });

  describe('MCP 注册 — server identity', () => {
    it("getServerVersion() name === 'dev-team' 且 version === '2.8.11'（字节级）", () => {
      const info = client.getServerVersion();
      expect(info?.name).toBe('dev-team');
      expect(info?.version).toBe('2.8.11');
    });

    it("name 不得为 '' / DevTeam / dev_team / undefined；version 不得为 '' / 0 / 缺字段", () => {
      const info = client.getServerVersion();
      expect(info?.name).not.toBe('');
      expect(info?.name).not.toBe('DevTeam');
      expect(info?.name).not.toBe('dev_team');
      expect(info?.name).toBeDefined();
      expect(info?.version).not.toBe('');
      expect(info?.version).not.toBe('0');
      expect(info).toHaveProperty('version');
    });

    it("version 字符串长度与精确字面量 '2.8.11' 一致（禁止仅 /^2\\./ 宽松匹配）", () => {
      const info = client.getServerVersion();
      expect(info?.version).toBe('2.8.11');
      expect(info?.version?.length).toBe('2.8.11'.length);
    });
  });

  describe('MCP 注册 — 无 list_changed handler', () => {
    it('服务端未注册 notifications/roots/list_changed handler（能力广告/handler 探测均无该通知）', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).not.toContain('notifications/roots/list_changed');
      expect(names.join(',')).not.toMatch(/list_changed/i);
    });
  });

  // -------------------------------------------------------------------------
  // MCP schema — 无 input project_root
  // -------------------------------------------------------------------------

  describe('MCP schema — 无 input project_root', () => {
    const toolsWithoutProjectRoot = [
      'change_list',
      'config_get',
      'archi_query',
      'archi_validate',
      'archi_write',
      'archi_check',
      'test_detect_frameworks',
      'test_resolve_paths',
    ];

    it('listTools 中 change_list / config_get / archi_* / test_* 的 inputSchema.properties 均不含 project_root 与 projectRoot (AC-4)', async () => {
      for (const name of toolsWithoutProjectRoot) {
        const schema = await getToolInputSchema(client, name);
        expect(schema).toBeDefined();
        const props = schema?.properties ?? {};
        expect(props).not.toHaveProperty('project_root');
        expect(props).not.toHaveProperty('projectRoot');
      }
    });

    it('直接 import changeListInputSchema：shape 无 project_root；safeParse({}) 成功 (AC-4)', () => {
      expect(Object.keys(changeListInputSchema.shape)).not.toContain('project_root');
      expect(Object.keys(changeListInputSchema.shape)).not.toContain('projectRoot');
      const parsed = changeListInputSchema.safeParse({});
      expect(parsed.success).toBe(true);
    });

    it("callTool('change_list', { project_root: otherDir })：若成功则输出 project_root === lockedRoot !== otherDir；若失败则 isError === true（禁止用 otherDir 覆盖锁定根）(AC-4)", async () => {
      const { dir, cleanup } = setupTempProject();
      const other = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-other-root-'));
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'change_list',
          arguments: { project_root: other },
        });
        if (isToolError(result)) {
          expect(isToolError(result)).toBe(true);
        } else {
          const data: { project_root: string } = JSON.parse(extractText(result));
          expect(data.project_root).toBe(dir);
          expect(data.project_root).not.toBe(other);
        }
      } finally {
        cleanup();
        fs.rmSync(other, { recursive: true, force: true });
      }
    });
  });
  // -------------------------------------------------------------------------
  // MCP 工具调用 — 未锁定入口校验
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — 未锁定入口校验', () => {
    it('requireLocked 抛锁错误时 phase_log / phase_next / backtrack 均 isError === true，content[0].text 匹配 /not locked|Project root/i，且 cwd 目录列表不变 (AC-5)', async () => {
      setLockedRoot(null);
      const cwdBefore = fs.readdirSync(process.cwd());

      const phaseLog = await client.callTool({
        name: 'phase_log',
        arguments: {
          change: 'no-such-change',
          phase: 'proposal',
          report: 'x',
          checklist: [{ item: 'i', pass: true, evidence: 'e' }],
        },
      });
      expect(isToolError(phaseLog)).toBe(true);
      expect(extractText(phaseLog)).toMatch(/not locked|Project root/i);

      const phaseNext = await client.callTool({
        name: 'phase_next',
        arguments: { change: 'no-such-change' },
      });
      expect(isToolError(phaseNext)).toBe(true);
      expect(extractText(phaseNext)).toMatch(/not locked|Project root/i);

      const backtrack = await client.callTool({
        name: 'backtrack',
        arguments: {
          change: 'no-such-change',
          phase: 'proposal',
          backtrack_to: 'explore',
          backtrack_reason: 'test',
        },
      });
      expect(isToolError(backtrack)).toBe(true);
      expect(extractText(backtrack)).toMatch(/not locked|Project root/i);

      expect(fs.readdirSync(process.cwd())).toEqual(cwdBefore);
    });

    it('未锁定时 change_list / config_get / test_resolve_paths / test_detect_frameworks / archi_query 均 isError === true 且 text 非空 (AC-5)', async () => {
      setLockedRoot(null);

      for (const tool of [
        { name: 'change_list', arguments: {} },
        { name: 'config_get', arguments: { key: 'schema' } },
        { name: 'test_resolve_paths', arguments: { modules: [] } },
        { name: 'test_detect_frameworks', arguments: {} },
        { name: 'archi_query', arguments: {} },
      ] as const) {
        const result = await client.callTool(tool);
        expect(isToolError(result)).toBe(true);
        expect(extractText(result).length).toBeGreaterThan(0);
      }
    });
  });

  describe('MCP 工具调用 — isError content 精确文本', () => {
    afterEach(() => {
      setLockedRoot(process.cwd());
    });

    it('mock requireLocked 抛固定 MSG 时 change_list / phase_next / backtrack / config_get：isError===true、content[0].type===text、text===MSG (AC-2/AC-5)', async () => {
      vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
        throw new ProjectRootLockError('not_locked', LOCK_ERROR_FIXTURE_MSG);
      });
      vi.mocked(getMcpCachedProjectRoot).mockReturnValue(null);

      for (const tool of [
        { name: 'change_list', arguments: {} },
        { name: 'phase_next', arguments: { change: 'x' } },
        {
          name: 'backtrack',
          arguments: {
            change: 'x',
            phase: 'proposal',
            backtrack_to: 'explore',
            backtrack_reason: 'r',
          },
        },
        { name: 'config_get', arguments: { key: 'schema' } },
      ] as const) {
        const result = await client.callTool(tool);
        assertLockErrorResult(result, LOCK_ERROR_FIXTURE_MSG);
        expect(extractText(result)).not.toBe(LOCK_ERROR_FIXTURE_MSG + 'x');
        expect(extractText(result)).not.toBe(LOCK_ERROR_FIXTURE_MSG.slice(0, 20));
      }
    });

    it('multi_root / literal_env 锁错误仍返回 isError 且 text === 抛出 message 原文 (AC-2)', async () => {
      for (const [code, msg] of [
        ['multi_root', 'Project root is not locked: multi usable paths from WORKSPACE'],
        ['literal_env', 'Project root is not locked: literal ${workspaceFolder} rejected'],
      ] as const) {
        vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
          throw new ProjectRootLockError(code, msg);
        });
        vi.mocked(getMcpCachedProjectRoot).mockReturnValue(null);
        const result = await client.callTool({ name: 'change_list', arguments: {} });
        assertLockErrorResult(result, msg);
      }
    });
  });

  describe('MCP 工具调用 — withLockedProjectRoot 错误映射', () => {
    afterEach(() => {
      setLockedRoot(process.cwd());
    });

    it("鸭类型 Error{name:'ProjectRootLockError', code:'not_locked'}（非 instanceof）经 handler 仍返回 isError，且 content[0].text === 抛出 message 原文 (AC-5)", async () => {
      const duckMessage = 'Project root is not locked: duck-typed module copy exact';
      const guardSpy = vi.mocked(isProjectRootLockError);
      guardSpy.mockClear();
      vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
        const err = new Error(duckMessage);
        err.name = 'ProjectRootLockError';
        Object.defineProperty(err, 'code', { value: 'not_locked' });
        throw err;
      });
      vi.mocked(getMcpCachedProjectRoot).mockReturnValue(null);

      const result = await client.callTool({ name: 'change_list', arguments: {} });
      assertLockErrorResult(result, duckMessage);
      // 杀死 if (!isProjectRootLockError(err)) 被恒 true/false 替换：守卫必须被调用
      expect(guardSpy).toHaveBeenCalled();
      expect(guardSpy.mock.results.some((r) => r.type === 'return' && r.value === true)).toBe(true);
    });

    it("仅 name='ProjectRootLockError' 但无 code 时不得被 isProjectRootLockError 识别；handler 不得返回成功 change_list JSON（与 isLockError 假分支一致）(AC-5)", async () => {
      const err = new Error('named-only-no-code');
      err.name = 'ProjectRootLockError';
      expect(isProjectRootLockError(err)).toBe(false);

      const guardSpy = vi.mocked(isProjectRootLockError);
      guardSpy.mockClear();
      vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
        throw err;
      });
      vi.mocked(getMcpCachedProjectRoot).mockReturnValue(null);

      let threw = false;
      let result: unknown;
      try {
        result = await client.callTool({ name: 'change_list', arguments: {} });
      } catch {
        threw = true;
      }

      expect(guardSpy).toHaveBeenCalled();
      expect(guardSpy.mock.results.some((r) => r.type === 'return' && r.value === false)).toBe(
        true,
      );
      if (!threw) {
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).not.toMatch(/"changes"\s*:/);
      }
    });

    it("requireLocked 抛普通 Error('boom')（非锁错误）时守卫返回 false 且被调用；不得返回成功 change_list JSON (AC-5)", async () => {
      const boom = new Error('boom');
      expect(isProjectRootLockError(boom)).toBe(false);

      const guardSpy = vi.mocked(isProjectRootLockError);
      guardSpy.mockClear();
      vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
        throw boom;
      });
      vi.mocked(getMcpCachedProjectRoot).mockReturnValue(null);

      let threw = false;
      let result: unknown;
      try {
        result = await client.callTool({ name: 'change_list', arguments: {} });
      } catch {
        threw = true;
      }

      expect(guardSpy).toHaveBeenCalled();
      expect(guardSpy).toHaveBeenCalledWith(boom);
      expect(guardSpy.mock.results.some((r) => r.type === 'return' && r.value === false)).toBe(
        true,
      );
      if (!threw) {
        expect(isToolError(result)).toBe(true);
        expect(extractText(result)).not.toMatch(/"changes"\s*:/);
        expect(extractText(result)).toMatch(/boom/i);
      }
    });
  });

  // -------------------------------------------------------------------------
  // MCP 工具调用 — change_list 回显锁定根
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — change_list 回显与锁定根接线', () => {
    afterEach(() => {
      vi.restoreAllMocks();
      setLockedRoot(process.cwd());
    });

    it('fixture 锁定根 dir !== cwd 时无参 change_list：isError === false，解析 JSON 后 project_root === dir，project_root !== process.cwd()，且 count === changes.length (AC-6)', async () => {
      const { dir, cleanup } = setupTempProject();
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', 'wired-a'), { recursive: true });
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', 'wired-b'), { recursive: true });
      expect(path.resolve(dir)).not.toBe(path.resolve(process.cwd()));
      setLockedRoot(dir);
      try {
        const result = await client.callTool({ name: 'change_list', arguments: {} });
        expect(isToolError(result)).toBe(false);
        const data: {
          project_root: string;
          changes: unknown[];
          count: number;
        } = JSON.parse(extractText(result));
        expect(data.project_root).toBe(dir);
        expect(data.project_root).not.toBe(process.cwd());
        expect(data.count).toBe(data.changes.length);
        expect(data.count).toBe(2);
      } finally {
        cleanup();
      }
    });

    it('spy runChangeList：无参调用时被以锁定根字符串调用恰好一次；不得出现 undefined 或 cwd (AC-6)', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      const spy = vi.spyOn(changeListCmd, 'runChangeList');
      try {
        const result = await client.callTool({ name: 'change_list', arguments: {} });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        expect(spy).toHaveBeenCalledWith(dir);
        const arg = spy.mock.calls[0]?.[0];
        expect(arg).toBe(dir);
        expect(arg).not.toBe(process.cwd());
        const parsed = changeListOutputSchema.safeParse(JSON.parse(extractText(result)));
        expect(parsed.success).toBe(true);
        if (parsed.success) {
          expect(parsed.data.project_root).toBe(dir);
          expect(parsed.data).toHaveProperty('changes');
          expect(parsed.data).toHaveProperty('count');
        }
      } finally {
        cleanup();
      }
    });

    it('未锁定时 change_list：isError === true；响应 text 不得包含成功 JSON 的 "project_root":"<cwd>" (AC-6)', async () => {
      setLockedRoot(null);
      const result = await client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(true);
      const text = extractText(result);
      expect(text).toMatch(/not locked|Project root/i);
      expect(text).not.toMatch(
        new RegExp(`"project_root"\\s*:\\s*"${process.cwd().replace(/\\/g, '\\\\')}"`),
      );
    });
  });
  // -------------------------------------------------------------------------
  // MCP 工具调用 — 有参工具改用锁定根
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — 有参工具改用锁定根', () => {
    afterEach(() => {
      vi.restoreAllMocks();
      setLockedRoot(process.cwd());
    });

    it('spy runConfigGet：config_get 调用参数 projectRoot === lockedRoot 且等于 fixture，不等于 cwd (AC-4)', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      expect(path.resolve(dir)).not.toBe(path.resolve(process.cwd()));
      setLockedRoot(dir);
      const spy = vi.spyOn(configGetCmd, 'runConfigGet');
      try {
        const result = await client.callTool({
          name: 'config_get',
          arguments: { key: 'schema' },
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        expect(spy.mock.calls[0]?.[0]).toMatchObject({ key: 'schema', projectRoot: dir });
        expect(spy.mock.calls[0]?.[0].projectRoot).not.toBe(process.cwd());
      } finally {
        cleanup();
      }
    });

    it('spy runTestResolvePaths：传入的 project_root 为锁定根；即使 arguments 误带其它字段也不得改写该根 (AC-4)', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*.test.ts'] }],
      });
      fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');
      setLockedRoot(dir);
      const spy = vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths');
      try {
        const result = await client.callTool({
          name: 'test_resolve_paths',
          arguments: { modules: ['src/foo.ts'] },
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        const arg = spy.mock.calls[0]?.[0];
        expect(arg?.project_root).toBe(dir);
        expect(arg?.project_root).not.toBe(process.cwd());
      } finally {
        cleanup();
      }
    });

    it('config_get / test_detect_frameworks / test_resolve_paths 不传 project_root 时依赖锁定根读 fixture 成功 (AC-4)', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*.test.ts'] }],
      });
      fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');
      setLockedRoot(dir);
      try {
        const configResult = await client.callTool({
          name: 'config_get',
          arguments: { key: 'schema' },
        });
        expect(isToolError(configResult)).toBe(false);
        const configData: ConfigGetResult = JSON.parse(extractText(configResult));
        expect(configData).toMatchObject({ key: 'schema', exists: true });

        const detectResult = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: {},
        });
        expect(isToolError(detectResult)).toBe(false);
        const detectData: TestDetectFrameworksResult = JSON.parse(extractText(detectResult));
        expect(detectData).toHaveProperty('detected');

        const resolveResult = await client.callTool({
          name: 'test_resolve_paths',
          arguments: { modules: ['src/foo.ts'] },
        });
        expect(isToolError(resolveResult)).toBe(false);
        const resolveData: ResolveTestPathsResult = JSON.parse(extractText(resolveResult));
        expect(resolveData).toHaveProperty('unit_tests');
      } finally {
        cleanup();
      }
    });

    it('未锁定时 config_get / test_resolve_paths 返回 isError，且对应 command spy 调用次数为 0（证明在 requireLocked 处短路）(AC-4/AC-5)', async () => {
      setLockedRoot(null);
      const configSpy = vi.spyOn(configGetCmd, 'runConfigGet');
      const resolveSpy = vi.spyOn(testResolvePathsCmd, 'runTestResolvePaths');

      const configResult = await client.callTool({
        name: 'config_get',
        arguments: { key: 'schema' },
      });
      expect(isToolError(configResult)).toBe(true);
      expect(configSpy).toHaveBeenCalledTimes(0);

      const resolveResult = await client.callTool({
        name: 'test_resolve_paths',
        arguments: { modules: [] },
      });
      expect(isToolError(resolveResult)).toBe(true);
      expect(resolveSpy).toHaveBeenCalledTimes(0);
    });

    it('spy runTestDetectFrameworks：传入 { files, projectRoot: lockedRoot } 对象字面量 (AC-4)', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      const spy = vi.spyOn(testDetectFrameworksCmd, 'runTestDetectFrameworks');
      try {
        const result = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: { files: ['src/a.ts'] },
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        expect(spy).toHaveBeenCalledWith({ files: ['src/a.ts'], projectRoot: dir });
        expect(spy.mock.calls[0]?.[0]).toEqual({ files: ['src/a.ts'], projectRoot: dir });
      } finally {
        cleanup();
      }
    });
  });

  // -------------------------------------------------------------------------
  // MCP 工具调用 — phase_log / backtrack / archi_* handler 覆盖
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — phase_log / backtrack（锁定根 + spy）', () => {
    afterEach(() => {
      vi.restoreAllMocks();
      setLockedRoot(process.cwd());
    });

    it('spy runPhaseLog：锁定后 callTool 成功且 handler 被调用（非 undefined）', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      const spy = vi.spyOn(phaseLogCmd, 'runPhaseLog').mockReturnValue({
        written: true,
        phase: 'proposal',
        attempt: 1,
      });
      try {
        const result = await client.callTool({
          name: 'phase_log',
          arguments: {
            change: 'c',
            phase: 'proposal',
            report: 'r',
            checklist: [{ item: 'i', pass: true, evidence: 'e' }],
          },
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        const data = JSON.parse(extractText(result));
        expect(data).toMatchObject({ written: true, phase: 'proposal', attempt: 1 });
      } finally {
        cleanup();
      }
    });

    it('spy runBacktrack：锁定后 callTool 成功且返回 modified/phase/target', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      const spy = vi.spyOn(backtrackCmd, 'runBacktrack').mockReturnValue({
        modified: true,
        phase: 'proposal',
        target: 'explore',
      });
      try {
        const result = await client.callTool({
          name: 'backtrack',
          arguments: {
            change: 'c',
            phase: 'proposal',
            backtrack_to: 'explore',
            backtrack_reason: 'reason',
          },
        });
        expect(isToolError(result)).toBe(false);
        expect(spy).toHaveBeenCalledTimes(1);
        const data = JSON.parse(extractText(result));
        expect(data).toEqual({ modified: true, phase: 'proposal', target: 'explore' });
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — archi_* 锁定根接线与 archi_check files/staged', () => {
    afterEach(() => {
      vi.restoreAllMocks();
      setLockedRoot(process.cwd());
    });

    it('archi_query / archi_validate / archi_write 将锁定根传入 lazy lib（非 cwd）', async () => {
      const { dir, cleanup } = setupTempProject();
      expect(path.resolve(dir)).not.toBe(path.resolve(process.cwd()));
      setLockedRoot(dir);
      const querySpy = vi.spyOn(archiQuery, 'queryModel');
      const validateSpy = vi.spyOn(archiValidate, 'validateDsl');
      const writeSpy = vi.spyOn(archiWrite, 'writeDsl');
      try {
        const q = await client.callTool({ name: 'archi_query', arguments: { element: 'sys' } });
        expect(isToolError(q)).toBe(false);
        expect(querySpy).toHaveBeenCalledWith(dir, 'sys');

        const v = await client.callTool({
          name: 'archi_validate',
          arguments: { source: 'model m' },
        });
        expect(isToolError(v)).toBe(false);
        expect(validateSpy).toHaveBeenCalledWith(dir, 'model m');

        const w = await client.callTool({
          name: 'archi_write',
          arguments: { source: 'model m', path: 'models/x.likec4' },
        });
        expect(isToolError(w)).toBe(false);
        expect(writeSpy).toHaveBeenCalledWith(dir, 'model m', 'models/x.likec4');
        const wData = JSON.parse(extractText(w));
        expect(wData).toMatchObject({ success: true, path: 'models/x.likec4' });
      } finally {
        cleanup();
      }
    });

    it('archi_check：files 逗号分隔经 trim/filter(Boolean)；staged 经 !! 传入 runCrossRefCheck', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      const spy = vi.spyOn(c4CrossRef, 'runCrossRefCheck');
      try {
        const withFiles = await client.callTool({
          name: 'archi_check',
          arguments: { files: ' a.ts , , b.ts ', staged: true },
        });
        expect(isToolError(withFiles)).toBe(false);
        expect(spy).toHaveBeenCalledWith(dir, { staged: true, files: ['a.ts', 'b.ts'] });

        spy.mockClear();
        const noFiles = await client.callTool({
          name: 'archi_check',
          arguments: { staged: false },
        });
        expect(isToolError(noFiles)).toBe(false);
        expect(spy).toHaveBeenCalledWith(dir, { staged: false, files: undefined });

        spy.mockClear();
        const stagedOmitted = await client.callTool({
          name: 'archi_check',
          arguments: { files: 'only.ts' },
        });
        expect(isToolError(stagedOmitted)).toBe(false);
        expect(spy).toHaveBeenCalledWith(dir, { staged: false, files: ['only.ts'] });
      } finally {
        cleanup();
      }
    });

    it("files: '' / ',' / 仅空白：lib 收到 files === undefined 或空过滤后无有效项；未锁定时 isError 且 spy 次数为 0", async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      const spy = vi.spyOn(c4CrossRef, 'runCrossRefCheck');
      try {
        for (const files of ['', ',', '  ,  ']) {
          spy.mockClear();
          const result = await client.callTool({
            name: 'archi_check',
            arguments: { files, staged: true },
          });
          expect(isToolError(result)).toBe(false);
          const arg = spy.mock.calls[0]?.[1] as { staged: boolean; files?: string[] };
          expect(arg.staged).toBe(true);
          // '' / ',' / 空白经 split+trim+filter(Boolean) 后为空数组（不得含空串）
          expect(
            arg.files === undefined || (Array.isArray(arg.files) && arg.files.length === 0),
          ).toBe(true);
          if (Array.isArray(arg.files)) {
            expect(arg.files.every((f) => f.length > 0)).toBe(true);
          }
        }

        setLockedRoot(null);
        spy.mockClear();
        const unlocked = await client.callTool({
          name: 'archi_check',
          arguments: { staged: true },
        });
        expect(isToolError(unlocked)).toBe(true);
        expect(spy).toHaveBeenCalledTimes(0);
      } finally {
        cleanup();
      }
    });
  });

  // -------------------------------------------------------------------------
  // MCP 工具调用 — 通过 client.callTool() + extractText() 验证 handler 行为
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — config_get（锁定根）', () => {
    it('返回 key、value 和 exists 字段', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'config_get',
          arguments: { key: 'schema' },
        });

        const data: ConfigGetResult = JSON.parse(extractText(result));
        expect(data).toMatchObject({ key: 'schema', exists: true });
        expect(data).toHaveProperty('value');
      } finally {
        cleanup();
      }
    });

    it('未设置的 key 返回 exists: false', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'config_get',
          arguments: { key: 'context' },
        });

        const data: ConfigGetResult = JSON.parse(extractText(result));
        expect(data.exists).toBe(false);
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — test_detect_frameworks（锁定根）', () => {
    it('返回 detected 字段', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: {},
        });

        const data: TestDetectFrameworksResult = JSON.parse(extractText(result));
        expect(data).toHaveProperty('detected');
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — test_resolve_paths（锁定根）', () => {
    it('返回 unit_tests 数组', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      setLockedRoot(dir);
      try {
        fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
        fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');

        const result = await client.callTool({
          name: 'test_resolve_paths',
          arguments: {
            modules: ['src/foo.ts'],
          },
        });

        const data: ResolveTestPathsResult = JSON.parse(extractText(result));
        expect(data).toHaveProperty('unit_tests');
        expect(Array.isArray(data.unit_tests)).toBe(true);
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — change_list（锁定根）', () => {
    it('返回 changes 数组', async () => {
      const { dir, cleanup } = setupTempProject();
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'change_list',
          arguments: {},
        });

        const data = JSON.parse(extractText(result));
        expect(data).toHaveProperty('changes');
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — phase_next', () => {
    it('返回下一阶段信息（只读，使用真实 change 目录）', async () => {
      const { dir, cleanup } = setupTempProject();
      const changeName = 'phase-next-fixture';
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', changeName), { recursive: true });
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'phase_next',
          arguments: { change: changeName },
        });

        expect(isToolError(result)).toBe(false);
        const data = JSON.parse(extractText(result));
        expect(typeof data === 'object' && data !== null).toBe(true);
        expect(data).toHaveProperty('next_phase');
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — archi_validate', () => {
    it('返回 valid 或 errors 字段（只读，验证当前模型 DSL）', async () => {
      const result = await client.callTool({
        name: 'archi_validate',
        arguments: {},
      });

      expect(isToolError(result)).toBe(false);
      const data: ArchiValidateResult = JSON.parse(extractText(result));
      expect(data).toMatchObject({ valid: true, errors: [] });
    });
  });

  describe('MCP 工具调用 — archi_query', () => {
    it('返回 elements 与 relationships 字段（只读）', async () => {
      const result = await client.callTool({
        name: 'archi_query',
        arguments: {},
      });

      expect(isToolError(result)).toBe(false);
      const data: ArchiQueryResult = JSON.parse(extractText(result));
      expect(data).toMatchObject({ elements: [], relationships: [] });
    });
  });

  describe('MCP 工具调用 — archi_check', () => {
    it('返回 violations 与 status 字段（只读，经 lazy c4-cross-ref）', async () => {
      const result = await client.callTool({
        name: 'archi_check',
        arguments: { staged: false },
      });

      expect(isToolError(result)).toBe(false);
      const data: ArchiCheckResult = JSON.parse(extractText(result));
      expect(data).toHaveProperty('violations');
      expect(data).toHaveProperty('status');
      expect(data.status).toBe('clean');
      expect(Array.isArray(data.violations)).toBe(true);
    });
  });

  describe('MCP 工具调用 — test_detect_frameworks（tests[]）', () => {
    it('既有 callTool 返回 detected 字段路径保持（夹具配置迁 tests[]）', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*.test.ts'] }],
      });
      setLockedRoot(dir);
      try {
        fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
        fs.writeFileSync(path.join(dir, 'src', 'foo.test.ts'), '', 'utf-8');
        const result = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: { files: ['src/foo.test.ts'] },
        });
        const data: TestDetectFrameworksResult = JSON.parse(extractText(result));
        expect(data).toHaveProperty('detected');
        expect(data).toHaveProperty('plan');
        expect(data.plan.length).toBeGreaterThan(0);
      } finally {
        cleanup();
      }
    });

    it('无有效 tests（或 tests: []）时 callTool 返回空 detected/plan 或明确错误结构，不抛未处理异常', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven', tests: [] });
      setLockedRoot(dir);
      try {
        const result = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: {},
        });
        const data: TestDetectFrameworksResult = JSON.parse(extractText(result));
        expect(data.plan).toEqual([]);
        expect(Array.isArray(data.detected)).toBe(true);
      } finally {
        cleanup();
      }
    });
  });

  // -------------------------------------------------------------------------
  // MCP 工具调用 — 全 11 工具 handler 非 undefined
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — 全 11 工具 handler 非 undefined', () => {
    afterEach(() => {
      setLockedRoot(process.cwd());
    });

    it('锁定后对 11 个工具各 callTool 一次：每一个 isError === false，且 content[0].text 非空 JSON；spy 计数表长度恰好 11', async () => {
      const { dir, cleanup } = setupTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest', includes: ['**/*'] }],
      });
      const changeName = 'handler-fixture';
      fs.mkdirSync(path.join(dir, 'openspec', 'changes', changeName), { recursive: true });
      fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');
      setLockedRoot(dir);

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
          const result = await client.callTool({ name, arguments: args });
          expect(isToolError(result)).toBe(false);
          const text = extractText(result);
          expect(text.length).toBeGreaterThan(0);
          expect(() => JSON.parse(text)).not.toThrow();
        }

        expect(Object.keys(spies)).toHaveLength(11);
        for (const [name, spy] of Object.entries(spies)) {
          expect(spy.mock.calls.length).toBeGreaterThanOrEqual(1);
          void name;
        }
      } finally {
        for (const spy of Object.values(spies)) {
          spy.mockRestore();
        }
        cleanup();
      }
    });

    it('未锁定时对全部 11 个工具各 callTool 一次：每一个 isError === true，且对应 command/lib spy 调用次数均为 0 (AC-5)', async () => {
      setLockedRoot(null);
      vi.mocked(requireLockedProjectRoot).mockImplementation(() => {
        throw new ProjectRootLockError('not_locked', LOCK_ERROR_FIXTURE_MSG);
      });
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
      expect(calls).toHaveLength(11);

      try {
        for (const { name, args } of calls) {
          const result = await client.callTool({ name, arguments: args });
          expect(isToolError(result)).toBe(true);
          expect(extractText(result).length).toBeGreaterThan(0);
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
  });
});

// ===========================================================================
// MCP 注册 — registerTool spy 精确字面量（独立 connect，避开共享 beforeAll）
// ===========================================================================

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

  it('按注册顺序对每次调用断言 name/description 字节级 toBe；调用次数恰好 11；handler 均为 function', async () => {
    mockLockedRoot.value = process.cwd();
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
        const handler = call[2];
        names.push(name);
        expect(name).toBe(REGISTER_ORDER[i]);
        expect(name).not.toBe('');
        expect(name).not.toBeUndefined();
        expect(name).not.toBe('changeList');
        expect(typeof config.description).toBe('string');
        expect(config.description).toBe(
          EXPECTED_TOOL_DESCRIPTIONS[name as (typeof EXPECTED_TOOL_NAMES)[number]],
        );
        expect(typeof handler).toBe('function');
        expect(handler).not.toBeUndefined();
      }
      expect(names).toEqual([...REGISTER_ORDER]);
    } finally {
      registerSpy.mockRestore();
      await client?.close();
      await server?.close();
    }
  });
});
