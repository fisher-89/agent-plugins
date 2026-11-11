//! `checklist.rs`（静态清单表解析器）的单元测试：三条解析规则的正向 / 边界 /
//! 反例面——段落边界（只扫 `## 静态清单` 起的段落）、行首 `| <ID> |` 形态约束
//!（两侧空格 + `[A-Z]+[0-9]+`）、出现序去重，以及第二列「检查项」权威正文取值。
//!
//! 纯 `&str` 扫描的纯函数，全部输入为测试体内内存字符串字面量；零文件 IO、
//! 零网络、零子进程、零环境变量，故不设 Mock。

use crate::model::checklist::{checklist_entries, ChecklistEntry};

/// ID 序列投影（用例判据面：集合 / 顺序断言的主读数）。
fn ids(prompt: &str) -> Vec<String> {
    checklist_entries(prompt)
        .into_iter()
        .map(|entry| entry.id)
        .collect()
}

/// 三列清单表段（`## 静态清单` 标题 + 表头 + 分隔行 + 逐行表项）。
fn section(rows: &[(&str, &str, &str)]) -> String {
    let mut text = String::from("## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n");
    for (id, item, basis) in rows {
        text.push_str(&format!("| {id} | {item} | {basis} |\n"));
    }
    text
}

// ---------------------------------------------------------------------------
// 正向：三列解析与第二列权威正文
// ---------------------------------------------------------------------------

#[test]
fn 正向三列解析_id按出现序且item取第二列() {
    let prompt = section(&[
        ("A1", "检查项甲", "判断依据甲"),
        ("A2", "检查项乙", "判断依据乙"),
        ("A3", "检查项丙", "判断依据丙"),
    ]);
    let entries = checklist_entries(&prompt);
    assert_eq!(entries.len(), 3, "三行表项逐行收入，实际: {entries:?}");
    assert_eq!(
        ids(&prompt),
        vec!["A1".to_owned(), "A2".to_owned(), "A3".to_owned()],
        "ID 按出现序"
    );
    assert_eq!(
        entries[0].item, "检查项甲",
        "第二列逐字（不含首尾空格、不含第三列）"
    );
    assert_eq!(entries[2].item, "检查项丙", "第三行第二列逐字");
}

#[test]
fn 第二列权威正文_第三列不参与且含中文标点时逐字保真() {
    let prompt = section(&[(
        "C1",
        "变更代码中无安全漏洞",
        "检查：SQL注入、XSS…（全角括号）",
    )]);
    let entries = checklist_entries(&prompt);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].item, "变更代码中无安全漏洞",
        "第三列（判断依据）不进入 item"
    );
    assert!(
        !entries[0].item.contains("SQL"),
        "第三列内容零外溢: {:?}",
        entries[0].item
    );

    // 第二列内嵌空格 / 全角括号 / 中文标点逐字保真，前后多余空格被 trim
    let prompt = section(&[("C1", "  含 空格 与（全角）：逐字  ", "依据")]);
    assert_eq!(
        checklist_entries(&prompt)[0].item,
        "含 空格 与（全角）：逐字",
        "仅 trim 首尾空格，正文内空格保真"
    );
}

// ---------------------------------------------------------------------------
// 边界：段落边界（规则 ①）
// ---------------------------------------------------------------------------

#[test]
fn 段落边界_标题前与下一个二级标题后均不收() {
    let prompt = format!(
        "## 前置附录\n\n| Z9 | 段外项 | 依据 |\n\n{}\n## 附录\n\n| Y1 | 段后项 | 依据 |\n",
        section(&[("A1", "段内项", "依据")])
    );
    let got = ids(&prompt);
    assert_eq!(got, vec!["A1".to_owned()], "仅静态清单段内被收集");
    assert!(!got.contains(&"Z9".to_owned()), "标题前段落零命中");
    assert!(!got.contains(&"Y1".to_owned()), "下一个二级标题后零命中");
}

