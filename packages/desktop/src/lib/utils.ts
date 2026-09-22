import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Tailwind 类名组合与冲突消解：clsx 条件拼接 + tailwind-merge 去重（shadcn canonical 工具） */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
