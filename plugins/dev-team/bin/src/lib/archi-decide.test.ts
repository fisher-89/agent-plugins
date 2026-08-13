/**
 * 单元测试: archi-decide.ts — ADR create / list / update
 *
 * @see openspec/changes/migrate-archi-decide-to-mcp/test-design.md
 */

import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const fsReadMock = vi.hoisted(() => ({
  failPath: null as string | null,
}));

vi.mock('node:fs', async (importOriginal) => {
  const actual: Record<string, unknown> = await importOriginal();
  const originalReadFileSync = actual.readFileSync as (
    filepath: string,
    options?: unknown,
  ) => string | Buffer;
  return {
    ...actual,
    readFileSync: (filepath: string, options?: unknown) => {
      if (fsReadMock.failPath !== null && filepath === fsReadMock.failPath) {
        const err = new Error('EACCES: permission denied') as NodeJS.ErrnoException;
        err.code = 'EACCES';
        throw err;
      }
      return originalReadFileSync(filepath, options);
    },
  };
});

import * as fs from 'node:fs';

import type { AdrStatus, ArchiDecideCreateInput } from '../schemas/archi-decide.schema';
import { createAdr, listAdrs, updateAdr } from './archi-decide';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const FIXED_DATE = new Date('2026-08-12T10:00:00');

function setupProjectRoot(): { dir: string; cleanup: () => void } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'archi-decide-'));
  return {
    dir,
    cleanup: () => fs.rmSync(dir, { recursive: true, force: true }),
  };
}

function decisionsDir(projectRoot: string): string {
  return path.join(projectRoot, 'openspec', 'architecture', 'decisions');
}

function minimalCreate(overrides: Partial<ArchiDecideCreateInput> = {}): ArchiDecideCreateInput {
  return {
    title: 'Use PostgreSQL',
    background: 'We need a relational database for persistence.',
    decision: 'Adopt PostgreSQL as the primary datastore.',
    ...overrides,
  };
}

function writeAdrFile(projectRoot: string, filename: string, content: string): string {
  const dir = decisionsDir(projectRoot);
  fs.mkdirSync(dir, { recursive: true });
  const filepath = path.join(dir, filename);
  fs.writeFileSync(filepath, content, 'utf-8');
  return filepath;
}

function makeAdrMarkdown(opts: {
  title: string;
  date: string;
  status: string;
  scope?: string[];
}): string {
  const scopeBody = (opts.scope ?? ['model.sys.db']).map((s) => `- ${s}`).join('\n');
  return `# ADR: ${opts.title}
- **日期**: ${opts.date}
- **状态**: ${opts.status}

## 背景

Background text.

## 决策

Decision text.

## 后果

### 正面后果

- positive

### 负面后果

- negative

## 备选方案

(none)

## 影响范围

${scopeBody}
`;
}

function useFixedDate(): void {
  vi.useFakeTimers();
  vi.setSystemTime(FIXED_DATE);
}

// ---------------------------------------------------------------------------
// createAdr
// ---------------------------------------------------------------------------

describe('createAdr — 写入与字段 (AC-02)', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('合法 title/background/decision 时成功写入 YYYY-MM-DD-<kebab>.md 并返回 path/filename', () => {
    const result = createAdr(project.dir, minimalCreate());
    expect(result.success).toBe(true);
    expect(result.filename).toBe('2026-08-12-use-postgresql.md');
    expect(result.path).toBe(path.join(decisionsDir(project.dir), '2026-08-12-use-postgresql.md'));
    expect(fs.existsSync(result.path!)).toBe(true);

    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('# ADR: Use PostgreSQL');
    expect(content).toContain('We need a relational database');
    expect(content).toContain('Adopt PostgreSQL');
    expect(content).toMatch(/\*\*日期\*\*:\s*2026-08-12/);
    expect(content).toMatch(/\*\*状态\*\*:\s*proposed/);
    expect(content).toContain('## 背景');
    expect(content).toContain('## 决策');
    expect(content).toContain('## 后果');
    expect(content).toContain('## 备选方案');
    expect(content).toContain('## 影响范围');
  });
});

