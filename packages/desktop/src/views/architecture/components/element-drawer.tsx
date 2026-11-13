import { useEffect, useState } from 'react';

import { FileViewer } from '@/components/file/file-viewer';
import { Button } from '@/components/ui/button';

import {
  commands,
  type ArchiApiFinding,
  type ArchiApiRef,
  type ArchiElement,
  type ArchiModuleDetail,
  type ArchiRelation,
  type GateTier,
} from '../../../types/generated/bindings';
import { useArchiModule } from '../hooks/use-archi-module';

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
  /** 契约判据（父级同一份漂移报告，零新 invoke） */
  findings: ArchiApiFinding[];
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
        <div className="wrap-break-word text-muted-foreground">{element.summary}</div>
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
}: {
  root: string;
  path: string;
  title: string;
  source: string;
  kind: string;
}): React.JSX.Element {
  return (
    <div className="flex flex-col gap-0.5" data-testid="archi-doc-item" data-source={source}>
      <FileViewer
        root={root}
        files={[{ path }]}
        trigger={
          <Button
            variant="ghost"
            size="sm"
            className="justify-start"
            disabled={kind !== 'adr'}
            title={path}
          >
            {title}
            <span className="ml-1 text-xs text-muted-foreground">
              {source} · {kind}
            </span>
          </Button>
        }
      />
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

/**
 * 契约段的一条路径行（`status = declared` 才渲；「查看」经通用 `read_file`）。
 * 路径为 `null` 时行仍渲、「查看」禁用：行数是「declared 三行」的契约面，缺席由
 * 禁用态呈现，MUST NOT 静默少渲一行。
 */
function ContractPathRow({
  root,
  label,
  path,
}: {
  root: string;
  label: string;
  path: string | null;
}): React.JSX.Element {
  return (
    <div
      className="flex items-center gap-1"
      data-testid="archi-contract-path"
      title={path ?? undefined}
    >
      <span className="shrink-0 text-xs text-muted-foreground">{label}</span>
      <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted-foreground">
        {path ?? '（该路径未生成）'}
      </span>
      {path === null ? (
        // 路径缺席不吞行：行仍在场、「查看」禁用——`declared` 却缺路径的缺口要一眼可见
        <Button variant="ghost" size="sm" disabled>
          查看
        </Button>
      ) : (
        <FileViewer
          root={root}
          files={[{ path }]}
          trigger={
            <Button variant="ghost" size="sm">
              查看
            </Button>
          }
        />
      )}
    </div>
  );
}

/** 契约段（概览与文档之间）：`api.status` + 三路径行 + 本元素判据。 */
function ContractSection({
  root,
  element,
  api,
  findings,
}: {
  root: string | null;
  element: ArchiElement;
  api: ArchiApiRef;
  findings: ArchiApiFinding[];
}): React.JSX.Element {
  const own = findings.filter((finding) => finding.element === element.fqn);
  return (
    <div className="flex flex-col gap-1">
      <h3 className="m-0 text-[13px] font-medium text-muted-foreground">契约</h3>
      {api.status === 'none' || root === null ? (
        <div
          className="text-[13px] text-muted-foreground"
          data-testid="archi-contract-drawer-empty"
        >
          无契约（`api = none` 或契约文件不在案）
        </div>
      ) : (
        <>
          <div className="text-xs text-muted-foreground">api {api.status}</div>
          <ContractPathRow label="契约" path={api.contractPath} root={root} />
          <ContractPathRow label="schema" path={api.schemaPath} root={root} />
          <ContractPathRow label="生成物" path={api.generatedPath} root={root} />
          {own.length === 0 ? (
            <div
              className="text-[13px] text-muted-foreground"
              data-testid="archi-contract-drawer-empty"
            >
              本元素无漂移
            </div>
          ) : (
            own.map((finding, index) => (
              <div
                className="flex flex-col gap-0.5"
                data-kind={finding.kind}
                key={`${finding.kind}:${finding.target ?? ''}:${index}`}
              >
                <span className="text-[11px] text-muted-foreground">
                  {finding.kind}
                  {finding.target === null ? '' : ` · ${finding.target}`}
                </span>
                <span className="wrap-break-word text-xs">{finding.message}</span>
              </div>
            ))
          )}
        </>
      )}
    </div>
  );
}

/** 文档关联段（四字段条目按强度序；ADR 条目展开才取正文）。 */
function DocsSection({
  root,
  docs,
}: {
  root: string;
  docs: ArchiModuleDetail['docs'];
}): React.JSX.Element {
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
 * 右抽屉：概览 → 契约 → 文档关联 → 豁免 →（`incremental`）改档入口 /（`full`）只读
 * 提示。未选中元素时呈空态（不发 `archi_module` 取数）；契约段数据全部来自既有
 * `ArchiModuleDetail.api` 与父级同一份判据（零新 invoke）。
 */
export function ElementDrawer({
  root,
  mode,
  element,
  relations,
  findings,
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
      {root !== null && module.detail !== null && (
        <ContractSection
          root={root}
          element={element}
          api={module.detail.api}
          findings={findings}
        />
      )}
      {root && <DocsSection root={root} docs={module.detail?.docs ?? []} />}
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
