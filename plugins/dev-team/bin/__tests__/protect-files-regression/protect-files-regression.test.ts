/**
 * 集成测试: protect-files.mjs — 配置驱动的写入保护端到端回归
 *
 * @see openspec/changes/write-protection-config/test-design.md
 *
 * 通过临时目录模拟项目环境，向 protect-files.mjs 注入 stdin JSON 并验证输出。
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const scriptPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/protect-files.mjs');
const mcpPath = path.join(projectRoot, 'plugins/dev-team/bin/src/mcp.ts');
const schemasIndexPath = path.join(projectRoot, 'plugins/dev-team/bin/src/schemas/index.ts');

// ---------------------------------------------------------------------------
// 类型定义 & 辅助函数
// ---------------------------------------------------------------------------

interface HookResult {
  permissionDecision: string;
  permissionDecisionReason?: string;
}

/**
 * 运行 protect-files.mjs 并返回解析后的结果。
 * 若指定 tempProjectRoot，会设置 CLAUDE_PROJECT_ROOT 环境变量指向该目录。
 */
function runProtectFiles(stdinJson: string, tempProjectRoot?: string): HookResult {
  const env = { ...process.env };
  if (tempProjectRoot) {
    env.CLAUDE_PROJECT_ROOT = tempProjectRoot;
  }
  const stdout = execFileSync(process.execPath, [scriptPath], {
    input: stdinJson,
    encoding: 'utf-8',
    env,
  });
  const parsed: {
    hookSpecificOutput?: {
      permissionDecision?: string;
      permissionDecisionReason?: string;
    };
  } = JSON.parse(stdout.trim());
  return {
    permissionDecision: parsed.hookSpecificOutput?.permissionDecision ?? '',
    permissionDecisionReason: parsed.hookSpecificOutput?.permissionDecisionReason,
  };
}

/** createTempProject — 在临时目录中创建 openspec/config.json */
function createTempProject(configContent: string | null) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'protect-files-integ-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  if (configContent !== null) {
    fs.writeFileSync(path.join(openspecDir, 'config.json'), configContent, 'utf-8');
  }
  return {
    root,
    cleanup: () => {
      try {
        fs.rmSync(root, { recursive: true, force: true });
      } catch {
        // ignore cleanup errors
      }
    },
  };
}

// ---------------------------------------------------------------------------
// AC-2 / AC-3 / AC-10: 向后兼容 — 未配置 write_protection
// ---------------------------------------------------------------------------

