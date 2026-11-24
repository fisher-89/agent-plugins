import { Handle, type Node, type NodeProps, Position } from '@xyflow/react';

import { Badge, Card, CardHeader, CardTitle } from '@/components/ui';

type ModelNodeData = { label: string; mark: string };

type ModelFlowNode = Node<ModelNodeData, 'model'>;

/** 隐形把手（连线锚点：入边左 / 出边右）；xyflow 无把手即不求边位置，边不入 DOM。 */
function NodeHandles(): React.JSX.Element {
  const rounded = {
    width: '100%',
    height: '100%',
    top: 0,
    left: 0,
    opacity: 0,
    transform: 'none',
  };
  return (
    <>
      <Handle type="target" position={Position.Left} isConnectable={false} style={rounded} />
      <Handle type="source" position={Position.Right} isConnectable={false} style={rounded} />
    </>
  );
}

/** 分组节点（聚焦元素有子节点时）：标题栏 + 子节点渲染在其区域内的容器。 */
export function ElementGroupNode({
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

/** 叶子节点短名徽章：邻域成员按内容自适应；分组内容区里的子节点行带定尺，
 * 铺满整行（`w-full` / `h-full` 覆盖徽章的 `w-fit` / 定高）。 */
export function ElementNode({ data }: NodeProps<ModelFlowNode>): React.JSX.Element {
  return (
    <Badge variant="secondary" className="p-3" data-testid="archi-canvas-node">
      <NodeHandles />
      {data.label}
    </Badge>
  );
}
