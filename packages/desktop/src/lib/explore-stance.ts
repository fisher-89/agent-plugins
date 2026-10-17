/**
 * explore stance 前导模板（desktop 包自持；与插件
 * `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链——stance 立场段
 * 与笔记落盘约定同源叙述，改动需双侧同步；双源面随占位 / 已定名两形扩大——
 * 命名指令形与笔记定位形的「笔记」段约定两侧同步）。
 */

/**
 * 占位名前缀「未定名」判别口径（与后端 store `EXPLORE_PLACEHOLDER_PREFIX`
 * 字面量双源互链——前端不引后端常量，改动需同步）。
 */
const DRAFT_NAME_PREFIX = 'draft-';

/** 模式说明 + 立场五条（两形公共段） */
function modeAndStance(): string {
  return `你在探索模式下工作。这是思考与调研时间，不是实现时间：可以读文件、搜代码、调查仓库，但绝不写应用代码，也绝不为了存放笔记而创建 change 目录（用户明确要求产出 OpenSpec 工件时除外）。

## 立场
- 好奇而非说教：问题从对话中自然生长，不套固定清单
- 多开线索而非审问：呈现多个有趣方向，让用户选择跟进哪条
- 善用 ASCII 图解：结构、状态机、数据流，能画就画
- 随线索转向：新信息出现时调整方向，不急于收敛结论
- 落地调研：探索真实代码库，不凭空推演`;
}

/** 两行约定 + 追加纪律（两形公共段） */
function commonNoteRules(): string {
  return `- 笔记首行写 \`# <标题>\`（人类可读标题，用于探索清单展示；写中文标题亦可）
- 笔记第二行写 \`workflow_type: <requirement|test-only>\`（推算变更类型：目标是补测试 / 生成测试用例 → test-only，否则 requirement）
- 默认追加；仅用户明确要求时才整文件重写`;
}

/**
 * 命名指令形「笔记」段（占位记录）：指引生成 kebab 最终名并经
 * `create_explore_note(name, content)` 原子定名落盘。零路径插值——路径知识
 * 归工具描述与返回值。
 */
function namingStance(): string {
  return `## 笔记定名与落盘（本次探索尚未定名）
- 调研成型后尽快为本次探索起一个 kebab-case 最终名（小写字母/数字、\`-\` 连接，如 \`api-retry-strategy\`）
- 笔记初稿经专用工具 \`create_explore_note(name, content)\` 落盘：\`name\` 即最终名，服务端原子完成文件创建与记录定名，返回笔记路径（repo 相对路径）；此后用 \`edit\` 修改该路径
- 优先使用 \`create_explore_note\` 而非 bash 造文件（bash 造出的笔记不会与探索记录绑定）
${commonNoteRules()}`;
}

/** 笔记定位形「笔记」段（已定名记录）：插值记录名与笔记路径，指引 edit 修改、缺失同名补建 */
function locatingStance(recordName: string): string {
  return `## 笔记定位（本次探索已定名）
- 本次探索的笔记在 openspec/explores/${recordName}.md（记录名 = ${recordName}）：修改笔记用 \`edit\` 编辑该文件
- 文件缺失（如被会话外删除）时，先以 \`create_explore_note(name="${recordName}", content=…)\` 同名补建，再继续用 \`edit\`
${commonNoteRules()}`;
}

/**
 * 组装 explore run 的 prompt：stance 前导 + 空行分隔 + 用户输入。stance 按
 * `recordName` 是否为占位前缀分两形——占位（draft-）→ 命名指令形（指引经
 * `create_explore_note` 原子定名，零路径插值）；已定名 → 笔记定位形（插值
 * 记录名与笔记路径）。`recordName` 恒为当前 `ExploreRecord.name`（调用方从
 * 记录直供）。
 */
export function buildExplorePrompt(userInput: string, recordName: string): string {
  const noteSection = recordName.startsWith(DRAFT_NAME_PREFIX)
    ? namingStance()
    : locatingStance(recordName);
  return `${modeAndStance()}\n\n${noteSection}\n\n## 其他\n- 回答与提问一律用中文\n\n---\n\n${userInput}`;
}
