use std::path::PathBuf;

use store::{GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME};

use super::{exit_code, render, Probe, Summary, PROD_REJECT_MESSAGE, SELF_CHECK_ARG};

// ---------------------------------------------------------------------------
// 装置：摘要 / 事实 fixture（字段序即渲染键序）
// ---------------------------------------------------------------------------

/// 十键键序（设计口径，测试侧独立留档）。
const KEY_ORDER: [&str; 10] = [
    "mode",
    "home",
    "data_root",
    "data_root_source",
    "global_db",
    "global_db_open",
    "workspaces_dir",
    "workspaces_dir_exists",
    "workspace_count",
    "models",
];

/// 全字段已知的摘要（各用例按需改写单字段）。
fn summary() -> Summary {
    let home = PathBuf::from("home");
    let data_root = PathBuf::from("data-root");
    Summary {
        mode: "dev",
        home,
        data_root: data_root.clone(),
        source: "dev-default",
        global_db: data_root.join(GLOBAL_DB_FILE_NAME),
        global_db_open: true,
        workspaces_dir: data_root.join(WORKSPACES_DIR_NAME),
        workspaces_dir_exists: false,
        workspace_count: Some(0),
        models: Some(vec!["workspace".to_owned()]),
        error: None,
    }
}

/// 全通事实（零错误）。
fn probe_ok() -> Probe {
    Probe {
        global_db_open: true,
        workspace_count: Some(0),
        models: Some(vec!["workspace".to_owned()]),
        workspaces_dir_exists: false,
        error: None,
    }
}

/// 渲染文本的行键序。
fn keys_of(text: &str) -> Vec<&str> {
    text.lines()
        .map(|line| line.split('=').next().unwrap_or_default())
        .collect()
}

/// 某键的行值（`key=value` 的 `value` 部分；未知值即空串）。
fn value_of<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    text.lines().find_map(|line| line.strip_prefix(&prefix))
}

// ---------------------------------------------------------------------------
// SELF_CHECK_ARG / PROD_REJECT_MESSAGE：参数名与拒绝文案单点（AC-11 / AC-12）
// ---------------------------------------------------------------------------

#[test]
fn self_check_arg为双横线小写kebab参数名单点() {
    assert_eq!(SELF_CHECK_ARG, "--self-check", "参数名单点逐字");
    assert!(SELF_CHECK_ARG.starts_with("--"), "以 `--` 起手");
    assert!(!SELF_CHECK_ARG.contains('='), "不含 `=`（无内联值形态）");
    assert!(
        !SELF_CHECK_ARG.contains(char::is_whitespace),
        "不含空白（单 token）"
    );
    assert_eq!(SELF_CHECK_ARG.trim(), SELF_CHECK_ARG, "无前后空白");

    // `args().skip(1)` 语义：逐项与常量精确相等即命中
    let args = ["--self-check", "--root", "/tmp/data"];
    assert!(args.contains(&SELF_CHECK_ARG), "首参数精确命中");
    assert_eq!(args[0], SELF_CHECK_ARG);
}

#[test]
fn self_check_arg为精确匹配而非前缀或大小写宽容() {
    for other in [
        "--self-checkx",
        "--SELF-CHECK",
        "--self_check",
        "--self-check=1",
    ] {
        assert_ne!(
            other, SELF_CHECK_ARG,
            "常量不得被前缀 / 大小写 / 内联值命中"
        );
    }
    // 组合形态：`--self-check --root <path>` 只命中首参数（`--root` 被忽略）
    let args = ["--self-check", "--root", "/tmp/data"];
    let mut hits = 0;
    for arg in args {
        if arg == SELF_CHECK_ARG {
            hits += 1;
        }
    }
    assert_eq!(hits, 1, "组合参数中恰命中首参数（本期不支持 --root）");
}

#[test]
fn prod_reject_message逐字且要素齐备() {
    assert_eq!(
        PROD_REJECT_MESSAGE, "--self-check 仅在 dev 构建可用（当前为生产构建）。",
        "拒绝文案逐字（含全角括号与句末句号）"
    );
    assert!(
        PROD_REJECT_MESSAGE.contains(SELF_CHECK_ARG),
        "含参数名要素（指针可照抄）"
    );
    assert!(
        PROD_REJECT_MESSAGE.contains("生产构建"),
        "含「当前为生产构建」要素"
    );
    assert!(!PROD_REJECT_MESSAGE.is_empty(), "文案非空");
    assert!(!PROD_REJECT_MESSAGE.contains('\n'), "单行 stderr");
}

