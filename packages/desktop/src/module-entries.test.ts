import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 取数面：全部用例只读源文本（磁盘面断言），零运行期 import —— 12 个 barrel 的
// 实现模块与三方依赖一概不装载，故本文件零组件渲染、零 mock、零 DOM 断言，
// 运行环境沿用仓默认（环境归 vite.config.ts，本文件不自带口径）。
// vitest 以包根 packages/desktop 为 cwd，`src/**` 是稳定相对面。
// ---------------------------------------------------------------------------

/** 包内某文件的源码全文。 */
function sourceOf(relative: string): string {
  return readFileSync(resolve(process.cwd(), relative), 'utf8');
}

/** 包内某目录的一级条目名。 */
function filesOf(relative: string): string[] {
  return readdirSync(resolve(process.cwd(), relative));
}

function isCommentLine(line: string): boolean {
  return line.startsWith('//') || line.startsWith('/*') || line.startsWith('*');
}

/** 有效语句行：去空行、去注释行（入口正文只有语句，注释是 ui 的唯一例外）。 */
function codeLines(source: string): string[] {
  return source
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '' && !isCommentLine(line));
}

interface EntryBlock {
  /** `export type { … }` 形态（`isolatedModules` 下类型出口必须走此形态）。 */
  typeOnly: boolean;
  /** 出口名，按书写序。 */
  names: string[];
  /** 来源 specifier。 */
  spec: string;
}

/** barrel 的 `export { … } from '<spec>';` 与 `export type { … } from '<spec>';` 语句。 */
function entryBlocksOf(source: string): EntryBlock[] {
  return [...source.matchAll(/export\s+(?:type\s+)?\{([^}]*)\}\s*from\s*'([^']*)'\s*;/g)].map(
    (match) => ({
      typeOnly: match[0].startsWith('export type'),
      names: namesIn(match[1]),
      spec: match[2],
    }),
  );
}

/** `{ A, B }` 块里的出口名（跨行块与行内 `type` 修饰一并认）。 */
function namesIn(block: string): string[] {
  return block
    .split(',')
    .map((name) => name.trim())
    .filter((name) => name !== '')
    .map((name) => name.replace(/^type\s+/, ''));
}

/** 每条 `from '<spec>'` 的 specifier，按出现序。 */
function specifiersOf(source: string): string[] {
  return [...source.matchAll(/from\s+'([^']*)'/g)].map((match) => match[1]);
}

/** 模块的具名导出（`export { … }` 块、`export type { … }` 块与 `export <decl> <名>`）。 */
function declaredNamesOf(source: string): string[] {
  const names = new Set<string>();
  for (const match of source.matchAll(/export\s+(?:type\s+)?\{([^}]*)\}/g)) {
    for (const name of namesIn(match[1])) names.add(name);
  }
  const declarations =
    /export\s+(?:declare\s+)?(?:async\s+)?(?:function|const|class|interface|type|enum|let|var)\s+([A-Za-z_$][\w$]*)/g;
  for (const match of source.matchAll(declarations)) names.add(match[1]);
  return [...names];
}

/** 重复名（跨模块重名会被 `export *` 静默吞并）。 */
function duplicatesOf(names: string[]): string[] {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const name of names) {
    if (seen.has(name)) duplicates.add(name);
    seen.add(name);
  }
  return [...duplicates];
}

/**
 * 单元素内直属文件的 specifier：`./<小写连字符名>`，零目录段 / 扩展名 / 别名，
 * 零自指 `./index`，名字上界 64 字符（最长实际名 24 = `session-transcript-panel`）。
 */
function isDirectSpecifier(spec: string): boolean {
  return /^\.\/[a-z0-9-]{1,64}$/.test(spec) && spec !== './index';
}

/** 实现语句行（入口不承载实现：零 const / function / import / 类型定义）。 */
const IMPLEMENTATION_LINE = /^(const|let|var|function|class|interface|type|enum|import)\s/;

/** specifier 非法形态：空 / 父目录 / 子目录段 / hooks / lib / 别名 / 扩展名 / 自指 / 大写 / 超长 / 特殊字符。 */
const ILLEGAL_SPECIFIERS = [
  '',
  '../architecture-view',
  './components/agent-panel',
  './hooks/use-agent-session',
  './lib/util',
  '@/components/agent',
  './agent-debug-view.tsx',
  './index',
  './AgentDebugView',
  `./${'a'.repeat(1200)}`,
  './agent\ndebug',
  `./agent${String.fromCharCode(0)}debug`,
  './agent-😀',
];

