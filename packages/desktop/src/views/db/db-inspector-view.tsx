import { useEffect, useState } from 'react';

import { StandardDrawer } from '@/components/standard/drawer';
import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

import type { ModelInfo, RecordEnvelope } from '../../types/dto';
import { useDbInspector, type DbInspectorState } from './hooks/use-db-inspector';

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

/** 顶层对象判定（非 null / 非数组 / 非标量——列推导与单元格取值同判） */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** 表格列推导（数据驱动纯函数）：`key` 恒首列；对象 `value` 行逐行逐键首见
 * 追加并集列（缺失键空单元格）；非对象 `value` 行经 `value` 尾列承载摘要
 * ——全非对象页恰 `key` + `value` 两列，混合页并集列 + `value` 尾列并存。
 * 零模型特定分支，新注册模型零改动自动出表。 */
function deriveColumns(records: RecordEnvelope[]): string[] {
  const columns = ['key'];
  let hasNonObject = false;
  for (const record of records) {
    if (isPlainObject(record.value)) {
      for (const field of Object.keys(record.value)) {
        if (!columns.includes(field)) columns.push(field);
      }
    } else {
      hasNonObject = true;
    }
  }
  if (hasNonObject && !columns.includes('value')) columns.push('value');
  return columns;
}

/** 单元格摘要：key 列呈主键摘要；对象行按列名取字段摘要（缺失键空单元格）；
 * 非对象行经 `value` 尾列呈整值摘要（120 截断口径同 valueSummary） */
function cellText(record: RecordEnvelope, column: string): string {
  if (column === 'key') return valueSummary(record.key);
  if (isPlainObject(record.value)) {
    return column in record.value ? valueSummary(record.value[column]) : '';
  }
  return column === 'value' ? valueSummary(record.value) : '';
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

/** scope 双 tab（全局库 / workspace 库，aria-pressed 模式同 agent 页切换行）：
 * 两库清单互不混列，切换即重置选中与分页并重取清单 */
function ScopeToggle({ state }: { state: DbInspectorState }): React.JSX.Element {
  return (
    <span className="flex items-center gap-2">
      <span className="text-xs text-muted-foreground">数据库</span>
      <Button
        aria-pressed={state.scope === 'workspace'}
        className={state.scope === 'workspace' ? undefined : 'bg-muted text-muted-foreground'}
        data-testid="db-scope-workspace"
        onClick={() => state.setScope('workspace')}
      >
        workspace 库
      </Button>
      <Button
        aria-pressed={state.scope === 'user'}
        className={state.scope === 'user' ? undefined : 'bg-muted text-muted-foreground'}
        data-testid="db-scope-user"
        onClick={() => state.setScope('user')}
      >
        全局库
      </Button>
    </span>
  );
}

/** 数据模型区：scope 双 tab + 模型列表（scope / root 变更即重取，刷新按钮；
 * 失败态由页级错误区承载） */
function ModelListSection({ state }: { state: DbInspectorState }): React.JSX.Element {
  return (
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <div className="mb-2 flex items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">数据模型</h2>
        <span className="flex items-center gap-2">
          <ScopeToggle state={state} />
          <Button disabled={state.loading} data-testid="db-refresh" onClick={state.refresh}>
            刷新数据
          </Button>
        </span>
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

/** 分页控件行：模型名标题 + 上一页 / 真实页界（第 page / totalPages 页，共
 * total 条）/ 下一页（页界全部由接口 total 导出） */
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
        <span className="text-xs text-muted-foreground">
          第 {state.page} / {state.totalPages} 页（共 {state.total} 条）
        </span>
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

/** 记录分页区：翻页控件 + inline 持久错误 + 空态 + 自动表格 + 抽屉单条详情 */
function RecordsSection({
  state,
  selectedModel,
}: {
  state: DbInspectorState;
  selectedModel: ModelInfo;
}): React.JSX.Element {
  // 行选中与抽屉开合单状态：open ⇔ selectedIndex !== null（点行选中并开抽屉）
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  // 列表换血（换模型 / 翻页 / 切 scope / 刷新）即收起抽屉，详情不残留
  useEffect(() => {
    setSelectedIndex(null);
  }, [state.selected, state.offset, state.records]);
  const selectedRecord = selectedIndex !== null ? (state.records[selectedIndex] ?? null) : null;
  const columns = deriveColumns(state.records);

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
      {state.records.length > 0 && (
        <Table data-testid="db-records-table">
          <TableHeader>
            <TableRow>
              {columns.map((column) => (
                <TableHead key={column}>{column}</TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {state.records.map((envelope, index) => (
              <TableRow
                className="cursor-pointer"
                data-state={selectedIndex === index ? 'selected' : undefined}
                data-testid="db-record-row"
                key={rowKey(envelope, index)}
                onClick={() => setSelectedIndex(index)}
              >
                {columns.map((column) => (
                  <TableCell key={column}>{cellText(envelope, column)}</TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      <StandardDrawer
        title={selectedRecord !== null ? JSON.stringify(selectedRecord.key) : ''}
        content={
          <pre
            className="m-0 overflow-x-auto rounded-md bg-muted p-3 text-xs"
            data-testid="db-record-json"
          >
            {selectedRecord !== null ? JSON.stringify(selectedRecord, null, 2) : ''}
          </pre>
        }
        containerClassName="w-[640px] max-w-[85vw]"
        open={selectedIndex !== null}
        onOpenChange={(open) => {
          if (!open) setSelectedIndex(null);
        }}
      />
    </section>
  );
}

export interface DbInspectorViewProps {
  /** 当前 workspace root（workspace 库 scope 的寻址基准；全局库 scope 忽略） */
  root: string;
}

/**
 * 数据库页（只读四件套）：scope 双 tab（全局库 / 当前 workspace 库，默认
 * workspace 库）+ 数据模型 + 计数、选中模型自动表格分页扫描、单条记录抽屉
 * 查看、空态 / inline 持久错误态。取数全部经 useDbInspector 的用户显式动作触发
 * （进页 / 切 scope / 选中模型 / 翻页 / 刷新），无轮询；无任何写操作入口。
 */
export function DbInspectorView({ root }: DbInspectorViewProps): React.JSX.Element {
  const state = useDbInspector(root);
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
