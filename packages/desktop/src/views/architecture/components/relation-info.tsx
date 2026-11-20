import { FileViewer } from '@/components/file/file-viewer';
import { Badge, Button, FieldDescription, FieldLegend, FieldSet } from '@/components/ui';
import type { ArchiRelation } from '@/types/generated/bindings';

import type { ArchiRelationState, RelationEdge } from '../hooks/use-archi-relation';

export interface RelationInfoProps {
  /** 观察根（「查看」经通用 `read_file` 打开模型文件） */
  root: string | null;
  /** 选中边的元素对身份（面板标题与命中面的锚） */
  edge: RelationEdge;
  /** 图里的关系投影（声明清单 / 标记 / 豁免理由零取数） */
  relation: ArchiRelation;
  /** 按需取数状态（实际使用 / 两桶 / 命中点 / 模型行号） */
  state: ArchiRelationState;
  /** 豁免来源（父级从漂移报告的 `suppressed[]` 查得；非豁免边为 null） */
  exemptVia: 'config' | 'inline' | null;
  /** 重试入口（取数失败时的唯一再取通道） */
  onRetry: () => void;
}

/** 段标题（与元素态面板的契约段同款：13px 中等字重、静音色）。 */
function SectionTitle({ children }: { children: React.ReactNode }): React.JSX.Element {
  return <h3 className="m-0 text-[13px] font-medium text-muted-foreground">{children}</h3>;
}

/** 名清单段（声明 API / 实际使用共用；空清单显「—」）。 */
function NameSection({
  label,
  names,
  testId,
}: {
  label: string;
  names: string[];
  testId: string;
}): React.JSX.Element {
  return (
    <div className="flex flex-col gap-0.5">
      <SectionTitle>
        {label} ({names.length})
      </SectionTitle>
      <div className="wrap-break-word font-mono text-xs" data-testid={testId}>
        {names.length === 0 ? '—' : names.join(' · ')}
      </div>
    </div>
  );
}

/** 两桶段：`usedNotDeclared` / `declaredNotUsed` 两行（空行显「—」）。 */
function BucketSection({ state }: { state: ArchiRelationState }): React.JSX.Element | null {
  if (state.detail === null) return null;
  return (
    <div className="flex flex-col gap-0.5">
      <SectionTitle>两桶</SectionTitle>
      <div
        className="wrap-break-word font-mono text-xs"
        data-testid="archi-relation-used-not-declared"
      >
        用了没声明：
        {state.detail.usedNotDeclared.length === 0 ? '—' : state.detail.usedNotDeclared.join(' · ')}
      </div>
      <div
        className="wrap-break-word font-mono text-xs"
        data-testid="archi-relation-declared-not-used"
      >
        声明没用：
        {state.detail.declaredNotUsed.length === 0 ? '—' : state.detail.declaredNotUsed.join(' · ')}
      </div>
      {state.detail.declaredNotUsed.length === 0 ? null : (
        <FieldDescription data-testid="archi-relation-declared-not-used-note">
          可能已不再依赖，也可能名字写错或该依赖以方法调用 / 内联限定路径 / glob
          形态存在而扫描看不见
        </FieldDescription>
      )}
    </div>
  );
}

/** ⑥ 命中点段（空即空态②：扫描面无该关系的 import）。 */
function OccurrenceSection({ state }: { state: ArchiRelationState }): React.JSX.Element {
  const occurrences = state.detail?.occurrences ?? [];
  return (
    <div className="flex flex-col gap-0.5">
      <SectionTitle>命中点 ({occurrences.length})</SectionTitle>
      {occurrences.length === 0 ? (
        <FieldDescription data-testid="archi-relation-no-import">
          扫描面无该关系的 import
        </FieldDescription>
      ) : (
        occurrences.map((occurrence, index) => (
          <div
            className="wrap-break-word font-mono text-[11px] text-muted-foreground"
            data-testid="archi-relation-occurrence"
            key={`${occurrence.file}:${occurrence.line}:${index}`}
          >
            {occurrence.file}:{occurrence.line}
            {occurrence.specifier === null ? '' : `  ${occurrence.specifier}`}
          </div>
        ))
      )}
    </div>
  );
}