describe('12 个目录模块入口：在盘、唯一与零自指', () => {
  const ENTRY_DIRS = [
    'src/views/agent',
    'src/views/agents',
    'src/views/architecture',
    'src/views/config',
    'src/views/db',
    'src/views/info',
    'src/views/changes',
    'src/views/changes/agent',
    'src/views/changes/flow',
    'src/views/changes/renderers',
    'src/views/explores',
    'src/components/ui',
  ];

  it('应 12 个目录各恰一个 index.ts（components/agent 的 index.tsx 不在列）', () => {
    expect(ENTRY_DIRS).toHaveLength(12);
    for (const dir of ENTRY_DIRS) {
      const entries = filesOf(dir).filter((file) => /^index\.tsx?$/.test(file));
      expect(entries, `${dir} 恰一个入口文件`).toEqual(['index.ts']);
    }
  });

  it('应每个入口正文非空且零自指 specifier', () => {
    for (const dir of ENTRY_DIRS) {
      const source = sourceOf(`${dir}/index.ts`);
      expect(source.trim(), `${dir}/index.ts 正文非空`).not.toBe('');
      expect(source.includes("from './index'"), `${dir}/index.ts 零自指`).toBe(false);
    }
  });
});

describe('views/agent 入口：单值出口与 Props 不外露', () => {
  const ENTRY = 'src/views/agent/index.ts';

  it('应恰一条具名 re-export 且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual([
      "export { AgentDebugView } from './agent-debug-view';",
    ]);
  });

  it('应只导 AgentDebugView（零通配、零默认导出）', () => {
    const source = sourceOf(ENTRY);
    const blocks = entryBlocksOf(source);
    expect(blocks.flatMap((block) => block.names)).toEqual(['AgentDebugView']);
    expect(blocks.map((block) => block.spec)).toEqual(['./agent-debug-view']);
    expect(blocks.every((block) => !block.typeOnly)).toBe(true);
    expect(source.includes('export *')).toBe(false);
    expect(source.includes('export default')).toBe(false);
  });

  it('不应外露 AgentDebugViewProps（*Props 不进公共面）', () => {
    expect(sourceOf(ENTRY).includes('AgentDebugViewProps')).toBe(false);
  });

  it('应零实现语句与零注释（四类全零命中）', () => {
    const source = sourceOf(ENTRY);
    expect(codeLines(source).filter((line) => IMPLEMENTATION_LINE.test(line))).toEqual([]);
    expect(
      source
        .split('\n')
        .map((line) => line.trim())
        .filter(isCommentLine),
    ).toEqual([]);
    expect(source.includes('export default')).toBe(false);
  });

  it('specifier 应只指向本元素直属文件', () => {
    const specs = specifiersOf(sourceOf(ENTRY));
    expect(specs).toEqual(['./agent-debug-view']);
    expect(specs.filter((spec) => !isDirectSpecifier(spec))).toEqual([]);
  });

  it('specifier 形态应拒绝空值 / 父目录 / 子目录 / hooks / lib / 别名 / 扩展名 / 自指 / 超长 / 特殊字符', () => {
    const rejected = ILLEGAL_SPECIFIERS.filter((spec) => !isDirectSpecifier(spec));
    expect(rejected).toEqual(ILLEGAL_SPECIFIERS);
  });

  it('应把无 specifier 的裸出口判为非入口语句', () => {
    expect(entryBlocksOf('export { AgentDebugView };')).toEqual([]);
  });

  it('抽取器判红面自证：空源与仅注释源解析为空', () => {
    expect(codeLines('')).toEqual([]);
    expect(codeLines("// export { AgentDebugView } from './agent-debug-view';")).toEqual([]);
  });
});

