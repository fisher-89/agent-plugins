/**
 * 集成测试: MCP 项目根锁定 → 工具入口强制校验
 *
 * 不 mock project-root 模块；通过真实 connectToServer + InMemoryTransport 验证。
 *
 * @see openspec/changes/mcp-project-root-lock/test-design.md
 */

import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { Client } from '@modelcontextprotocol/sdk/client';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory';
import { type McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { ListRootsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const hooksBundlePath = path.join(
  fileURLToPath(new URL('../../', import.meta.url)),
  'dev-team-hooks.cjs',
);

function runHooksProtectFiles(
  event: Record<string, unknown>,
  env: NodeJS.ProcessEnv,
): { stdout: string; status: number | null } {
  const result = spawnSync(process.execPath, [hooksBundlePath, 'protect-files'], {
    input: JSON.stringify(event),
    encoding: 'utf-8',
    env: { ...process.env, ...env },
    cwd: path.dirname(hooksBundlePath),
  });
  return { stdout: result.stdout?.toString() ?? '', status: result.status };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ENV_KEYS = ['CLAUDE_PROJECT_DIR', 'WORKSPACE_FOLDER_PATHS'] as const;

function saveEnv(): Record<(typeof ENV_KEYS)[number], string | undefined> {
  return {
    CLAUDE_PROJECT_DIR: process.env.CLAUDE_PROJECT_DIR,
    WORKSPACE_FOLDER_PATHS: process.env.WORKSPACE_FOLDER_PATHS,
  };
}

function restoreEnv(saved: ReturnType<typeof saveEnv>): void {
  for (const key of ENV_KEYS) {
    if (saved[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = saved[key];
    }
  }
}

function clearProjectEnv(): void {
  delete process.env.CLAUDE_PROJECT_DIR;
  delete process.env.WORKSPACE_FOLDER_PATHS;
}

function setupTempProject(config?: Record<string, unknown>): {
  dir: string;
  cleanup: () => void;
} {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-root-lock-'));
  fs.mkdirSync(path.join(dir, 'openspec', 'changes'), { recursive: true });
  fs.writeFileSync(
    path.join(dir, 'openspec', 'config.json'),
    JSON.stringify(config ?? { schema: 'spec-driven' }),
  );
  return {
    dir,
    cleanup: () => fs.rmSync(dir, { recursive: true, force: true }),
  };
}

function extractText(result: unknown): string {
  if (typeof result !== 'object' || result === null || !('content' in result)) {
    throw new Error('expected callTool content result');
  }
  const content: unknown = result.content;
  if (!Array.isArray(content) || content.length === 0) {
    throw new Error('expected non-empty content');
  }
  const item: unknown = content[0];
  if (typeof item !== 'object' || item === null || !('type' in item) || !('text' in item)) {
    throw new Error('expected text content item');
  }
  if (item.type !== 'text' || typeof item.text !== 'string') {
    throw new Error('expected text string');
  }
  return item.text;
}

function isToolError(result: unknown): boolean {
  return (
    typeof result === 'object' && result !== null && 'isError' in result && result.isError === true
  );
}

function openspecExistsUnderCwd(): boolean {
  return fs.existsSync(path.join(process.cwd(), 'openspec', 'changes', 'lock-side-effect'));
}

interface ConnectedPair {
  server: McpServer;
  client: Client;
  close: () => Promise<void>;
}

async function connectPair(options?: {
  roots?: { uri: string; name?: string }[];
  /** 握手完成后再二次 init（规避 connect 内 listRoots 与 client 就绪竞态） */
  reinitAfterConnect?: boolean;
}): Promise<ConnectedPair & { listRootsCalls: number }> {
  vi.resetModules();
  const { connectToServer } = await import('../../src/mcp');
  const projectRoot = await import('../../src/lib/project-root');
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();

  let listRootsCalls = 0;
  const client = new Client(
    { name: 'lock-itest-client', version: '1.0.0' },
    { capabilities: options?.roots ? { roots: { listChanged: false } } : {} },
  );

  if (options?.roots) {
    client.setRequestHandler(ListRootsRequestSchema, async () => {
      listRootsCalls += 1;
      return { roots: options.roots! };
    });
  }

  // 并行握手：connectToServer 在 server.connect 后立刻 init/listRoots，可能早于 client 就绪
  const serverPromise = connectToServer(serverTransport);
  const clientPromise = client.connect(clientTransport);
  const [server] = await Promise.all([serverPromise, clientPromise]);

  if (options?.reinitAfterConnect && !projectRoot.getMcpCachedProjectRoot()) {
    await projectRoot.initProjectRootFromMcp(server.server);
  }

  return {
    server,
    client,
    listRootsCalls,
    close: async () => {
      await client.close();
      await server.close();
    },
  };
}

// ===========================================================================
// 场景: 唯一根锁定后工具可用
// ===========================================================================

describe('唯一根锁定后工具可用', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('CLAUDE_PROJECT_DIR 指向 temp 项目时 change_list 成功且不依赖 input project_root (AC-1)', async () => {
    const { dir, cleanup } = setupTempProject();
    fs.mkdirSync(path.join(dir, 'openspec', 'changes', 'active-one'), { recursive: true });
    process.env.CLAUDE_PROJECT_DIR = dir;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(false);
      const data: {
        project_root: string;
        changes: { name: string }[];
      } = JSON.parse(extractText(result));
      expect(data.project_root).toBe(dir);
      expect(data.changes.map((c) => c.name)).toContain('active-one');
    } finally {
      await pair.close();
      cleanup();
    }
  });

  it('roots 恰 1 且无 env 时锁定后 phase_next 可读 change 目录 (AC-1/AC-5)', async () => {
    const { dir, cleanup } = setupTempProject();
    const changeName = 'roots-phase-next';
    fs.mkdirSync(path.join(dir, 'openspec', 'changes', changeName), { recursive: true });
    const pair = await connectPair({
      roots: [{ uri: pathToFileURL(dir).href, name: 'temp' }],
      reinitAfterConnect: true,
    });
    try {
      expect(pair.listRootsCalls).toBeGreaterThan(0);

      const listResult = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(listResult)).toBe(false);
      const listData: { project_root: string } = JSON.parse(extractText(listResult));
      expect(listData.project_root).toBe(dir);

      const result = await pair.client.callTool({
        name: 'phase_next',
        arguments: { change: changeName },
      });
      expect(isToolError(result)).toBe(false);
      const data: { next_phase?: string } = JSON.parse(extractText(result));
      expect(data).toHaveProperty('next_phase');
    } finally {
      await pair.close();
      cleanup();
    }
  });

  it('仅 WORKSPACE_FOLDER_PATHS 单值（无 CLAUDE、无 roots）时 change_list.project_root 严格等于该 WORKSPACE 路径 (AC-1)', async () => {
    const { dir, cleanup } = setupTempProject();
    fs.mkdirSync(path.join(dir, 'openspec', 'changes', 'ws-only'), { recursive: true });
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(false);
      const data: { project_root: string } = JSON.parse(extractText(result));
      expect(data.project_root).toBe(dir);
    } finally {
      await pair.close();
      cleanup();
    }
  });

  it('三通道均失败时 phase_log/backtrack/config_get 返回 isError，temp cwd 下无 openspec/changes 副作用 (AC-2/AC-5)', async () => {
    clearProjectEnv();
    const pair = await connectPair();
    const marker = path.join(process.cwd(), 'openspec', 'changes', 'lock-side-effect');
    const existedBefore = fs.existsSync(marker);
    try {
      const phaseLog = await pair.client.callTool({
        name: 'phase_log',
        arguments: {
          change: 'lock-side-effect',
          phase: 'proposal',
          report: 'should not write',
          checklist: [{ item: 'i', pass: true, evidence: 'e' }],
        },
      });
      expect(isToolError(phaseLog)).toBe(true);

      const backtrack = await pair.client.callTool({
        name: 'backtrack',
        arguments: {
          change: 'lock-side-effect',
          phase: 'proposal',
          backtrack_to: 'explore',
          backtrack_reason: 'should not write',
        },
      });
      expect(isToolError(backtrack)).toBe(true);

      const configGet = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'schema' },
      });
      expect(isToolError(configGet)).toBe(true);

      if (!existedBefore) {
        expect(openspecExistsUnderCwd()).toBe(false);
      }
    } finally {
      await pair.close();
    }
  });

  it('WORKSPACE_FOLDER_PATHS 含 2 个绝对路径时全部 tool 失败且不取 [0] (AC-2)', async () => {
    const a = setupTempProject();
    const b = setupTempProject();
    process.env.WORKSPACE_FOLDER_PATHS = `${a.dir};${b.dir}`;
    const pair = await connectPair();
    try {
      const changeList = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(changeList)).toBe(true);

      const configGet = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'schema' },
      });
      expect(isToolError(configGet)).toBe(true);

      // 不得成功回显 a.dir（取 [0]）
      if (!isToolError(changeList)) {
        const data: { project_root: string } = JSON.parse(extractText(changeList));
        expect(data.project_root).not.toBe(a.dir);
      }
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });

  it('${workspaceFolder} 字面量 env 时 tool 错误信息可观测（含 literal / 未锁定语义）(AC-2)', async () => {
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    process.env.WORKSPACE_FOLDER_PATHS = '${workspaceFolder}';
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(true);
      const text = extractText(result);
      expect(text).toMatch(/not locked|literal|\$\{|Project root/i);
    } finally {
      await pair.close();
    }
  });

  it('CLAUDE_PROJECT_DIR / WORKSPACE_FOLDER_PATHS 均为 undefined（未注入）且 roots 不可用时全部 tool 失败 (AC-2)', async () => {
    clearProjectEnv();
    const pair = await connectPair();
    try {
      for (const name of ['change_list', 'config_get', 'phase_next'] as const) {
        const args =
          name === 'phase_next' ? { change: 'x' } : name === 'config_get' ? { key: 'schema' } : {};
        const result = await pair.client.callTool({ name, arguments: args });
        expect(isToolError(result)).toBe(true);
      }
    } finally {
      await pair.close();
    }
  });
});

