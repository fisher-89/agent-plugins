/**
 * 单元测试: home-install.mjs — 路径展开与 managed merge 纯函数
 */

import { describe, expect, it } from 'vite-plus/test';

import { expandHomePathTokens, mergeManagedHooks, mergeManagedMcp } from './home-install.mjs';

describe('expandHomePathTokens', () => {
  it('文本中双路径 token 均替换为给定绝对 root', () => {
    const root = 'C:/Users/me/.cursor';
    expect(expandHomePathTokens('a=__DEV_TEAM_ROOT__; b=__DEV_TEAM_RUNTIME_ROOT__', root)).toBe(
      `a=${root}; b=${root}`,
    );
  });

  it('Windows 风格反斜杠在非 JSON 文本中规范为正斜杠', () => {
    expect(expandHomePathTokens('root=__DEV_TEAM_ROOT__\\bin', 'D:\\cursor\\home')).toBe(
      'root=D:/cursor/home/bin',
    );
  });

  it('text 为 null / undefined 时抛错', () => {
    expect(() => expandHomePathTokens(null, '/root')).toThrow();
    expect(() => expandHomePathTokens(undefined, '/root')).toThrow();
  });

  it('absoluteRoot 为 null / undefined 时抛错', () => {
    expect(() => expandHomePathTokens('x', null)).toThrow();
    expect(() => expandHomePathTokens('x', undefined)).toThrow();
  });

  it('text 为空字符串时返回空字符串', () => {
    expect(expandHomePathTokens('', '/abs')).toBe('');
  });

  it('超长文本含多处 token 时全部替换', () => {
    const root = '/opt/cursor';
    const text = 'prefix __DEV_TEAM_ROOT__ mid __DEV_TEAM_RUNTIME_ROOT__ end '.repeat(50);
    expect(text.length).toBeGreaterThan(1000);
    const result = expandHomePathTokens(text, root);
    expect(result).not.toContain('__DEV_TEAM_ROOT__');
    expect(result).not.toContain('__DEV_TEAM_RUNTIME_ROOT__');
    expect(result).toContain(root);
  });

  it('absoluteRoot 为空字符串时抛错或拒绝', () => {
    expect(() => expandHomePathTokens('__DEV_TEAM_ROOT__', '')).toThrow();
  });
});

describe('mergeManagedHooks', () => {
  const prefix = 'dev-team_';

  it('已有用户自定义 hook + 传入托管 hook → 结果含两者；托管 command 被更新', () => {
    const existing = {
      version: 1,
      hooks: {
        preToolUse: [
          { matcher: 'Write', command: 'echo user-hook' },
          { matcher: 'Write', command: 'node /old/dev-team_hooks.cjs protect-files' },
        ],
      },
    };
    const incoming = {
      hooks: {
        preToolUse: [
          { matcher: 'Write|StrReplace', command: 'node /new/dev-team_hooks.cjs protect-files' },
        ],
      },
    };
    const merged = mergeManagedHooks(existing, incoming, prefix);
    expect(merged.hooks.preToolUse).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ command: 'echo user-hook' }),
        expect.objectContaining({
          command: 'node /new/dev-team_hooks.cjs protect-files',
        }),
      ]),
    );
    expect(merged.hooks.preToolUse.some((e) => e.command.includes('/old/dev-team_hooks'))).toBe(
      false,
    );
  });

  it('镜像侧删除某托管 hook 后，merge 结果移除对应托管项且保留用户项', () => {
    const existing = {
      hooks: {
        preToolUse: [
          { matcher: 'Write', command: 'echo user' },
          { matcher: 'Shell', command: 'node /x/dev-team_hooks.cjs protect-files' },
        ],
        subagentStop: [
          { matcher: 'dev-team_impl', command: 'node /x/dev-team_hooks.cjs static-check' },
        ],
      },
    };
    const incoming = {
      hooks: {
        preToolUse: [{ matcher: 'Shell', command: 'node /y/dev-team_hooks.cjs protect-files' }],
      },
    };
    const merged = mergeManagedHooks(existing, incoming, prefix);
    expect(merged.hooks.preToolUse).toEqual(
      expect.arrayContaining([expect.objectContaining({ command: 'echo user' })]),
    );
    expect(merged.hooks.subagentStop.every((e) => !e.command.includes(prefix))).toBe(true);
  });

  it('existing / incoming 为 null 时按空对象处理且不崩溃', () => {
    const fromNullExisting = mergeManagedHooks(
      null,
      {
        hooks: { preToolUse: [{ matcher: 'Write', command: 'node /a/dev-team_hooks.cjs' }] },
      },
      prefix,
    );
    expect(fromNullExisting.hooks.preToolUse).toHaveLength(1);

    const fromNullIncoming = mergeManagedHooks(
      { hooks: { preToolUse: [{ matcher: 'Write', command: 'echo user' }] } },
      null,
      prefix,
    );
    expect(fromNullIncoming.hooks.preToolUse).toEqual([
      expect.objectContaining({ command: 'echo user' }),
    ]);
  });

  it('existing 为 {} 时仅写入托管项', () => {
    const merged = mergeManagedHooks(
      {},
      {
        hooks: {
          preToolUse: [{ matcher: 'Write', command: 'node /z/dev-team_hooks.cjs' }],
        },
      },
      prefix,
    );
    expect(merged.hooks.preToolUse).toHaveLength(1);
    expect(merged.hooks.preToolUse[0].command).toContain(prefix);
  });

  it('无托管识别项时不改写用户条目', () => {
    const existing = {
      hooks: {
        preToolUse: [{ matcher: 'Write', command: 'echo only-user' }],
      },
    };
    const incoming = {
      hooks: {
        preToolUse: [{ matcher: 'Write', command: 'echo also-user' }],
      },
    };
    const merged = mergeManagedHooks(existing, incoming, prefix);
    expect(merged.hooks.preToolUse.map((e) => e.command)).toEqual([
      'echo only-user',
      'echo also-user',
    ]);
  });
});

