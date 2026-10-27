/**
 * 右抽屉：① 元素概览（`kind` / `path` / `gate` / `modelFile:line`——位置可直接用于
 * `edit`）；② 文档关联列表（四字段条目，按来源强度序）；③ ADR 条目按需取正文并渲
 * 染 markdown（取数只接受关联集内路径）；④ `incremental` 模式下 `gate` 编辑入口。
 *
 * 本期边界（归 check 侧，零占位）：无「相邻报告」区块、无漂移统计、无门禁结论、
 * 无豁免 reason 编辑入口。`full` 模式只读——不渲染任何可提交的改档控件。
 */
import { useEffect, useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import { Button } from '@/components/ui/button';

import {
  commands,
  type ArchiElement,
  type ArchiModuleDetail,
  type ArchiRelation,
  type GateTier,
} from '../../../types/generated/bindings';
import { useArchiDoc, useArchiModule } from '../hooks/use-archi-module';

/** 模块级稳定引用：react-markdown 每次渲染都重建 processor，插件数组不必每拍新建 */
const REMARK_PLUGINS = [remarkGfm];

/** gate 三档（封闭值域；文案 = 线格式词）。 */
const GATE_TIERS: GateTier[] = ['off', 'warn', 'strict'];

/** 线值 → 档位收窄（类型守卫；词汇外即不认——不靠断言，UI 不作非法取值）。 */
function asGateTier(value: string): GateTier | null {
  return GATE_TIERS.find((tier) => tier === value) ?? null;
}

export type CanvasMode = 'full' | 'incremental';

export interface ElementDrawerProps {
  /** 观察根（当前工作根；`null` 时取数不发起） */
  root: string | null;
  /** 能力差异：`full` 只读 / `incremental` 可改档 */
  mode: CanvasMode;
  /** 当前聚焦元素 */
  element: ArchiElement | null;
  /** 图关系（`exempt` 边的 reason 行数据源） */
  relations: ArchiRelation[];
  /** 改档成功后的父级重取（图与抽屉同步刷新） */
  onChanged: () => void;
}

/** 概览行（`modelFile:line` 行号未知时只渲文件）。 */
function Overview({ element }: { element: ArchiElement }): React.JSX.Element {
  const position =
    element.modelLine === 0 ? element.modelFile : `${element.modelFile}:${element.modelLine}`;
  return (
    <div className="flex flex-col gap-0.5 text-[13px]">
      <div className="break-all font-medium" data-testid="archi-drawer-fqn">
        {element.fqn}
      </div>
      <div className="text-muted-foreground">
        kind {element.kind} · gate {element.gate}
      </div>
      <div className="break-all text-muted-foreground">
        {element.path.length === 0 ? 'path （未声明）' : `path ${element.path.join('、')}`}
      </div>
      <div
        className="break-all font-mono text-xs text-muted-foreground"
        data-testid="archi-drawer-position"
      >
        {position}
      </div>
      {element.summary !== null && (
        <div className="break-words text-muted-foreground">{element.summary}</div>
      )}
    </div>
  );
}

/** 文档条目：展开 ADR 才取正文（越界 / 读盘失败行内呈现）。 */
function DocRow({
  root,
  path,
  title,
  source,
  kind,
  selected,
  onToggle,
}: {
  root: string | null;
  path: string;
  title: string;
  source: string;
  kind: string;
  selected: boolean;
  onToggle: () => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-col gap-0.5" data-testid="archi-doc-item" data-source={source}>
      <button
        className="break-all text-left text-[13px]"
        onClick={onToggle}
        disabled={kind !== 'adr'}
        title={path}
      >
        {title}
        <span className="ml-1 text-xs text-muted-foreground">
          {source} · {kind}
        </span>
      </button>
      <div className="break-all font-mono text-[11px] text-muted-foreground">{path}</div>
      {selected && <AdrBody root={root} path={path} />}
    </div>
  );
}

/** ADR 正文（`archi_read_doc` 按需取；`data-testid="archi-adr-body"`）。 */
function AdrBody({ root, path }: { root: string | null; path: string }): React.JSX.Element {
  const doc = useArchiDoc(root);
  const { load, reset } = doc;
  // 展开即取（挂载 / 换条目两触发）；收起（父级卸载）与换条目（依赖变化）皆清态——
  // 上一份正文不跨条目残留
  useEffect(() => {
    load(path);
    return () => {
      reset();
    };
  }, [load, path, reset]);
  if (doc.loading) return <div className="text-xs text-muted-foreground">正文加载中…</div>;
  if (doc.error !== null) {
    return (
      <div
        className="break-all rounded-md bg-fail-bg px-2 py-1 text-xs text-fail"
        data-testid="archi-doc-error"
      >
        {doc.error}
      </div>
    );
  }
  if (doc.body === null) return <div className="text-xs text-muted-foreground">（空正文）</div>;
  return (
    <div className="prose prose-sm max-w-none text-[13px]" data-testid="archi-adr-body">
      <Markdown remarkPlugins={REMARK_PLUGINS}>{doc.body}</Markdown>
    </div>
  );
}

/** 改档入口（`incremental` 才有；写命令 + 成功重取 + 失败行内错误）。 */
function GateEditor({
  root,
  element,
  onChanged,
}: {
  root: string | null;
  element: ArchiElement;
  onChanged: () => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState<GateTier>(element.gate);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // 权威档位一变即重取草稿：草稿是「待提交的本地意图」，MUST NOT 跨元素存活——
  // 否则切换聚焦元素后的「写回」会把上一个元素的档位写到新元素（父级重取回来的
  // 新档位同理）
  useEffect(() => {
    setDraft(element.gate);
    setError(null);
  }, [element.gate]);
  const submit = (): void => {
    if (root === null) {
      setError('无工作根，无法改档');
      return;
    }
    setSaving(true);
    setError(null);
    commands
      .archiSetGate(root, element.fqn, draft)
      .then(() => {
        setSaving(false);
        onChanged();
      })
      .catch((reason: unknown) => {
        setSaving(false);
        setError(String(reason));
      });
  };
  return (
    <div className="mt-2 flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <label className="text-xs text-muted-foreground" htmlFor="archi-gate-select">
          改档
        </label>
        <select
          id="archi-gate-select"
          className="h-8 rounded-md border border-border bg-background px-2 text-sm"
          data-testid="archi-gate-select"
          value={draft}
          onChange={(event) => {
            const next = asGateTier(event.target.value);
            if (next !== null) setDraft(next);
          }}
        >
          {GATE_TIERS.map((tier) => (
            <option key={tier} value={tier}>
              {tier}
            </option>
          ))}
        </select>
        <Button disabled={saving} onClick={submit} data-testid="archi-gate-submit">
          写回
        </Button>
      </div>
      {error !== null && (
        <div
          className="break-all rounded-md bg-fail-bg px-2 py-1 text-xs text-fail"
          data-testid="archi-gate-error"
        >
          {error}
        </div>
      )}
    </div>
  );
}

/** 文档关联段（四字段条目按强度序；ADR 条目展开才取正文）。 */
function DocsSection({
  root,
  docs,
}: {
  root: string | null;
  docs: ArchiModuleDetail['docs'];
}): React.JSX.Element {
  const [openDoc, setOpenDoc] = useState<string | null>(null);
  return (
    <div className="flex flex-col gap-1">
      <h3 className="m-0 text-[13px] font-medium text-muted-foreground">文档</h3>
      {docs.length === 0 ? (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-docs-empty">
          无关联文档
        </div>
      ) : (
        docs.map((doc) => (
          <DocRow
            key={doc.path}
            path={doc.path}
            title={doc.title}
            source={doc.source}
            kind={doc.kind}
            root={root}
            selected={openDoc === doc.path}
            onToggle={() => setOpenDoc(openDoc === doc.path ? null : doc.path)}
          />
        ))
      )}
    </div>
  );
}

/** 豁免段（`exempt` 边的 reason 行；无豁免即整段隐藏）。 */
function ExemptSection({
  element,
  relations,
}: {
  element: ArchiElement;
  relations: ArchiRelation[];
}): React.JSX.Element | null {
  const exempt = relations.filter(
    (relation) =>
      relation.reason !== null && (relation.from === element.fqn || relation.to === element.fqn),
  );
  if (exempt.length === 0) return null;
  return (
    <div className="flex flex-col gap-0.5">
      <h3 className="m-0 text-[13px] font-medium text-muted-foreground">豁免</h3>
      {exempt.map((relation) => (
        <div
          className="break-all text-xs text-muted-foreground"
          data-testid="archi-exempt-reason"
          key={`${relation.from}->${relation.to}`}
        >
          {relation.from} → {relation.to}：{relation.reason}
        </div>
      ))}
    </div>
  );
}

/**
 * 右抽屉：概览 → 文档关联 → 豁免 →（`incremental`）改档入口 /（`full`）只读提示。
 * 未选中元素时呈空态（不发 `archi_module` 取数）。
 */
export function ElementDrawer({
  root,
  mode,
  element,
  relations,
  onChanged,
}: ElementDrawerProps): React.JSX.Element {
  const module = useArchiModule(root, element === null ? null : element.fqn);
  if (element === null) {
    return (
      <aside
        className="min-h-0 w-72 shrink-0 overflow-auto border-l border-border pl-2"
        data-testid="archi-drawer"
      >
        <div className="text-[13px] text-muted-foreground">未选中元素</div>
      </aside>
    );
  }
  return (
    <aside
      className="flex min-h-0 w-72 shrink-0 flex-col gap-2 overflow-auto border-l border-border pl-2"
      data-testid="archi-drawer"
    >
      <Overview element={element} />
      {module.error !== null && (
        <div
          className="break-all rounded-md bg-fail-bg px-2 py-1 text-xs text-fail"
          data-testid="archi-drawer-error"
        >
          {module.error}
        </div>
      )}
      <DocsSection root={root} docs={module.detail?.docs ?? []} />
      <ExemptSection element={element} relations={relations} />
      {mode === 'incremental' ? (
        <GateEditor key={element.fqn} root={root} element={element} onChanged={onChanged} />
      ) : (
        <div className="mt-2 text-xs text-muted-foreground" data-testid="archi-gate-readonly">
          改档需要开一个 change（本页为只读视图）
        </div>
      )}
    </aside>
  );
}
