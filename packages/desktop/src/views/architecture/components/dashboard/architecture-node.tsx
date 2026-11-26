import '@xyflow/react/dist/style.css';
import { FileViewerBlock, type FileViewerProps } from '@/components/file/file-viewer';
import {
  Card,
  CardContent,
  CardHeader,
  ResizablePanel,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import {
  type ArchiApiFinding,
  type ArchiElement,
  type ArchiGraph,
  type ArchiRelation,
} from '@/types/generated/bindings';

import { useArchiModule } from '../../hooks/use-archi-module';
import { NodeRelationGraph } from '../graph/node-relation-graph';
import { type CanvasMode, ModelInfo } from '../model-info';
import { RelationInfo } from './relation-info';
import { type ArchiRelationState, type RelationEdge } from './use-archi-relation';

/** 关系态面板的全部入参（选中边 + 图投影 + 取数状态 + 豁免来源）。 */
interface RelationSelection {
  edge: RelationEdge;
  relation: ArchiRelation;
  state: ArchiRelationState;
  exemptVia: 'config' | 'inline' | null;
}

/** 右栏面板的入参（元素态面板 + 关系态面板共用的一份装配面）。 */
interface InspectorInputs {
  root: string | null;
  mode: CanvasMode;
  focused: ArchiElement | null;
  relations: ArchiRelation[];
  findings: ArchiApiFinding[];
  onChanged: () => void;
  selection: RelationSelection | null;
  onRetry: () => void;
}

interface ArchitectureNodeProps {
  graph: ArchiGraph;
  focus: string | null;
  setFocusOverride: (focus: string) => void;
  driftError: string | null;
  selectedEdge: RelationEdge | null;
  onSelectEdge: (edge: RelationEdge | null) => void;
  inspector: InspectorInputs;
}

/** 右栏 `CardHeader` 两态：无选中 → 元素态（`ModelInfo`），有选中 → 关系态
 * （`RelationInfo`）。同一面板位、互斥呈现，MUST NOT 新开抽屉或边旁浮卡。 */
function InspectorPanels(inputs: InspectorInputs): React.JSX.Element {
  const { root, mode, focused, relations, findings, onChanged, selection, onRetry } = inputs;
  if (selection === null) {
    return (
      <ModelInfo
        root={root}
        mode={mode}
        element={focused}
        relations={relations}
        findings={findings}
        onChanged={onChanged}
      />
    );
  }
  return (
    <RelationInfo
      root={root}
      edge={selection.edge}
      relation={selection.relation}
      state={selection.state}
      exemptVia={selection.exemptVia}
      onRetry={onRetry}
    />
  );
}

function ArchitectureNodeViews({
  graph,
  focus,
  setFocusOverride,
  driftError,
  selectedEdge,
  onSelectEdge,
  inspector: { root, focused },
}: ArchitectureNodeProps): React.JSX.Element {
  const module = useArchiModule(root, focused === null ? null : focused.fqn);
  const files: FileViewerProps['files'] = [];
  // 三条路径各自独立在盘事实：contractPath = 文档（.md）、schemaPath = 接口（.schema.json，
  // 手写真相）、generatedPath = 生成物（.i.rs / .i.ts）——只有一条在盘时只出那一条页签
  if (module.detail?.api?.contractPath) {
    files.push({ path: module.detail.api.contractPath, title: '文档' });
  }
  if (module.detail?.api?.schemaPath) {
    files.push({ path: module.detail.api.schemaPath, title: '接口契约' });
  }
  if (module.detail?.api?.generatedPath) {
    files.push({ path: module.detail.api.generatedPath, title: '接口代码' });
  }
  return (
    <Tabs className="h-full">
      <TabsList variant="line" className="mb-2">
        <TabsTrigger value="doc">文档</TabsTrigger>
        <TabsTrigger value="api">接口</TabsTrigger>
        <TabsTrigger value="relations">关系图</TabsTrigger>
      </TabsList>
      <TabsContent value="doc">{root && <FileViewerBlock root={root} files={files} />}</TabsContent>
      <TabsContent value="api">
        <div className="p-5">敬请期待</div>
      </TabsContent>
      <TabsContent value="relations">
        <NodeRelationGraph
          graph={graph}
          focus={focus}
          setFocusOverride={setFocusOverride}
          driftError={driftError}
          selectedEdge={selectedEdge}
          onSelectEdge={onSelectEdge}
        />
      </TabsContent>
    </Tabs>
  );
}

/** 右栏（`Card` = 面板位 + 画布）：`CardHeader` 承载两态面板，`CardContent` 是
 * xyflow 画布（与面板两态切换互不影响）。 */
export function ArchitectureNode(props: ArchitectureNodeProps): React.JSX.Element {
  const { inspector } = props;
  return (
    <ResizablePanel>
      <Card className="h-full flex flex-col">
        <CardHeader>
          <InspectorPanels {...inspector} />
        </CardHeader>
        <CardContent className="min-h-0 flex-1 rounded-md" data-testid="archi-canvas">
          <ArchitectureNodeViews {...props} />
        </CardContent>
      </Card>
    </ResizablePanel>
  );
}