// ===========================================================================
// 场景: 无参 change_list 回显锁定根
// ===========================================================================

describe('无参 change_list 回显锁定根', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('无参 change_list 返回 project_root 等于锁定根 (AC-6)', async () => {
    const { dir, cleanup } = setupTempProject();
    process.env.CLAUDE_PROJECT_DIR = dir;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(false);
      const data: { project_root: string } = JSON.parse(extractText(result));
      expect(data.project_root).toBe(dir);
    } finally {
      await pair.close();
      cleanup();
    }
  });

  it('无参 config_get 读取锁定根下 openspec/config.json 成功 (AC-4)', async () => {
    const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
    process.env.CLAUDE_PROJECT_DIR = dir;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'schema' },
      });
      expect(isToolError(result)).toBe(false);
      const data: {
        key: string;
        exists: boolean;
        value?: unknown;
      } = JSON.parse(extractText(result));
      expect(data).toMatchObject({ key: 'schema', exists: true, value: 'spec-driven' });
    } finally {
      await pair.close();
      cleanup();
    }
  });

  it('传入 project_root 字段时 tool 校验失败或忽略该字段且仍使用锁定根（以实现为准）(AC-4)', async () => {
    const { dir, cleanup } = setupTempProject();
    const other = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-root-other-'));
    process.env.CLAUDE_PROJECT_DIR = dir;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({
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
      await pair.close();
      cleanup();
      fs.rmSync(other, { recursive: true, force: true });
    }
  });

  it('未锁定时无参 change_list 返回 isError，不得成功回显 cwd (AC-6)', async () => {
    clearProjectEnv();
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(true);
      const text = extractText(result);
      expect(text).toMatch(/not locked|Project root/i);
      expect(text).not.toContain(`"project_root":"${process.cwd().replace(/\\/g, '\\\\')}"`);
    } finally {
      await pair.close();
    }
  });

  it('锁定根与 process.cwd() 不同时输出/读写均指向锁定根而非 cwd (AC-6)', async () => {
    const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
    expect(path.resolve(dir)).not.toBe(path.resolve(process.cwd()));
    process.env.CLAUDE_PROJECT_DIR = dir;
    const pair = await connectPair();
    try {
      const listResult = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(listResult)).toBe(false);
      const listData: { project_root: string } = JSON.parse(extractText(listResult));
      expect(listData.project_root).toBe(dir);
      expect(listData.project_root).not.toBe(process.cwd());

      const configResult = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'schema' },
      });
      expect(isToolError(configResult)).toBe(false);
      const configData: {
        exists: boolean;
        value?: unknown;
      } = JSON.parse(extractText(configResult));
      expect(configData.exists).toBe(true);
      expect(configData.value).toBe('spec-driven');
    } finally {
      await pair.close();
      cleanup();
    }
  });
});

