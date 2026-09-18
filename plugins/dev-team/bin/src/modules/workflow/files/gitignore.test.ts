/**
 * 单元测试: modules/workflow/files/gitignore.ts — 层级 .gitignore 过滤器（自研装载与组装层）
 *
 * 覆盖范围（openspec/changes/move-files-write-into-workflow-module/test-design.md）:
 * - AC-1: 根层实样装载（本仓库 .gitignore 实样判定）、层级应用（深层优先 /
 *   根层规则对完整路径生效 / 目录短路 / 祖先链逐层行走）、缺失层视为无规则、
 *   读取/解析/判定失败 fail-open + onWarn 诊断、输入边界（空路径 / 特殊字符 / 层缓存）
 *
 * `ignore` 包的单文件语法与匹配语义由库自身保障、不逐项验证（只测自研提取/组装层）。
 * 文件系统不 mock：mkdtempSync 临时项目内真实写入各层 .gitignore（含实样内容、
 * 子目录规则、目录形态的 .gitignore），真实读写判定；onWarn 注入 vi.fn() 捕获诊断。
 * 唯一例外：「解析抛错」用例经 vi.mock('ignore') 注入哨兵——ignore v7 对任何规则
 * 内容都不抛错（非法模式被静默过滤），故该用例需人为制造解析异常；被测对象仍是
 * 包装层 loadLayer 的 fail-open catch 分支，其余用例全部走真实库语义。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { type Ignore } from 'ignore';
import { describe, expect, it, vi } from 'vite-plus/test';

import { type GitignoreFilter, isGitIgnored, loadGitignoreFilter } from './gitignore';

// ---------------------------------------------------------------------------
// Mock：ignore 包解析哨兵（仅「解析抛错」用例布防，其余用例走真实库语义）
// ---------------------------------------------------------------------------

const { parseBombArmed } = vi.hoisted(() => ({
  parseBombArmed: { current: false },
}));

vi.mock('ignore', async (importOriginal) => {
  const actual = await importOriginal<{ default?: unknown }>();
  // ignore v7 为 CJS（module.exports = factory），vitest interop 下 default 即工厂函数
  const realIgnore = actual.default as (options?: unknown) => {
    add: (pattern: unknown) => boolean;
  };
  const wrappedIgnore = (options?: unknown): { add: (pattern: unknown) => boolean } => {
    const instance = realIgnore(options);
    const realAdd = instance.add.bind(instance);
    instance.add = (pattern: unknown): boolean => {
      if (
        parseBombArmed.current &&
        typeof pattern === 'string' &&
        pattern.includes('__PARSE_BOMB__')
      ) {
        throw new Error('模拟 ignore 解析异常');
      }
      return realAdd(pattern);
    };
    return instance;
  };
  return { ...actual, default: wrappedIgnore };
});

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'gitignore-test-'): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

/** 在 dir 目录写入一个层（目录 + .gitignore 文件）。 */
function writeGitignore(dir: string, content: string): void {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, '.gitignore'), content, 'utf-8');
}

/** 本仓库 .gitignore 实样（根层装载用例）。 */
const REPO_SAMPLE_GITIGNORE = [
  '.*',
  '!.claude-plugin',
  '!.cursor-plugin',
  '!.gitignore',
  'memory',
  '',
  'node_modules',
  '',
  '# 突变测试',
  '_stryker-tmp',
  '',
  '# 构建相关',
  'plugins/*/.pack-staging/',
  '*/dev-team/bin/*.cjs.map',
  '',
].join('\n');

// ===========================================================================
// loadGitignoreFilter / isGitIgnored — 根层实样装载 (AC-1)
// ===========================================================================