describe('mergeManagedMcp', () => {
  const prefix = 'dev-team_';

  it('已有 user-other server + 片段 dev-team_mcp → 两者皆在；再次安装更新 args', () => {
    const existing = {
      mcpServers: {
        'user-other': { command: 'npx', args: ['other'] },
        'dev-team_mcp': { command: 'node', args: ['/old/dev-team_mcp.cjs'] },
      },
    };
    const fragment = {
      mcpServers: {
        'dev-team_mcp': { command: 'node', args: ['/new/dev-team_mcp.cjs'] },
      },
    };
    const merged = mergeManagedMcp(existing, fragment, prefix);
    expect(merged.mcpServers['user-other']).toEqual({ command: 'npx', args: ['other'] });
    expect(merged.mcpServers['dev-team_mcp'].args).toEqual(['/new/dev-team_mcp.cjs']);

    const again = mergeManagedMcp(
      merged,
      {
        mcpServers: {
          'dev-team_mcp': { command: 'node', args: ['/newer/dev-team_mcp.cjs'] },
        },
      },
      prefix,
    );
    expect(again.mcpServers['user-other']).toBeDefined();
    expect(again.mcpServers['dev-team_mcp'].args).toEqual(['/newer/dev-team_mcp.cjs']);
  });

  it('用户 server key 不含 dev-team_ 前缀时永不删除', () => {
    const existing = {
      mcpServers: {
        custom: { command: 'keep-me', extra: true },
      },
    };
    const merged = mergeManagedMcp(existing, { mcpServers: {} }, prefix);
    expect(merged.mcpServers.custom).toEqual({ command: 'keep-me', extra: true });
  });

  it('existing 缺少 mcpServers 时视为空对象再合并', () => {
    const merged = mergeManagedMcp(
      { other: 1 },
      { mcpServers: { 'dev-team_mcp': { command: 'node' } } },
      prefix,
    );
    expect(merged.other).toBe(1);
    expect(merged.mcpServers['dev-team_mcp']).toEqual({ command: 'node' });
  });

  it('fragment 为 {} / 无 servers 时不删除用户 servers', () => {
    const existing = {
      mcpServers: {
        custom: { command: 'user' },
        'dev-team_mcp': { command: 'old' },
      },
    };
    const mergedEmpty = mergeManagedMcp(existing, {}, prefix);
    expect(mergedEmpty.mcpServers.custom).toEqual({ command: 'user' });
    expect(mergedEmpty.mcpServers['dev-team_mcp']).toBeUndefined();

    const mergedNoServers = mergeManagedMcp(existing, { mcpServers: {} }, prefix);
    expect(mergedNoServers.mcpServers.custom).toEqual({ command: 'user' });
  });

  it('多余字段保留在非托管 server 对象上', () => {
    const existing = {
      mcpServers: {
        'user-other': { command: 'npx', args: ['x'], customFlag: 42, nested: { a: 1 } },
      },
    };
    const merged = mergeManagedMcp(
      existing,
      { mcpServers: { 'dev-team_mcp': { command: 'node' } } },
      prefix,
    );
    expect(merged.mcpServers['user-other']).toEqual({
      command: 'npx',
      args: ['x'],
      customFlag: 42,
      nested: { a: 1 },
    });
  });
});
