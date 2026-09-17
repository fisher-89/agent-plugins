import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

/**
 * Relative project-root POSIX-style path: no leading separator, no Windows
 * drive prefix, no backslashes, no `..` traversal segments.
 */
const relativePosixPathSchema = z
  .string()
  .min(1)
  .refine((p) => !p.startsWith('/'), '路径必须相对项目根（不接受绝对路径）')
  .refine((p) => !/^[A-Za-z]:/.test(p), '路径必须相对项目根（不接受 Windows 盘符路径）')
  .refine((p) => !p.includes('\\'), '路径必须为 POSIX 风格（使用 / 分隔）')
  .refine((p) => !p.split('/').includes('..'), '路径不允许包含 `..` 穿越段');

/**
 * Input schema for `change_files` MCP tool.
 *
 * `written` / `deleted` are optional individually but at least one must be
 * provided (refined below).
 */
export const changeFilesInputSchema = z
  .object({
    change: z.string().min(1).describe('Target change name (openspec/changes/<change>)'),
    op: z
      .enum(['append', 'set'])
      .describe(
        'append：按折叠规则逐路径合并入净状态（去重、保留既有来源）；set：整体覆写指定桶（未提供的桶不动），用于显式修正净状态',
      ),
    written: z
      .array(relativePosixPathSchema)
      .optional()
      .describe('写入桶路径列表（相对项目根 POSIX 风格）'),
    deleted: z
      .array(relativePosixPathSchema)
      .optional()
      .describe('删除桶路径列表（相对项目根 POSIX 风格）'),
    project_root: projectRootSchema,
  })
  .refine((v) => (v.written?.length ?? 0) > 0 || (v.deleted?.length ?? 0) > 0, {
    message: 'written 与 deleted 至少提供其一',
    path: ['written'],
  });

export type ChangeFilesInput = z.input<typeof changeFilesInputSchema>;

/** Output schema for `change_files`: the net state after the operation. */
export const changeFilesOutputSchema = z.object({
  written: z.array(z.string()).describe('操作后的净写入路径列表'),
  deleted: z.array(z.string()).describe('操作后的净删除路径列表'),
});

export type ChangeFilesOutput = z.output<typeof changeFilesOutputSchema>;
