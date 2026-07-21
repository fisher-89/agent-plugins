/**
 * 单元测试: mcp.ts — MCP 工具注册与调用验证 (via InMemoryTransport)
 *
 * 使用 McpServer + Client + InMemoryTransport 进行真实 MCP 协议级测试，
 * 替代原先的 Mock McpServer 方案。
 *
 * @see openspec/changes/write-protection-config/test-design.md
 * @see openspec/changes/remove-integration-test-path-resolution/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { Client } from '@modelcontextprotocol/sdk/client';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory';
import { type McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vite-plus/test';

import type { ConfigGetResult } from './commands/config-get';
import type { ResolveTestPathsResult } from './commands/test-resolve-paths';
import type { ArchiCheckResult, ArchiQueryResult, ArchiValidateResult } from './lib/c4-types';
import type { TestDetectFrameworksResult } from './schemas';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------
vi.mock('./lib/project-root', () => ({
  initProjectRootFromMcp: vi.fn().mockResolvedValue(undefined),
  getMcpCachedProjectRoot: vi.fn(() => process.cwd()),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * 从 callTool() 返回值中安全提取第一个 text content 的字符串值。
 *
 * `callTool()` 返回值是 discriminated union:
 *   { content: (TextContent|ImageContent|AudioContent|...)[], ... }
 *   | { toolResult: unknown, ... }
 *
 * 全程通过 runtime 类型收窄，零 type assertion。
 * 调用侧用已有的 Result 类型标注 JSON.parse 结果，获得编译期类型检查。
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

// ---------------------------------------------------------------------------
// MCP Server — 集成测试套件
// ---------------------------------------------------------------------------

describe('MCP Server (via InMemoryTransport)', () => {
  let server: McpServer;
  let client: Client;

  beforeAll(async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const { connectToServer } = await import('./mcp');
    server = await connectToServer(serverTransport);
    client = new Client({ name: 'test-client', version: '1.0.0' }, { capabilities: {} });
    await client.connect(clientTransport);
  });

  afterAll(async () => {
    await client.close();
    await server.close();
  });

  // -------------------------------------------------------------------------
  // MCP 注册 — backtrack 工具
  // -------------------------------------------------------------------------

  describe('MCP 注册 — backtrack 工具', () => {
    it('backtrack 工具的 description 不为空', async () => {
      const desc = await getToolDescription(client, 'backtrack');
      expect(desc).toBeDefined();
      expect(desc!.length).toBeGreaterThan(0);
    });
  });

  // -------------------------------------------------------------------------
  // MCP 注册 — 工具清单
  // -------------------------------------------------------------------------

  describe('MCP 注册 — 工具清单', () => {
    const allTools = [
      'backtrack',
      'phase_log',
      'archi_query',
      'archi_validate',
      'archi_write',
      'archi_check',
      'config_get',
      'phase_next',
      'test_detect_frameworks',
      'test_resolve_paths',
      'change_list',
    ];

    it('所有预期工具均已注册，且没有多余的工具', async () => {
      const names = await getRegisteredToolNames(client);
      expect(names).toHaveLength(allTools.length);
      for (const toolName of allTools) {
        expect(names).toContain(toolName);
      }
    });
  });

  // -------------------------------------------------------------------------
  // MCP 注册 -- test_resolve_paths description (AC-5)
  //
  // @see openspec/changes/remove-integration-test-path-resolution/test-design.md
  // -------------------------------------------------------------------------

  describe('MCP 注册 -- test_resolve_paths description (AC-5)', () => {
    it('description 不含 "integration" 或 "__tests__" 字符串', async () => {
      const desc = await getToolDescription(client, 'test_resolve_paths');
      expect(desc).toBeDefined();
      expect(desc).not.toContain('integration');
      expect(desc).not.toContain('__tests__');
    });
  });

  // -------------------------------------------------------------------------
  // MCP 工具调用 — 通过 client.callTool() + extractText() 验证 handler 行为
  // -------------------------------------------------------------------------

  describe('MCP 工具调用 — config_get', () => {
    it('返回 key、value 和 exists 字段', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      try {
        const result = await client.callTool({
          name: 'config_get',
          arguments: { key: 'schema', project_root: dir },
        });

        const data: ConfigGetResult = JSON.parse(extractText(result));
        expect(data).toMatchObject({ key: 'schema', exists: true });
        expect(data).toHaveProperty('value');
      } finally {
        cleanup();
      }
    });

    it('未设置的 key 返回 exists: false', async () => {
      // config_get 的 key 参数使用 z.enum()，须传入 schema 中已定义的 key。
      // temp config 仅含 { schema: 'spec-driven' }，因此 'context' 存在但值为 undefined。
      const { dir, cleanup } = setupTempProject();
      try {
        const result = await client.callTool({
          name: 'config_get',
          arguments: { key: 'context', project_root: dir },
        });

        const data: ConfigGetResult = JSON.parse(extractText(result));
        expect(data.exists).toBe(false);
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — test_detect_frameworks', () => {
    it('返回 detected 字段', async () => {
      const { dir, cleanup } = setupTempProject();
      try {
        const result = await client.callTool({
          name: 'test_detect_frameworks',
          arguments: { project_root: dir },
        });

        const data: TestDetectFrameworksResult = JSON.parse(extractText(result));
        expect(data).toHaveProperty('detected');
      } finally {
        cleanup();
      }
    });
  });

  describe('MCP 工具调用 — test_resolve_paths', () => {
    it('返回 unit_tests 数组', async () => {
      const { dir, cleanup } = setupTempProject({ schema: 'spec-driven' });
      try {
        fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
        fs.writeFileSync(path.join(dir, 'src', 'foo.ts'), '', 'utf-8');

        const result = await client.callTool({
          name: 'test_resolve_paths',
          arguments: {
            modules: ['src/foo.ts'],
            project_root: dir,
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

  describe('MCP 工具调用 — change_list', () => {
    it('返回 changes 数组', async () => {
      const { dir, cleanup } = setupTempProject();
      try {
        const result = await client.callTool({
          name: 'change_list',
          arguments: { project_root: dir },
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
      const result = await client.callTool({
        name: 'phase_next',
        arguments: { change: 'remove-integration-test-path-resolution' },
      });

      const data = JSON.parse(extractText(result));
      expect(typeof data === 'object' && data !== null).toBe(true);
    });
  });

  describe('MCP 工具调用 — archi_validate', () => {
    it('返回 valid 或 errors 字段（只读，验证当前模型 DSL）', async () => {
      const result = await client.callTool({
        name: 'archi_validate',
        arguments: {},
      });

      const data: ArchiValidateResult = JSON.parse(extractText(result));
      expect(typeof data.valid === 'boolean' || Array.isArray(data.errors)).toBe(true);
    });
  });

  describe('MCP 工具调用 — archi_query', () => {
    it('返回 elements 或 error 字段（只读）', async () => {
      const result = await client.callTool({
        name: 'archi_query',
        arguments: {},
      });

      const data: ArchiQueryResult = JSON.parse(extractText(result));
      const hasElements = 'elements' in data;
      const hasError = 'error' in data;
      expect(hasElements || hasError).toBe(true);
    });
  });

  describe('MCP 工具调用 — archi_check', () => {
    it('返回 violations 或 passed 字段（只读）', async () => {
      const result = await client.callTool({
        name: 'archi_check',
        arguments: { staged: false },
      });

      const data: ArchiCheckResult = JSON.parse(extractText(result));
      const hasViolations = 'violations' in data;
      const hasPassed = 'passed' in data;
      expect(hasViolations || hasPassed).toBe(true);
    });
  });
});