describe('loadGitignoreFilter / isGitIgnored — 根层实样装载 (AC-1)', () => {
  it('项目根 .gitignore 为本仓库实样（.* + !.claude-plugin + !.cursor-plugin + !.gitignore + memory）时，.claude/agent-memory/x.md 判定 true、.claude-plugin/marketplace.json 判定 false、memory/notes.md 判定 true（提取准确性代表用例）', () => {
    const project = createTempProject();
    try {
      writeGitignore(project.root, REPO_SAMPLE_GITIGNORE);

      const filter = loadGitignoreFilter(project.root);
      expect(isGitIgnored(filter, '.claude/agent-memory/x.md')).toBe(true);
      expect(isGitIgnored(filter, '.claude-plugin/marketplace.json')).toBe(false);
      expect(isGitIgnored(filter, 'memory/notes.md')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('同一实样下未命中路径 src/a.ts 判定为 false（无规则路径放行）', () => {
    const project = createTempProject();
    try {
      writeGitignore(project.root, REPO_SAMPLE_GITIGNORE);

      const filter = loadGitignoreFilter(project.root);
      expect(isGitIgnored(filter, 'src/a.ts')).toBe(false);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// isGitIgnored — 层级应用 (AC-1)
// ===========================================================================

describe('isGitIgnored — 层级应用 (AC-1)', () => {
  it('子目录规则覆盖根规则（深层优先）：根 *.log、sub/.gitignore 含 !keep.log → sub/keep.log 为 false 且 sub/other.log 为 true', () => {
    const project = createTempProject();
    try {
      writeGitignore(project.root, '*.log\n');
      writeGitignore(path.join(project.root, 'sub'), '!keep.log\n');

      const filter = loadGitignoreFilter(project.root);
      expect(isGitIgnored(filter, 'sub/keep.log')).toBe(false);
      expect(isGitIgnored(filter, 'sub/other.log')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('深层层存在时根层规则仍对完整路径生效：根 *.log、a/b/.gitignore 规则不相关（*.tmp）→ a/b/c.log 仍为 true（激活层集合按「相对该层的路径后缀」逐层测试）', () => {
    const project = createTempProject();
    try {
      writeGitignore(project.root, '*.log\n');
      writeGitignore(path.join(project.root, 'a', 'b'), '*.tmp\n');

      const filter = loadGitignoreFilter(project.root);
      // 根层规则经完整路径后缀仍命中深层路径
      expect(isGitIgnored(filter, 'a/b/c.log')).toBe(true);
      // 深层层自身规则照常生效（证明深层确已装载参与判定）
      expect(isGitIgnored(filter, 'a/b/other.tmp')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('中间目录被排除（根 ignored/）、ignored/.gitignore 含 !rescue.txt → ignored/rescue.txt 仍为 true（目录短路：被排除目录内 .gitignore 不装载、不参与判定）', () => {
    const project = createTempProject();
    try {
      writeGitignore(project.root, 'ignored/\n');
      writeGitignore(path.join(project.root, 'ignored'), '!rescue.txt\n');

      const filter = loadGitignoreFilter(project.root);
      // 若深层 !rescue.txt 参与，rescue.txt 应为 false；目录短路使其不生效
      expect(isGitIgnored(filter, 'ignored/rescue.txt')).toBe(true);
      expect(isGitIgnored(filter, 'ignored/other.txt')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('a/b/c.ts 多层嵌套 → a/.gitignore 与 a/b/.gitignore 均装载参与判定（祖先链逐层行走）', () => {
    const project = createTempProject();
    try {
      writeGitignore(path.join(project.root, 'a'), '*.ts\n');
      writeGitignore(path.join(project.root, 'a', 'b'), '!c.ts\n');

      const filter = loadGitignoreFilter(project.root);
      // a/b 层负模式定论（深层优先）
      expect(isGitIgnored(filter, 'a/b/c.ts')).toBe(false);
      // a 层规则对未经 a/b 层负模式覆盖的路径生效
      expect(isGitIgnored(filter, 'a/b/d.ts')).toBe(true);
      // 仅 a 层激活的路径
      expect(isGitIgnored(filter, 'a/x.ts')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// loadGitignoreFilter — 缺失与失败层 (AC-1)
// ===========================================================================

describe('loadGitignoreFilter — 缺失与失败层 (AC-1)', () => {
  it('某层目录缺 .gitignore → 该层视为无规则且不触发 onWarn（缺失为正常路径、非告警），判定继续', () => {
    const project = createTempProject();
    try {
      const onWarn = vi.fn();
      // 根与子目录均无 .gitignore
      const filter = loadGitignoreFilter(project.root, onWarn);
      expect(isGitIgnored(filter, 'src/a.ts')).toBe(false);
      expect(isGitIgnored(filter, 'src/deep/b.ts')).toBe(false);
      expect(onWarn).not.toHaveBeenCalled();

      // 根层有规则、子层缺失 → 缺失层视为无规则，根规则照常参与判定
      writeGitignore(project.root, '*.log\n');
      const filter2 = loadGitignoreFilter(project.root, onWarn);
      expect(isGitIgnored(filter2, 'sub/keep.log')).toBe(true);
      expect(onWarn).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('层上 .gitignore 为目录（读取失败）→ onWarn 收到含该层路径的诊断消息，该层按无规则处理，整体不抛错', () => {
    const project = createTempProject();
    try {
      const onWarn = vi.fn();
      // 根层 .gitignore 为目录（装载期即刻失败）
      fs.mkdirSync(path.join(project.root, '.gitignore'), { recursive: true });
      // 子层 .gitignore 为目录（判定期惰性装载失败）
      fs.mkdirSync(path.join(project.root, 'src', '.gitignore'), { recursive: true });

      const filter = loadGitignoreFilter(project.root, onWarn);
      expect(() => isGitIgnored(filter, 'src/a.ts')).not.toThrow();
      expect(isGitIgnored(filter, 'src/a.ts')).toBe(false);

      // 根层 1 次（eager）+ src 层 1 次（惰性且缓存后不再重复）
      expect(onWarn).toHaveBeenCalledTimes(2);
      const messages = onWarn.mock.calls.map((c) => String(c[0]));
      expect(messages.some((m) => m.includes(path.join(project.root, '.gitignore')))).toBe(true);
      expect(messages.some((m) => m.includes(path.join(project.root, 'src', '.gitignore')))).toBe(
        true,
      );
    } finally {
      project.cleanup();
    }
  });

  it('层 .gitignore 含使 ignore 解析抛错的规则内容 → 解析异常被吞、onWarn 诊断、该层按无规则处理（fail-open）', () => {
    const project = createTempProject();
    try {
      const onWarn = vi.fn();
      parseBombArmed.current = true;
      // 两层各含哨兵内容 + 若能解析本应命中的规则
      writeGitignore(project.root, '__PARSE_BOMB__\n*.log\n');
      writeGitignore(path.join(project.root, 'sub'), '__PARSE_BOMB__\nz.tmp\n');

      const filter = loadGitignoreFilter(project.root, onWarn);
      // 根层解析抛错 → *.log 未生效（整层按无规则处理）
      expect(isGitIgnored(filter, 'x.log')).toBe(false);
      // 惰性子层解析抛错 → z.tmp 未生效
      expect(isGitIgnored(filter, 'sub/z.tmp')).toBe(false);

      const messages = onWarn.mock.calls.map((c) => String(c[0]));
      expect(
        messages.some(
          (m) =>
            m.includes('模拟 ignore 解析异常') && m.includes(path.join(project.root, '.gitignore')),
        ),
      ).toBe(true);
      expect(
        messages.some(
          (m) =>
            m.includes('模拟 ignore 解析异常') &&
            m.includes(path.join(project.root, 'sub', '.gitignore')),
        ),
      ).toBe(true);
    } finally {
      parseBombArmed.current = false;
      project.cleanup();
    }
  });

  it('isGitIgnored 判定过程抛错 → 返回 false（fail-open）且 onWarn 诊断，不向调用方传播异常', () => {
    const onWarn = vi.fn();
    // 手工构造判定即抛错的根层（GitignoreFilter 为导出的结构化接口）
    const bombLayer = {
      test: () => {
        throw new Error('模拟判定爆炸');
      },
    } as unknown as Ignore;
    const filter: GitignoreFilter = {
      projectRoot: 'irrelevant-root',
      rootLayer: bombLayer,
      layers: new Map(),
      onWarn,
    };

    expect(() => isGitIgnored(filter, 'src/a.ts')).not.toThrow();
    expect(isGitIgnored(filter, 'src/a.ts')).toBe(false);
    expect(onWarn).toHaveBeenCalledWith(expect.stringContaining('src/a.ts'));
    expect(onWarn).toHaveBeenCalledWith(expect.stringContaining('模拟判定爆炸'));
  });
});

// ===========================================================================
// isGitIgnored — 输入边界 (AC-1)
// ===========================================================================

describe('isGitIgnored — 输入边界 (AC-1)', () => {
  it('relPath 为空字符串 → 直接返回 false（fail-open 短路，不触发任何层装载与告警）', () => {
    const onWarn = vi.fn();
    // 根层判定即抛错：若空路径未在入口短路，将触发 onWarn 而非静默 false
    const bombLayer = {
      test: () => {
        throw new Error('不应触达');
      },
    } as unknown as Ignore;
    const filter: GitignoreFilter = {
      projectRoot: 'irrelevant-root',
      rootLayer: bombLayer,
      layers: new Map(),
      onWarn,
    };

    expect(isGitIgnored(filter, '')).toBe(false);
    expect(onWarn).not.toHaveBeenCalled();
  });

  it('relPath 含中文 / 空格 / emoji → 按字面参与匹配，组装层不做语义裁剪', () => {
    const project = createTempProject();
    try {
      writeGitignore(project.root, '含 空格.txt\n中文*.md\nemoji-🧪.ts\n');

      const filter = loadGitignoreFilter(project.root);
      expect(isGitIgnored(filter, '含 空格.txt')).toBe(true);
      expect(isGitIgnored(filter, '中文路径.md')).toBe(true);
      expect(isGitIgnored(filter, 'emoji-🧪.ts')).toBe(true);
      // 未命中的特殊字符路径照常放行
      expect(isGitIgnored(filter, 'src/ok.ts')).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('同一 filter 重复判定同层路径（.claude/a 后 .claude/b）→ 层缓存复用、结论一致（单 hook 调用内缓存契约）', () => {
    const project = createTempProject();
    try {
      writeGitignore(path.join(project.root, '.claude'), '*\n');
      const filter = loadGitignoreFilter(project.root);

      expect(isGitIgnored(filter, '.claude/a')).toBe(true);
      expect(isGitIgnored(filter, '.claude/b')).toBe(true);

      // 改写磁盘规则：若层被缓存复用，新规则对该 filter 不生效（结论保持一致）
      fs.writeFileSync(
        path.join(project.root, '.claude', '.gitignore'),
        'nothing-matches\n',
        'utf-8',
      );
      expect(isGitIgnored(filter, '.claude/b')).toBe(true);

      // 对照：新建 filter 重新装载 → 新规则生效（证明上述确为缓存复用而非规则恒真）
      expect(isGitIgnored(loadGitignoreFilter(project.root), '.claude/b')).toBe(false);
    } finally {
      project.cleanup();
    }
  });
});