#[test]
fn prod_reject_message单行可读且无装饰() {
    assert!(
        !PROD_REJECT_MESSAGE.contains('['),
        "无 ANSI 转义序列（纯文本 stderr）"
    );
    assert_eq!(
        PROD_REJECT_MESSAGE.trim(),
        PROD_REJECT_MESSAGE,
        "无首尾空白"
    );
    assert!(
        PROD_REJECT_MESSAGE.chars().count() < 200,
        "单行长度可控（不被终端截断）"
    );
    assert!(
        PROD_REJECT_MESSAGE.ends_with('。'),
        "单句收尾即完整说明（不拼接第二句）"
    );
}

// ---------------------------------------------------------------------------
// render：十键键序与值面（AC-11 / AC-2 / AC-5 / AC-12）
// ---------------------------------------------------------------------------

#[test]
fn render十键键序固定且单行无装饰以单换行收尾() {
    let text = render(&summary());

    assert_eq!(keys_of(&text), KEY_ORDER.to_vec(), "十键键序即字段序");
    assert_eq!(text.matches('\n').count(), 10, "十行十换行，无 error 行");
    assert!(text.ends_with('\n'), "以换行收尾");
    assert!(!text.ends_with("\n\n"), "末尾恰一个换行");
    assert!(!text.contains('\u{1b}'), "无 ANSI 转义");
    for line in text.lines() {
        assert!(!line.is_empty(), "无空行");
        assert!(line.contains('='), "每行 `key=value` 单行形态：{line}");
    }
}

#[test]
fn render_mode两值原样透传() {
    for value in ["dev", "prod"] {
        let mut view = summary();
        view.mode = value;
        assert_eq!(value_of(&render(&view), "mode"), Some(value));
    }
}

#[test]
fn render_source三值原样透传且两两不等() {
    let mut seen: Vec<&str> = Vec::new();
    for value in ["prod", "dev-default", "env"] {
        let mut view = summary();
        view.source = value;
        let text = render(&view);
        assert_eq!(value_of(&text, "data_root_source"), Some(value));
        assert!(!seen.contains(&value), "三行值两两不等");
        seen.push(value);
    }
}

#[test]
fn render_global_db_open与workspaces_dir_exists双向断言() {
    for open in [true, false] {
        let mut view = summary();
        view.global_db_open = open;
        let want = if open { "ok" } else { "err" };
        assert_eq!(
            value_of(&render(&view), "global_db_open"),
            Some(want),
            "global_db_open 为 ok / err 双值"
        );
    }
    for exists in [true, false] {
        let mut view = summary();
        view.workspaces_dir_exists = exists;
        let want = if exists { "true" } else { "false" };
        assert_eq!(
            value_of(&render(&view), "workspaces_dir_exists"),
            Some(want),
            "workspaces_dir_exists 双向"
        );
    }
}

#[test]
fn render未知值与空清单均渲染空值但行仍在() {
    // 未知计数 / 未知清单
    let mut view = summary();
    view.workspace_count = None;
    view.models = None;
    let text = render(&view);
    assert_eq!(value_of(&text, "workspace_count"), Some(""), "未知计数空值");
    assert_eq!(value_of(&text, "models"), Some(""), "未知清单空值");
    assert_eq!(keys_of(&text), KEY_ORDER.to_vec(), "行仍在且键序不变");

    // 计数 0 与空清单（与 None 区分）
    let mut zero = summary();
    zero.workspace_count = Some(0);
    zero.models = Some(Vec::new());
    let text = render(&zero);
    assert_eq!(value_of(&text, "workspace_count"), Some("0"));
    assert_eq!(value_of(&text, "models"), Some(""));

    // 单元素清单
    let mut one = summary();
    one.models = Some(vec!["workspace".to_owned()]);
    assert_eq!(value_of(&render(&one), "models"), Some("workspace"));
}

