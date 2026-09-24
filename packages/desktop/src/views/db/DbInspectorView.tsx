import { useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';

import type { ModelInfo, RecordEnvelope } from '../../types/dto';
import { useDbInspector, type DbInspectorState } from './hooks/useDbInspector';

/** 记录行唯一键：key JSON 文本 + 行号兜底（同 key 理论上不重） */
function rowKey(envelope: RecordEnvelope, index: number): string {
  const key = JSON.stringify(envelope.key);
  return key === undefined ? String(index) : key;
}

/** value 摘要：单行 JSON 截断 */
function valueSummary(value: unknown, max = 120): string {
  const text = JSON.stringify(value) ?? '';
  return text.length > max ? `${text.slice(0, max)}…` : text;
}

/** 数据模型行：名称 + 计数，点击选中（计数 0 仍可选，呈空态） */
function ModelItem({
  count,
  name,
  selected,
  onSelect,
}: {
  count: number;
  name: string;
  selected: boolean;
  onSelect: (name: string) => void;
}): React.JSX.Element {
  return (
    <li>
      <button
        type="button"
        className={`block w-full cursor-pointer border-0 bg-transparent px-0 py-1.5 text-left hover:text-primary ${
          selected ? 'font-medium text-primary' : ''
        }`}
        data-model={name}
        data-testid="db-model-item"
        onClick={() => onSelect(name)}
      >
        <span className="flex items-center justify-between gap-2">
          <span>{name}</span>
          <span className="text-xs text-muted-foreground">{count}</span>
        </span>
      </button>
    </li>
  );
}

/** 数据模型区：模型列表（挂载取数一次，刷新按钮；失败态由页级错误区承载） */
function ModelListSection({ state }: { state: DbInspectorState }): React.JSX.Element {
  return (
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <div className="mb-2 flex items-center justify-between">
        <h2 className="m-0 text-[15px]">数据模型</h2>
        <Button disabled={state.loading} data-testid="db-refresh" onClick={state.refresh}>
          刷新数据
        </Button>
      </div>
      <ul className="m-0 list-none p-0" data-testid="db-model-list">
        {state.models.map((model) => (
          <ModelItem
            count={model.count}
            key={model.name}
            name={model.name}
            selected={model.name === state.selected}
            onSelect={state.selectModel}
          />
        ))}
      </ul>
      {state.loading && (
        <div className="text-muted-foreground" data-testid="db-models-loading">
          加载中…
        </div>
      )}
    </section>
  );
}

/** 单条记录行：key 预览 + value 摘要，点击打开 JSON 详情 */
function RecordRow({
  envelope,
  index,
  open,
  onOpen,
}: {
  envelope: RecordEnvelope;
  index: number;
  open: boolean;
  onOpen: (index: number) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className={`block w-full cursor-pointer border-0 border-b border-b-border bg-transparent px-0 py-2 text-left last:border-b-0 hover:text-primary ${
        open ? 'text-primary' : ''
      }`}
      data-index={index}
      data-testid="db-record-item"
      key={rowKey(envelope, index)}
      onClick={() => onOpen(index)}
    >
      <span className="mb-0.5 flex items-center gap-2 text-xs text-muted-foreground">
        <span className="truncate">key: {JSON.stringify(envelope.key)}</span>
      </span>
      <span className="block truncate text-sm">{valueSummary(envelope.value)}</span>
    </button>
  );
}

/** 单条记录详情：key 与 value 信封完整呈现（pretty JSON） */
function RecordDetail({ record }: { record: RecordEnvelope }): React.JSX.Element {
  return (
    <div className="mt-3" data-testid="db-record-detail">
      <div className="mb-1 text-xs text-muted-foreground">记录详情（信封 JSON）</div>
      <pre
        className="m-0 overflow-x-auto rounded-md bg-muted p-3 text-xs"
        data-testid="db-record-json"
      >
        {JSON.stringify(record, null, 2)}
      </pre>
    </div>
  );
}

/** 分页控件行：模型名标题 + 上一页 / offset / 下一页 */
function RecordsHeader({
  state,
  modelName,
}: {
  state: DbInspectorState;
  modelName: string;
}): React.JSX.Element {
  return (
    <div className="mb-2 flex items-center justify-between gap-2">
      <h2 className="m-0 text-[15px]">{modelName}</h2>
      <span className="flex items-center gap-2">
        <Button
          aria-label="上一页"
          data-testid="db-page-prev"
          disabled={state.recordsLoading || state.offset === 0}
          onClick={state.prevPage}
        >
          上一页
        </Button>
        <span className="text-xs text-muted-foreground">offset {state.offset}</span>
        <Button
          aria-label="下一页"
          data-testid="db-page-next"
          disabled={state.recordsLoading || !state.hasMore}
          onClick={state.nextPage}
        >
          下一页
        </Button>
      </span>
    </div>
  );
}

/** 记录分页区：翻页控件 + inline 持久错误 + 空态 + 记录行 + 单条 JSON 详情 */
function RecordsSection({
  state,
  selectedModel,
}: {
  state: DbInspectorState;
  selectedModel: ModelInfo;
}): React.JSX.Element {
  const [openIndex, setOpenIndex] = useState<number | null>(null);
  // 换模型 / 换页即收起单条详情（记录列表已换血）
  useEffect(() => {
    setOpenIndex(null);
  }, [state.selected, state.offset]);
  const openRecord = openIndex !== null ? (state.records[openIndex] ?? null) : null;

  return (
    <section className="rounded-lg border border-border bg-card px-4 py-3.5">
      <RecordsHeader modelName={selectedModel.name} state={state} />
      {state.recordsError !== null && (
        <div className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail">
          记录扫描失败：{state.recordsError}
        </div>
      )}
      {state.recordsLoading && (
        <div className="text-muted-foreground" data-testid="db-records-loading">
          加载中…
        </div>
      )}
      {!state.recordsLoading && state.records.length === 0 && state.recordsError === null && (
        <div className="text-muted-foreground" data-testid="db-records-empty">
          该模型暂无记录。
        </div>
      )}
      {state.records.map((envelope, index) => (
        <RecordRow
          envelope={envelope}
          index={index}
          key={rowKey(envelope, index)}
          onOpen={setOpenIndex}
          open={openIndex === index}
        />
      ))}
      {openRecord !== null && <RecordDetail record={openRecord} />}
    </section>
  );
}

/**
 * 数据库页（只读四件套）：数据模型 + 计数、选中模型分页扫描、单条记录
 * JSON 查看、空态 / inline 持久错误态。取数全部经 useDbInspector 的用户显式
 * 动作触发（进页 / 选中模型 / 翻页 / 刷新），无轮询；无任何写操作入口。
 */
export function DbInspectorView(): React.JSX.Element {
  const state = useDbInspector();
  const selectedModel = state.models.find((model) => model.name === state.selected) ?? null;

  return (
    <div>
      {state.error !== null && (
        <div
          className="mb-4 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="db-inspector-error"
        >
          模型加载失败：{state.error}
        </div>
      )}
      <ModelListSection state={state} />
      {selectedModel !== null && <RecordsSection selectedModel={selectedModel} state={state} />}
    </div>
  );
}
