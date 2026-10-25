// ---------------------------------------------------------------------------
// node / python 内联脚本写入判别（**关联式**，非全文共现式）
//
// 每条正则把「写 API 名」与其**第一个字符串实参**绑定（`api(` + 引号字面量
// 紧邻），对整条命令文本扫描：命中受保护路径即由调用方拒绝。全文共现式（脚本
// 段内同时出现写 API 名与受保护路径字符串即拒）会把日志 / 文档文本误拦
// （`console.log('writeFileSync','<path>')` 必须放行），故 MUST NOT 采用。
//
// 与插件端的硬性偏离：插件端对「`python` / `node` 起手」的命令整体豁免（连提取
// 器都不进），本端 **MUST NOT 复刻**——`node -e "…writeFileSync('受保护文件',…)"` 与
// `python -c "open('受保护文件','w')…"` 必须被拒。
//
// 不切脚本段（不识别 `-e` / `--eval` / `-c` / `--command` 边界）：因此
// `node -c`（`--check`，只做语法检查）不构成误判来源（文本里没有写 API 调用
// 就不命中），`python -c` 亦无需特判；嵌套形态（`bash -c 'node -e "…"'`、
// `pwsh -Command "…"`）一并覆盖（关联式不依赖脚本段定位与行首命令名）。
//
// 已知漏面（知情边界，与 `xargs` / `sh -c` 递归拆解同类）：生成脚本文件再执行、
// 变量中转路径（`const p='…'; writeFileSync(p,…)`）、base64 编码脚本、`printf`
// 拼串后喂 stdin——均不覆盖。裸脚本 / 模块入口（`node x.js` / `python x.py` /
// `node -m`）放行：脚本内容不在命令行文本内，不可静态判别；该放行 MUST NOT
// 扩展到同一条命令里的重定向 / `cp` / 删除族（那些仍按各自族判定）。
//
// 保守方向留痕：`copyFile*` / `shutil.copy*` / `os.rename*` 的**首参是源**，
// 故「从受保护文件复制出去」也会被拒（over-block，安全侧）。
//
// 转义纪律：正则容忍引号前的 `\`（`\"` 形态，嵌套引号命令文本的实况），
// 路径捕获容许尾随 `\`（归一在 `rules::normalize` 内做）。故本模块消费命令
// **原文**而非 `\`→`/` 归一文本——归一化会把 `\"` 变成 `/"` 并破坏引号面。
// ---------------------------------------------------------------------------

use std::sync::OnceLock;

use regex::Regex;

/// 单实参写 API 表（JS / Node）：名称后紧跟其首个字符串实参。
const JS_SINGLE_ARG_APIS: [&str; 13] = [
    "writeFileSync",
    "writeFile",
    "appendFileSync",
    "appendFile",
    "createWriteStream",
    "copyFileSync",
    "copyFile",
    "renameSync",
    "rename",
    "unlinkSync",
    "unlink",
    "rmSync",
    "rm",
];

/// 预编译写 API 正则表（进程内编译一次；单量词线性形态，无嵌套量词 / 回溯
/// 爆炸面；**不设命令文本长度上限**——设上限等于送出一条绕过路径）。
fn write_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        let mut patterns: Vec<String> = JS_SINGLE_ARG_APIS
            .iter()
            // 单实参族：`api(` + 首个引号字面量即路径
            .map(|api| format!(r#"\b{api}\s*\(\s*\\?['"]([^'"\n]+)['"]"#))
            .collect();
        // `openSync(p, 'w' | 'a' | 'wx')`：需模式实参校验（只读模式不拦）
        patterns.push(
            r#"\bopenSync\s*\(\s*\\?['"]([^'"\n]+)['"]\s*,\s*['"][wax][^'"\n]*['"]"#.to_owned(),
        );
        // PY：`open(p, 'w' | 'a' | 'x' | 'wb' | 'w+')`（首模式字符 ∈ wax；`'r'` 是纯读）
        patterns
            .push(r#"\bopen\s*\(\s*\\?['"]([^'"\n]+)['"]\s*,\s*['"][wax][^'"\n]*['"]"#.to_owned());
        // PY：`Path(p).write_text` / `Path(p).write_bytes`
        patterns.push(
            r#"\bPath\s*\(\s*\\?['"]([^'"\n]+)['"]\s*\)\s*\.\s*(?:write_text|write_bytes)"#
                .to_owned(),
        );
        // PY：`shutil.copy` / `copy2` / `copyfile` / `move`（首参为源，保守拦截）
        patterns.push(
            r#"\bshutil\s*\.\s*(?:copy2|copyfile|copy|move)\s*\(\s*\\?['"]([^'"\n]+)['"]"#
                .to_owned(),
        );
        // PY：`os.remove` / `unlink` / `rename` / `replace`
        patterns.push(
            r#"\bos\s*\.\s*(?:remove|unlink|rename|replace)\s*\(\s*\\?['"]([^'"\n]+)['"]"#
                .to_owned(),
        );
        // 预编译失败按不命中收敛（fail-open；静态字面量非法属编译期缺陷，
        // 不 panic、不拒绝启动）
        patterns
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect()
    })
}

/// 逐个写 API 正则 `captures_iter`，聚成候选路径（首个捕获组 = 引号内字面
/// 量）。变量实参（无引号）不捕获 → 自然放行；只报候选、不判命中（命中判定
/// 归 `mod.rs`，本函数零规则知识）。
pub(super) fn write_targets(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for regex in write_regexes() {
        for captures in regex.captures_iter(command) {
            if let Some(target) = captures.get(1) {
                targets.push(target.as_str().to_owned());
            }
        }
    }
    targets
}
