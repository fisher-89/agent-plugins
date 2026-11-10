import { render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const { createHighlighterCoreMock, createJavaScriptRegexEngineMock } = vi.hoisted(() => ({
  createHighlighterCoreMock: vi.fn(),
  createJavaScriptRegexEngineMock: vi.fn(),
}));

// 进程边界 Mock：只替 shiki 的装配入口（建实例 / 建引擎），高亮器行为由可控
// 替身给出——被测的是 `highlighter.tsx` 自己的语言解析、按行组装与 state 透传，
// shiki 的语法语义不在这里重测。语言 loader 由替身真实执行，用来校验登记表与
// shiki 模块对齐（路径写错时 hook 侧会静默回退纯文本，没有别处能拦住）。
vi.mock('shiki/core', () => ({ createHighlighterCore: createHighlighterCoreMock }));
vi.mock('shiki/engine/javascript', () => ({
  createJavaScriptRegexEngine: createJavaScriptRegexEngineMock,
}));

interface TokenizeOptions {
  lang: string;
  themes: { light: string; dark: string };
  defaultColor: boolean;
  grammarState?: unknown;
}

interface FakeToken {
  content: string;
  htmlStyle?: Record<string, string>;
}

interface FakeShikiOptions {
  /** 实例创建时已装载的语言（模拟「上一轮已装载过」） */
  loaded?: string[];
  /** 按 code 产出 token 行；缺省时每行一枚无样式 token */
  tokens?: (code: string) => FakeToken[][];
}

const ENGINE = { name: 'js-engine' };
const STATE = { lang: 'fake-grammar-state' };

/** 高亮器替身：记录装载 / tokenize 调用，行与 token 由用例定制。 */
function makeShiki(options: FakeShikiOptions = {}) {
  const loaded = [...(options.loaded ?? [])];
  const tokensFor =
    options.tokens ??
    ((code: string): FakeToken[][] => code.split('\n').map((line) => [{ content: line }]));

  return {
    codeToTokens: vi.fn((code: string, _options: TokenizeOptions) => ({ tokens: tokensFor(code) })),
    loadLanguage: vi.fn(
      async (
        loader: () => Promise<{ default: ReadonlyArray<{ name: string; aliases?: string[] }> }>,
      ) => {
        const module = await loader();
        for (const grammar of module.default) {
          loaded.push(grammar.name, ...(grammar.aliases ?? []));
        }
      },
    ),
    getLastGrammarState: vi.fn(() => STATE),
    getLoadedLanguages: () => [...loaded],
  };
}

type FakeShiki = ReturnType<typeof makeShiki>;

/** 每个用例重置模块缓存：高亮器实例在模块级单例缓存（`highlighter ??=`）。 */
beforeEach(() => {
  vi.resetModules();
  createHighlighterCoreMock.mockReset();
  createJavaScriptRegexEngineMock.mockReset();
  createJavaScriptRegexEngineMock.mockReturnValue(ENGINE);
});

async function setup(shiki: FakeShiki) {
  createHighlighterCoreMock.mockResolvedValue(shiki);
  return (await import('./highlighter')).loadHighlighter;
}

/** 装载指定语言并断言非 null（未登记语言的用例走 setup 单独断言）。 */
async function loadFor(shiki: FakeShiki, language: string) {
  const loadHighlighter = await setup(shiki);
  const highlighter = await loadHighlighter(language);

  if (!highlighter) {
    throw new Error(`语言 ${language} 未解析出高亮器`);
  }

  return highlighter;
}

function tokenizeOptions(shiki: FakeShiki, index = 0): TokenizeOptions {
  return shiki.codeToTokens.mock.calls[index][1];
}

function renderLines(lines: ReactNode[]) {
  return render(
    <div>
      {lines.map((line, index) => (
        <div data-line key={index}>
          {line}
        </div>
      ))}
    </div>,
  );
}

function rows(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>('[data-line]')];
}

function spans(row: HTMLElement): HTMLElement[] {
  return [...row.querySelectorAll<HTMLElement>('span')];
}

// ---------------------------------------------------------------------------
// 语言名解析：登记表 / 别名 / 大小写 / ansi / 未登记
// ---------------------------------------------------------------------------

const ALIASES: Array<[string, string]> = [
  ['js', 'javascript'],
  ['md', 'markdown'],
  ['py', 'python'],
  ['sh', 'bash'],
  ['shell', 'bash'],
  ['shellscript', 'bash'],
  ['ts', 'typescript'],
  ['yml', 'yaml'],
  ['zsh', 'bash'],
];

