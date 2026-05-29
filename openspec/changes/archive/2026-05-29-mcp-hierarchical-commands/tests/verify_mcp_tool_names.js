// verify_mcp_tool_names.js -- 静态验证 MCP tool 名称使用 xx/yy 层级格式
//
// 覆盖:
//   AC-1: 6 个 MCP tool 名称从 xx_yy 改为 xx/yy (验证 TOOLS 数组)
//   AC-2: handleToolCall switch 分支匹配新名称 (验证 switch case)
//   边界: 未知 tool 名称抛出 "Unknown tool: <name>" 错误
//   边界: 旧名称作为遗留调用传入时进入 default 分支
//   边界: TOOLS name 与 switch case 不一致时检测不匹配
//
// 使用:
//   node verify_mcp_tool_names.js
//
// 注意: 本脚本直接解析 mcp.ts 源码文件。当源文件变更时，请更新
// 不同导入路径的注释。

import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

// ===========================================================================
// 配置
// ===========================================================================

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// mcp.ts 源文件路径 (根据测试执行目录可能需要调整)
const MCP_TS_PATH = resolve(__dirname, '../../../../plugins/dev-team/bin/src/mcp.ts');

// 预期的层级名称映射: 旧名称 -> 新名称
const NAME_MAP = {
  'eval_log': 'eval/log',
  'eval_check': 'eval/check',
  'archi_query': 'archi/query',
  'archi_validate': 'archi/validate',
  'archi_write': 'archi/write',
  'archi_check': 'archi/check',
};

const EXPECTED_NEW_NAMES = Object.values(NAME_MAP);       // ['eval/log', 'eval/check', ...]
const OLD_NAMES = Object.keys(NAME_MAP);                    // ['eval_log', 'eval_check', ...]

// 新名称到业务逻辑函数的路由映射 (预期)
const ROUTE_MAP = {
  'eval/log': 'runEvalLog',
  'eval/check': 'runEvalCheck',
  'archi/query': 'queryModel',
  'archi/validate': 'validateDsl',
  'archi/write': 'writeDsl',
  'archi/check': 'runCrossRefCheck',
};

// ===========================================================================
// 帮助函数
// ===========================================================================

let PASS_COUNT = 0;
let FAIL_COUNT = 0;

function pass(name) {
  console.log(`  PASS: ${name}`);
  PASS_COUNT++;
}

function fail(name, detail) {
  console.log(`  FAIL: ${name} -- ${detail}`);
  FAIL_COUNT++;
}

function assertTrue(condition, testName, detail) {
  if (condition) {
    pass(testName);
  } else {
    fail(testName, detail);
  }
}

function assertFalse(condition, testName, detail) {
  if (!condition) {
    pass(testName);
  } else {
    fail(testName, detail);
  }
}

// ===========================================================================
// 源码解析
// ===========================================================================

function readSource() {
  try {
    return readFileSync(MCP_TS_PATH, 'utf-8');
  } catch (err) {
    console.error(`ERROR: 无法读取源文件 ${MCP_TS_PATH}: ${err.message}`);
    console.error(`      请确认路径正确，或手动设置 MCP_TS_PATH。`);
    process.exit(1);
  }
}

/**
 * 从源码中提取 TOOLS 数组中的所有 name 值。
 */
function extractToolNames(source) {
  // 简单但健壮的正则: 匹配 name: "xxx" 出现在 TOOLS 定义上下文中
  // 查找每个工具定义中的 name 字段
  const names = [];
  const toolRegex = /name:\s*"([^"]+)"/g;
  let match;
  while ((match = toolRegex.exec(source)) !== null) {
    names.push(match[1]);
  }
  return names;
}

/**
 * 从源码中提取 handleToolCall switch 语句中的所有 case 标签。
 */
function extractSwitchCases(source) {
  // 查找 handleToolCall 函数体内的 switch case 标签
  // 限制在 handleToolCall 函数范围内，排除 main() 中 JSON-RPC method switch
  const funcStart = source.indexOf('async function handleToolCall');
  if (funcStart === -1) {
    console.error('ERROR: 未找到 handleToolCall 函数定义');
    return [];
  }

  // 找到 handleToolCall 函数的结束位置 (下一个顶层函数之前)
  const mainStart = source.indexOf('function main()', funcStart);
  const funcEnd = mainStart !== -1 ? mainStart : source.length;
  const funcBody = source.slice(funcStart, funcEnd);

  const cases = [];
  const caseRegex = /case\s+"([^"]+)":/g;
  let match;
  while ((match = caseRegex.exec(funcBody)) !== null) {
    cases.push(match[1]);
  }
  return cases;
}