describe('views/agents 入口：单文件目录形态', () => {
  const ENTRY = 'src/views/agents/index.ts';

  it('应恰一条具名 re-export 且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(["export { AgentsView } from './agents-view';"]);
  });

  it('单文件目录应有且仅有一个非测试顶层源文件（agents-view.tsx）', () => {
    const topLevel = filesOf('src/views/agents').filter(
      (file) => file.endsWith('.tsx') && !file.endsWith('.test.tsx') && file !== 'index.ts',
    );
    expect(topLevel).toEqual(['agents-view.tsx']);
  });

  it('应只导 AgentsView 且零 Props 出口', () => {
    expect(entryBlocksOf(sourceOf(ENTRY)).flatMap((block) => block.names)).toEqual(['AgentsView']);
    expect(sourceOf(ENTRY).includes('Props')).toBe(false);
  });

  it('多导一条跨子目录语句应判红（行集合与 specifier 双判据同时命中）', () => {
    const tampered = `${sourceOf(ENTRY).trimEnd()}\nexport { AgentPanel } from './components/agent-panel';\n`;
    expect(codeLines(tampered)).not.toEqual(["export { AgentsView } from './agents-view';"]);
    expect(specifiersOf(tampered).filter((spec) => !isDirectSpecifier(spec))).toEqual([
      './components/agent-panel',
    ]);
  });

  it('边界形态：空源零条、空出口块零名', () => {
    expect(codeLines('')).toEqual([]);
    const emptyBlock = 'export {};';
    expect(codeLines(emptyBlock)).toHaveLength(1);
    expect(entryBlocksOf(emptyBlock).flatMap((block) => block.names)).toEqual([]);
  });
});

describe('views/architecture 入口：画布组件不进公共面', () => {
  const ENTRY = 'src/views/architecture/index.ts';

  it('应恰一条具名 re-export 且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual([
      "export { ArchitectureView } from './architecture-view';",
    ]);
  });

  it('应只导 ArchitectureView', () => {
    expect(entryBlocksOf(sourceOf(ENTRY)).flatMap((block) => block.names)).toEqual([
      'ArchitectureView',
    ]);
  });

  it('应零命中 ArchitectureCanvas（D6 留白：画布不进公共面）', () => {
    const source = sourceOf(ENTRY);
    expect(source.includes('ArchitectureCanvas')).toBe(false);
    expect(source.includes('./components/')).toBe(false);
  });

  it('specifier 形态应拒绝子目录与父目录指向', () => {
    for (const spec of ['./components/architecture-canvas', '../architecture-view']) {
      expect(isDirectSpecifier(spec), `${spec} 非法`).toBe(false);
    }
  });
});

describe('views/config 入口：子目录与 hooks 不外露', () => {
  const ENTRY = 'src/views/config/index.ts';

  it('应恰一条具名 re-export 且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(["export { ConfigView } from './config-view';"]);
  });

  it('应只导 ConfigView（5 个 section 与 hooks 均不外露）', () => {
    expect(entryBlocksOf(sourceOf(ENTRY)).flatMap((block) => block.names)).toEqual(['ConfigView']);
  });

  it('应零 components/ 与零 hooks/ specifier', () => {
    const specs = specifiersOf(sourceOf(ENTRY));
    expect(specs.filter((spec) => spec.startsWith('./components/'))).toEqual([]);
    expect(specs.filter((spec) => spec.startsWith('./hooks/'))).toEqual([]);
  });

  it('应恰一条非空行、零注释行、零 export default', () => {
    const source = sourceOf(ENTRY);
    expect(source.split('\n').filter((line) => line.trim() !== '')).toHaveLength(1);
    expect(
      source
        .split('\n')
        .map((line) => line.trim())
        .filter(isCommentLine),
    ).toEqual([]);
    expect(source.includes('export default')).toBe(false);
  });
});

describe('views/db 入口：Props 不外露与 specifier 形态', () => {
  const ENTRY = 'src/views/db/index.ts';

  it('应恰一条具名 re-export 且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual([
      "export { DbInspectorView } from './db-inspector-view';",
    ]);
  });

  it('应只导 DbInspectorView', () => {
    expect(entryBlocksOf(sourceOf(ENTRY)).flatMap((block) => block.names)).toEqual([
      'DbInspectorView',
    ]);
  });

  it('不应外露 DbInspectorViewProps', () => {
    expect(sourceOf(ENTRY).includes('DbInspectorViewProps')).toBe(false);
  });

  it('specifier 形态应拒绝扩展名与 hooks 子目录', () => {
    for (const spec of ['./db-inspector-view.tsx', './hooks/use-db-inspector']) {
      expect(isDirectSpecifier(spec), `${spec} 非法`).toBe(false);
    }
    expect(specifiersOf(sourceOf(ENTRY))).toEqual(['./db-inspector-view']);
  });
});