#[test]
fn render_models以逗号连接且序不变() {
    let mut view = summary();
    view.models = Some(vec![
        "workspace".to_owned(),
        "agent_provider".to_owned(),
        "agent_instance".to_owned(),
    ]);
    assert_eq!(
        value_of(&render(&view), "models"),
        Some("workspace,agent_provider,agent_instance"),
        "无空格、序不变（与 list_models 注册表短名同形）"
    );

    // 1000 项大列表：不 panic、键序不变
    let mut big = summary();
    big.models = Some((0..1000).map(|index| format!("m{index}")).collect());
    let text = render(&big);
    assert_eq!(keys_of(&text), KEY_ORDER.to_vec(), "大列表不改键序");
    let models = value_of(&text, "models").expect("models 行在场");
    assert_eq!(models.split(',').count(), 1000, "1000 项全部出线");
    assert_eq!(models.split(',').next(), Some("m0"), "序不变");
}

#[test]
fn render_error行追加在末且有值才出线() {
    let mut failed = summary();
    failed.error = Some("数据根不可打开".to_owned());
    let text = render(&failed);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 11, "十键 + error 行");
    assert_eq!(lines[10], "error=数据根不可打开", "error 行在末位");
    assert_eq!(keys_of(&text)[10], "error", "末行为 error 键");
    assert!(!text.contains('—'), "无装饰分隔横幅");
    assert_eq!(
        value_of(&render(&summary()), "error"),
        None,
        "无错时无 error 行"
    );
    assert_eq!(
        render(&summary()).matches("error=").count(),
        0,
        "error= 零命中"
    );
}

#[test]
fn render路径成分取自store常量() {
    let view = summary();
    let text = render(&view);

    let expected_global_db = view.global_db.display().to_string();
    let expected_workspaces_dir = view.workspaces_dir.display().to_string();
    let expected_data_root = view.data_root.display().to_string();
    let expected_home = view.home.display().to_string();

    assert_eq!(
        value_of(&text, "global_db"),
        Some(expected_global_db.as_str()),
        "global_db = 数据根 + GLOBAL_DB_FILE_NAME"
    );
    assert_eq!(
        value_of(&text, "workspaces_dir"),
        Some(expected_workspaces_dir.as_str()),
        "workspaces_dir = 数据根 + WORKSPACES_DIR_NAME"
    );
    assert_eq!(
        value_of(&text, "data_root"),
        Some(expected_data_root.as_str()),
        "data_root 为入参原样"
    );
    assert_eq!(
        value_of(&text, "home"),
        Some(expected_home.as_str()),
        "home 为入参原样"
    );
    assert!(
        text.contains(GLOBAL_DB_FILE_NAME),
        "库文件名来自 store 公开常量（壳层零字面量）"
    );
    assert!(
        text.contains(WORKSPACES_DIR_NAME),
        "workspaces 子树名来自 store 公开常量"
    );
}

#[test]
fn render特殊字符与超长值不panic() {
    let long = "长".repeat(1200);
    let cases = [
        "中文错误：数据根不可用",
        "emoji🙂 错误",
        "含\u{0}字符",
        "含=等号与\n换行",
        long.as_str(),
    ];

    for error in cases {
        let mut view = summary();
        view.error = Some(error.to_owned());
        let text = render(&view);
        assert!(text.starts_with("mode="), "首键仍为 mode");
        assert!(text.contains(error), "值内特殊字符原样出线（不抛错）");
        assert!(
            text.trim_end().ends_with(error),
            "error 行仍在末尾（值内换行不改变键序）：{error}"
        );
    }

    // 特殊字符落在 models 元素内：末行为 models（error 为 None）
    let mut view = summary();
    view.models = Some(vec!["中文🙂".to_owned(), "a=b".to_owned(), long]);
    let text = render(&view);
    assert_eq!(
        text.lines().last().map(|line| line.starts_with("models=")),
        Some(true),
        "末行仍为 models"
    );
    assert!(text.contains("models=中文🙂,a=b,"));
}

// ---------------------------------------------------------------------------
// exit_code：0 / 1 映射与值域（AC-11 / AC-12）
// ---------------------------------------------------------------------------