// ===========================================================================
// 场景: CLI 显式根不依赖 MCP 锁定
// ===========================================================================

describe('CLI 显式根不依赖 MCP 锁定', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('无 MCP 缓存时 runConfigGet({ projectRoot: dir }) 与 runTestResolvePaths({ project_root: dir }) 均成功 (AC-7)', async () => {
    const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
    fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
    fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), 'export {};\n');
    vi.resetModules();
    try {
      const { getMcpCachedProjectRoot } = await import('../../src/lib/project-root');
      const { runConfigGet } = await import('../../src/commands/config-get');
      const { runTestResolvePaths } = await import('../../src/commands/test-resolve-paths');

      expect(getMcpCachedProjectRoot()).toBeNull();

      const cfg = runConfigGet({ key: 'schema', projectRoot: dir });
      expect(cfg.exists).toBe(true);
      expect(cfg.value).toBe('spec-driven');

      const resolved = runTestResolvePaths({
        modules: ['src/foo.ts'],
        project_root: dir,
      });
      expect(resolved.errors).toHaveLength(0);
      expect(resolved.unit_tests).toHaveLength(1);
      expect(resolved.unit_tests[0]?.source).toBe('src/foo.ts');
    } finally {
      cleanup();
    }
  });

  it('设置可用 CLAUDE_PROJECT_DIR=dir 后 hooks protect-files 对 dir/openspec/config.json 的 Write 返回 deny (AC-7/AC-10)', () => {
    const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
    try {
      expect(fs.existsSync(hooksBundlePath)).toBe(true);
      const { stdout, status } = runHooksProtectFiles(
        {
          tool_name: 'Write',
          tool_input: { file_path: path.join(dir, 'openspec', 'config.json') },
        },
        { CLAUDE_PROJECT_DIR: dir, WORKSPACE_FOLDER_PATHS: '' },
      );
      expect(status).toBe(0);
      const parsed: { hookSpecificOutput?: { permissionDecision?: string } } = JSON.parse(stdout);
      expect(parsed.hookSpecificOutput?.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('无缓存且无显式根、env 不可用时 getProjectDir() === process.cwd()，但 MCP change_list 仍 isError（对照 AC-2/AC-7 分离）', async () => {
    clearProjectEnv();
    vi.resetModules();
    const { getProjectDir, getMcpCachedProjectRoot } = await import('../../src/lib/project-root');
    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getProjectDir()).toBe(process.cwd());

    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(result)).toBe(true);
      const text = extractText(result);
      expect(text.startsWith('Project root is not locked:')).toBe(true);
    } finally {
      await pair.close();
    }
  });

  it('显式根≠cwd 时 CLI 读显式根配置；hooks 经 CLAUDE_PROJECT_DIR=显式根时保护命中该根路径', async () => {
    const { dir, cleanup } = setupTempProject({
      schema: 'spec-driven',
      context: 'from-explicit-root',
    });
    expect(path.resolve(dir)).not.toBe(path.resolve(process.cwd()));
    clearProjectEnv();

    vi.resetModules();
    try {
      const { runConfigGet } = await import('../../src/commands/config-get');
      const cfg = runConfigGet({ key: 'context', projectRoot: dir });
      expect(cfg.exists).toBe(true);
      expect(cfg.value).toBe('from-explicit-root');

      expect(fs.existsSync(hooksBundlePath)).toBe(true);
      const { stdout, status } = runHooksProtectFiles(
        {
          tool_name: 'Write',
          tool_input: { file_path: path.join(dir, 'openspec', 'config.json') },
        },
        { CLAUDE_PROJECT_DIR: dir, WORKSPACE_FOLDER_PATHS: '' },
      );
      expect(status).toBe(0);
      const parsed: { hookSpecificOutput?: { permissionDecision?: string } } = JSON.parse(stdout);
      expect(parsed.hookSpecificOutput?.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });
});
