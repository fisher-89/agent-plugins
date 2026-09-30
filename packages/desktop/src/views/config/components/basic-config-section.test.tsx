// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { WorkspaceConfig } from '../../../types/generated/bindings';
import { BasicConfigSection } from './basic-config-section';

// ---------------------------------------------------------------------------
// BasicConfigSection 单测（AC-5）：纯组件 fixture 直传入参渲染断言，无 mock。
// 「未设（默认 N）」标注以 defaultedPaths 对位驱动，标注文本用正则全局检索。
// ---------------------------------------------------------------------------

function makeConfig(overrides: Partial<WorkspaceConfig> = {}): WorkspaceConfig {
  return {
    $schema: 'https://example.com/spec-driven.schema.json',
    schema: 'spec-driven',
    context: '桌面端插件仓库',
    rules: { proposal: ['提案规则一', '提案规则二'], tasks: ['任务规则一'] },
    staticAnalysis: 'clippy',
    tests: [],
    writeProtection: null,
    extra: [],
    ...overrides,
  };
}

describe('BasicConfigSection：基础配置分区呈现（AC-5）', () => {
  it('五字段呈现：$schema / schema / context / static_analysis / rules（proposal / tasks 列表逐项）', () => {
    render(<BasicConfigSection config={makeConfig()} defaultedPaths={new Set()} />);

    expect(screen.getByTestId('config-basic') !== null).toBe(true);
    expect(screen.getByText('$schema') !== null).toBe(true);
    expect(screen.getByText('https://example.com/spec-driven.schema.json') !== null).toBe(true);
    expect(screen.getByText('schema') !== null).toBe(true);
    expect(screen.getByText('spec-driven') !== null).toBe(true);
    expect(screen.getByText('context') !== null).toBe(true);
    expect(screen.getByText('桌面端插件仓库') !== null).toBe(true);
    expect(screen.getByText('static_analysis') !== null).toBe(true);
    expect(screen.getByText('clippy') !== null).toBe(true);
    expect(screen.getByText('rules.proposal') !== null).toBe(true);
    expect(screen.getByText('提案规则一') !== null).toBe(true);
    expect(screen.getByText('提案规则二') !== null).toBe(true);
    expect(screen.getByText('rules.tasks') !== null).toBe(true);
    expect(screen.getByText('任务规则一') !== null).toBe(true);
  });

  it('schema path 在 defaultedPaths：呈现「未设（默认 spec-driven）」，与文件显式设值形态可区分', () => {
    render(<BasicConfigSection config={makeConfig()} defaultedPaths={new Set(['schema'])} />);

    expect(screen.getByText('未设（默认 spec-driven）') !== null).toBe(true);
    // 文件显式设值形态（空集）下无该标注，两形态可区分（对照见空集用例）
    expect(screen.getByText('spec-driven') !== null).toBe(true);
  });

  it('可选字段 None（context / static_analysis / rules）：弱化「未配置」占位行，面板在内容空', () => {
    render(
      <BasicConfigSection
        config={makeConfig({ context: null, staticAnalysis: null, rules: null })}
        defaultedPaths={new Set()}
      />,
    );

    expect(screen.getByTestId('config-basic') !== null).toBe(true);
    expect(screen.getAllByText('未配置')).toHaveLength(3);
    expect(screen.queryByText('提案规则一')).toBeNull();
  });

  it('defaultedPaths 为空集：任何字段不出现「未设（默认 N）」标注', () => {
    render(<BasicConfigSection config={makeConfig()} defaultedPaths={new Set()} />);

    expect(screen.queryAllByText(/未设（默认/)).toHaveLength(0);
  });

  it('rules 空数组与多元素列表两形态渲染；path 不对位的 defaultedPaths 条目不污染渲染', () => {
    render(
      <BasicConfigSection
        config={makeConfig({ rules: { proposal: [], tasks: ['仅一条'] } })}
        defaultedPaths={new Set(['tests[0].coverage.lines', 'context'])}
      />,
    );

    // 空数组 → （空列表）；多元素 → 逐项列表
    expect(screen.getByText('（空列表）') !== null).toBe(true);
    expect(screen.getByText('仅一条') !== null).toBe(true);
    // defaultedPaths 条目均不对位本分区（schema 未在集内），零标注
    expect(screen.queryAllByText(/未设（默认/)).toHaveLength(0);
  });
});
