import { createContext, useContext } from 'react';

/**
 * 提案卡的写回动作面：`root` 是写面命令的 store 寻址半边，`onApplied` 是页面唯一的
 * 取数刷新钩子（页面取数路径只有 `useWorkspaceConfig`——把 apply 返回的报告塞进页面
 * state 就是第二真相源）。
 */
export interface ConfigApplyActions {
  /** 主仓 root（写面命令的 store 寻址半边） */
  root: string;
  /** 写回成功后的页面刷新钩子（页面唯一取数路径） */
  onApplied: () => void;
}

const ConfigApplyContext = createContext<ConfigApplyActions | null>(null);

/**
 * 写回动作面 Provider（面板侧提供、卡片侧消费）：骨架 `SessionPanel` 的 props 与
 * `SessionPanelState` 不得为写面加字段（骨架是全局面板契约），故走 React context
 * 这条不动骨架的通道。
 */
export function ConfigApplyScope(props: {
  value: ConfigApplyActions;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <ConfigApplyContext.Provider value={props.value}>{props.children}</ConfigApplyContext.Provider>
  );
}

/**
 * 卡片侧读取：`null` = 无写回通道（卡片呈只读态，应用按钮禁用 + 文案说明）。
 * 缺席降级而非抛错——既有卡片渲染面（不包 provider）零改动即可跑。
 */
export function useConfigApplyActions(): ConfigApplyActions | null {
  return useContext(ConfigApplyContext);
}