/** ⑦ 模型段：`modelFile:modelLine` + 「查看」（经通用 `read_file`，零专用命令）。 */
function ModelSection({
  root,
  state,
}: {
  root: string | null;
  state: ArchiRelationState;
}): React.JSX.Element | null {
  if (state.detail === null) return null;
  const path = state.detail.modelFile;
  const location = `${path}:${state.detail.modelLine}`;
  return (
    <div className="flex flex-col gap-0.5">
      <SectionTitle>模型</SectionTitle>
      <div className="flex items-center gap-1" data-testid="archi-relation-model" title={location}>
        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted-foreground">
          {location}
        </span>
        {root === null ? (
          // 无根时行仍在场、「查看」禁用（行数是契约面，不静默少渲一行）
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
    </div>
  );
}

/** 取数面：四空态优先级（失败 > 豁免 > 未声明 > 无 import）+ 四个事实段。 */
function RelationFacts({
  root,
  relation,
  state,
  exemptVia,
  onRetry,
}: Omit<RelationInfoProps, 'edge'>): React.JSX.Element {
  if (state.error !== null) {
    return (
      <div className="flex flex-col gap-1">
        <div
          className="break-all rounded-md bg-fail-bg px-2 py-1 text-xs text-fail"
          data-testid="archi-relation-error"
        >
          {state.error}
        </div>
        <Button variant="ghost" size="sm" onClick={onRetry} data-testid="archi-relation-retry">
          重试
        </Button>
      </div>
    );
  }
  if (relation.mark === 'exempt') {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-relation-exempt">
        已豁免（{exemptVia ?? '未知来源'}）：{relation.reason ?? '无理由'}
      </div>
    );
  }
  if (relation.apis.length === 0) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-relation-undeclared">
        未声明依赖 API（选填）
      </div>
    );
  }
  if (state.loading && state.detail === null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-relation-loading">
        取数中…
      </div>
    );
  }
  return (
    <>
      <NameSection label="实际使用" names={state.detail?.used ?? []} testId="archi-relation-used" />
      <BucketSection state={state} />
      <OccurrenceSection state={state} />
      <ModelSection root={root} state={state} />
    </>
  );
}

/**
 * 关系信息面板（右栏 `CardHeader` 的关系态）：七段固定序 + 四空态 + 行内错误与重试。
 *
 * 两段取数分工：声明清单取图里的 `relation.apis`（零 invoke）；实际使用 / 两桶 /
 * 命中点 / 模型行号取 `archi_relation`（由父级 hook 发起）。**不自行求差**、不读漂移
 * 报告重算——两桶是命令返回值。
 */
export function RelationInfo({
  root,
  edge,
  relation,
  state,
  exemptVia,
  onRetry,
}: RelationInfoProps): React.JSX.Element {
  return (
    <div
      className="flex min-h-0 shrink-0 flex-col gap-2 overflow-auto"
      data-testid="archi-relation-info"
      data-from={edge.from}
      data-to={edge.to}
    >
      {/* ① 关系 + ② 说明（`description` 为 null 时不占文案） */}
      <FieldSet>
        <FieldLegend className="flex flex-wrap items-center gap-1 break-all text-[13px]">
          <span data-testid="archi-relation-fqn">
            {relation.from} → {relation.to}
          </span>
          <Badge variant="secondary">{relation.mark}</Badge>
        </FieldLegend>
        {relation.description === null ? null : (
          <FieldDescription className="wrap-break-word">{relation.description}</FieldDescription>
        )}
      </FieldSet>
      {/* ③ 声明 API（图里的模型拷贝，零取数） */}
      <NameSection label="声明 API" names={relation.apis} testId="archi-relation-declared" />
      {/* ④–⑦ 事实面 + 四空态 */}
      <RelationFacts
        root={root}
        relation={relation}
        state={state}
        exemptVia={exemptVia}
        onRetry={onRetry}
      />
    </div>
  );
}
