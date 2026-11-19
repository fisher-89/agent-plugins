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
    projects: [],
    tests: [],
    writeProtection: null,
    extra: [],
    ...overrides,
  };
}

describe('BasicConfigSection：基础配置分区呈现（AC-5）', () => {
  it('基础字段呈现：$schema / schema / context / static_analysis / projects / rules（列表逐项）', () => {
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
    expect(screen.getByText('projects') !== null).toBe(true);
    expect(screen.getByText('（空列表）') !== null).toBe(true);
    expect(screen.getByText('rules.proposal') !== null).toBe(true);
    expect(screen.getByText('提案规则一') !== null).toBe(true);
    expect(screen.getByText('提案规则二') !== null).toBe(true);
    expect(screen.getByText('rules.tasks') !== null).toBe(true);
    expect(screen.getByText('任务规则一') !== null).toBe(true);
  });

  it('projects 非空：逐项目呈现 root 且保持配置顺序', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [
            { root: 'packages/desktop', language: null, dependsOn: null },
            { root: 'packages/desktop/src-tauri', language: null, dependsOn: null },
          ],
        })}
        defaultedPaths={new Set()}
      />,
    );

    const group = screen.getByTestId('config-projects');
    const roots = Array.from(group.querySelectorAll('li')).map(
      (item) => item.querySelector('span')?.textContent,
    );
    expect(roots).toEqual(['packages/desktop', 'packages/desktop/src-tauri']);
    expect(screen.queryByText('（空列表）')).toBeNull();
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
        config={makeConfig({
          projects: [{ root: 'packages/desktop', language: null, dependsOn: null }],
          rules: { proposal: [], tasks: ['仅一条'] },
        })}
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

describe('ProjectsGroup：projects 区两协议字段呈现（AC-7）', () => {
  it('单项目已设两字段：该 li 文本依次含 root / language：<值> / depends_on：<值>', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [
            {
              root: 'packages/desktop',
              language: 'typescript',
              dependsOn: ['packages/desktop/src-tauri'],
            },
          ],
        })}
        defaultedPaths={new Set()}
      />,
    );

    const group = screen.getByTestId('config-projects');
    const items = group.querySelectorAll('li');
    expect(items).toHaveLength(1);
    expect(items[0]?.textContent).toBe(
      'packages/desktoplanguage：typescriptdepends_on：packages/desktop/src-tauri',
    );
    expect(screen.getByText('language：typescript') !== null).toBe(true);
    expect(screen.getByText('depends_on：packages/desktop/src-tauri') !== null).toBe(true);
  });

  it('多项目不同值并列：逐项呈现不串行，顺序 = 配置序', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [
            { root: 'web', language: 'typescript', dependsOn: ['api'] },
            { root: 'api', language: 'rust', dependsOn: null },
          ],
        })}
        defaultedPaths={new Set()}
      />,
    );

    const group = screen.getByTestId('config-projects');
    const items = Array.from(group.querySelectorAll('li')).map((item) => item.textContent);
    expect(items).toEqual([
      'weblanguage：typescriptdepends_on：api',
      'apilanguage：rustdepends_on：未设',
    ]);
  });

  it('两字段均未设：两处「未设」，root 仍呈现（未设不吞行）', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [{ root: 'packages/desktop', language: null, dependsOn: null }],
        })}
        defaultedPaths={new Set()}
      />,
    );

    expect(screen.getByText('language：未设') !== null).toBe(true);
    expect(screen.getByText('depends_on：未设') !== null).toBe(true);
    expect(screen.getByText('packages/desktop') !== null).toBe(true);
  });

  it('dependsOn 三形态：空数组呈（空列表）、单元素、三元素以「、」连接', () => {
    const cases: { dependsOn: string[]; text: string }[] = [
      { dependsOn: [], text: 'depends_on：（空列表）' },
      { dependsOn: ['only'], text: 'depends_on：only' },
      { dependsOn: ['a', 'b', 'c'], text: 'depends_on：a、b、c' },
    ];

    for (const { dependsOn, text } of cases) {
      const { unmount } = render(
        <BasicConfigSection
          config={makeConfig({
            projects: [{ root: 'web', language: null, dependsOn }],
          })}
          defaultedPaths={new Set()}
        />,
      );

      expect(screen.getByText(text) !== null).toBe(true);
      unmount();
    }
  });

  it('已设空串 language：呈 language：空值形态，不落「未设」（与 null 可区分）', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [{ root: 'web', language: '', dependsOn: [] }],
        })}
        defaultedPaths={new Set()}
      />,
    );

    const group = screen.getByTestId('config-projects');
    const item = group.querySelector('li');
    expect(item?.textContent).toBe('weblanguage：depends_on：（空列表）');
    expect(screen.queryByText('language：未设')).toBeNull();
  });

  it('projects 空数组：呈「（空列表）」且零 li（既有空态不变）', () => {
    render(<BasicConfigSection config={makeConfig({ projects: [] })} defaultedPaths={new Set()} />);

    const group = screen.getByTestId('config-projects');
    expect(group.querySelectorAll('li')).toHaveLength(0);
    expect(screen.getByText('（空列表）') !== null).toBe(true);
  });

  it('超长值：language 1200 字符 + dependsOn 20 项完整呈现（不截断不抛错）', () => {
    const longLanguage = 'l'.repeat(1200);
    const deps = Array.from({ length: 20 }, (_, index) => `dep-${index}`);

    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [{ root: 'web', language: longLanguage, dependsOn: deps }],
        })}
        defaultedPaths={new Set()}
      />,
    );

    expect(screen.getByText(`language：${longLanguage}`) !== null).toBe(true);
    expect(screen.getByText(`depends_on：${deps.join('、')}`) !== null).toBe(true);
  });

  it('特殊字符：language 含换行与 emoji 时值原样进入呈现面', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [{ root: 'web', language: 'Type\nScript🙂', dependsOn: null }],
        })}
        defaultedPaths={new Set()}
      />,
    );

    // 正则文本查询容忍空白折叠：值不被吞、不转义错乱
    expect(screen.getByText(/language：Type\s*Script🙂/) !== null).toBe(true);
    expect(screen.queryByText('language：未设')).toBeNull();
  });

  it('只读边界：含两字段的 projects 区取到 0 个编辑控件（无编辑 / 保存入口）', () => {
    render(
      <BasicConfigSection
        config={makeConfig({
          projects: [{ root: 'web', language: 'typescript', dependsOn: ['api'] }],
        })}
        defaultedPaths={new Set()}
      />,
    );

    const group = screen.getByTestId('config-projects');
    expect(group.querySelectorAll('input, textarea, select, button')).toHaveLength(0);
    const section = screen.getByTestId('config-basic');
    expect(section.querySelectorAll('button')).toHaveLength(0);
  });
});
