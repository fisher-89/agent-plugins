/**
 * 单元测试: modules/workflow/index.ts — workflow.json 操作逻辑 barrel 出口
 *
 * 覆盖范围:
 * - openspec/changes/workflow-files-query-api/test-design.md AC-4:
 *   barrel re-export 而非复制
 * - openspec/changes/move-files-write-into-workflow-module/test-design.md:
 *   AC-3 导出扩充（recordFileOps / appendFileOps / setFileBuckets）与命令层收敛契约
 *   （命令文件无迁移实现残留、无 re-export shim），gitignore API 不进 barrel 的结构锁定
 *
 * 无 mock：纯模块导入与引用断言；源码残留扫描经真实 node:fs 读取两命令文件文本
 * （同 hooks.test.ts「fixture 路径断言」模式）。
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vite-plus/test';

import * as fileInventory from './files/file-inventory';
import * as filesQuery from './files/files-query';
import * as record from './files/record';
import * as workflow from './index';

/** 以 import.meta.url 为基准读取仓库内源文件文本（真实 fs）。 */
function readSource(relativeUrl: string): string {
  return readFileSync(fileURLToPath(new URL(relativeUrl, import.meta.url)), 'utf-8');
}

// ===========================================================================
// workflow barrel — 函数导出
// ===========================================================================

describe('workflow barrel — 函数导出', () => {
  it('5 个导出函数均为 function 类型 (AC-3 扩充后导出面)', () => {
    expect(typeof workflow.readFileInventory).toBe('function');
    expect(typeof workflow.getChangedFiles).toBe('function');
    expect(typeof workflow.recordFileOps).toBe('function');
    expect(typeof workflow.appendFileOps).toBe('function');
    expect(typeof workflow.setFileBuckets).toBe('function');
  });

  it('barrel 导出与源模块直接导入为同一函数引用（toBe）——re-export 而非复制', () => {
    expect(workflow.readFileInventory).toBe(fileInventory.readFileInventory);
    expect(workflow.getChangedFiles).toBe(filesQuery.getChangedFiles);
    // 扩充的三个导出（recordFileOps 自 files/record.ts；append/set 自 files/file-inventory.ts）
    expect(workflow.recordFileOps).toBe(record.recordFileOps);
    expect(workflow.appendFileOps).toBe(fileInventory.appendFileOps);
    expect(workflow.setFileBuckets).toBe(fileInventory.setFileBuckets);
  });
});

// ===========================================================================
// workflow barrel — 导出面精确
// ===========================================================================

describe('workflow barrel — 导出面精确', () => {
  it('运行时导出键恰好等于 5 个函数名，无多余导出（防意外面与 knip dead export）', () => {
    expect(Object.keys(workflow).sort()).toEqual([
      'appendFileOps',
      'getChangedFiles',
      'readFileInventory',
      'recordFileOps',
      'setFileBuckets',
    ]);
  });

  it('不含 loadGitignoreFilter / isGitIgnored（gitignore API 不进 barrel 的结构锁定）', () => {
    expect(workflow).not.toHaveProperty('loadGitignoreFilter');
    expect(workflow).not.toHaveProperty('isGitIgnored');
  });

  it('不含 writeFileInventory / foldFileOps（落盘原语为模块内私有，不进 barrel）', () => {
    expect(workflow).not.toHaveProperty('writeFileInventory');
    expect(workflow).not.toHaveProperty('foldFileOps');
  });
});

// ===========================================================================
// workflow barrel — 命令层收敛契约 (AC-3)
// ===========================================================================

describe('workflow barrel — 命令层收敛契约 (AC-3)', () => {
  const recordFilesSource = readSource('../../commands/record-files.ts');
  const changeFilesSource = readSource('../../commands/change-files.ts');

  it('commands/record-files.ts 源码文本不含 normalizeRecordedPath / isExcludedFromInventory / collectRecordedOps / foldFileOps / writeFileInventory（无实现残留）', () => {
    for (const identifier of [
      'normalizeRecordedPath',
      'isExcludedFromInventory',
      'collectRecordedOps',
      'foldFileOps',
      'writeFileInventory',
    ]) {
      expect(recordFilesSource.includes(identifier)).toBe(false);
    }
  });

  it('commands/change-files.ts 源码文本不含 applyAppend / applySet / dedupe（无实现残留）', () => {
    for (const identifier of ['applyAppend', 'applySet', 'dedupe']) {
      expect(changeFilesSource.includes(identifier)).toBe(false);
    }
  });

  it('两命令文件均无对迁移函数的 re-export shim（export { ... } 形态不出现）', () => {
    const migratedNames = [
      'recordFileOps',
      'appendFileOps',
      'setFileBuckets',
      'foldFileOps',
      'writeFileInventory',
      'readFileInventory',
      'loadGitignoreFilter',
      'isGitIgnored',
    ];
    for (const source of [recordFilesSource, changeFilesSource]) {
      for (const name of migratedNames) {
        expect(source).not.toMatch(new RegExp(`export\\s*\\{[^}]*\\b${name}\\b`));
      }
    }
  });
});