#[test]
fn 段落边界_三态表驱动() {
    // 形态一：标题前的同名形态表项
    let before = format!(
        "## 概述\n\n| Z9 | 段外项 | 依据 |\n\n{}",
        section(&[("A1", "段内项", "依据")])
    );
    assert_eq!(ids(&before), vec!["A1".to_owned()], "标题前表项不收");

    // 形态二：段落之后（第二个二级标题起）的表项
    let after = format!(
        "{}\n## 附录\n\n| Y1 | 段后项 | 依据 |\n| Y2 | 段后项二 | 依据 |\n",
        section(&[("A1", "段内项", "依据")])
    );
    assert_eq!(ids(&after), vec!["A1".to_owned()], "标题后表项不收");

    // 形态三：`## 静态清单` 为最后一段（无后续 `## `）→ 正常收，不 panic
    let tail = section(&[("B1", "末段项", "依据"), ("B2", "末段项二", "依据")]);
    assert_eq!(
        ids(&tail),
        vec!["B1".to_owned(), "B2".to_owned()],
        "段落无上界时照常收"
    );
}

#[test]
fn 三级子表零命中_仿测试设计评估器合成样本() {
    let prompt = format!(
        "{}\n### T8 格式检查详细项\n\n| ID | 子项 | 判定 |\n|---|---|---|\n\
         | T8-U1 | 子项甲 | 判定甲 |\n| T8-G1 | 子项乙 | 判定乙 |\n| T8-U2 | 子项丙 | 判定丙 |\n",
        section(&[("T1", "主清单甲", "依据甲"), ("T2", "主清单乙", "依据乙")])
    );
    let got = ids(&prompt);
    assert_eq!(got.len(), 2, "`###` 子表零收入，实际: {got:?}");
    assert_eq!(got, vec!["T1".to_owned(), "T2".to_owned()]);
    for fake in ["T8-U1", "T8-G1", "T8-U2"] {
        assert!(!got.contains(&fake.to_owned()), "子项 {fake} 零命中");
    }
    // 双保险反证：若放宽为无边界 `[A-Z]+[0-9]+` 全行扫描，子项会被切成假 ID
    let loose_hits = prompt.lines().filter(|line| line.contains("T8-")).count();
    assert_eq!(loose_hits, 3, "合成样本含三行子表（宽松扫描的假 ID 来源）");
}

// ---------------------------------------------------------------------------
// 边界：ID 形态（规则 ②）
// ---------------------------------------------------------------------------

#[test]
fn id形态边界_七拒二收() {
    let rejected = [
        "| t1 | 小写 | 依据 |",
        "| T1a | 尾随字母 | 依据 |",
        "| 1T | 首字符数字 | 依据 |",
        "| T | 无数字 | 依据 |",
        "| T1| 右无空格 | 依据 |",
        "|\tT1 \t| 制表符 | 依据 |",
        "> | T1 | 非行首 | 依据 |",
    ];
    for line in rejected {
        let prompt = format!("## 静态清单\n{line}\n");
        assert!(
            checklist_entries(&prompt).is_empty(),
            "形态 {line:?} 必须不收，实际: {:?}",
            ids(&prompt)
        );
    }

    let prompt = "## 静态清单\n| AB12 | 多字母前缀 | 依据 |\n| T1 | 单字母 | 依据 |\n";
    assert_eq!(
        ids(prompt),
        vec!["AB12".to_owned(), "T1".to_owned()],
        "多字母前缀与单字母两种合法形态均收"
    );
}

#[test]
fn 表头与分隔行与空行不收() {
    let prompt = "## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n\n说明性段落文字\n";
    assert!(
        checklist_entries(prompt).is_empty(),
        "表头 / 分隔行 / 空行 / 说明段均不收，实际: {:?}",
        ids(prompt)
    );
}

// ---------------------------------------------------------------------------
// 异常：出现序去重（规则 ③）
// ---------------------------------------------------------------------------

