/**
 * 集成测试: MCP 候选采集 → per-call project_root resolve → 业务 I/O
 *
 * 不 mock project-root 模块；通过真实 connectToServer + InMemoryTransport 验证。
 *
 * @see openspec/changes/mcp-workspace-root/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { pathToFileURL } from 'node:url';

import { Client } from '@modelcontextprotocol/sdk/client';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory';
import { type McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { ListRootsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-ws-root-'));
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

function openspecSideEffectUnderCwd(): boolean {
  return fs.existsSync(path.join(process.cwd(), 'openspec', 'changes', 'lock-side-effect'));
}

interface ConnectedPair {
  server: McpServer;
  client: Client;
  close: () => Promise<void>;
  getCandidates: () => readonly string[];
}

async function connectPair(options?: {
  roots?: { uri: string; name?: string }[];
  /** 握手完成后再二次 collect（规避 connect 内 listRoots 与 client 就绪竞态） */
  recollectAfterConnect?: boolean;
}): Promise<ConnectedPair> {
  vi.resetModules();
  const { connectToServer } = await import('../../src/mcp');
  const projectRoot = await import('../../src/lib/project-root');
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();

  const client = new Client(
    { name: 'ws-root-itest-client', version: '1.0.0' },
    { capabilities: options?.roots ? { roots: { listChanged: false } } : {} },
  );

  if (options?.roots) {
    client.setRequestHandler(ListRootsRequestSchema, async () => {
      return { roots: options.roots! };
    });
  }

  const serverPromise = connectToServer(serverTransport);
  const clientPromise = client.connect(clientTransport);
  const [server] = await Promise.all([serverPromise, clientPromise]);

  if (options?.recollectAfterConnect) {
    await projectRoot.collectProjectRootCandidates(server.server);
  }

  return {
    server,
    client,
    getCandidates: () => projectRoot.getProjectRootCandidates(),
    close: async () => {
      await client.close();
      await server.close();
    },
  };
}

const TOOLS_OMIT_PROJECT_ROOT: Array<{ name: string; args: Record<string, unknown> }> = [
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
  { name: 'archi_check', args: { staged: false } },
  { name: 'config_get', args: { key: 'schema' } },
  { name: 'test_detect_frameworks', args: {} },
  { name: 'test_resolve_paths', args: { modules: [] } },
  { name: 'change_list', args: {} },
];

// ===========================================================================
// 场景: 多候选下显式选型成功
// ===========================================================================

