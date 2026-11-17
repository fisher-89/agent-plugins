import { Handle, type Node, type NodeProps, Position } from '@xyflow/react';

import { Badge } from '@/components/ui/badge';

type ModuleNodeData = { label: string; mark: string };

export type ModuleFlowNode = Node<ModuleNodeData, 'archi'>;

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

export function ModuleNode({ data }: NodeProps<ModuleFlowNode>): React.JSX.Element {
  return (
    <Badge variant="secondary" className="p-3" data-testid="archi-canvas-node">
      <NodeHandles />
      {data.label}
    </Badge>
  );
}
