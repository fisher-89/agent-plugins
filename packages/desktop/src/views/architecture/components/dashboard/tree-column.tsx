import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui';
import { type ArchiApiFinding, type ArchiElement } from '@/types/generated/bindings';

import { type CanvasMode } from '../model-info';
import { ContractPanel } from './contract-panel';
import { ElementTree } from './element-tree';
import { FileTree } from './file-tree';

/** 左栏树视图（分段控件三值之一；缺省值由 architecture-panel.tsx 的 treeMode 持有）。 */
export type TreeMode = 'file' | 'element' | 'contract';

/** 左栏树栏位：分段控件 + 三态行（缺省元素树，可切文件 / 契约视图）。切换只换左栏
 * 组件，`focus` 共用；取数失败只落本栏行内提示，画布与抽屉照常渲染。 */
export function TreeColumn({
  root,
  canvasMode,
  mode,
  onModeChange,
  elements,
  focused,
  onFocus,
  findings,
}: {
  root: string | null;
  canvasMode: CanvasMode;
  mode: TreeMode;
  onModeChange: (mode: TreeMode) => void;
  elements: ArchiElement[];
  focused: string | null;
  onFocus: (fqn: string) => void;
  findings: ArchiApiFinding[];
}): React.JSX.Element {
  const options: { mode: TreeMode; label: string; content: React.JSX.Element }[] = [
    {
      mode: 'element',
      label: '元素',
      content: <ElementTree elements={elements} focused={focused} onFocus={onFocus} />,
    },
    {
      mode: 'file',
      label: '文件',
      content: <FileTree root={root} focused={focused} onFocus={onFocus} />,
    },
    {
      mode: 'contract',
      label: '契约',
      content: (
        <ContractPanel root={root} mode={canvasMode} findings={findings} onFocus={onFocus} />
      ),
    },
  ];
  return (
    <Tabs className="bg-card p-4 w-full h-full" value={mode} onValueChange={onModeChange}>
      <TabsList variant="line" className="mb-2" data-testid="archi-tree-mode" role="group">
        {options.map((option) => (
          <TabsTrigger value={option.mode}>{option.label}</TabsTrigger>
        ))}
      </TabsList>
      {options.map((option) => (
        <TabsContent value={option.mode}>{option.content}</TabsContent>
      ))}
    </Tabs>
  );
}