// ===========================================================================
// 测试: AC-1 — TOOLS 数组中的 name 格式
// ===========================================================================

function testAc1ToolNamesFormat(source) {
  console.log('\n=== AC-1: 验证 TOOLS 数组中的 name 字段使用 xx/yy 格式 ===\n');

  const names = extractToolNames(source);

  // 应有 6 个 name (每个工具一个)
  assertTrue(
    names.length === 6,
    'AC-1.a: TOOLS 数组包含 6 个 name 条目',
    `预期 6 个, 实际 ${names.length} 个 name`,
  );

  // 每个 name 使用 xx/yy 格式
  for (const name of names) {
    const isHierarchical = /^[a-z]+\/[a-z]+$/.test(name);
    assertTrue(
      isHierarchical,
      `AC-1.b: name "${name}" 使用 xx/yy 层级格式`,
      `name "${name}" 不符合 xx/yy 格式`,
    );
  }

  // 所有预期的 6 个名称都在 TOOLS 数组中
  for (const expected of EXPECTED_NEW_NAMES) {
    assertTrue(
      names.includes(expected),
      `AC-1.c: TOOLS 包含 "${expected}"`,
      `TOOLS 中未找到 "${expected}"`,
    );
  }

  // TOOLS 数组中不存在旧名称 (xx_yy 格式)
  for (const old of OLD_NAMES) {
    assertFalse(
      names.includes(old),
      `AC-1.d: TOOLS 不包含旧名称 "${old}"`,
      `TOOLS 中仍存在旧名称 "${old}"`,
    );
  }
}

// ===========================================================================
// 测试: AC-2 — handleToolCall switch 分支
// ===========================================================================

function testAc2SwitchCases(source) {
  console.log('\n=== AC-2: 验证 handleToolCall switch 分支使用 xx/yy 格式 ===\n');

  const cases = extractSwitchCases(source);

  // 应有 6 个 case (每个工具一个)
  assertTrue(
    cases.length === 6,
    'AC-2.a: switch 语句包含 6 个 case',
    `预期 6 个, 实际 ${cases.length} 个 case`,
  );

  // 每个 case 使用 xx/yy 格式
  for (const c of cases) {
    const isHierarchical = /^[a-z]+\/[a-z]+$/.test(c);
    assertTrue(
      isHierarchical,
      `AC-2.b: case "${c}" 使用 xx/yy 层级格式`,
      `case "${c}" 不符合 xx/yy 格式`,
    );
  }

  // 所有预期的 6 个名称都在 switch case 中
  for (const expected of EXPECTED_NEW_NAMES) {
    assertTrue(
      cases.includes(expected),
      `AC-2.c: switch 包含 case "${expected}"`,
      `switch 中未找到 case "${expected}"`,
    );
  }

  // switch 中不存在旧名称
  for (const old of OLD_NAMES) {
    assertFalse(
      cases.includes(old),
      `AC-2.d: switch 不包含旧名称 "${old}"`,
      `switch 中仍存在旧名称 "${old}"`,
    );
  }

  // TOOLS 的 name 与 switch case 交叉检查: 每个 TOOLS 中的 name 必须在 switch 中有对应 case
  const toolNames = extractToolNames(source);
  for (const name of toolNames) {
    assertTrue(
      cases.includes(name),
      `AC-2.e: TOOLS name "${name}" 在 switch 中有对应 case`,
      `TOOLS 包含 "${name}" 但 switch 中缺少对应 case`,
    );
  }

  // 每个 switch case 在 TOOLS 中有对应 entry
  for (const c of cases) {
    assertTrue(
      toolNames.includes(c),
      `AC-2.f: switch case "${c}" 在 TOOLS 中有对应 entry`,
      `switch 包含 case "${c}" 但 TOOLS 中无对应 name`,
    );
  }
}

// ===========================================================================
// 测试: 边界场景 — 未知 tool 名称
// ===========================================================================