describe('protect-files-regression — 向后兼容 (AC-2, AC-3, AC-10)', () => {
  const noWpConfig = () => JSON.stringify({ schema: 'spec-driven' }); // no write_protection

  it('未配置 write_protection 时，Bash > 写 eval.json 返回 deny', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'echo "[]" > openspec/changes/test/eval.json' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，Bash heredoc 写 eval.json 返回 deny', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，python 写 eval.json 仍豁免返回 allow', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'python plugins/dev-team/utils/eval-check.py --change test' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('allow');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，node 写 eval.json 仍豁免返回 allow', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'node scripts/write-eval.mjs' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('allow');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，cat 只读 eval.json 仍返回 allow', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'cat openspec/changes/test/eval.json' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('allow');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，Bash tee 写入 eval.json 返回 deny', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: "echo '[]' | tee openspec/changes/test/eval.json" },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，PowerShell Set-Content 写 eval.json 返回 deny', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'PowerShell',
          tool_input: { command: 'Set-Content -Path openspec/changes/test/eval.json -Value "[]"' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('未配置 write_protection 时，PowerShell Get-Content 只读返回 allow', () => {
    const { root, cleanup } = createTempProject(noWpConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'PowerShell',
          tool_input: { command: 'Get-Content openspec/changes/test/eval.json' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('allow');
    } finally {
      cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-2: config 驱动保护
// ---------------------------------------------------------------------------

describe('protect-files-regression — config 驱动保护 (AC-2)', () => {
  const customConfig = () =>
    JSON.stringify({
      write_protection: {
        files: [{ glob: 'openspec/data/*.json', reason: '自定义保护 %s' }],
      },
    });

  it('通过临时 config.json 注入配置后，Bash 写入受保护自定义文件返回 deny', () => {
    const { root, cleanup } = createTempProject(customConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'echo "x" > openspec/data/settings.json' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
      expect(result.permissionDecisionReason).toContain('自定义保护');
    } finally {
      cleanup();
    }
  });

  it('通过临时 config.json 注入配置后，PowerShell 写入受保护自定义文件返回 deny', () => {
    const { root, cleanup } = createTempProject(customConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'PowerShell',
          tool_input: { command: 'Set-Content -Path openspec/data/settings.json -Value "x"' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-4: 自定义 glob
// ---------------------------------------------------------------------------

describe('protect-files-regression — 自定义 glob (AC-4)', () => {
  const secretsConfig = () =>
    JSON.stringify({
      write_protection: {
        files: [{ glob: 'secrets/**/*.env', reason: '机密文件 %s' }],
      },
    });

  it('配置自定义 glob 匹配非 eval.json 文件，Bash > 写入返回 deny', () => {
    const { root, cleanup } = createTempProject(secretsConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'echo "API_KEY=xxx" > secrets/prod.env' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('配置自定义 glob，Bash tee 写入返回 deny', () => {
    const { root, cleanup } = createTempProject(secretsConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'echo "KEY=val" | tee secrets/staging.env' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
    } finally {
      cleanup();
    }
  });

  it('配置自定义 glob 不匹配时，写入未被保护的其它文件返回 allow', () => {
    const { root, cleanup } = createTempProject(secretsConfig());
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'echo "x" > src/helper.ts' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('allow');
    } finally {
      cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-5: 自定义拒绝文案
// ---------------------------------------------------------------------------

describe('protect-files-regression — 自定义拒绝文案 (AC-5)', () => {
  it('配置自定义 reason（含 %s/%t），拒绝时输出的 reason 含该自定义文案', () => {
    const { root, cleanup } = createTempProject(
      JSON.stringify({
        write_protection: {
          files: [
            {
              glob: 'critical/*.json',
              reason: '禁止写入: %s, 工具: %t',
            },
          ],
        },
      }),
    );
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Write',
          tool_input: { file_path: 'critical/config.json' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
      expect(result.permissionDecisionReason).toContain('禁止写入:');
      expect(result.permissionDecisionReason).toContain('critical/config.json');
      expect(result.permissionDecisionReason).toContain('Write');
    } finally {
      cleanup();
    }
  });

  it('未配置自定义 reason 时，拒绝时输出内置默认文案', () => {
    const { root, cleanup } = createTempProject(
      JSON.stringify({ schema: 'spec-driven' }), // no custom reason
    );
    try {
      const result = runProtectFiles(
        JSON.stringify({
          tool_name: 'Bash',
          tool_input: { command: 'echo "x" > openspec/changes/test/eval.json' },
        }),
        root,
      );
      expect(result.permissionDecision).toBe('deny');
      expect(result.permissionDecisionReason).toContain('受写入保护');
    } finally {
      cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-8: fail-open 行为
// ---------------------------------------------------------------------------

describe('protect-files-regression — fail-open (AC-8)', () => {
  it('空 stdin 输入脚本返回 allow', () => {
    const result = runProtectFiles('');
    expect(result.permissionDecision).toBe('allow');
  });

  it('无效 JSON stdin 输入脚本返回 allow', () => {
    const result = runProtectFiles('{not json');
    expect(result.permissionDecision).toBe('allow');
  });

  it('Write 缺失 tool_input.file_path 返回 allow', () => {
    const input = JSON.stringify({ tool_name: 'Write', tool_input: {} });
    const result = runProtectFiles(input);
    expect(result.permissionDecision).toBe('allow');
  });

  it('未知工具 Read 应返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Read',
      tool_input: { file_path: 'openspec/config.json' },
    });
    const result = runProtectFiles(input);
    expect(result.permissionDecision).toBe('allow');
  });
});

// ---------------------------------------------------------------------------
// AC-6: MCP 工具移除
// ---------------------------------------------------------------------------

describe('protect-files-regression — MCP 工具移除 (AC-6)', () => {
  it('mcp.ts 中不引用 runConfigSet', () => {
    const content = fs.readFileSync(mcpPath, 'utf-8');
    expect(content).not.toMatch(/runConfigSet/);
  });

  it('mcp.ts 中不引用 runConfigUnset', () => {
    const content = fs.readFileSync(mcpPath, 'utf-8');
    expect(content).not.toMatch(/runConfigUnset/);
  });

  it('mcp.ts 中不引用 runConfigContext', () => {
    const content = fs.readFileSync(mcpPath, 'utf-8');
    expect(content).not.toMatch(/runConfigContext/);
  });
});

// ---------------------------------------------------------------------------
// AC-7: 删除文件确认
// ---------------------------------------------------------------------------

const deletedFiles = [
  'plugins/dev-team/bin/src/commands/config-set.ts',
  'plugins/dev-team/bin/src/commands/config-unset.ts',
  'plugins/dev-team/bin/src/commands/config-context.ts',
  'plugins/dev-team/bin/src/schemas/config-set.schema.ts',
  'plugins/dev-team/bin/src/schemas/config-unset.schema.ts',
  'plugins/dev-team/bin/src/schemas/config-context.schema.ts',
];

describe('protect-files-regression — 删除文件确认 (AC-7)', () => {
  for (const filePath of deletedFiles) {
    const absolutePath = path.resolve(projectRoot, filePath);
    it(`${path.basename(filePath)} 不存在`, () => {
      expect(fs.existsSync(absolutePath)).toBe(false);
    });
  }
});

// ---------------------------------------------------------------------------
// AC-6: schemas/index.ts 不再导出已删除的 schema
// ---------------------------------------------------------------------------

describe('protect-files-regression — schemas/index.ts 导出验证 (AC-6)', () => {
  it('configSetInputSchema 未从 index.ts 导出', () => {
    const content = fs.readFileSync(schemasIndexPath, 'utf-8');
    expect(content).not.toMatch(/configSetInputSchema/);
  });

  it('configUnsetInputSchema 未从 index.ts 导出', () => {
    const content = fs.readFileSync(schemasIndexPath, 'utf-8');
    expect(content).not.toMatch(/configUnsetInputSchema/);
  });

  it('configContextInputSchema 未从 index.ts 导出', () => {
    const content = fs.readFileSync(schemasIndexPath, 'utf-8');
    expect(content).not.toMatch(/configContextInputSchema/);
  });
});

// ---------------------------------------------------------------------------
// AC-9: 脚本可用性与版本升级
// ---------------------------------------------------------------------------

describe('protect-files-regression — 脚本可用性 (AC-9)', () => {
  it('protect-files.mjs 存在且非空', () => {
    expect(fs.existsSync(scriptPath)).toBe(true);
    expect(fs.statSync(scriptPath).size).toBeGreaterThan(0);
  });

  it('plugin.json 版本号已升级 (>= 2.9.0)', () => {
    const pluginJsonPath = path.resolve(projectRoot, 'plugins/dev-team/.claude-plugin/plugin.json');
    const pluginConfig: Record<string, unknown> = JSON.parse(
      fs.readFileSync(pluginJsonPath, 'utf-8'),
    );
    expect(pluginConfig.version).toBeDefined();
    expect(typeof pluginConfig.version).toBe('string');
    const parts = String(pluginConfig.version).split('.').map(Number);
    expect(parts[0]).toBeGreaterThanOrEqual(2);
    if (parts[0] === 2) {
      expect(parts[1]).toBeGreaterThanOrEqual(9);
    }
  });

  it('hooks.json 中 3 处 PreToolUse 均引用 protect-files.mjs', () => {
    const hooksJsonPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/hooks.json');
    const hooksConfig = JSON.parse(fs.readFileSync(hooksJsonPath, 'utf-8'));
    const preToolUse = hooksConfig.hooks.PreToolUse;
    expect(preToolUse).toHaveLength(3);
    for (const entry of preToolUse) {
      expect(entry.hooks[0].command).toContain('protect-files.mjs');
    }
  });

  it('protect-eval.mjs 不再被 hooks.json 引用', () => {
    const hooksJsonPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/hooks.json');
    const hooksConfig = JSON.parse(fs.readFileSync(hooksJsonPath, 'utf-8'));
    const preToolUse = hooksConfig.hooks.PreToolUse;
    for (const entry of preToolUse) {
      expect(entry.hooks[0].command).not.toContain('protect-eval.mjs');
    }
  });
});
