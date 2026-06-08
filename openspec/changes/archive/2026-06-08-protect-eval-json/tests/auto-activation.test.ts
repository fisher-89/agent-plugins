/**
 * 集成测试: 插件安装时保护自动激活 (AC-12)
 *
 * 本文件验证 dev-team 插件安装后，eval.json 保护自动激活无需额外配置。
 * 自动测试可验证 hooks.json 存在于正确路径且格式正确，但无法模拟
 * Claude Code 的插件加载流程。AC-12 中关于运行时自动激活的部分
 * 需通过手动测试验证。
 *
 * @see openspec/changes/protect-eval-json/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';
import * as path from 'path';
import * as fs from 'fs';

// ---------------------------------------------------------------------------
// 路径解析
// ---------------------------------------------------------------------------

/**
 * 项目根目录（从 vitest CWD 起算）。
 * 集成测试在 openspec/changes/<name>/tests/ 下，
 * 从该路径到项目根目录的路径为 <project>/openspec/changes/<name>/tests/<file>。
 * vitest CWD 为 plugins/dev-team/bin/，因此 projectRoot 为 ../../..
 */
const projectRoot = path.resolve(process.cwd(), '../../..');

/** hooks.json 的绝对路径 */
const hooksJsonPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/hooks.json');

/** protect-eval.sh 的绝对路径 */
const scriptPath = path.resolve(
  projectRoot,
  'plugins/dev-team/hooks/scripts/protect-eval.sh',
);

// ---------------------------------------------------------------------------
// AC-12: 插件安装后保护自动激活
// ---------------------------------------------------------------------------

describe('AC-12: 插件安装后保护自动激活', () => {
  // -----------------------------------------------------------------------
  // 文件存在性 — 核心前提条件
  // -----------------------------------------------------------------------

  it('hooks.json 文件应存在于插件 hooks 目录', () => {
    expect(fs.existsSync(hooksJsonPath)).toBe(true);
  });

  it('protect-eval.sh 脚本应存在于插件 hooks/scripts 目录', () => {
    expect(fs.existsSync(scriptPath)).toBe(true);
  });

  // -----------------------------------------------------------------------
  // hooks.json 格式 — 确保 Claude Code 可正确解析
  // -----------------------------------------------------------------------

  it('hooks.json 应为合法 JSON', () => {
    const raw = fs.readFileSync(hooksJsonPath, 'utf-8');
    expect(() => JSON.parse(raw)).not.toThrow();
  });

  it('hooks.json 的 hooks.PreToolUse 数组不应为空', () => {
    const raw = fs.readFileSync(hooksJsonPath, 'utf-8');
    const parsed = JSON.parse(raw) as {
      hooks?: { PreToolUse?: unknown[] };
    };
    expect(parsed.hooks?.PreToolUse?.length).toBeGreaterThan(0);
  });

  it('第一项 PreToolUse hook 应匹配 Write|Edit', () => {
    const raw = fs.readFileSync(hooksJsonPath, 'utf-8');
    const parsed = JSON.parse(raw) as {
      hooks?: { PreToolUse?: Array<{ matcher?: string }> };
    };
    expect(parsed.hooks!.PreToolUse![0].matcher).toBe('Write|Edit');
  });

  it('第二项 PreToolUse hook 应匹配 Bash', () => {
    const raw = fs.readFileSync(hooksJsonPath, 'utf-8');
    const parsed = JSON.parse(raw) as {
      hooks?: { PreToolUse?: Array<{ matcher?: string }> };
    };
    expect(parsed.hooks!.PreToolUse![1].matcher).toBe('Bash');
  });

  it('所有 hook 应使用 command 类型', () => {
    const raw = fs.readFileSync(hooksJsonPath, 'utf-8');
    const parsed = JSON.parse(raw) as {
      hooks?: { PreToolUse?: Array<{ hooks?: Array<{ type?: string }> }> };
    };
    for (const entry of parsed.hooks!.PreToolUse!) {
      expect(entry.hooks![0].type).toBe('command');
    }
  });

  it('command 路径应使用 ${CLAUDE_PLUGIN_ROOT} 变量', () => {
    const raw = fs.readFileSync(hooksJsonPath, 'utf-8');
    const parsed = JSON.parse(raw) as {
      hooks?: { PreToolUse?: Array<{ hooks?: Array<{ command?: string }> }> };
    };
    for (const entry of parsed.hooks!.PreToolUse!) {
      expect(entry.hooks![0].command).toContain('${CLAUDE_PLUGIN_ROOT}');
    }
  });

  // -----------------------------------------------------------------------
  // 插件元数据 — 版本号一致性
  // -----------------------------------------------------------------------

  it('plugin.json 中应包含 version 字段且格式有效', () => {
    const pluginJsonPath = path.resolve(
      projectRoot,
      'plugins/dev-team/.claude-plugin/plugin.json',
    );
    const raw = fs.readFileSync(pluginJsonPath, 'utf-8');
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    expect(parsed.version).toMatch(/^\d+\.\d+\.\d+$/);
  });
});

