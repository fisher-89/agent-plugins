/**
 * 单元测试: expand-includes.ts — fragment 解析与 __INCLUDE: 展开
 */

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it, vi, type Mock } from 'vite-plus/test';

import { getEnv, type ProductEnv } from './env';
import { expandIncludes } from './expand-includes';

const tempRoots: string[] = [];
let cwdSpy: Mock | undefined;

function makeFragmentRoot(setup: (fragmentsDir: string) => void): string {
  const root = mkdtempSync(join(tmpdir(), 'expand-includes-'));
  tempRoots.push(root);
  const fragmentsDir = join(root, '_fragments');
  mkdirSync(fragmentsDir, { recursive: true });
  setup(fragmentsDir);
  return root;
}

function mockCwd(root: string): void {
  cwdSpy?.mockRestore();
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(root);
}

afterEach(() => {
  cwdSpy?.mockRestore();
  cwdSpy = undefined;
  while (tempRoots.length > 0) {
    const dir = tempRoots.pop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
});

describe('expandIncludes', () => {
  it('Cursor env 展开 __INCLUDE:static-analysis-gate__ 为 .cursor.md 正文（含 run_static_analysis）', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'static-analysis-gate.cursor.md'), 'run_static_analysis step\n');
      writeFileSync(join(dir, 'static-analysis-gate.md'), '');
    });
    mockCwd(root);
    const out = expandIncludes('before __INCLUDE:static-analysis-gate__ after', getEnv('cursor'));
    expect(out).toContain('run_static_analysis');
    expect(out).not.toContain('__INCLUDE:');
    expect(out.startsWith('before ')).toBe(true);
    expect(out.endsWith(' after')).toBe(true);
  });

  it('同时存在平台档与 default 时优先嵌入平台档正文（非 default）', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'prefer.cursor.md'), 'PLATFORM');
      writeFileSync(join(dir, 'prefer.md'), 'DEFAULT');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:prefer__', getEnv('cursor'))).toBe('PLATFORM');
  });

  it('仅存在 .cursor.md 时 env.agent=cursor 嵌入平台档正文', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'only-cursor.cursor.md'), 'cursor only');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:only-cursor__', getEnv('cursor'))).toBe('cursor only');
  });

  it('Claude env + 空 default 文件时 token 位置替换为空串（宿主前后文保留）', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'static-analysis-gate.md'), '');
    });
    mockCwd(root);
    const out = expandIncludes('head __INCLUDE:static-analysis-gate__ tail', getEnv('claude'));
    expect(out).toBe('head  tail');
  });

  it('嵌套 include 按深度递归展开且最终无残留 __INCLUDE:', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'outer.md'), 'O[__INCLUDE:inner__]');
      writeFileSync(join(dir, 'inner.md'), 'INNER');
    });
    mockCwd(root);
    const out = expandIncludes('__INCLUDE:outer__', getEnv('claude'));
    expect(out).toBe('O[INNER]');
    expect(out).not.toContain('__INCLUDE:');
  });

  it('展开结果内含 __BIN:cli__ 等 env token 时原样保留', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'bin-hold.md'), 'use __BIN:cli__ here');
    });
    mockCwd(root);
    const out = expandIncludes('__INCLUDE:bin-hold__', getEnv('cursor'));
    expect(out).toContain('__BIN:cli__');
  });

  it('环状 fragment 互相引用时 error message 含完整环路径', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'a.md'), '__INCLUDE:b__');
      writeFileSync(join(dir, 'b.md'), '__INCLUDE:a__');
    });
    mockCwd(root);
    expect(() => expandIncludes('__INCLUDE:a__', getEnv('cursor'))).toThrow(
      /Include cycle detected: a → b → a/,
    );
  });

  it('fragment 互相引用形成环时抛错', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'a.md'), '__INCLUDE:b__');
      writeFileSync(join(dir, 'b.md'), '__INCLUDE:a__');
    });
    mockCwd(root);
    expect(() => expandIncludes('__INCLUDE:a__', getEnv('cursor'))).toThrow(
      /Include cycle detected/,
    );
  });

  it('引用不存在的 id 时抛错且 message 含缺失 fragment id', () => {
    const root = makeFragmentRoot(() => {});
    mockCwd(root);
    expect(() => expandIncludes('__INCLUDE:not-there__', getEnv('cursor'))).toThrow(
      /Missing fragment "not-there"/,
    );
  });

  it('text 为 null / undefined 时抛错', () => {
    expect(() => expandIncludes(null as unknown as string, getEnv('cursor'))).toThrow();
    expect(() => expandIncludes(undefined as unknown as string, getEnv('cursor'))).toThrow();
  });

  it('env 为 null / undefined 时抛错', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'x.md'), 'x');
    });
    mockCwd(root);
    expect(() => expandIncludes('__INCLUDE:x__', null as unknown as ProductEnv)).toThrow();
    expect(() => expandIncludes('__INCLUDE:x__', undefined as unknown as ProductEnv)).toThrow();
  });

  it('env={} 在无匹配 fragment 时抛错', () => {
    const root = makeFragmentRoot(() => {});
    mockCwd(root);
    expect(() => expandIncludes('__INCLUDE:missing__', {} as ProductEnv)).toThrow();
  });

  it('嵌套深度超过软上限 16 时抛错', () => {
    const root = makeFragmentRoot((dir) => {
      for (let i = 0; i < 20; i++) {
        const next = i + 1;
        const body = next <= 19 ? `__INCLUDE:depth-${next}__` : 'leaf';
        writeFileSync(join(dir, `depth-${i}.md`), body);
      }
    });
    mockCwd(root);
    expect(() => expandIncludes('__INCLUDE:depth-0__', getEnv('claude'))).toThrow(
      /Include depth exceeded/,
    );
  });

  it('text 为空字符串时返回空字符串', () => {
    expect(expandIncludes('', getEnv('cursor'))).toBe('');
  });

  it('text 无 include token 时原样返回', () => {
    const text = 'plain markdown\n';
    expect(expandIncludes(text, getEnv('cursor'))).toBe(text);
  });

  it('超长文本含多枚合法 include 时全部展开且不崩溃', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'chip.md'), 'X');
    });
    mockCwd(root);
    const token = '__INCLUDE:chip__ ';
    const text = token.repeat(120);
    expect(text.length).toBeGreaterThan(1000);
    const out = expandIncludes(text, getEnv('claude'));
    expect(out).not.toContain('__INCLUDE:');
    expect(out.replace(/X /g, '').trim()).toBe('');
  });

  it('嵌入体 trim 后 splice；宿主全文本身不被 trim', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'pad.md'), '\n  body  \n');
    });
    mockCwd(root);
    expect(expandIncludes('  host __INCLUDE:pad__ end  ', getEnv('cursor'))).toBe(
      '  host body end  ',
    );
  });

  it('stack=[] 时从空链正常展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'solo.md'), 'solo');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:solo__', getEnv('cursor'), [])).toBe('solo');
  });

  it('stack 为单元素且不含当前 id 时正常展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'b.md'), 'B');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:b__', getEnv('cursor'), ['a'])).toBe('B');
  });

  it('stack 已含当前 id（环）时抛错', () => {
    expect(() => expandIncludes('__INCLUDE:a__', getEnv('cursor'), ['a'])).toThrow(
      /Include cycle detected/,
    );
  });

  it('stack 为超大列表且不含当前 id 时仍可展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'leaf.md'), 'leaf');
    });
    mockCwd(root);
    const bigStack = Array.from({ length: 1000 }, (_, i) => `id-${i}`);
    expect(expandIncludes('__INCLUDE:leaf__', getEnv('cursor'), bigStack)).toBe('leaf');
  });

  it('stack=null 时抛错', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'solo.md'), 'solo');
    });
    mockCwd(root);
    expect(() =>
      expandIncludes('__INCLUDE:solo__', getEnv('cursor'), null as unknown as string[]),
    ).toThrow();
  });

  it('stack=undefined 时使用默认空栈正常展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'solo.md'), 'solo');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:solo__', getEnv('cursor'), undefined)).toBe('solo');
  });

  it('depth=0 时可展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'd0.md'), 'zero');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:d0__', getEnv('cursor'), [], 0)).toBe('zero');
  });

  it('depth=16 且无 include 时返回原文', () => {
    expect(expandIncludes('no tokens', getEnv('cursor'), [], 16)).toBe('no tokens');
  });

  it('depth=17 时立即抛错且 message 含 depth 上限', () => {
    expect(() => expandIncludes('any', getEnv('cursor'), [], 17)).toThrow(
      /Include depth exceeded 16/,
    );
  });

  it('depth=-1 时仍可展开一层 include', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'neg.md'), 'neg');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:neg__', getEnv('cursor'), [], -1)).toBe('neg');
  });

  it('depth=Number.MAX_SAFE_INTEGER 时立即抛错', () => {
    expect(() => expandIncludes('text', getEnv('cursor'), [], Number.MAX_SAFE_INTEGER)).toThrow(
      /Include depth exceeded/,
    );
  });

  it('depth=undefined 等同从 0 起算并可展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'undef.md'), 'ok');
    });
    mockCwd(root);
    expect(expandIncludes('__INCLUDE:undef__', getEnv('cursor'), [], undefined)).toBe('ok');
  });

  it('depth=null 时按 0 处理并可展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'null-depth.md'), 'ok');
    });
    mockCwd(root);
    expect(
      expandIncludes('__INCLUDE:null-depth__', getEnv('cursor'), [], null as unknown as number),
    ).toBe('ok');
  });

  it('env 含多余字段时仍按 agent 优先平台档展开', () => {
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, 'gate.cursor.md'), 'cursor-gate');
      writeFileSync(join(dir, 'gate.md'), 'default-gate');
    });
    mockCwd(root);
    const env = { ...getEnv('cursor'), extra: true } as ProductEnv;
    expect(expandIncludes('__INCLUDE:gate__', env)).toBe('cursor-gate');
  });

  it('id 为超长合法串时按文件系统存在性判定（不崩溃）', () => {
    const longId = `a-${'b'.repeat(200)}`;
    const root = makeFragmentRoot((dir) => {
      writeFileSync(join(dir, `${longId}.md`), 'long');
    });
    mockCwd(root);
    expect(expandIncludes(`__INCLUDE:${longId}__`, getEnv('claude'))).toBe('long');
    rmSync(join(root, '_fragments', `${longId}.md`));
    expect(() => expandIncludes(`__INCLUDE:${longId}__`, getEnv('claude'))).toThrow(
      new RegExp(`Missing fragment "${longId}"`),
    );
  });

  it('非法字面量 __INCLUDE:Foo__ / __INCLUDE:a/b__ 不被替换，原文保留', () => {
    const text = 'keep __INCLUDE:Foo__ and __INCLUDE:a/b__';
    expect(expandIncludes(text, getEnv('cursor'))).toBe(text);
  });
});
