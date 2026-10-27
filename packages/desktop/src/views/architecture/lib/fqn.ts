/**
 * FQN 展示映射（纯函数，单点）：末段短名由元素树与画布节点共用同一取法——两处各
 * 写一份会各自演化（一处按 `.` 切分、一处按首字母缩写），同一元素在树与画布上呈现
 * 不同名字。
 */
export function shortName(fqn: string): string {
  const segments = fqn.split('.');
  return segments[segments.length - 1] ?? fqn;
}