describe('views/info 入口：子目录 / hooks / lib 零穿透', () => {
  const ENTRY = 'src/views/info/index.ts';

  it('应恰一条具名 re-export 且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(["export { InfoView } from './info-view';"]);
  });

  it('应只导 InfoView（树 / 表格 / 统计三段与 use-code-stats 不外露）', () => {
    expect(entryBlocksOf(sourceOf(ENTRY)).flatMap((block) => block.names)).toEqual(['InfoView']);
  });

  it('应零 components/ / hooks/ / lib/ 三类 specifier', () => {
    const specs = specifiersOf(sourceOf(ENTRY));
    for (const segment of ['./components/', './hooks/', './lib/']) {
      expect(
        specs.filter((spec) => spec.startsWith(segment)),
        `${segment} 零命中`,
      ).toEqual([]);
    }
  });

  it('六个单文件目录应同构：各 1 条 1 名、零注释、零实现语句', () => {
    const SINGLE_FILE_ENTRIES = [
      ['src/views/agent/index.ts', 'AgentDebugView', './agent-debug-view'],
      ['src/views/agents/index.ts', 'AgentsView', './agents-view'],
      ['src/views/architecture/index.ts', 'ArchitectureView', './architecture-view'],
      ['src/views/config/index.ts', 'ConfigView', './config-view'],
      ['src/views/db/index.ts', 'DbInspectorView', './db-inspector-view'],
      ['src/views/info/index.ts', 'InfoView', './info-view'],
    ];
    for (const [entry, name, spec] of SINGLE_FILE_ENTRIES) {
      const source = sourceOf(entry);
      const blocks = entryBlocksOf(source);
      expect(blocks, `${entry} 恰一条语句`).toHaveLength(1);
      expect(blocks[0]!.names, `${entry} 恰一名`).toEqual([name]);
      expect(blocks[0]!.spec, `${entry} 的来源模块`).toBe(spec);
      expect(
        source
          .split('\n')
          .map((line) => line.trim())
          .filter(isCommentLine),
        `${entry} 零注释`,
      ).toEqual([]);
      expect(
        codeLines(source).filter((line) => IMPLEMENTATION_LINE.test(line)),
        `${entry} 零实现语句`,
      ).toEqual([]);
    }
  });
});

describe('views/changes 入口：两值出口与零跨子元素', () => {
  const ENTRY = 'src/views/changes/index.ts';
  const EXPECTED = [
    "export { ChangeDetailView } from './change-detail-view';",
    "export { ChangeListView } from './change-list-view';",
  ];
  const SUBDOMAIN_NAMES = [
    'buildFlowGraph',
    'ChangeFlowGraph',
    'DetailDrawer',
    'RunControlPanel',
    'mountMaterials',
    'runStepNodes',
    'unifiedRunEdges',
    'unifiedRunNodes',
    'ChangeChatPanel',
    'ArtifactTabs',
    'MarkdownDocRenderer',
  ];

  it('应恰两条语句且逐字等于期望行（按来源模块名字典序）', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(EXPECTED);
    const specs = specifiersOf(sourceOf(ENTRY));
    expect(specs).toEqual([...specs].sort());
  });

  it('两个出口应各来自本元素直属文件（值形态、零 type 修饰）', () => {
    const blocks = entryBlocksOf(sourceOf(ENTRY));
    expect(blocks.map((block) => [block.spec, ...block.names])).toEqual([
      ['./change-detail-view', 'ChangeDetailView'],
      ['./change-list-view', 'ChangeListView'],
    ]);
    expect(blocks.every((block) => !block.typeOnly)).toBe(true);
  });

  it('应零跨子元素 re-export（子域公共面名逐个零命中）', () => {
    const source = sourceOf(ENTRY);
    for (const name of SUBDOMAIN_NAMES) {
      expect(source.includes(name), `changes 入口零 ${name}`).toBe(false);
    }
    for (const spec of specifiersOf(source)) {
      expect(isDirectSpecifier(spec), `${spec} 是直属文件`).toBe(true);
    }
  });

  it('应恰 2 条非空行、零注释、零实现语句、零 export default', () => {
    const source = sourceOf(ENTRY);
    expect(source.split('\n').filter((line) => line.trim() !== '')).toHaveLength(2);
    expect(
      source
        .split('\n')
        .map((line) => line.trim())
        .filter(isCommentLine),
    ).toEqual([]);
    expect(codeLines(source).filter((line) => IMPLEMENTATION_LINE.test(line))).toEqual([]);
    expect(source.includes('export default')).toBe(false);
  });
});