describe('loadHighlighter：语言名解析', () => {
  it.each(ALIASES)('别名 %s → 按 %s 高亮', async (alias, resolved) => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, alias);

    highlighter.highlight('x');

    expect(tokenizeOptions(shiki).lang).toBe(resolved);
  });

  it.each(['TypeScript', 'TYPESCRIPT', 'Ts'])('大小写归一：%s → typescript', async (name) => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, name);

    highlighter.highlight('x');

    expect(tokenizeOptions(shiki).lang).toBe('typescript');
  });

  it.each([
    ['JSON', 'json'],
    ['Markdown', 'markdown'],
    ['Py', 'python'],
  ])('大小写归一命中别名：%s → %s', async (name, resolved) => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, name);

    highlighter.highlight('x');

    expect(tokenizeOptions(shiki).lang).toBe(resolved);
  });

  it('ansi（含大小写变体）不请求装载语言', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ANSI');

    highlighter.highlight('x');

    expect(tokenizeOptions(shiki).lang).toBe('ansi');
    expect(shiki.loadLanguage).not.toHaveBeenCalled();
  });

  it.each(['rust', 'go', '', ' ', 'text'])(
    '未登记语言 %j → null，且不创建高亮器实例（惰性）',
    async (name) => {
      const shiki = makeShiki();
      const loadHighlighter = await setup(shiki);

      expect(await loadHighlighter(name)).toBeNull();
      expect(createHighlighterCoreMock).not.toHaveBeenCalled();
    },
  );
});

// ---------------------------------------------------------------------------
// 装配接线：引擎 / 主题 / 单例 / 装载缓存
// ---------------------------------------------------------------------------

describe('loadHighlighter：高亮器单例与装载缓存', () => {
  it('引擎与双主题只装配一次：不同语言共用同一实例', async () => {
    const shiki = makeShiki();
    const loadHighlighter = await setup(shiki);

    await loadHighlighter('ts');
    await loadHighlighter('md');

    expect(createHighlighterCoreMock).toHaveBeenCalledTimes(1);
    expect(createJavaScriptRegexEngineMock).toHaveBeenCalledTimes(1);
    expect(createJavaScriptRegexEngineMock).toHaveBeenCalledWith({ forgiving: true });

    const [config] = createHighlighterCoreMock.mock.calls[0];
    expect(config.langs).toEqual([]);
    expect(config.engine).toBe(ENGINE);
    // 主题以动态 import 传入（shiki 接受模块命名空间，取其 default）
    const themes = await Promise.all(config.themes);
    expect(themes.map((theme) => theme.default.name)).toEqual(['github-light', 'github-dark']);
  });

  it('同一语言重复装载（含别名）只装载一次', async () => {
    const shiki = makeShiki();
    const loadHighlighter = await setup(shiki);

    await loadHighlighter('ts');
    await loadHighlighter('typescript');

    expect(shiki.loadLanguage).toHaveBeenCalledTimes(1);
  });

  it('实例已装载该语言：跳过装载直接 tokenize', async () => {
    const shiki = makeShiki({ loaded: ['python'] });
    const highlighter = await loadFor(shiki, 'py');

    highlighter.highlight('x');

    expect(shiki.loadLanguage).not.toHaveBeenCalled();
    expect(tokenizeOptions(shiki).lang).toBe('python');
  });

  it('不同语言各自装载一次', async () => {
    const shiki = makeShiki();
    const loadHighlighter = await setup(shiki);

    await loadHighlighter('py');
    await loadHighlighter('ts');

    expect(shiki.loadLanguage).toHaveBeenCalledTimes(2);
  });
});

// ---------------------------------------------------------------------------
// 登记表与 shiki 模块对齐（真实 loader）：路径 / 语言名任一写错都会在此红
// ---------------------------------------------------------------------------

const DECLARED_LANGUAGES = [
  'bash',
  'css',
  'diff',
  'html',
  'javascript',
  'json',
  'jsx',
  'markdown',
  'python',
  'sql',
  'tsx',
  'typescript',
  'yaml',
];

