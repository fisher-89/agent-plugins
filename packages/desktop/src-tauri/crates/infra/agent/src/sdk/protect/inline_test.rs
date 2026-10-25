//! `inline` 的单元测试（AC-7）：node / python 内联脚本写入判别（关联式——写
//! API 名与其**第一个字符串实参**绑定，非全文共现式）。
//!
//! 断言口径：
//! - 零 mock：命令文本即被测 API 显式入参，纯 `regex` 文本扫描，不真跑
//!   `node` / `python`、不触文件系统；
//! - 反向断言对象：插件端「`python` / `node` 起手整体豁免」口径 MUST NOT 复刻
//!   （内联写入必须被提取）；
//! - 已知漏面（裸脚本 / 变量中转 / base64 / 生成脚本再执行）以「放行」形态显式
//!   留痕，是知情边界而非缺口。

use super::inline;

/// 受保护文件路径样本（判命中归 `protect/mod.rs`，本文件只锚候选提取）。
const PROTECTED: &str = "/ws/openspec/config.json";

fn targets(command: &str) -> Vec<String> {
    inline::write_targets(command)
}

/// 候选集中是否含指定路径（回调形态，避免借用与所有权摩擦）。
fn contains(command: &str, expected: &str) -> bool {
    targets(command).iter().any(|target| target == expected)
}

/// 候选集中是否含指定路径（容忍尾随转义反斜杠：嵌套引号命令文本的实参常以
/// `\"` 收尾，尾随 `\` 由 `rules::normalize` 吸收）。
fn contains_trimmed(command: &str, expected: &str) -> bool {
    targets(command)
        .iter()
        .any(|target| target.trim_end_matches('\\') == expected)
}

// ---------------------------------------------------------------------------
// JS / Node 单实参写 API 表
// ---------------------------------------------------------------------------