describe('views/changes/agent 入口：四名只出面板', () => {
  const ENTRY = 'src/views/changes/agent/index.ts';

  it('应恰一条语句且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual([
      "export { ChangeChatPanel } from './change-chat-panel';",
    ]);
  });

  it('应只导 ChangeChatPanel', () => {
    expect(entryBlocksOf(sourceOf(ENTRY)).flatMap((block) => block.names)).toEqual([
      'ChangeChatPanel',
    ]);
  });

  it('未出口三名应逐个零命中', () => {
    const source = sourceOf(ENTRY);
    for (const name of ['ChangeChatComposer', 'ChangeChatPanelProps', 'ChangeChatComposerProps']) {
      expect(source.includes(name), `入口零 ${name}`).toBe(false);
    }
  });

  it('应零 hooks/ specifier（hooks 的 owner 就是本视图）', () => {
    expect(specifiersOf(sourceOf(ENTRY)).filter((spec) => spec.startsWith('./hooks/'))).toEqual([]);
  });

  it('同元素但非公共面的文件应判红（行数与名单同时不等）', () => {
    const tampered = `${sourceOf(ENTRY).trimEnd()}\nexport { ChangeChatComposer } from './change-chat-composer';\n`;
    expect(codeLines(tampered)).not.toEqual([
      "export { ChangeChatPanel } from './change-chat-panel';",
    ]);
    expect(codeLines(tampered)).toHaveLength(2);
    expect(specifiersOf(tampered)).toContain('./change-chat-composer');
  });
});

