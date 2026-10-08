import type { Node, NodeProps } from '@xyflow/react';

/**
 * phase 列容器自定义节点（nodeTypes 键 `column`）：
 * 列头 phase 名 + 该站过程文档徽章（徽章仅呈计数，文档明细进抽屉），点击
 * 经 data.onSelect 上抛 `{ scope: 'column', phase }` 打开右侧抽屉。
 */
import { Badge } from '@/components/ui/badge';

import type { ArtifactEnvelope } from '../../../types/dto';
import { COLUMN_HEADER_H } from './layout';
import type { DrawerSelection } from './types';

/** 列容器节点载荷（onSelect 可缺省：直渲染场景点击为 no-op，不抛错） */
type ColumnNodeData = {
  phase: string;
  docs: ArtifactEnvelope[];
  onSelect?: (selection: DrawerSelection) => void;
};

export type ColumnFlowNode = Node<ColumnNodeData, 'column'>;

/** 列容器节点：容器宽高由 ChangeFlowGraph 映射层给定，本体铺满父尺寸 */
export function PhaseColumnNode({ data }: NodeProps<ColumnFlowNode>): React.JSX.Element {
  const open = () => data.onSelect?.({ scope: 'column', phase: data.phase });
  return (
    <div
      className="h-full w-full cursor-pointer rounded-lg border border-border bg-muted/40 hover:border-primary"
      data-testid="flow-column"
      onClick={open}
    >
      <div
        className="flex items-center gap-1.5 overflow-hidden border-b border-border px-2.5"
        style={{ height: COLUMN_HEADER_H }}
      >
        <span className="shrink-0 text-[13px] font-semibold">{data.phase}</span>
        {data.docs.length > 0 && (
          <Badge variant="outline" className="shrink-0" data-testid="column-docs">
            {data.docs.length} 份文档
          </Badge>
        )}
      </div>
    </div>
  );
}