function testEdgeUnknownTool(source) {
  console.log('\n=== 边界: 未知 tool 名称 ===\n');

  const funcStart = source.indexOf('async function handleToolCall');
  const funcBody = source.slice(funcStart);
  const defaultMatch = funcBody.match(/default:\s*$/m);

  assertTrue(
    defaultMatch !== null,
    '边界.a: handleToolCall 存在 default 分支',
    '未找到 default 分支',
  );

  // 检查 default 分支抛出 "Unknown tool: ${name}" 错误
  const defaultSection = funcBody.slice(defaultMatch.index);
  const hasThrowUnknown = /throw\s+new\s+Error\(\s*`Unknown tool:\s*\$\{?\w+\}?`/.test(defaultSection) ||
                          /throw\s+new\s+Error\(\s*['"]Unknown tool:/.test(defaultSection);

  assertTrue(
    hasThrowUnknown,
    '边界.b: default 分支抛出 "Unknown tool: <name>" 错误',
    'default 分支未抛出预期的错误消息',
  );
}

// ===========================================================================
// 测试: 边界场景 — 旧名称作为遗留调用传入
// ===========================================================================

function testEdgeOldNamesRejected(source) {
  console.log('\n=== 边界: 旧名称被 default 分支捕获 ===\n');

  const cases = extractSwitchCases(source);

  // 旧名称不应匹配任何 case，应被 default 捕获
  for (const oldName of OLD_NAMES) {
    assertFalse(
      cases.includes(oldName),
      `边界.c: 旧名称 "${oldName}" 不在 switch case 中 (将被 default 捕获)`,
      `旧名称 "${oldName}" 仍有对应的 case 分支`,
    );
  }
}

// ===========================================================================
// 测试: 边界场景 — 路由一致性 (新名称路由到正确函数)
// ===========================================================================

function testEdgeRouteConsistency(source) {
  console.log('\n=== 边界: 路由一致性验证 ===\n');

  // 从源码中提取每个 case 分支中的函数调用
  const funcStart = source.indexOf('async function handleToolCall');
  const funcBody = source.slice(funcStart);

  for (const [newName, expectedFunc] of Object.entries(ROUTE_MAP)) {
    // 查找 case 和对应函数调用
    const caseRegex = new RegExp(
      `case\\s+"${escapeRegex(newName)}"[^]*?(?=case\\s+"|default:)`,
    );
    const caseMatch = funcBody.match(caseRegex);

    if (caseMatch) {
      const caseBody = caseMatch[0];
      const hasFuncCall = caseBody.includes(expectedFunc);
      assertTrue(
        hasFuncCall,
        `边界.d: "${newName}" 路由到 ${expectedFunc}`,
        `"${newName}" 的 case 中未找到对 ${expectedFunc} 的调用`,
      );
    } else {
      fail(`边界.d: "${newName}" 路由到 ${expectedFunc}`, `未找到 case "${newName}"`);
    }
  }
}

function escapeRegex(str) {
  return str.replace(/[.*+?^${}()|[\]\\\/]/g, '\\$&');
}

// ===========================================================================
// 测试: 源代码中不存在旧名称引入
// ===========================================================================

function testSourceNoOldName(SOURCE) {
  // Skip: this validates that the source doesn't reference old name patterns in tool definitions
  // We already checked TOOLS and switch cases above
}

// ===========================================================================
// 测试运行器
// ===========================================================================

function runSuite() {
  console.log('==================================================');
  console.log('  MCP Tool 名称层级格式验证');
  console.log(`  源文件: ${MCP_TS_PATH}`);
  console.log('==================================================');

  const source = readSource();

  testAc1ToolNamesFormat(source);
  testAc2SwitchCases(source);
  testEdgeUnknownTool(source);
  testEdgeOldNamesRejected(source);
  testEdgeRouteConsistency(source);

  // =========================================================================
  // 汇总
  // =========================================================================

  const total = PASS_COUNT + FAIL_COUNT;
  console.log('\n==================================================');
  console.log(`  结果: ${PASS_COUNT} passed, ${FAIL_COUNT} failed (共 ${total} 项)`);
  console.log('==================================================\n');

  process.exit(FAIL_COUNT > 0 ? 1 : 0);
}

runSuite();