describe('createAdr — 默认状态 (AC-03)', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('省略 status 时文件 **状态** 为 proposed', () => {
    const result = createAdr(project.dir, minimalCreate());
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toMatch(/\*\*状态\*\*:\s*proposed/);
  });
});

describe('createAdr — 显式状态', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it.each(['proposed', 'accepted', 'deprecated', 'superseded'] as const)(
    'status 为 %s 时写入对应状态字面量',
    (status) => {
      const result = createAdr(project.dir, minimalCreate({ title: `Status ${status}`, status }));
      expect(result.success).toBe(true);
      const content = fs.readFileSync(result.path!, 'utf-8');
      expect(content).toMatch(new RegExp(`\\*\\*状态\\*\\*:\\s*${status}`));
    },
  );
});

describe('createAdr — 模板章节 (AC-07)', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('产出含 templates/adr.md 全部章节标题', () => {
    const result = createAdr(project.dir, minimalCreate());
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    for (const heading of [
      '## 背景',
      '## 决策',
      '## 后果',
      '### 正面后果',
      '### 负面后果',
      '## 备选方案',
      '## 影响范围',
    ]) {
      expect(content).toContain(heading);
    }
  });
});

describe('createAdr — alternatives / scope', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('alternatives 含 name/description/pros/cons（string 与 string[]）时渲染方案节', () => {
    const result = createAdr(project.dir, {
      ...minimalCreate({ title: 'Alt Render Test' }),
      alternatives: [
        {
          name: 'MySQL',
          description: 'Popular SQL database',
          pros: 'Wide adoption',
          cons: ['License concerns', 'Feature gaps'],
        },
        {
          name: 'SQLite',
          pros: ['Embedded', 'Zero config'],
          cons: 'Single writer',
        },
      ],
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('### 方案 1：MySQL');
    expect(content).toContain('Popular SQL database');
    expect(content).toContain('**优点**: Wide adoption');
    expect(content).toContain('**缺点**: License concerns');
    expect(content).toContain('### 方案 2：SQLite');
  });

  it('scope 渲染为 - item 列表', () => {
    const result = createAdr(project.dir, {
      ...minimalCreate({ title: 'Scope Render' }),
      scope: ['model.api', 'model.db'],
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('- model.api');
    expect(content).toContain('- model.db');
  });
});

describe('createAdr — 非法 status', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('status 为 draft 时 success 为 false 且不创建新文件', () => {
    const result = createAdr(
      project.dir,
      minimalCreate({ status: 'draft' as unknown as AdrStatus }),
    );
    expect(result.success).toBe(false);
    expect(result.error).toMatch(/Invalid status/);
    expect(fs.existsSync(decisionsDir(project.dir))).toBe(false);
  });
});

describe('createAdr — 文件已存在', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('目标文件已存在时 success 为 false 且原文件内容不变', () => {
    const filename = '2026-08-12-use-postgresql.md';
    const original = '# ADR: Original\n- **状态**: accepted\n';
    writeAdrFile(project.dir, filename, original);

    const result = createAdr(project.dir, minimalCreate());
    expect(result.success).toBe(false);
    expect(result.error).toContain('ADR already exists');
    expect(fs.readFileSync(path.join(decisionsDir(project.dir), filename), 'utf-8')).toBe(original);
  });
});

describe('createAdr — 模板缺失', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('模板缺失时 success 为 false 且含明确 error，不创建 decisions 文件', () => {
    const origExists = fs.existsSync.bind(fs);
    const existsSpy = vi.spyOn(fs, 'existsSync').mockImplementation((p) => {
      const normalized = String(p).replace(/\\/g, '/');
      if (normalized.includes('templates/adr.md')) {
        return false;
      }
      return origExists(p);
    });
    try {
      const result = createAdr(project.dir, minimalCreate());
      expect(result.success).toBe(false);
      expect(result.error).toContain('ADR template not found');
      expect(fs.existsSync(decisionsDir(project.dir))).toBe(false);
    } finally {
      existsSpy.mockRestore();
    }
  });
});

