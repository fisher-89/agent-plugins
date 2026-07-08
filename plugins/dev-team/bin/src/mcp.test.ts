/**
 * 单元测试: mcp.ts — MCP 工具注册验证
 *
 * @see openspec/changes/write-protection-config/test-design.md
 *
 * Mock McpServer 构造函数，验证 registerTool 被调用次数和工具名称。
 */

import { describe, it, expect, vi } from 'vite-plus/test';

import * as changeListModule from './commands/change-list';
import * as configGetModule from './commands/config-get';
import * as phaseLogModule from './commands/phase-log';
import * as phaseNextModule from './commands/phase-next';
import * as testDetectModule from './commands/test-detect-frameworks';
import * as testResolveModule from './commands/test-resolve-paths';
import * as archiQueryModule from './lib/archi-query';
import * as archiValidateModule from './lib/archi-validate';
import * as archiWriteModule from './lib/archi-write';
import * as crossRefModule from './lib/c4-cross-ref';
// Static imports for module integrity verification tests
import * as schemasModule from './schemas';
// Import mcp.ts — triggers main() which creates a McpServer instance
// and registers all tools via registerTool mock
import './mcp';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockRegisterTool = vi.hoisted(() => vi.fn().mockReturnValue(undefined));

vi.mock('@modelcontextprotocol/sdk/server/mcp', () => ({
  McpServer: vi.fn().mockImplementation(function () {
    return {
      registerTool: mockRegisterTool,
      connect: vi.fn().mockResolvedValue(undefined),
      sendLoggingMessage: vi.fn(),
      server: {},
    };
  }),
}));

vi.mock('@modelcontextprotocol/sdk/server/stdio', () => ({
  StdioServerTransport: vi.fn(),
}));

vi.mock('./lib/project-root', () => ({
  initProjectRootFromMcp: vi.fn().mockResolvedValue(undefined),
  getMcpCachedProjectRoot: vi.fn(() => process.cwd()),
}));

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

function getRegisteredToolNames(): string[] {
  return mockRegisterTool.mock.calls.map((call) => String(call[0]));
}

// ---------------------------------------------------------------------------
// MCP 注册 — 工具已移除 (AC-6)
// ---------------------------------------------------------------------------

describe('MCP 注册 — 工具已移除 (AC-6)', () => {
  it('registerTool 未注册 config_set', () => {
    expect(getRegisteredToolNames()).not.toContain('config_set');
  });

  it('registerTool 未注册 config_unset', () => {
    expect(getRegisteredToolNames()).not.toContain('config_unset');
  });

  it('registerTool 未注册 config_context', () => {
    expect(getRegisteredToolNames()).not.toContain('config_context');
  });
});

// ---------------------------------------------------------------------------
// MCP 注册 — config_get 保留 (AC-6)
// ---------------------------------------------------------------------------

describe('MCP 注册 — config_get 保留 (AC-6)', () => {
  it('registerTool 已注册 config_get', () => {
    expect(getRegisteredToolNames()).toContain('config_get');
  });
});

// ---------------------------------------------------------------------------
// MCP 注册 — 其他工具保留
// ---------------------------------------------------------------------------

describe('MCP 注册 — 其他工具保留', () => {
  const expectedTools = [
    'phase_log',
    'archi_query',
    'archi_validate',
    'archi_write',
    'archi_check',
    'phase_next',
    'test_detect_frameworks',
    'test_resolve_paths',
    'change_list',
  ];

  for (const toolName of expectedTools) {
    it(`registerTool 已注册 ${toolName}`, () => {
      expect(getRegisteredToolNames()).toContain(toolName);
    });
  }

  it('总共注册了 10 个工具', () => {
    expect(mockRegisterTool).toHaveBeenCalledTimes(10);
  });
});

// ---------------------------------------------------------------------------
// MCP 注册 — import 完整性（补充存量覆盖）
// ---------------------------------------------------------------------------

describe('MCP 注册 — import 完整性', () => {
  it('schemas 模块可成功导入', () => {
    // mcp.ts 从 schemas/index.ts 导入了多个 input/output schema 对象
    // 验证该模块可被完整加载且包含关键导出
    expect(schemasModule).toHaveProperty('phaseLogInputSchema');
    expect(schemasModule).toHaveProperty('phaseLogOutputSchema');
    expect(schemasModule).toHaveProperty('configGetInputSchema');
    expect(schemasModule).toHaveProperty('configGetOutputSchema');
    expect(schemasModule).toHaveProperty('configSchema');
  });

  it('所有注册的工具函数模块可成功导入', () => {
    // mcp.ts 中每个 register*Tool 函数对应一个 command/lib 模块
    // 验证这些模块均可独立导入
    expect(changeListModule).toHaveProperty('runChangeList');
    expect(configGetModule).toHaveProperty('runConfigGet');
    expect(phaseLogModule).toHaveProperty('runPhaseLog');
    expect(phaseNextModule).toHaveProperty('runPhaseNext');
    expect(testDetectModule).toHaveProperty('runTestDetectFrameworks');
    expect(testResolveModule).toHaveProperty('runTestResolvePaths');
  });

  it('architecture lib 模块可成功导入', () => {
    expect(archiQueryModule).toHaveProperty('queryModel');
    expect(archiValidateModule).toHaveProperty('validateDsl');
    expect(archiWriteModule).toHaveProperty('writeDsl');
    expect(crossRefModule).toHaveProperty('runCrossRefCheck');
  });
});