describe('多候选下显式选型成功', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('双 WORKSPACE 可用根时传 ∈ 列表的 A：change_list 成功且 project_root===A (AC-1/AC-3)', async () => {
    const a = setupTempProject();
    const b = setupTempProject();
    fs.mkdirSync(path.join(a.dir, 'openspec', 'changes', 'active-a'), { recursive: true });
    process.env.WORKSPACE_FOLDER_PATHS = `${a.dir},${b.dir}`;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: a.dir },
      });
      expect(isToolError(result)).toBe(false);
      const data: { project_root: string; changes: { name: string }[] } = JSON.parse(
        extractText(result),
      );
      expect(data.project_root).toBe(path.resolve(a.dir));
      expect(data.changes.map((c) => c.name)).toContain('active-a');
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });

  it('同进程再传 ∈ 列表的 B：成功且 project_root===B（证明无不可变单根 lock）(AC-1/AC-3)', async () => {
    const a = setupTempProject();
    const b = setupTempProject();
    fs.mkdirSync(path.join(b.dir, 'openspec', 'changes', 'active-b'), { recursive: true });
    process.env.WORKSPACE_FOLDER_PATHS = `${a.dir},${b.dir}`;
    const pair = await connectPair();
    try {
      const first = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: a.dir },
      });
      expect(isToolError(first)).toBe(false);

      const second = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: b.dir },
      });
      expect(isToolError(second)).toBe(false);
      const data: { project_root: string; changes: { name: string }[] } = JSON.parse(
        extractText(second),
      );
      expect(data.project_root).toBe(path.resolve(b.dir));
      expect(data.changes.map((c) => c.name)).toContain('active-b');
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });

  it('省略 project_root：change_list/config_get/phase_next 失败，command 无副作用 (AC-2)', async () => {
    const a = setupTempProject();
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    const before = openspecSideEffectUnderCwd();
    try {
      for (const tool of [
        { name: 'change_list', arguments: {} },
        { name: 'config_get', arguments: { key: 'schema' } },
        { name: 'phase_next', arguments: { change: 'x' } },
      ] as const) {
        const result = await pair.client.callTool(tool);
        expect(isToolError(result)).toBe(true);
      }
      expect(openspecSideEffectUnderCwd()).toBe(before);
    } finally {
      await pair.close();
      a.cleanup();
    }
  });

  it('len==1 候选时仍必须传 project_root；传该唯一候选则成功 (AC-1/AC-2)', async () => {
    const a = setupTempProject();
    fs.mkdirSync(path.join(a.dir, 'openspec', 'changes', 'solo'), { recursive: true });
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    try {
      const omitted = await pair.client.callTool({ name: 'change_list', arguments: {} });
      expect(isToolError(omitted)).toBe(true);

      const ok = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: a.dir },
      });
      expect(isToolError(ok)).toBe(false);
      expect(JSON.parse(extractText(ok)).project_root).toBe(path.resolve(a.dir));
    } finally {
      await pair.close();
      a.cleanup();
    }
  });
});

// ===========================================================================
// 场景: ∉ 候选拒绝且无 cwd 副作用
// ===========================================================================

describe('∉ 候选拒绝且无 cwd 副作用', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('候选含 A 时传 A：change_list/config_get 成功且无 cwd 副作用 (AC-3/AC-8)', async () => {
    const a = setupTempProject({ schema: 'spec-driven', context: 'A' });
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    const before = openspecSideEffectUnderCwd();
    try {
      const list = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: a.dir },
      });
      expect(isToolError(list)).toBe(false);
      const cfg = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'context', project_root: a.dir },
      });
      expect(isToolError(cfg)).toBe(false);
      expect(JSON.parse(extractText(cfg))).toMatchObject({ exists: true, value: 'A' });
      expect(openspecSideEffectUnderCwd()).toBe(before);
    } finally {
      await pair.close();
      a.cleanup();
    }
  });

  it('合法 B∉candidates：isError，code===not_in_candidates，candidates 含 A，含 force 说明 (AC-4)', async () => {
    const a = setupTempProject();
    const b = setupTempProject();
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: b.dir },
      });
      expect(isToolError(result)).toBe(true);
      const body = JSON.parse(extractText(result));
      expect(body.code).toBe('not_in_candidates');
      expect(body.candidates.map((c: string) => path.resolve(c))).toContain(path.resolve(a.dir));
      expect(String(body.force_hint)).toMatch(/force-add|identical complete arguments/i);
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });

  it('三通道失败（空候选）且传合法绝对路径：第一次必拒，candidates:[]，cwd 无副作用 (AC-6/AC-8)', async () => {
    const b = setupTempProject();
    clearProjectEnv();
    const before = openspecSideEffectUnderCwd();
    const pair = await connectPair({ roots: undefined });
    try {
      expect(pair.getCandidates()).toEqual([]);
      const result = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: b.dir },
      });
      expect(isToolError(result)).toBe(true);
      const body = JSON.parse(extractText(result));
      expect(body.code).toBe('not_in_candidates');
      expect(body.candidates).toEqual([]);
      expect(openspecSideEffectUnderCwd()).toBe(before);
    } finally {
      await pair.close();
      b.cleanup();
    }
  });

  it('子目录工作区（上层有 .git/openspec）作为候选并传入：成功回显等于子目录 (AC-9)', async () => {
    const parent = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-ws-parent-'));
    fs.mkdirSync(path.join(parent, '.git'));
    fs.mkdirSync(path.join(parent, 'openspec'), { recursive: true });
    const child = path.join(parent, 'workspace');
    fs.mkdirSync(path.join(child, 'openspec', 'changes'), { recursive: true });
    fs.writeFileSync(
      path.join(child, 'openspec', 'config.json'),
      JSON.stringify({ schema: 'spec-driven' }),
    );
    process.env.CLAUDE_PROJECT_DIR = child;
    const pair = await connectPair();
    try {
      const result = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: child },
      });
      expect(isToolError(result)).toBe(false);
      const data: { project_root: string } = JSON.parse(extractText(result));
      expect(data.project_root).toBe(path.resolve(child));
      expect(data.project_root).not.toBe(path.resolve(parent));
    } finally {
      await pair.close();
      fs.rmSync(parent, { recursive: true, force: true });
    }
  });

  it('project_root: "" / 相对路径：结构化 invalid_path 或 schema 失败，cwd 无 openspec 写入 (AC-2/AC-8)', async () => {
    const a = setupTempProject();
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    const before = openspecSideEffectUnderCwd();
    try {
      for (const project_root of ['', 'relative/path']) {
        const result = await pair.client.callTool({
          name: 'change_list',
          arguments: { project_root },
        });
        expect(isToolError(result)).toBe(true);
        const text = extractText(result);
        // schema or resolve JSON
        if (text.startsWith('{')) {
          const body = JSON.parse(text);
          if (body.code) {
            expect(body.code).toBe('invalid_path');
          }
        }
      }
      expect(openspecSideEffectUnderCwd()).toBe(before);
    } finally {
      await pair.close();
      a.cleanup();
    }
  });
});