describe('createAdr — title 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('title 为空字符串时不抛未捕获异常且文件名 slug 为空段', () => {
    const result = createAdr(project.dir, minimalCreate({ title: '' }));
    expect(result.success).toBe(true);
    expect(result.filename).toBe('2026-08-12-.md');
  });

  it('title 超长（>1000 chars）时不抛未捕获异常且行为可预期', () => {
    const longTitle = 'x'.repeat(1001);
    expect(() => createAdr(project.dir, minimalCreate({ title: longTitle }))).not.toThrow();
    const result = createAdr(project.dir, minimalCreate({ title: `Alt ${longTitle}` }));
    expect(typeof result.success).toBe('boolean');
    if (result.success) {
      expect(fs.existsSync(result.path!)).toBe(true);
    } else {
      expect(result.error).toBeDefined();
    }
  });

  it('title 含空白、标点、emoji、换行时 kebab slug 合法', () => {
    const result = createAdr(project.dir, minimalCreate({ title: '  Hello World! 🐘\nFoo-Bar  ' }));
    expect(result.success).toBe(true);
    expect(result.filename).toMatch(/^2026-08-12-hello-world-foo-bar\.md$/);
  });

  it('title 为 undefined 时抛出可观测错误', () => {
    expect(() =>
      createAdr(project.dir, minimalCreate({ title: undefined as unknown as string })),
    ).toThrow();
  });

  it('title 为 null 时抛出可观测错误', () => {
    expect(() =>
      createAdr(project.dir, minimalCreate({ title: null as unknown as string })),
    ).toThrow();
  });
});

describe('createAdr — alternatives 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('alternatives 为空数组时仍含 ## 备选方案 章节', () => {
    const result = createAdr(project.dir, {
      ...minimalCreate({ title: 'Empty Alts' }),
      alternatives: [],
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('## 备选方案');
    expect(content).toContain('(无备选方案记录)');
  });

  it('alternatives 单元素与 50 项时方案序号连续且全部写出', () => {
    const single = createAdr(project.dir, {
      ...minimalCreate({ title: 'One Alt' }),
      alternatives: [{ name: 'Only' }],
    });
    expect(single.success).toBe(true);
    expect(fs.readFileSync(single.path!, 'utf-8')).toContain('### 方案 1：Only');

    const many = createAdr(project.dir, {
      ...minimalCreate({ title: 'Many Alts' }),
      alternatives: Array.from({ length: 50 }, (_, i) => ({ name: `Opt${i + 1}` })),
    });
    expect(many.success).toBe(true);
    const manyContent = fs.readFileSync(many.path!, 'utf-8');
    expect(manyContent).toContain('### 方案 1：Opt1');
    expect(manyContent).toContain('### 方案 50：Opt50');
  });

  it('alternatives 为 undefined 且元素缺 description/pros/cons 时不抛错', () => {
    const result = createAdr(project.dir, {
      ...minimalCreate({ title: 'Sparse Alt' }),
      alternatives: [{ name: 'Bare' }],
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('### 方案 1：Bare');
    expect(content).toContain('**描述**: Bare');
  });
});

describe('createAdr — scope 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('scope 为空数组或省略时影响范围为 - (none)', () => {
    for (const input of [
      minimalCreate({ title: 'No Scope', scope: [] }),
      minimalCreate({ title: 'Omit Scope' }),
    ]) {
      const result = createAdr(project.dir, input);
      expect(result.success).toBe(true);
      expect(fs.readFileSync(result.path!, 'utf-8')).toContain('- (none)');
    }
  });

  it('scope 单元素、超大列表与特殊字符 id 时逐条 - 列表', () => {
    const single = createAdr(project.dir, {
      ...minimalCreate({ title: 'One Scope' }),
      scope: ['only-one'],
    });
    expect(fs.readFileSync(single.path!, 'utf-8')).toContain('- only-one');

    const many = createAdr(project.dir, {
      ...minimalCreate({ title: 'Big Scope' }),
      scope: Array.from({ length: 30 }, (_, i) => `id-${i}`),
    });
    expect(fs.readFileSync(many.path!, 'utf-8')).toContain('- id-29');

    const special = createAdr(project.dir, {
      ...minimalCreate({ title: 'Special Scope' }),
      scope: ['model/api-v2', '元素🧪'],
    });
    const specialContent = fs.readFileSync(special.path!, 'utf-8');
    expect(specialContent).toContain('- model/api-v2');
    expect(specialContent).toContain('- 元素🧪');
  });
});