describe('loadHighlighter：登记语言与 shiki 模块对齐', () => {
  it.each(DECLARED_LANGUAGES)('%s 的 loader 可装载并登记该语言名', async (language) => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, language);

    expect(highlighter).toBeDefined();
    expect(shiki.loadLanguage).toHaveBeenCalledTimes(1);
    expect(shiki.getLoadedLanguages()).toContain(language);
  });
});

// ---------------------------------------------------------------------------
// highlight：token → span 组装与 state 透传
// ---------------------------------------------------------------------------

describe('highlight：行 / span 组装', () => {
  it('每个 token 一枚 span：文本、固定类名、内联样式', async () => {
    const shiki = makeShiki({
      tokens: () => [
        [
          {
            content: 'const',
            htmlStyle: { '--shiki-light': '#d73a49', '--shiki-dark': '#f97583' },
          },
          { content: ' a' },
        ],
        [{ content: '= 1' }],
      ],
    });
    const highlighter = await loadFor(shiki, 'ts');

    const { container } = renderLines(highlighter.highlight('const a\n= 1').lines);
    const [firstRow, secondRow] = rows(container);
    const [styled, plain] = spans(firstRow);

    expect(rows(container)).toHaveLength(2);
    expect(spans(firstRow)).toHaveLength(2);
    expect(styled.textContent).toBe('const');
    expect(styled.className).toBe('text-(--shiki-light) dark:text-(--shiki-dark)');
    expect(styled.style.getPropertyValue('--shiki-light')).toBe('#d73a49');
    expect(styled.style.getPropertyValue('--shiki-dark')).toBe('#f97583');
    expect(plain.getAttribute('style')).toBeNull();
    expect(spans(secondRow).map((span) => span.textContent)).toEqual(['= 1']);
  });

  it('空代码产出一行零 token（不塌成无行）', async () => {
    const shiki = makeShiki({ tokens: () => [[]] });
    const highlighter = await loadFor(shiki, 'ts');

    const { container } = renderLines(highlighter.highlight('').lines);

    expect(rows(container)).toHaveLength(1);
    expect(spans(rows(container)[0])).toHaveLength(0);
  });

  it('把本次 token 序列交给 getLastGrammarState，并原样返回该 state', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ts');

    const result = highlighter.highlight('a');
    const tokens = shiki.codeToTokens.mock.results[0]?.value.tokens;

    expect(shiki.getLastGrammarState).toHaveBeenCalledTimes(1);
    expect(shiki.getLastGrammarState).toHaveBeenCalledWith(tokens);
    expect(result.state).toBe(STATE);
  });

  it('续接：首次 grammarState 为空，传入的 state 原样透传给 codeToTokens', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ts');

    const first = highlighter.highlight('a');
    highlighter.highlight('b', first.state);

    expect(tokenizeOptions(shiki, 0).grammarState).toBeUndefined();
    expect(tokenizeOptions(shiki, 1).grammarState).toBe(first.state);
  });

  it('固定 tokenize 选项：语言、双主题、关闭默认取色', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ts');

    highlighter.highlight('x');

    const options = tokenizeOptions(shiki);
    expect(options.lang).toBe('typescript');
    expect(options.themes).toEqual({ light: 'github-light', dark: 'github-dark' });
    expect(options.defaultColor).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// ANSI：逐行独立 tokenize（无 grammar 续接）
// ---------------------------------------------------------------------------

describe('highlight：ANSI 逐行 tokenize', () => {
  it('多行按 \\n 拆开逐行 tokenize，行数与拆分一致', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ansi');

    const result = highlighter.highlight('a\nb\nc');

    expect(shiki.codeToTokens.mock.calls.map(([code]) => code)).toEqual(['a', 'b', 'c']);
    expect(result.lines).toHaveLength(3);
  });

  it('尾随换行保留空行：`a\\n` 拆成两行 tokenize', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ansi');

    const result = highlighter.highlight('a\n');

    expect(shiki.codeToTokens).toHaveBeenCalledTimes(2);
    expect(shiki.codeToTokens.mock.calls[1][0]).toBe('');
    expect(result.lines).toHaveLength(2);
  });

  it('对照：非 ansi 语言整段一次 tokenize（不按行拆）', async () => {
    const shiki = makeShiki();
    const highlighter = await loadFor(shiki, 'ts');

    highlighter.highlight('a\nb');

    expect(shiki.codeToTokens).toHaveBeenCalledTimes(1);
    expect(shiki.codeToTokens.mock.calls[0][0]).toBe('a\nb');
  });
});
