/**
 * 集成测试: MCP archi_decide → withResolvedProjectRoot → lib/archi-decide
 *
 * @see openspec/changes/migrate-archi-decide-to-mcp/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { Client } from '@modelcontextprotocol/sdk/client';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory';
import { type McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

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

function setupTempProject(): { dir: string; cleanup: () => void } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'archi-mcp-it-'));
  fs.mkdirSync(path.join(dir, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(dir, 'openspec', 'config.json'),
    JSON.stringify({ schema: 'spec-driven' }),
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
  if (typeof item !== 'object' || item === null || !('text' in item)) {
    throw new Error('expected text content item');
  }
  return String((item as { text: unknown }).text);
}

function isToolError(result: unknown): boolean {
  return (
    typeof result === 'object' && result !== null && 'isError' in result && result.isError === true
  );
}

function decisionsDir(projectRoot: string): string {
  return path.join(projectRoot, 'openspec', 'architecture', 'decisions');
}

function writeSampleAdr(projectRoot: string, filename: string): void {
  const dir = decisionsDir(projectRoot);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, filename),
    `# ADR: Sample
- **日期**: 2026-07-01
- **状态**: proposed

## 背景
bg

## 决策
dec

## 后果
### 正面后果
- p

### 负面后果
- n

## 备选方案
(none)

## 影响范围
- item
`,
    'utf-8',
  );
}

async function connectClient(projectDir?: string): Promise<{
  server: McpServer;
  client: Client;
  close: () => Promise<void>;
}> {
  if (projectDir) {
    process.env.CLAUDE_PROJECT_DIR = projectDir;
  } else {
    delete process.env.CLAUDE_PROJECT_DIR;
  }
  delete process.env.WORKSPACE_FOLDER_PATHS;

  vi.resetModules();
  const { connectToServer } = await import('../../src/mcp');
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = await connectToServer(serverTransport);
  const client = new Client({ name: 'archi-decide-itest', version: '1.0.0' }, { capabilities: {} });
  await client.connect(clientTransport);
  return {
    server,
    client,
    close: async () => {
      await client.close();
      await server.close();
    },
  };
}

describe('MCP archi_decide 集成 — listTools 含 underscore 工具名', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  let pair: Awaited<ReturnType<typeof connectClient>> | undefined;

  beforeEach(async () => {
    savedEnv = saveEnv();
    pair = await connectClient();
  });

  afterEach(async () => {
    await pair?.close();
    restoreEnv(savedEnv);
  });

  it('listTools 含合法名 archi_decide 且无 archi/decide', async () => {
    const { tools } = await pair!.client.listTools();
    const names = tools.map((t) => t.name);
    expect(names).toContain('archi_decide');
    expect(names).not.toContain('archi/decide');
  });

  it('缺 project_root 的 callTool 被拒', async () => {
    const result = await pair!.client.callTool({
      name: 'archi_decide',
      arguments: { action: 'list' },
    });
    expect(isToolError(result)).toBe(true);
  });

  it('project_root 为空串时走解析失败且不写盘', async () => {
    const project = setupTempProject();
    try {
      const localPair = await connectClient(project.dir);
      try {
        const result = await localPair.client.callTool({
          name: 'archi_decide',
          arguments: {
            action: 'create',
            project_root: '',
            title: 'T',
            background: 'B',
            decision: 'D',
          },
        });
        expect(isToolError(result)).toBe(true);
        expect(fs.existsSync(decisionsDir(project.dir))).toBe(false);
      } finally {
        await localPair.close();
      }
    } finally {
      project.cleanup();
    }
  });
});

describe('MCP archi_decide 集成 — list 解析 scope', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('## 影响范围 为最后一节时 list 返回非空 scope[]（回归 \\Z→Z bug）', async () => {
    const project = setupTempProject();
    const filename = '2026-08-01-scope-last.md';
    const dir = decisionsDir(project.dir);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(
      path.join(dir, filename),
      `# ADR: MCP Scope Parse
- **日期**: 2026-08-01
- **状态**: proposed

## 背景

bg

## 决策

dec

## 后果

### 正面后果

- p

### 负面后果

- n

## 备选方案

(none)

## 影响范围

- model.mcp.gateway
- model.mcp.store
`,
      'utf-8',
    );
    const pair = await connectClient(project.dir);
    try {
      const result = await pair.client.callTool({
        name: 'archi_decide',
        arguments: {
          project_root: project.dir,
          action: 'list',
        },
      });
      expect(isToolError(result)).toBe(false);
      const body = JSON.parse(extractText(result));
      expect(body.count).toBe(1);
      expect(body.adrs[0]?.scope).toEqual(['model.mcp.gateway', 'model.mcp.store']);
      expect(body.adrs[0]?.scope.length).toBeGreaterThan(0);
    } finally {
      await pair.close();
      project.cleanup();
    }
  });
});

describe('MCP archi_decide 集成 — update 为 accepted', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('update 为 accepted 后文件状态变更且 MCP 非 isError', async () => {
    const project = setupTempProject();
    const filename = '2026-07-01-sample.md';
    writeSampleAdr(project.dir, filename);
    const pair = await connectClient(project.dir);
    try {
      const result = await pair.client.callTool({
        name: 'archi_decide',
        arguments: {
          project_root: project.dir,
          action: 'update',
          file: filename,
          status: 'accepted',
        },
      });
      expect(isToolError(result)).toBe(false);
      const body = JSON.parse(extractText(result));
      expect(body).toMatchObject({ action: 'update', success: true, new_status: 'accepted' });
      const content = fs.readFileSync(path.join(decisionsDir(project.dir), filename), 'utf-8');
      expect(content).toMatch(/\*\*状态\*\*:\s*accepted/);
    } finally {
      await pair.close();
      project.cleanup();
    }
  });
});

describe('MCP archi_decide 集成 — superseded 无 superseded_by', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('superseded 无 superseded_by 时失败且文件不变', async () => {
    const project = setupTempProject();
    const filename = '2026-07-02-sample.md';
    writeSampleAdr(project.dir, filename);
    const pair = await connectClient(project.dir);
    const filepath = path.join(decisionsDir(project.dir), filename);
    const before = fs.readFileSync(filepath, 'utf-8');
    try {
      const result = await pair.client.callTool({
        name: 'archi_decide',
        arguments: {
          project_root: project.dir,
          action: 'update',
          file: filename,
          status: 'superseded',
        },
      });
      expect(isToolError(result)).toBe(false);
      const body = JSON.parse(extractText(result));
      expect(body.success).toBe(false);
      expect(fs.readFileSync(filepath, 'utf-8')).toBe(before);
    } finally {
      await pair.close();
      project.cleanup();
    }
  });
});

describe('MCP archi_decide 集成 — update 路径穿越拒绝', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('经 MCP update 传入路径穿越 file 时失败且 decisions 外无写入', async () => {
    const project = setupTempProject();
    const filename = '2026-07-04-safe.md';
    writeSampleAdr(project.dir, filename);
    const pair = await connectClient(project.dir);
    const filepath = path.join(decisionsDir(project.dir), filename);
    const before = fs.readFileSync(filepath, 'utf-8');
    try {
      const result = await pair.client.callTool({
        name: 'archi_decide',
        arguments: {
          project_root: project.dir,
          action: 'update',
          file: '../../../outside.md',
          status: 'accepted',
        },
      });
      expect(isToolError(result)).toBe(false);
      const body = JSON.parse(extractText(result));
      expect(body).toMatchObject({ action: 'update', success: false });
      expect(body.error).toMatch(/outside decisions directory/);
      expect(fs.readFileSync(filepath, 'utf-8')).toBe(before);
      expect(fs.existsSync(path.join(project.dir, 'outside.md'))).toBe(false);
    } finally {
      await pair.close();
      project.cleanup();
    }
  });
});

describe('MCP archi_decide 集成 — resolve 根与 args 不一致', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('resolve 根与 args 中 project_root 不一致时以 resolve 根读写 ADR', async () => {
    const resolvedRoot = setupTempProject();
    const argsRoot = setupTempProject();
    const filename = '2026-07-03-sample.md';
    writeSampleAdr(argsRoot.dir, filename);
    const pair = await connectClient(resolvedRoot.dir);
    const listArgs = {
      project_root: argsRoot.dir,
      action: 'list' as const,
    };
    try {
      const first = await pair.client.callTool({ name: 'archi_decide', arguments: listArgs });
      expect(isToolError(first)).toBe(true);
      const firstBody = JSON.parse(extractText(first));
      expect(firstBody.code).toBe('not_in_candidates');

      const second = await pair.client.callTool({ name: 'archi_decide', arguments: listArgs });
      expect(isToolError(second)).toBe(false);
      const body = JSON.parse(extractText(second));
      expect(body.count).toBe(1);
      expect(body.adrs[0]?.path).toContain(decisionsDir(argsRoot.dir));
      expect(body.adrs[0]?.path).not.toContain(decisionsDir(resolvedRoot.dir));
      expect(fs.existsSync(path.join(decisionsDir(resolvedRoot.dir), filename))).toBe(false);
    } finally {
      await pair.close();
      resolvedRoot.cleanup();
      argsRoot.cleanup();
    }
  });
});