// ===========================================================================
// 场景: 候选外路径二次确认放行
// ===========================================================================

describe('候选外路径二次确认放行', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('同全参再调 force：第二次 config_get 成功且读取 B 下 config (AC-5)', async () => {
    const a = setupTempProject({ schema: 'spec-driven', context: 'A' });
    const b = setupTempProject({ schema: 'spec-driven', context: 'B-only' });
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    const args = { key: 'context', project_root: b.dir };
    try {
      const first = await pair.client.callTool({ name: 'config_get', arguments: args });
      expect(isToolError(first)).toBe(true);
      const firstBody = JSON.parse(extractText(first));
      expect(firstBody.code).toBe('not_in_candidates');
      expect(firstBody.force_hint).toBeTruthy();

      const second = await pair.client.callTool({ name: 'config_get', arguments: args });
      expect(isToolError(second)).toBe(false);
      expect(JSON.parse(extractText(second))).toMatchObject({
        exists: true,
        value: 'B-only',
      });
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });

  it('第二次仅改 key 字段：仍失败，不 force (AC-7)', async () => {
    const a = setupTempProject();
    const b = setupTempProject({ schema: 'spec-driven', context: 'B' });
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    try {
      const first = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'context', project_root: b.dir },
      });
      expect(isToolError(first)).toBe(true);

      const second = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'schema', project_root: b.dir },
      });
      expect(isToolError(second)).toBe(true);
      expect(JSON.parse(extractText(second)).code).toBe('not_in_candidates');
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });

  it('空候选两次相同全参：第二次放行并可读 B (AC-6)', async () => {
    const b = setupTempProject({ schema: 'spec-driven', context: 'empty-escape' });
    clearProjectEnv();
    const pair = await connectPair();
    const args = { key: 'context', project_root: b.dir };
    try {
      expect(pair.getCandidates()).toEqual([]);
      const first = await pair.client.callTool({ name: 'config_get', arguments: args });
      expect(isToolError(first)).toBe(true);

      const second = await pair.client.callTool({ name: 'config_get', arguments: args });
      expect(isToolError(second)).toBe(false);
      expect(JSON.parse(extractText(second))).toMatchObject({
        exists: true,
        value: 'empty-escape',
      });
    } finally {
      await pair.close();
      b.cleanup();
    }
  });

  it('force 成功后第三次传 B 直接成功（已在 candidates）(AC-5/AC-7)', async () => {
    const a = setupTempProject();
    const b = setupTempProject({ schema: 'spec-driven', context: 'third' });
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    const args = { key: 'context', project_root: b.dir };
    try {
      await pair.client.callTool({ name: 'config_get', arguments: args });
      const forced = await pair.client.callTool({ name: 'config_get', arguments: args });
      expect(isToolError(forced)).toBe(false);

      const third = await pair.client.callTool({ name: 'config_get', arguments: args });
      expect(isToolError(third)).toBe(false);
      expect(JSON.parse(extractText(third)).value).toBe('third');
      expect(pair.getCandidates().map((c) => path.resolve(c))).toContain(path.resolve(b.dir));
    } finally {
      await pair.close();
      a.cleanup();
      b.cleanup();
    }
  });
});