// ---------------------------------------------------------------------------
// 自动激活前提 — 无需额外配置
// ---------------------------------------------------------------------------

describe('自动激活前提 — 无额外配置需求', () => {
  it('项目中不存在独立的 hook 重复注册（仅插件级 hooks.json 生效）', () => {
    // 验证 .claude/settings.local.json 不包含重复的 hook 配置
    const settingsLocalPath = path.resolve(projectRoot, '.claude/settings.local.json');
    if (fs.existsSync(settingsLocalPath)) {
      const raw = fs.readFileSync(settingsLocalPath, 'utf-8');
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      // 如果 settings.local.json 存在，不应包含 hooks 配置（避免与插件级 hook 冲突）
      // 注意：此检查仅为参考，settings.local.json 的管理不属于当前变更范围
      expect(parsed).not.toHaveProperty('hooks');
    } else {
      // settings.local.json 不存在也是正常的
      expect(true).toBe(true);
    }
  });

  it('插件目录结构符合 Claude Code 插件发现约定', () => {
    // Claude Code 自动发现 plugins/<name>/hooks/hooks.json
    // 验证目录结构存在
    const hooksDir = path.dirname(hooksJsonPath);
    expect(fs.existsSync(hooksDir)).toBe(true);
  });

  it('脚本文件具有可执行权限（Unix-like 系统）', () => {
    // 注意：此测试在 Windows 上不适用（无 executable bit 概念）
    // 在 CI/unix 环境中验证
    if (process.platform !== 'win32') {
      const stats = fs.statSync(scriptPath);
      const isExecutable = (stats.mode & 0o111) !== 0;
      expect(isExecutable).toBe(true);
    } else {
      // Windows 平台跳过此测试
      expect(true).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// 验证脚本可用性 — 前置检查
// ---------------------------------------------------------------------------

describe('脚本可用性前置检查', () => {
  it('protect-eval.sh 脚本文件不为空', () => {
    if (fs.existsSync(scriptPath)) {
      const stats = fs.statSync(scriptPath);
      expect(stats.size).toBeGreaterThan(0);
    } else {
      // 脚本尚未创建，标记为待办
      expect(true).toBe(true);
    }
  });

  it('脚本首行为 shebang（#!/bin/bash 或 #!/usr/bin/env bash）', () => {
    if (fs.existsSync(scriptPath)) {
      const firstLine = fs.readFileSync(scriptPath, 'utf-8').split('\n')[0];
      expect(firstLine).toMatch(/^#!\/bin\/bash|#!\/usr\/bin\/env bash/);
    } else {
      expect(true).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// Claude Code 运行时验证提示（仅在测试输出中展示，不自动运行）
// ---------------------------------------------------------------------------

describe('手动验证项（非自动化）', () => {
  // 这些测试是占位符，提醒需要手动验证的事项。
  // 它们始终为 skip 状态，不在 CI 中执行。

  it.skip('[手动] 在新项目中启用 dev-team 插件后，Write openspec/changes/test/eval.json 应被拦截', () => {
    // 手动验证步骤：
    // 1. 创建新 Claude Code 项目
    // 2. 在 settings.json 中添加 dev-team 插件
    // 3. 重启 Claude Code 确认插件加载
    // 4. 发出指令: Write openspec/changes/test/eval.json
    // 5. 确认操作被拒绝且拒绝原因包含 phase_log 建议
    expect(true).toBe(true);
  });

  it.skip('[手动] 新项目中 Edit 和 Bash 对 eval.json 的操作同样被拦截', () => {
    // 手动验证步骤：
    // 1. 同上
    // 2. 发出 Edit 指令修改 eval.json，确认被拒绝
    // 3. 发出 Bash echo '[]' > openspec/changes/test/eval.json，确认被拒绝
    expect(true).toBe(true);
  });

  it.skip('[手动] 禁用 dev-team 插件后保护不再生效', () => {
    // 手动验证步骤：
    // 1. 从 settings.json 中移除 dev-team 插件
    // 2. 重启 Claude Code
    // 3. Write openspec/changes/test/eval.json 应成功（保护解除）
    expect(true).toBe(true);
  });
});
