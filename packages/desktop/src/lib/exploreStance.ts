/**
 * explore stance 前导模板（desktop 包自有，归属本包）。
 *
 * 双源互链：`plugins/dev-team/skills/openspec-explore/SKILL.md`（CLI skill
 * 注入通道）。desktop 无 skill 上下文注入通道，故内置精简前导拼进每条
 * explore run 的 prompt 头部；两源刻意保持独立、接受漂移——本模板为 desktop
 * 页面体验裁剪（落盘路径约定与单文件投影语义），不追踪 SKILL.md 的演进。
 *
 * 目录位置说明：模板正文中的笔记目录路径是发给 agent 的指令内容（agent 会话
 * 按其落盘），非前端的路径推导逻辑——前端零目录名常量，文档路径一律经
 * `explore_doc_path` 命令布局派生（design D6）。
 */

/**
 * 探索立场前导（精简版，与 SKILL.md 同源不同文）：
 * - 探索模式是思考而非实现：可读码调研，MUST NOT 写应用代码、MUST NOT 创建
 *   change 目录（除非用户明确要求产出 OpenSpec 工件）；
 * - 立场：好奇不预设、多开线索不审问、多用 ASCII 图解、随新信息转向、
 *   探索真实代码库而非空谈；
 * - 笔记落盘约定：单篇主题一个 md 文件，路径 `<workspace>/openspec/explores/<topic>.md`
 *   （topic 用 kebab-case，与对话给定的主题一致）；默认追加而非整文件重写，
 *   无内容也先建轻量骨架（记录清单依赖文件可读）；
 * - 输出语言：中文。
 */
const STANCE_PREAMBLE = `你在探索模式下工作。这是思考与调研时间，不是实现时间：可以读文件、搜代码、调查仓库，但绝不写应用代码，也绝不为了存放笔记而创建 change 目录（用户明确要求产出 OpenSpec 工件时除外）。

## 立场
- 好奇而非说教：问题从对话中自然生长，不套固定清单
- 多开线索而非审问：呈现多个有趣方向，让用户选择跟进哪条
- 善用 ASCII 图解：结构、状态机、数据流，能画就画
- 随线索转向：新信息出现时调整方向，不急于收敛结论
- 落地调研：探索真实代码库，不凭空推演

## 笔记落盘
- 本次探索的笔记写入 workspace 下 openspec/explores/<topic>.md（topic 为 kebab-case 主题名，与用户给定的主题一致）
- 默认追加；仅用户明确要求时才整文件重写
- 若文件尚不存在，先写入一段简短的主题与背景骨架

## 其他
- 回答与提问一律用中文`;

/** 组装 explore run 的 prompt：stance 前导 + 空行分隔 + 用户输入。 */
export function buildExplorePrompt(userInput: string): string {
  return `${STANCE_PREAMBLE}\n\n---\n\n${userInput}`;
}