describe('views/changes/flow 入口：压力样本 11 名与类型出口', () => {
  const ENTRY = 'src/views/changes/flow/index.ts';
  const DIR = 'src/views/changes/flow';
  const EXPECTED = [
    "export { mountMaterials } from './attachments';",
    "export { ChangeFlowGraph } from './change-flow-graph';",
    "export { DetailDrawer } from './detail-drawer';",
    "export { buildFlowGraph } from './graph';",
    "export { RunControlPanel } from './run-control-panel';",
    "export { runStepNodes, unifiedRunEdges, unifiedRunNodes } from './run-state';",
    "export type { DrawerSelection, FlowGraph, FlowMaterials } from './types';",
  ];
  const VALUE_EXPORTS = [
    ['mountMaterials', './attachments'],
    ['ChangeFlowGraph', './change-flow-graph'],
    ['DetailDrawer', './detail-drawer'],
    ['buildFlowGraph', './graph'],
    ['RunControlPanel', './run-control-panel'],
    ['runStepNodes', './run-state'],
    ['unifiedRunEdges', './run-state'],
    ['unifiedRunNodes', './run-state'],
  ];
  const TYPE_EXPORTS = ['DrawerSelection', 'FlowGraph', 'FlowMaterials'];
  const UNEXPORTED = [
    'nodeIdentityKey',
    'runNodeKey',
    'UnifiedRunStep',
    'UnifiedRunEdge',
    'LiveRunFace',
    'FlowNode',
    'FlowEdge',
    'FlowColumn',
    'RuntimeFlowNode',
    'RoleSessionRef',
    'FlowEdgeKind',
    'RunStepGroup',
    'FlowRoleLabel',
    'graphTranslateExtent',
    'columnLayout',
    'nodeLayout',
    'edgeHandle',
    'settledDurationLabel',
    'runningDurationLabel',
    'formatTokens',
    'LabeledEdge',
    'PhaseColumnNode',
    'RunStepNode',
    'SessionTranscriptPanel',
    'StdoutPanel',
    'normalizeCarriageReturns',
    'isStuckToBottom',
    'COLUMN_HEADER_H',
    'ColumnFlowNode',
    'RunStepFlowNode',
  ];

  /** 目录内非测试源文件（入口自身不计）。 */
  function moduleFiles(): string[] {
    return filesOf(DIR)
      .filter((file) => /\.tsx?$/.test(file) && !/\.test\.tsx?$/.test(file) && file !== 'index.ts')
      .map((file) => `${DIR}/${file}`);
  }

  it('应恰 7 条语句且按来源模块名字典序', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(EXPECTED);
    const specs = specifiersOf(sourceOf(ENTRY)).map((spec) => spec.slice('./'.length));
    expect(specs).toEqual([...specs].sort());
  });

  it('8 个值出口应逐条落在对应来源的值出口语句内', () => {
    const blocks = entryBlocksOf(sourceOf(ENTRY)).filter((block) => !block.typeOnly);
    expect(blocks).toHaveLength(6);
    for (const [name, spec] of VALUE_EXPORTS) {
      const block = blocks.find((candidate) => candidate.spec === spec);
      expect(block, `${name} 的来源语句在场`).toBeDefined();
      expect(block!.names, `${name} 在 ${spec} 的值出口行内`).toContain(name);
    }
  });

  it('3 个类型出口应全部落在一条 export type 语句内', () => {
    const typeBlocks = entryBlocksOf(sourceOf(ENTRY)).filter((block) => block.typeOnly);
    expect(typeBlocks).toHaveLength(1);
    expect(typeBlocks[0]!.spec).toBe('./types');
    expect(typeBlocks[0]!.names).toEqual(TYPE_EXPORTS);
  });

  it('类型出口的值形态应判红（isolatedModules 纪律，双夹具对拍）', () => {
    const legal = entryBlocksOf(EXPECTED.join('\n')).filter((block) => block.typeOnly);
    const valueForm = entryBlocksOf(
      "export { DrawerSelection, FlowGraph, FlowMaterials } from './types';",
    );
    expect(legal[0]!.typeOnly).toBe(true);
    expect(valueForm[0]!.typeOnly).toBe(false);
    expect(valueForm[0]!.names).toEqual(TYPE_EXPORTS);
  });

  it('目录 40 个具名导出只出 11 个（其余 29 名逐个零命中）', () => {
    const declared = new Set(moduleFiles().flatMap((file) => declaredNamesOf(sourceOf(file))));
    expect(declared.size).toBe(40);
    for (const name of [...VALUE_EXPORTS.map(([name]) => name), ...TYPE_EXPORTS]) {
      expect(declared.has(name), `目录内有 ${name}`).toBe(true);
    }
    const source = sourceOf(ENTRY);
    for (const name of UNEXPORTED) {
      expect(source.includes(name), `flow 入口零 ${name}`).toBe(false);
    }
  });

  it('specifier 形态应拒绝父目录 / 子目录 / hooks / lib / 别名 / 扩展名', () => {
    for (const spec of [
      '../attachments',
      './hooks/use-flow-run',
      './lib/graph',
      '@/views/changes/flow/graph',
      './types.ts',
      '../changes/renderers',
    ]) {
      expect(isDirectSpecifier(spec), `${spec} 非法`).toBe(false);
    }
    expect(specifiersOf(sourceOf(ENTRY)).filter((spec) => !isDirectSpecifier(spec))).toEqual([]);
  });

  it('应零实现语句与零注释', () => {
    const source = sourceOf(ENTRY);
    expect(codeLines(source).filter((line) => IMPLEMENTATION_LINE.test(line))).toEqual([]);
    expect(
      source
        .split('\n')
        .map((line) => line.trim())
        .filter(isCommentLine),
    ).toEqual([]);
    expect(source.includes('export default')).toBe(false);
  });

  it('少导一名应判红：删 runStepNodes 后行集合与名单不等', () => {
    const tampered = sourceOf(ENTRY).replace(
      "export { runStepNodes, unifiedRunEdges, unifiedRunNodes } from './run-state';",
      "export { unifiedRunEdges, unifiedRunNodes } from './run-state';",
    );
    expect(codeLines(tampered)).not.toEqual(EXPECTED);
    expect(entryBlocksOf(tampered).flatMap((block) => block.names)).not.toContain('runStepNodes');
  });
});