#[test]
fn 出现序去重_同id两次收首现且保序() {
    let prompt = section(&[
        ("C1", "正文A", "依据"),
        ("C2", "正文B", "依据"),
        ("C1", "正文A", "依据"),
    ]);
    let entries = checklist_entries(&prompt);
    assert_eq!(entries.len(), 2, "同 ID 去重，实际: {entries:?}");
    assert_eq!(
        entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        vec!["C1", "C2"],
        "首现位置保序"
    );

    // 同 ID 两次但第二列文本不同 → 保留首现正文（去重按 ID，不按整行）
    let prompt = section(&[
        ("C1", "首现正文", "依据甲"),
        ("C2", "另一项", "依据乙"),
        ("C1", "后现正文", "依据丙"),
    ]);
    assert_eq!(
        checklist_entries(&prompt)[0].item,
        "首现正文",
        "去重键是 ID，首现正文权威"
    );
}

// ---------------------------------------------------------------------------
// 边界：空与缺席 / 换行与特殊字符 / 规模
// ---------------------------------------------------------------------------

#[test]
fn 空输入与缺席四形态() {
    assert!(checklist_entries("").is_empty(), "空串 → 零项（不 panic）");
    let no_section = "这是一段很长的说明文本\n".repeat(100);
    assert!(
        checklist_entries(&no_section).is_empty(),
        "无 `## 静态清单` 标题 → 零项"
    );
    let title_only = "## 静态清单\n\n说明段落没有表行\n";
    assert!(
        checklist_entries(title_only).is_empty(),
        "标题在场但段内零表行 → 零项"
    );
    let title_last = "## 静态清单\n| A1 | 末段项 | 依据 |";
    assert_eq!(
        ids(title_last),
        vec!["A1".to_owned()],
        "标题为末段且无后续 `## ` → 正常收"
    );
}

#[test]
fn 换行与特殊字符四形态() {
    // 形态一：CRLF 行尾 → 与 LF 形态逐字段相等（`\r` 不进 item）
    let lf = section(&[("C1", "检查项甲", "依据甲"), ("C2", "检查项乙", "依据乙")]);
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(
        checklist_entries(&crlf),
        checklist_entries(&lf),
        "CRLF 与 LF 结果逐字段相等"
    );
    assert!(
        checklist_entries(&crlf)
            .iter()
            .all(|entry| !entry.item.contains('\r')),
        "`\\r` 不污染 item"
    );

    // 形态二：判断依据列含 `\0` 与 emoji → 不 panic，item 不受污染
    let special = section(&[("C1", "检查项甲", "依据\0含空字节🚀")]);
    let entries = checklist_entries(&special);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].item, "检查项甲", "第三列特殊字符零外溢");

    // 形态三：100k 字符超长 prompt（前段填充 + 段内 1000 行表）→ 恰 1000 项
    let mut huge = "填充".repeat(50_000);
    huge.push('\n');
    huge.push_str("## 静态清单\n");
    for index in 0..1000 {
        huge.push_str(&format!("| A{index} | 检查项 {index} | 依据 |\n"));
    }
    assert!(
        huge.chars().count() > 100_000,
        "前置：prompt 超长（实际 {} 字符）",
        huge.chars().count()
    );
    assert_eq!(
        checklist_entries(&huge).len(),
        1000,
        "超长 prompt 不 panic 且逐行收入"
    );

    // 形态四：emoji 出现在 ID 前的填充文本 → 段外文本零影响
    let emoji_noise = format!("🚀🎉 无关段落\n\n{}", section(&[("A1", "检查项", "依据")]));
    assert_eq!(ids(&emoji_noise), vec!["A1".to_owned()]);
}

#[test]
fn 幂等与确定性_两次调用逐字段相等() {
    let prompt = section(&[("R1", "项一", "依据一"), ("R2", "项二", "依据二")]);
    let first: Vec<ChecklistEntry> = checklist_entries(&prompt);
    let second = checklist_entries(&prompt);
    assert_eq!(first, second, "零内部状态：两次调用逐字段相等");

    // 「段落内 ID 集 == 结果 ID 集」：无静默丢项
    let expected: Vec<String> = prompt
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("| ")?;
            let id: String = rest
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                .collect();
            (id.starts_with(|c: char| c.is_ascii_uppercase()) && id.chars().any(char::is_numeric))
                .then_some(id)
        })
        .collect();
    assert_eq!(ids(&prompt), expected, "解析结果覆盖段落内全部合法 ID");
}