#[test]
fn exit_code全通为0且与workspaces目录缺席无关() {
    let mut probe = probe_ok();
    probe.workspaces_dir_exists = false;
    assert_eq!(exit_code(&probe), 0, "workspaces/ 缺席是常态，不参与退出码");
    probe.workspaces_dir_exists = true;
    assert_eq!(exit_code(&probe), 0, "双向均 0（仅看 error）");
}

#[test]
fn exit_code有错为1且与其余字段无关() {
    for open in [true, false] {
        let probe = Probe {
            global_db_open: open,
            workspace_count: Some(3),
            models: Some(vec!["workspace".to_owned()]),
            workspaces_dir_exists: true,
            error: Some("redb: 文件损坏".to_owned()),
        };
        assert_eq!(exit_code(&probe), 1, "error 为 Some 即 1（不看其余字段）");
    }
}

#[test]
fn exit_code值域不含2且空串与超长错误串均判1() {
    let long = "错".repeat(1200);
    for error in ["", long.as_str(), "emoji🙂", "多行\n错误"] {
        let probe = Probe {
            error: Some(error.to_owned()),
            ..probe_ok()
        };
        let code = exit_code(&probe);
        assert_eq!(code, 1, "`Some` 即失败（不看内容）");
        assert_ne!(code, 2, "2 由 main 产出，不在本函数值域");
    }
    assert_eq!(exit_code(&probe_ok()), 0, "全通 0，值域仅 {{0, 1}}");
}

// ---------------------------------------------------------------------------
// Probe / Summary：字段序即键序 + run 的委托面钉住（AC-11 / AC-12）
// ---------------------------------------------------------------------------

#[test]
fn probe与summary字段序即键序且未知字段可留空() {
    // 全 None 事实：可构造且参与退出码（未知字段不必填）
    let unknown = Probe {
        global_db_open: false,
        workspace_count: None,
        models: None,
        workspaces_dir_exists: false,
        error: None,
    };
    assert_eq!(exit_code(&unknown), 0, "未知字段不构成失败");

    // 全 Some 事实：摘要按声明序投影，渲染键序与字段序一一对应
    let full = Probe {
        global_db_open: true,
        workspace_count: Some(2),
        models: Some(vec!["workspace".to_owned(), "session".to_owned()]),
        workspaces_dir_exists: true,
        error: Some("模型清单读取失败：锁冲突".to_owned()),
    };
    let mut view = summary();
    view.workspace_count = full.workspace_count;
    view.models = full.models.clone();
    view.global_db_open = full.global_db_open;
    view.workspaces_dir_exists = full.workspaces_dir_exists;
    view.error = full.error.clone();

    let text = render(&view);
    let keys = keys_of(&text);
    assert_eq!(
        &keys[..10],
        KEY_ORDER.as_slice(),
        "十键键序由类型形状保证（声明序 = 渲染序）"
    );
    assert_eq!(keys.get(10), Some(&"error"), "失败面追加 error 行在末位");
    assert_eq!(value_of(&text, "workspace_count"), Some("2"));
    assert_eq!(value_of(&text, "models"), Some("workspace,session"));
}

#[test]
fn run返回码语义由exit_code与render委托面钉住() {
    // 探库成功：无 error ⇒ 0，且摘要无 error 行
    let ok = probe_ok();
    assert_eq!(exit_code(&ok), 0, "run 的成功臂恒由 exit_code 产出 0");
    let mut view = summary();
    view.error = ok.error.clone();
    assert_eq!(render(&view).matches("error=").count(), 0);

    // 首错即停：error 有值 ⇒ 1，且 error 行在末位（可定位唯一原因）
    let failed = Probe {
        error: Some("全局库打开失败：redb 文件锁被占用".to_owned()),
        ..probe_ok()
    };
    assert_eq!(exit_code(&failed), 1, "run 的失败臂恒由 exit_code 产出 1");
    let mut view = summary();
    view.error = failed.error.clone();
    let text = render(&view);
    let last = text.lines().last().expect("至少一行");
    assert!(
        last.starts_with("error="),
        "失败原因唯一且可定位（不拼装多条原因）：{last}"
    );
    assert!(
        last.contains("全局库打开失败"),
        "错误串原样出线（不二次加工）"
    );
}