describe('views/changes/renderers 入口：两值出口与 registry 不外露', () => {
  const ENTRY = 'src/views/changes/renderers/index.ts';
  const EXPECTED = [
    "export { ArtifactTabs } from './artifact-tabs';",
    "export { MarkdownDocRenderer } from './markdown-doc-renderer';",
  ];

  it('应恰 2 条语句且逐字等于期望行', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(EXPECTED);
  });

  it('两个出口应各来自对应模块（值形态、memo 形态不影响导出名）', () => {
    const blocks = entryBlocksOf(sourceOf(ENTRY));
    expect(blocks.map((block) => [block.spec, ...block.names])).toEqual([
      ['./artifact-tabs', 'ArtifactTabs'],
      ['./markdown-doc-renderer', 'MarkdownDocRenderer'],
    ]);
    expect(blocks.every((block) => !block.typeOnly)).toBe(true);
  });

  it('registry 与其余 6 个导出应逐个零命中', () => {
    const source = sourceOf(ENTRY);
    for (const name of [
      'RendererComponent',
      'resolveRenderer',
      'ArtifactView',
      'EvalChecklistRenderer',
      'Fallback',
      'TasksProgressRenderer',
    ]) {
      expect(source.includes(name), `renderers 入口零 ${name}`).toBe(false);
    }
  });

  it('把 registry 的 resolveRenderer 加进来应判红（行集合与零命中双判据）', () => {
    const tampered = `${sourceOf(ENTRY).trimEnd()}\nexport { resolveRenderer } from './registry';\n`;
    expect(codeLines(tampered)).not.toEqual(EXPECTED);
    expect(specifiersOf(tampered)).toContain('./registry');
    expect(tampered.includes('resolveRenderer')).toBe(true);
  });

  it('应恰 2 条非空行、零注释、零实现语句', () => {
    const source = sourceOf(ENTRY);
    expect(source.split('\n').filter((line) => line.trim() !== '')).toHaveLength(2);
    expect(
      source
        .split('\n')
        .map((line) => line.trim())
        .filter(isCommentLine),
    ).toEqual([]);
    expect(codeLines(source).filter((line) => IMPLEMENTATION_LINE.test(line))).toEqual([]);
  });
});

describe('views/explores 入口：三名与路由上下文不外露', () => {
  const ENTRY = 'src/views/explores/index.ts';
  const EXPECTED = [
    "export { ExploreDetailView } from './explore-detail-view';",
    "export { ExploreLayout } from './explore-layout';",
    "export { ExploreListView } from './explore-list-view';",
  ];

  it('应恰 3 条语句且按来源模块名字典序', () => {
    expect(codeLines(sourceOf(ENTRY))).toEqual(EXPECTED);
    const specs = specifiersOf(sourceOf(ENTRY));
    expect(specs).toEqual([...specs].sort());
  });

  it('三名出口应各在对应来源语句内', () => {
    const blocks = entryBlocksOf(sourceOf(ENTRY));
    expect(blocks.map((block) => [block.spec, ...block.names])).toEqual([
      ['./explore-detail-view', 'ExploreDetailView'],
      ['./explore-layout', 'ExploreLayout'],
      ['./explore-list-view', 'ExploreListView'],
    ]);
    expect(blocks.every((block) => !block.typeOnly)).toBe(true);
  });

  it('应零命中未出口四名（详情面板与路由上下文）', () => {
    const source = sourceOf(ENTRY);
    for (const name of [
      'ExploreDetailPane',
      'ExploreDetailPaneProps',
      'ExploreRouteContext',
      'useExploreRouteContext',
    ]) {
      expect(source.includes(name), `explores 入口零 ${name}`).toBe(false);
    }
  });

  it('顶层应 4 个非测试源文件且零 hooks/ specifier', () => {
    const topLevel = filesOf('src/views/explores').filter(
      (file) => /\.tsx?$/.test(file) && !/\.test\.tsx?$/.test(file) && file !== 'index.ts',
    );
    expect(topLevel).toHaveLength(4);
    expect(specifiersOf(sourceOf(ENTRY)).filter((spec) => spec.startsWith('./hooks/'))).toEqual([]);
  });

  it('specifier 形态应拒绝子目录与跨元素父级', () => {
    for (const spec of ['./components/explore-composer', '../../changes/renderers']) {
      expect(isDirectSpecifier(spec), `${spec} 非法`).toBe(false);
    }
    expect(specifiersOf(sourceOf(ENTRY)).filter((spec) => !isDirectSpecifier(spec))).toEqual([]);
  });
});

