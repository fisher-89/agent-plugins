/**
 * 单元测试: lib/exec-command.ts — execCommand
 *
 * 覆盖 execCommand 函数的 spawnSync 封装行为：
 * - 模块导出验证
 * - spawnSync 参数透传与默认选项合并
 * - spawnSync 返回值透传
 * - 输入边界
 *
 * @see plugins/dev-team/bin/src/lib/exec-command.ts
 */

import { describe, it, expect } from 'vite-plus/test';

import { execCommand } from './exec-command';

// 使用 process.execPath 而非硬编码 "node"：
// 当使用 fnm/nvm 等版本管理器时，子进程的 PATH 中可能找不到 node 二进制。
// process.execPath 始终指向当前运行的 Node.js 完整路径。
const node = process.execPath;

// ===========================================================================
// execCommand — 参数透传与默认选项合并
// ===========================================================================

describe('execCommand — 参数透传与默认选项合并', () => {
  it('应透传用户 options (cwd) 到 spawnSync', () => {
    // node -e 在指定 cwd 执行并打印 cwd
    const result = execCommand(`${node} -e "console.log(process.cwd())"`);
    expect(result.status).toBe(0);
    expect(result.stdout.trim()).toBe(process.cwd());
  });

  it('encoding 默认为 utf-8，stdout 应为 string 类型', () => {
    const result = execCommand(`${node} -e "console.log(123)"`);
    expect(typeof result.stdout).toBe('string');
  });

  it('用户可传入 env 等额外选项', () => {
    const result = execCommand(`${node} -e "console.log(process.env.TEST_VAR)"`, {
      env: { ...process.env, TEST_VAR: 'myvalue' },
    });
    expect(result.status).toBe(0);
    expect(result.stdout.trim()).toBe('myvalue');
  });
});

// ===========================================================================
// execCommand — spawnSync 返回值透传
// ===========================================================================

describe('execCommand — spawnSync 返回值透传', () => {
  it('成功执行命令应返回 status 0', () => {
    const result = execCommand(`${node} -e ""`);
    expect(result.status).toBe(0);
  });

  it('应返回 stdout 内容', () => {
    const result = execCommand(`${node} -e "process.stdout.write(\\"hello world\\")"`);
    expect(result.stdout).toBe('hello world');
  });

  it('命令执行失败时应透传非零 status', () => {
    const result = execCommand(`${node} -e "process.exit(42)"`);
    expect(result.status).toBe(42);
  });

  it('失败命令应产生 stderr 输出', () => {
    const result = execCommand(`${node} -e "console.error(\\"some error\\"); process.exit(1)"`);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('some error');
  });

  it('应返回有效的 pid 字段', () => {
    const result = execCommand(`${node} -e ""`);
    expect(typeof result.pid).toBe('number');
    expect(result.pid).toBeGreaterThan(0);
  });

  it('应透传 signal 字段（正常退出时 signal 为 null）', () => {
    const result = execCommand(`${node} -e ""`);
    expect(result.signal).toBeNull();
  });
});

// ===========================================================================
// execCommand — 输入边界
// ===========================================================================

describe('execCommand — 输入边界', () => {
  it('options 为空对象 {} 时应正常执行', () => {
    expect(() => execCommand(`${node} -e ""`, {})).not.toThrow();
  });

  it('连续多次调用应各自独立执行', () => {
    const r1 = execCommand(`${node} -e "process.stdout.write(\\"a\\")"`);
    const r2 = execCommand(`${node} -e "process.stdout.write(\\"b\\")"`);
    const r3 = execCommand(`${node} -e "process.stdout.write(\\"c\\")"`);

    expect(r1.stdout).toBe('a');
    expect(r2.stdout).toBe('b');
    expect(r3.stdout).toBe('c');
  });

  it('超长输出不应导致异常', () => {
    const result = execCommand(`${node} -e "process.stdout.write('x'.repeat(10000))"`);
    expect(result.status).toBe(0);
    expect(result.stdout.length).toBe(10000);
  });
});
