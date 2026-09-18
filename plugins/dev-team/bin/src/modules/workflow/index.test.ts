/**
 * 单元测试: modules/workflow/index.ts — workflow.json 操作逻辑 barrel 出口
 *
 * 覆盖范围（openspec/changes/workflow-files-query-api/test-design.md）:
 * - AC-4: barrel 导出四函数（readFileInventory / foldFileOps / writeFileInventory /
 *   getChangedFiles）及 FileInventory / FileOp 类型；re-export 而非复制；
 *   导出面精确（无多余运行时导出，防意外面与 knip dead export）
 *
 * 无 mock：纯模块导入与引用断言，不涉及跨进程边界。
 */

import { describe, expect, it } from 'vite-plus/test';

import * as fileInventory from './file-inventory';
import * as filesQuery from './files-query';
import * as workflow from './index';

// ===========================================================================
// workflow barrel — 函数导出
// ===========================================================================

describe('workflow barrel — 函数导出', () => {
  it('readFileInventory / foldFileOps / writeFileInventory / getChangedFiles 均为 function 类型 (AC-4)', () => {
    expect(typeof workflow.readFileInventory).toBe('function');
    expect(typeof workflow.foldFileOps).toBe('function');
    expect(typeof workflow.writeFileInventory).toBe('function');
    expect(typeof workflow.getChangedFiles).toBe('function');
  });

  it('barrel 导出与源模块直接导入为同一函数引用（toBe）——re-export 而非复制 (AC-4)', () => {
    expect(workflow.readFileInventory).toBe(fileInventory.readFileInventory);
    expect(workflow.foldFileOps).toBe(fileInventory.foldFileOps);
    expect(workflow.writeFileInventory).toBe(fileInventory.writeFileInventory);
    expect(workflow.getChangedFiles).toBe(filesQuery.getChangedFiles);
  });
});

// ===========================================================================
// workflow barrel — 导出面精确
// ===========================================================================

describe('workflow barrel — 导出面精确', () => {
  it('运行时导出键恰好等于 4 个函数名，无多余导出（防意外面与 knip dead export）', () => {
    expect(Object.keys(workflow).sort()).toEqual([
      'foldFileOps',
      'getChangedFiles',
      'readFileInventory',
      'writeFileInventory',
    ]);
  });
});