describe('createAdr — consequences 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    useFixedDate();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('consequences 为空串、省略或超长文本时保留 ## 后果 子节', () => {
    for (const [title, consequences] of [
      ['Empty Cons', ''],
      ['Omit Cons', undefined],
      ['Long Cons', 'c'.repeat(2000)],
    ] as const) {
      const result = createAdr(
        project.dir,
        consequences === undefined
          ? minimalCreate({ title })
          : { ...minimalCreate({ title }), consequences },
      );
      expect(result.success).toBe(true);
      const content = fs.readFileSync(result.path!, 'utf-8');
      expect(content).toContain('## 后果');
      expect(content).toContain('### 正面后果');
      expect(content).toContain('### 负面后果');
    }
  });
});

// ---------------------------------------------------------------------------
// listAdrs
// ---------------------------------------------------------------------------

describe('listAdrs — 排序与过滤 (AC-04)', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('多份不同日期 ADR 时 adrs 按 date 降序且 count 正确', () => {
    writeAdrFile(
      project.dir,
      '2026-01-01-old.md',
      makeAdrMarkdown({ title: 'Old', date: '2026-01-01', status: 'proposed' }),
    );
    writeAdrFile(
      project.dir,
      '2026-06-15-mid.md',
      makeAdrMarkdown({ title: 'Mid', date: '2026-06-15', status: 'accepted' }),
    );
    writeAdrFile(
      project.dir,
      '2026-08-01-new.md',
      makeAdrMarkdown({ title: 'New', date: '2026-08-01', status: 'proposed' }),
    );

    const result = listAdrs(project.dir);
    expect(result.count).toBe(3);
    expect(result.adrs.map((a) => a.date)).toEqual(['2026-08-01', '2026-06-15', '2026-01-01']);
    expect(result.adrs[0]?.title).toBe('New');
  });

  it('status 为 accepted 时仅返回该状态条目', () => {
    writeAdrFile(
      project.dir,
      '2026-01-01-a.md',
      makeAdrMarkdown({ title: 'A', date: '2026-01-01', status: 'accepted' }),
    );
    writeAdrFile(
      project.dir,
      '2026-02-01-p.md',
      makeAdrMarkdown({ title: 'P', date: '2026-02-01', status: 'proposed' }),
    );

    const result = listAdrs(project.dir, 'accepted');
    expect(result.count).toBe(1);
    expect(result.adrs[0]?.status).toBe('accepted');
  });
});

describe('listAdrs — 空目录', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('decisions/ 不存在时返回空列表', () => {
    expect(listAdrs(project.dir)).toEqual({ adrs: [], count: 0 });
  });
});