// ===========================================================================
// 场景: 全工具省略 project_root 均失败
// ===========================================================================

describe('全工具省略 project_root 均失败', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('11 个 tool 省略 project_root 均失败 (AC-2)', async () => {
    const a = setupTempProject();
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    try {
      expect(TOOLS_OMIT_PROJECT_ROOT).toHaveLength(11);
      for (const { name, args } of TOOLS_OMIT_PROJECT_ROOT) {
        const result = await pair.client.callTool({ name, arguments: args });
        expect(isToolError(result)).toBe(true);
      }
    } finally {
      await pair.close();
      a.cleanup();
    }
  });

  it('同一连接补传候选内 project_root 后 change_list/config_get 成功 (AC-2/AC-3)', async () => {
    const a = setupTempProject({ schema: 'spec-driven', context: 'ok' });
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    try {
      expect(isToolError(await pair.client.callTool({ name: 'change_list', arguments: {} }))).toBe(
        true,
      );

      const list = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: a.dir },
      });
      expect(isToolError(list)).toBe(false);

      const cfg = await pair.client.callTool({
        name: 'config_get',
        arguments: { key: 'context', project_root: a.dir },
      });
      expect(isToolError(cfg)).toBe(false);
      expect(JSON.parse(extractText(cfg)).value).toBe('ok');
    } finally {
      await pair.close();
      a.cleanup();
    }
  });

  it('phase_log/phase_next/backtrack（原先无该字段）同样 required (AC-2)', async () => {
    const a = setupTempProject();
    process.env.CLAUDE_PROJECT_DIR = a.dir;
    const pair = await connectPair();
    try {
      for (const tool of TOOLS_OMIT_PROJECT_ROOT.filter((t) =>
        ['phase_log', 'phase_next', 'backtrack'].includes(t.name),
      )) {
        const result = await pair.client.callTool({ name: tool.name, arguments: tool.args });
        expect(isToolError(result)).toBe(true);
      }
    } finally {
      await pair.close();
      a.cleanup();
    }
  });
});

// roots 通道补充：确保 file:// 可进入候选
describe('roots 通道参与候选', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('仅 roots 提供候选时传该根 change_list 成功', async () => {
    const a = setupTempProject();
    fs.mkdirSync(path.join(a.dir, 'openspec', 'changes', 'from-roots'), { recursive: true });
    const pair = await connectPair({
      roots: [{ uri: pathToFileURL(a.dir).href }],
      recollectAfterConnect: true,
    });
    try {
      expect(pair.getCandidates().map((c) => path.resolve(c))).toContain(path.resolve(a.dir));
      const result = await pair.client.callTool({
        name: 'change_list',
        arguments: { project_root: a.dir },
      });
      expect(isToolError(result)).toBe(false);
      expect(
        JSON.parse(extractText(result)).changes.map((c: { name: string }) => c.name),
      ).toContain('from-roots');
    } finally {
      await pair.close();
      a.cleanup();
    }
  });
});