describe('components/ui 入口：41 条 export * 与零遮蔽前提', () => {
  const ENTRY = 'src/components/ui/index.ts';
  const DIR = 'src/components/ui';
  const STAR_EXPORT = /^export \* from '\.\/([a-z0-9][a-z0-9-]*)';$/;

  /** `export *` 行形态：模块名小写连字符、非自指 `./index`。 */
  function isStarExportLine(line: string): boolean {
    const match = STAR_EXPORT.exec(line);
    return match !== null && match[1] !== 'index';
  }
  const ZERO_REFERENCE_MODULES = ['button-group', 'context-menu', 'input-group', 'marker', 'sheet'];

  /** 41 个模块名（目录级非测试源文件，模块名 = 文件名去 `.tsx`）。 */
  function uiModules(): string[] {
    return filesOf(DIR)
      .filter((file) => file.endsWith('.tsx') && !file.endsWith('.test.tsx'))
      .map((file) => file.slice(0, -'.tsx'.length));
  }

  it('应恰 41 条 export * 且按模块名字典序', () => {
    const lines = codeLines(sourceOf(ENTRY));
    expect(lines).toHaveLength(41);
    expect(lines.filter((line) => !isStarExportLine(line))).toEqual([]);
    const modules = lines.map((line) => STAR_EXPORT.exec(line)![1]);
    expect(modules).toEqual([...modules].sort());
  });

  it('41 条 specifier 应与目录模块双向一一对应且零重复', () => {
    const specifiers = specifiersOf(sourceOf(ENTRY));
    expect(specifiers).toHaveLength(41);
    expect(new Set(specifiers).size).toBe(41);
    const specModules = specifiers.map((spec) => spec.slice('./'.length));
    expect([...specModules].sort()).toEqual([...uiModules()].sort());
    expect(filesOf(DIR).filter((file) => file.endsWith('.ts'))).toEqual(['index.ts']);
  });

  it('首行应是外部约束注释且其后零第二条注释', () => {
    const lines = sourceOf(ENTRY).split('\n');
    expect(lines[0].startsWith('//')).toBe(true);
    expect(lines[0]).toContain('入口');
    expect(lines[0]).toContain('勿据此判死代码');
    expect(
      lines
        .slice(1)
        .map((line) => line.trim())
        .filter(isCommentLine),
    ).toEqual([]);
  });

  it('应零 export { … } from 与零 export default（手维护白名单与默认导出均判红）', () => {
    const source = sourceOf(ENTRY);
    expect(source.includes('export {')).toBe(false);
    expect(entryBlocksOf(source)).toEqual([]);
    expect(source.includes('export default')).toBe(false);
  });

  it('应零实现语句（入口不承载实现、不做二次装载）', () => {
    expect(codeLines(sourceOf(ENTRY)).filter((line) => IMPLEMENTATION_LINE.test(line))).toEqual([]);
  });

  it('41 个模块应合计 247 名具名导出、零重名、零 export default', () => {
    const modules = uiModules();
    expect(modules).toHaveLength(41);
    const names = modules.flatMap((module) => declaredNamesOf(sourceOf(`${DIR}/${module}.tsx`)));
    expect(names).toHaveLength(247);
    expect(duplicatesOf(names)).toEqual([]);
    const withDefault = modules.filter((module) =>
      sourceOf(`${DIR}/${module}.tsx`).includes('export default'),
    );
    expect(withDefault).toEqual([]);
  });

  it('5 个已知零引用模块应在 41 条面内（本变更不清理）', () => {
    const source = sourceOf(ENTRY);
    for (const module of ZERO_REFERENCE_MODULES) {
      expect(source.includes(`export * from './${module}';`), `${module} 在面内`).toBe(true);
    }
  });

  it('specifier 形态应拒绝父目录 / 子目录 / 别名 / 扩展名 / 自指 / 大写 / 空值', () => {
    for (const spec of [
      '../lib',
      './hooks/use-mobile',
      './lib/cn',
      '@/components/ui/badge',
      './badge.tsx',
      './index',
      './Badge',
      '',
    ]) {
      expect(isStarExportLine(`export * from '${spec}';`), `${spec} 非法`).toBe(false);
    }
    expect(isStarExportLine("export * from './badge';")).toBe(true);
  });

  it('抽取器判红面自证：重名命中与跨行块两名', () => {
    const duplicated = [
      ...declaredNamesOf("export { A } from './x';"),
      ...declaredNamesOf("export { A } from './y';"),
    ];
    expect(duplicatesOf(duplicated)).toEqual(['A']);
    expect(declaredNamesOf('export {\n  A,\n  B,\n};')).toEqual(['A', 'B']);
  });

  it('行数下界：空源零条、单条一条', () => {
    expect(codeLines('')).toEqual([]);
    expect(codeLines("export * from './alert';")).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// routes.tsx / change-detail-view.tsx / detail-drawer.tsx / explore-preview.tsx
// 只改 import 形态（公共 API 与运行期行为零变化），故本文件对四者零新增用例：
// 收窄后的等价性由既有 app.test.tsx / change-detail-view.test.tsx /
// detail-drawer.test.tsx / explore-preview.test.tsx 承接；跨元素边集与
// `entry_state` 读数由全量架构检查承接（test-design 第 13–16 节与不可测试项 3 / 4）。
// ---------------------------------------------------------------------------