describe('listAdrs — 解析失败文件', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
  });

  afterEach(() => {
    fsReadMock.failPath = null;
    project.cleanup();
  });

  it('目录含不可读 .md 文件时跳过坏文件，其余仍返回', () => {
    writeAdrFile(
      project.dir,
      '2026-03-01-good.md',
      makeAdrMarkdown({ title: 'Good', date: '2026-03-01', status: 'proposed' }),
    );
    writeAdrFile(
      project.dir,
      '2026-03-02-unreadable.md',
      makeAdrMarkdown({ title: 'Bad', date: '2026-03-02', status: 'proposed' }),
    );

    const unreadablePath = path.join(decisionsDir(project.dir), '2026-03-02-unreadable.md');
    fsReadMock.failPath = unreadablePath;

    const result = listAdrs(project.dir);
    expect(result.count).toBe(1);
    expect(result.adrs[0]?.filename).toBe('2026-03-01-good.md');
  });

  it('目录含非 .md 文件时忽略', () => {
    const dir = decisionsDir(project.dir);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'notes.txt'), 'x', 'utf-8');
    writeAdrFile(
      project.dir,
      '2026-04-01-only.md',
      makeAdrMarkdown({ title: 'Only', date: '2026-04-01', status: 'proposed' }),
    );
    expect(listAdrs(project.dir).count).toBe(1);
  });

  it('目录含非文件 .md 条目时跳过，其余仍返回', () => {
    writeAdrFile(
      project.dir,
      '2026-03-01-good.md',
      makeAdrMarkdown({ title: 'Good', date: '2026-03-01', status: 'proposed' }),
    );
    const dir = decisionsDir(project.dir);
    fs.mkdirSync(path.join(dir, '2026-03-02-bad.md'), { recursive: true });

    const result = listAdrs(project.dir);
    expect(result.count).toBe(1);
    expect(result.adrs[0]?.filename).toBe('2026-03-01-good.md');
  });
});

describe('listAdrs — status 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    for (const status of ['proposed', 'accepted', 'deprecated', 'superseded'] as const) {
      writeAdrFile(
        project.dir,
        `2026-05-01-${status}.md`,
        makeAdrMarkdown({ title: status, date: '2026-05-01', status }),
      );
    }
  });

  afterEach(() => {
    project.cleanup();
  });

  it.each(['proposed', 'accepted', 'deprecated', 'superseded'] as const)(
    '过滤 status=%s 时仅返回该状态',
    (status) => {
      const result = listAdrs(project.dir, status);
      expect(result.count).toBe(1);
      expect(result.adrs[0]?.status).toBe(status);
    },
  );

  it('status 省略与 undefined 等价于不过滤', () => {
    expect(listAdrs(project.dir).count).toBe(4);
    expect(listAdrs(project.dir, undefined).count).toBe(4);
  });
});