#[test]
fn js单实参写api逐项提取首参() {
    let apis = [
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
    for api in apis {
        let command = format!("node -e \"fs.{api}('p','x')\"");
        assert_eq!(
            targets(&command),
            vec!["p".to_owned()],
            "API {api} 的首个字符串实参即候选路径"
        );
        // `fs.promises.<api>` 形态同（前缀不参与匹配）
        let promises = format!("node -e \"fs.promises.{api}('p','x')\"");
        assert!(contains(&promises, "p"), "promises 形态同提取: {promises}");
    }
}

#[test]
fn js_open族仅写模式提取只读模式不提取() {
    for mode in ["w", "a", "wx"] {
        let command = format!("node -e \"fs.openSync('p','{mode}')\"");
        assert_eq!(
            targets(&command),
            vec!["p".to_owned()],
            "openSync 写模式 {mode} 提取首参"
        );
    }
    assert!(
        targets("node -e \"fs.openSync('p','r')\"").is_empty(),
        "只读模式 `r` 不产出候选（纯读不误拦）"
    );
}

// ---------------------------------------------------------------------------
// PY 写 API 表
// ---------------------------------------------------------------------------

#[test]
fn py_open族写模式逐项提取只读模式不提取() {
    for mode in ["w", "a", "x", "wb", "w+"] {
        let command = format!("python -c \"open('p','{mode}').write('{{}}')\"");
        assert!(
            !targets(&command).is_empty(),
            "open 写模式 {mode} 应提取候选"
        );
        assert!(
            targets(&command).iter().all(|target| target == "p"),
            "open 写模式 {mode} 首参即候选，实际: {:?}",
            targets(&command)
        );
    }
    assert!(
        targets("python -c \"open('p','r')\"").is_empty(),
        "只读模式 `r` 零候选"
    );
}

#[test]
fn py_path写方法提取路径() {
    for command in [
        "python -c \"Path('p').write_text('x')\"",
        "python -c \"Path('p').write_bytes(b'x')\"",
    ] {
        assert!(
            !targets(command).is_empty() && targets(command).iter().all(|target| target == "p"),
            "Path(p).write_* 提取路径，命令: {command}"
        );
    }
}

#[test]
fn py_shutil与os族取首个字符串实参() {
    // shutil 族首参为源（保守拦截方向：从受保护文件复制出去亦被拦）
    for command in [
        "python -c \"shutil.copy('src','p')\"",
        "python -c \"shutil.copy2('src','p')\"",
        "python -c \"shutil.copyfile('src','p')\"",
        "python -c \"shutil.move('src','p')\"",
    ] {
        let extracted = targets(command);
        assert!(
            !extracted.is_empty() && extracted.iter().all(|target| target == "src"),
            "shutil 族取首参（源），命令: {command}，实际: {extracted:?}"
        );
    }
    for command in [
        "python -c \"os.remove('p')\"",
        "python -c \"os.unlink('p')\"",
    ] {
        let extracted = targets(command);
        assert!(
            !extracted.is_empty() && extracted.iter().all(|target| target == "p"),
            "os 删除族取首参，命令: {command}，实际: {extracted:?}"
        );
    }
    for command in [
        "python -c \"os.rename('src','p')\"",
        "python -c \"os.replace('src','p')\"",
    ] {
        let extracted = targets(command);
        assert!(
            !extracted.is_empty() && extracted.iter().all(|target| target == "src"),
            "os 改名族取首参，命令: {command}，实际: {extracted:?}"
        );
    }
}

#[test]
fn copy族首参为源的over_block行为锚定() {
    // 规格固定「路径取第一个字符串实参」，故「从受保护文件复制出去」亦被拒
    // （over-block，方向为安全侧；产品若改取第二实参，本用例随之改写）
    assert!(
        contains(
            &format!("python -c \"shutil.copy('{PROTECTED}','/tmp/x')\""),
            PROTECTED
        ),
        "shutil.copy 首参为受保护文件即入候选"
    );
    assert!(
        contains(
            &format!("node -e \"fs.copyFileSync('{PROTECTED}', '/tmp/x')\""),
            PROTECTED
        ),
        "fs.copyFileSync 首参为受保护文件即入候选"
    );
}

// ---------------------------------------------------------------------------
// 多路径 / 引号族
// ---------------------------------------------------------------------------

#[test]
fn 多路径按命令出现序返回且不折叠() {
    assert_eq!(
        targets("node -e \"fs.writeFileSync('a',''); fs.unlinkSync('b')\""),
        vec!["a".to_owned(), "b".to_owned()],
        "按命令出现序返回（不折叠）"
    );
    assert_eq!(
        targets("node -e \"fs.writeFileSync('p',''); fs.appendFileSync('p','')\""),
        vec!["p".to_owned(), "p".to_owned()],
        "同路径两 API → 两条（不去重）"
    );
}

#[test]
fn 引号族与特殊字符路径原样提取() {
    for command in [
        "node -e \"fs.writeFileSync('p','x')\"",
        "node -e \"fs.writeFileSync(\\\"p\\\",\\\"x\\\")\"",
        "node -e \"fs.writeFileSync('中文 路径🚀/a.md','x')\"",
        "node -e \"fs.writeFileSync('D:\\\\ws\\\\a.md','x')\"",
    ] {
        let extracted = targets(command);
        assert_eq!(extracted.len(), 1, "单调用单候选，命令: {command}");
        assert!(
            extracted[0].contains('p')
                || extracted[0].contains("中文")
                || extracted[0].contains("D:"),
            "路径原文（含空格 / 中文 / emoji / 反斜杠 / 转义引号）原样返回，实际: {extracted:?}"
        );
    }
    // 模板串（反引号）不在关联式引号面（`'` / `"` 两族），按未提取收敛
    assert!(
        targets("node -e \"fs.writeFileSync(`p`)\"").is_empty(),
        "反引号模板串不在关联式引号面（既定漏面，不误判）"
    );
}

// ---------------------------------------------------------------------------
// 精度：共现不误拦 / 裸脚本放行 / 变量中转
// ---------------------------------------------------------------------------

#[test]
fn 共现式文本不误拦() {
    assert!(
        targets(&format!(
            "node -e \"console.log('writeFileSync','{PROTECTED}')\""
        ))
        .is_empty(),
        "API 名与路径实参未绑定（日志文本）→ 零候选"
    );
    assert!(
        targets("node -e \"const s='open(p,\\\"w\\\")'\"").is_empty(),
        "引号内文本不是真实调用 → 零候选"
    );
}

#[test]
fn 裸脚本与模块入口放行() {
    for command in [
        "node scripts/setup.js /ws/openspec/config.json",
        "python tools/gen.py",
        "node -m pkg",
        "python -m mod",
        "node -c a.js",
        "node --check a.js",
    ] {
        assert!(
            targets(command).is_empty(),
            "脚本内容不在命令行文本内（已知漏面留痕），命令: {command}"
        );
    }
}

#[test]
fn 嵌套shell包裹形态一并覆盖() {
    // 关联式不依赖脚本段定位与行首命令名，故嵌套形态同拦
    assert!(
        contains_trimmed(
            "bash -c 'node -e \"require(\\\"fs\\\").rmSync(\\\"p\\\")\"'",
            "p"
        ),
        "嵌套 bash -c + rmSync 提取候选"
    );
    assert!(
        contains(
            "pwsh -Command \"node -e \\\"fs.writeFileSync('p','')\\\"\"",
            "p"
        ),
        "pwsh -Command 包裹同提取"
    );
    assert!(
        contains("bash -c \"python -c \\\"open('p','w')\\\"\"", "p"),
        "嵌套 python open 提取候选（关联式不依赖脚本段定位）"
    );
}

#[test]
fn 变量中转路径不提取() {
    assert!(
        targets(&format!(
            "node -e \"const p='{PROTECTED}'; fs.writeFileSync(p,'')\""
        ))
        .is_empty(),
        "无字符串实参（变量中转）→ 零候选（承认漏面）"
    );
    assert!(
        targets(&format!("python -c \"p='{PROTECTED}'; open(p,'w')\"")).is_empty(),
        "PY 变量中转同（承认漏面）"
    );
}

// ---------------------------------------------------------------------------
// 畸形与边界
// ---------------------------------------------------------------------------

#[test]
fn 畸形输入零候选且不panic() {
    for command in ["", "   ", "''", "\"", "node -e \"fs.writeFileSync('p"] {
        assert!(
            targets(command).is_empty(),
            "畸形 / 空 / 未闭合引号零候选，命令: {command:?}"
        );
    }
}

#[test]
fn 超长命令尾部写api仍被提取() {
    let command = format!(
        "node -e \"{}{}\"",
        "y".repeat(100_000),
        "fs.writeFileSync('p','')"
    );
    assert_eq!(
        targets(&command),
        vec!["p".to_owned()],
        "100 KB 命令尾部 1 处写 API 仍提取（不设长度上限）"
    );
}

#[test]
fn 非ascii路径正确提取() {
    assert_eq!(
        targets("node -e \"fs.writeFileSync('中文🚀/a.md','x')\""),
        vec!["中文🚀/a.md".to_owned()],
        "中文与 emoji 路径原样提取、不 panic"
    );
    assert_eq!(
        targets("python -c \"open('目录/文件🚀.md','w')\""),
        vec!["目录/文件🚀.md".to_owned()],
        "PY 侧非 ASCII 同样正确"
    );
}
