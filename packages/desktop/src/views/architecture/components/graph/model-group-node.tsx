import type { Node, NodeProps } from '@xyflow/react';

import { Card, CardHeader, CardTitle } from '@/components/ui';

import { type ModelNodeData, NodeHandles } from './model-node';

/** 分组节点（聚焦元素有子节点时）：标题栏 + 子节点渲染在其区域内的容器。 */
export function ModelGroupNode({
  data,
}: NodeProps<Node<ModelNodeData, 'modelGroup'>>): React.JSX.Element {
  return (
    <Card className="h-full w-full" data-testid="archi-canvas-node">
      <NodeHandles />
      <CardHeader>
        <CardTitle>{data.label}</CardTitle>
      </CardHeader>
    </Card>
  );
}