describe('listAdrs — 影响范围 scope 解析（回归 \\Z→Z bug）', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('## 影响范围 为文件最后一节时 scope[] 非空', () => {
    const content = `# ADR: Scope Last Section
- **日期**: 2026-08-01
- **状态**: proposed

## 背景

Background.

## 决策

Decision.

## 后果

### 正面后果

- positive

### 负面后果

- negative

## 备选方案

(none)

## 影响范围

- model.api.gateway
- model.db.users
`;
    writeAdrFile(project.dir, '2026-08-01-scope-last.md', content);

    const result = listAdrs(project.dir);
    expect(result.count).toBe(1);
    expect(result.adrs[0]?.scope).toEqual(['model.api.gateway', 'model.db.users']);
    expect(result.adrs[0]?.scope.length).toBeGreaterThan(0);
  });

  it('影响范围条目后直接 EOF（无尾随换行）时仍能解析 scope', () => {
    const content = `# ADR: EOF Scope
- **日期**: 2026-08-02
- **状态**: accepted

## 背景

bg

## 决策

dec

## 后果

### 正面后果

- p

### 负面后果

- n

## 备选方案

(none)

## 影响范围

- trailing.eof.item`;
    const dir = decisionsDir(project.dir);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, '2026-08-02-eof.md'), content, 'utf-8');

    const result = listAdrs(project.dir);
    expect(result.count).toBe(1);
    expect(result.adrs[0]?.scope).toEqual(['trailing.eof.item']);
    expect(result.adrs[0]?.scope.length).toBeGreaterThan(0);
  });

  it('create 后 listAdrs 能读回非空 scope', () => {
    useFixedDate();
    const created = createAdr(project.dir, {
      ...minimalCreate({ title: 'Roundtrip Scope' }),
      scope: ['model.core', 'model.edge'],
    });
    expect(created.success).toBe(true);

    const listed = listAdrs(project.dir);
    expect(listed.count).toBe(1);
    expect(listed.adrs[0]?.scope).toEqual(['model.core', 'model.edge']);
    expect(listed.adrs[0]?.scope.length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// updateAdr
// ---------------------------------------------------------------------------

describe('updateAdr — 改状态 (AC-05)', () => {
  let project: ReturnType<typeof setupProjectRoot>;
  const filename = '2026-07-01-update-me.md';

  beforeEach(() => {
    project = setupProjectRoot();
    writeAdrFile(
      project.dir,
      filename,
      makeAdrMarkdown({ title: 'Update Me', date: '2026-07-01', status: 'proposed' }),
    );
  });

  afterEach(() => {
    project.cleanup();
  });

  it('更新为 accepted 时 success 为 true 且文件状态变更', () => {
    const result = updateAdr(project.dir, { file: filename, status: 'accepted' });
    expect(result.success).toBe(true);
    expect(result.old_status).toBe('proposed');
    expect(result.new_status).toBe('accepted');
    const content = fs.readFileSync(path.join(decisionsDir(project.dir), filename), 'utf-8');
    expect(content).toMatch(/\*\*状态\*\*:\s*accepted/);
  });
});

describe('updateAdr — superseded (AC-05)', () => {
  let project: ReturnType<typeof setupProjectRoot>;
  const filename = '2026-07-01-update-me.md';

  beforeEach(() => {
    project = setupProjectRoot();
    writeAdrFile(
      project.dir,
      filename,
      makeAdrMarkdown({ title: 'Update Me', date: '2026-07-01', status: 'proposed' }),
    );
  });

  afterEach(() => {
    project.cleanup();
  });

  it('status 为 superseded 且提供 superseded_by 时插入 **取代者**', () => {
    const result = updateAdr(project.dir, {
      file: filename,
      status: 'superseded',
      superseded_by: '2026-08-01-new-decision.md',
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(path.join(decisionsDir(project.dir), filename), 'utf-8');
    expect(content).toMatch(/\*\*状态\*\*:\s*superseded/);
    expect(content).toContain('**取代者**: 2026-08-01-new-decision.md');
  });
});

describe('updateAdr — 缺 superseded_by (AC-05)', () => {
  let project: ReturnType<typeof setupProjectRoot>;
  const filename = '2026-07-01-update-me.md';

  beforeEach(() => {
    project = setupProjectRoot();
    writeAdrFile(
      project.dir,
      filename,
      makeAdrMarkdown({ title: 'Update Me', date: '2026-07-01', status: 'proposed' }),
    );
  });

  afterEach(() => {
    project.cleanup();
  });

  it('status 为 superseded 且无 superseded_by 时失败且文件字节级不变', () => {
    const filepath = path.join(decisionsDir(project.dir), filename);
    const before = fs.readFileSync(filepath, 'utf-8');
    const result = updateAdr(project.dir, { file: filename, status: 'superseded' });
    expect(result.success).toBe(false);
    expect(result.error).toMatch(/superseded_by/);
    expect(fs.readFileSync(filepath, 'utf-8')).toBe(before);
  });
});

describe('updateAdr — 文件不存在', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('file 指向不存在 ADR 时 success 为 false 且不创建新文件', () => {
    const result = updateAdr(project.dir, { file: 'missing.md', status: 'accepted' });
    expect(result.success).toBe(false);
    expect(result.error).toContain('ADR not found');
    expect(fs.existsSync(decisionsDir(project.dir))).toBe(false);
  });
});

describe('updateAdr — 非法 status', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('非法 status 时 success 为 false 且文件不变', () => {
    writeAdrFile(
      project.dir,
      '2026-07-02-x.md',
      makeAdrMarkdown({ title: 'X', date: '2026-07-02', status: 'proposed' }),
    );
    const filepath = path.join(decisionsDir(project.dir), '2026-07-02-x.md');
    const before = fs.readFileSync(filepath, 'utf-8');
    const result = updateAdr(project.dir, {
      file: '2026-07-02-x.md',
      status: 'draft' as unknown as AdrStatus,
    });
    expect(result.success).toBe(false);
    expect(fs.readFileSync(filepath, 'utf-8')).toBe(before);
  });
});

describe('updateAdr — 读失败', () => {
  let project: ReturnType<typeof setupProjectRoot>;
  const filename = '2026-07-05-read-fail.md';

  beforeEach(() => {
    project = setupProjectRoot();
    writeAdrFile(
      project.dir,
      filename,
      makeAdrMarkdown({ title: 'Read Fail', date: '2026-07-05', status: 'proposed' }),
    );
  });

  afterEach(() => {
    fsReadMock.failPath = null;
    project.cleanup();
  });

  it('readFileSync 抛 OSError 等价错误时 success 为 false 且文件不变', () => {
    const filepath = path.join(decisionsDir(project.dir), filename);
    const before = fs.readFileSync(filepath, 'utf-8');
    fsReadMock.failPath = filepath;

    const result = updateAdr(project.dir, { file: filename, status: 'accepted' });
    expect(result.success).toBe(false);
    expect(result.error).toMatch(/Failed to read ADR/);
    fsReadMock.failPath = null;
    expect(fs.readFileSync(filepath, 'utf-8')).toBe(before);
  });
});

describe('updateAdr — file 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;
  const filename = '2026-07-03-safe.md';

  beforeEach(() => {
    project = setupProjectRoot();
    writeAdrFile(
      project.dir,
      filename,
      makeAdrMarkdown({ title: 'Safe', date: '2026-07-03', status: 'proposed' }),
    );
  });

  afterEach(() => {
    project.cleanup();
  });

  it.each(['', '../../../etc/passwd', '..\\..\\etc\\passwd', 'x'.repeat(500)])(
    'file=%s 时失败且不写到 decisions 外',
    (file) => {
      const result = updateAdr(project.dir, { file, status: 'accepted' });
      expect(result.success).toBe(false);
      if (file.includes('..')) {
        expect(result.error).toMatch(/outside decisions directory/);
      }
      expect(fs.existsSync(path.join(project.dir, 'etc', 'passwd'))).toBe(false);
      const safePath = path.join(decisionsDir(project.dir), filename);
      expect(fs.readFileSync(safePath, 'utf-8')).toMatch(/\*\*状态\*\*:\s*proposed/);
    },
  );
});

describe('updateAdr — superseded_by 边界', () => {
  let project: ReturnType<typeof setupProjectRoot>;
  const filename = '2026-07-04-sup.md';

  beforeEach(() => {
    project = setupProjectRoot();
    writeAdrFile(
      project.dir,
      filename,
      makeAdrMarkdown({ title: 'Sup', date: '2026-07-04', status: 'proposed' }),
    );
  });

  afterEach(() => {
    project.cleanup();
  });

  it('superseded_by 为空字符串视为缺失并失败', () => {
    const filepath = path.join(decisionsDir(project.dir), filename);
    const before = fs.readFileSync(filepath, 'utf-8');
    const result = updateAdr(project.dir, {
      file: filename,
      status: 'superseded',
      superseded_by: '',
    });
    expect(result.success).toBe(false);
    expect(fs.readFileSync(filepath, 'utf-8')).toBe(before);
  });

  it('文件已有 **取代者** 时再次 superseded 不重复插入多行', () => {
    updateAdr(project.dir, {
      file: filename,
      status: 'superseded',
      superseded_by: 'first.md',
    });
    updateAdr(project.dir, {
      file: filename,
      status: 'superseded',
      superseded_by: 'second.md',
    });
    const content = fs.readFileSync(path.join(decisionsDir(project.dir), filename), 'utf-8');
    const matches = content.match(/\*\*取代者\*\*/g);
    expect(matches?.length).toBe(1);
  });
});
