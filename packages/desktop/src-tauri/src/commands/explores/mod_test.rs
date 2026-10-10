use std::fs;
use std::path::PathBuf;

use agent::{AgentEvent, AgentEventKind};
use tauri::{App, Manager};

use store::{AgentEngineKind, SessionConfigSnapshot, SessionRecord, WorkspaceStores};
use workflow::queries as queries_lib;

use super::{
    create_explore_record, delete_explore_record, explore_doc_path, list_explore_records,
    promote_explore_with, read_explore, rename_explore_record, scan_explores, update_explore_title,
};

// ---------------------------------------------------------------------------
// 装置：tempfile 数据根 + workspace 真实文件树
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("explore-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("explore-cmd-test-{tag}-ws-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    /// workspace root 字符串（explore 域 root 为不透明归属键，store/命令层均
    /// 不二次 canonicalize 归属键本体；for_root 的归一只作用于库文件派生）。
    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    /// 另一 workspace root（跨库隔离断言用）。
    fn other_root(&self) -> String {
        let dir = self.ws_root.path().join("other-ws");
        fs::create_dir_all(&dir).expect("创建另一 workspace 目录失败");
        dir.to_string_lossy().into_owned()
    }

    /// 笔记目录并写入一篇笔记（`explores_root/<name>.md`）。
    fn note(&self, name: &str, content: &str) -> PathBuf {
        let dir = foundation::layout::resolve(self.ws_root.path()).explores_root;
        fs::create_dir_all(&dir).expect("创建 explores 目录失败");
        let path = dir.join(format!("{name}.md"));
        fs::write(&path, content).expect("写笔记失败");
        path
    }

    /// 主仓初始化为含一个提交的 git 仓（promote 复用 create 的真实 git
    /// worktree 依赖；与产品硬依赖同口径）。
    fn as_git_repo(&self) {
        fs::write(self.ws_root.path().join("README.md"), "# 主仓夹具\n")
            .expect("写主仓初始文件失败");
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "desktop-test"],
            vec!["add", "README.md"],
            vec!["commit", "-m", "init"],
        ] {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(self.ws_root.path())
                .args(&args)
                .output()
                .expect("git 拉起失败");
            assert!(
                output.status.success(),
                "git {} 失败: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库；各 workspace 库经 for_root 惰性开）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

/// 建测用 app（promote 面）：除 WorkspaceStores 外另 manage 数据根（worktree
/// 落位派生注入面，与 main.rs setup 同构）。
fn app_with_data(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(env.data_dir.path().to_path_buf());
    app.manage(stores);
    app
}

/// 落一份 explore 归属会话行（source=explore，source_ref=记录 id 十进制串）。
fn seed_chat_session(store: &store::Store, id: &str, source_ref: Option<&str>) -> String {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: None,
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                model: Some("m-high".to_owned()),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
            },
            source: "explore".to_owned(),
            source_ref: source_ref.map(str::to_owned),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("create_session 应成功");
    id.to_owned()
}

/// 一条盖戳 Raw 事件（级联断言的最小载荷）。
fn raw_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: format!(r#"{{"type":"mystery","seq":{seq}}}"#),
        },
    )
}

// ---------------------------------------------------------------------------
// read_explore：薄包装不加工 + blank root / 防穿越边界
// ---------------------------------------------------------------------------

#[test]
fn read_explore有效root与已落盘笔记返回与workflow直查serde等值的doc() {
    let env = Env::new("read-passthrough");
    let _app = app_with_stores(&env); // manage WorkspaceStores：本命令无 State 入参，仅 app 存活期保库
    env.note("foo", "# 探索笔记\n全文内容");

    let via_command = read_explore(env.root(), "foo".to_owned()).expect("已落盘笔记应可读");
    let via_workflow =
        queries_lib::read_explore(&foundation::layout::resolve(env.ws_root.path()), "foo")
            .expect("workflow 直查应命中");

    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_workflow).unwrap(),
        "薄包装不加工：命令面与 workflow 直查 serde 等值"
    );
    assert_eq!(via_command.name, "foo");
    assert_eq!(via_command.content, "# 探索笔记\n全文内容");
}

#[test]
fn read_explore_blank_root返回none不panic() {
    let env = Env::new("read-blank");
    let _app = app_with_stores(&env);
    env.note("foo", "可读内容");

    assert_eq!(
        read_explore(String::new(), "foo".to_owned()),
        None,
        "空串 root 纪律"
    );
    assert_eq!(
        read_explore("   ".to_owned(), "foo".to_owned()),
        None,
        "空白 root 同口径"
    );
}

#[test]
fn read_explore穿越分量名在命令边界拒绝返回none() {
    let env = Env::new("read-traversal");
    let _app = app_with_stores(&env);
    env.note("x", "可读内容");

    for name in ["a/b", "..", "../x"] {
        assert_eq!(
            read_explore(env.root(), name.to_owned()),
            None,
            "敌意名 {name:?} 在命令边界拒绝（同 commands/changes 敌意 source 口径）"
        );
    }
}

// ---------------------------------------------------------------------------
// explore_doc_path：布局派生路径，无 IO
// ---------------------------------------------------------------------------

#[test]
fn explore_doc_path有效root与合法name返回与布局派生逐段相等的路径() {
    let env = Env::new("doc-path-hit");
    let _app = app_with_stores(&env);

    let via_command = explore_doc_path(env.root(), "foo".to_owned()).expect("合法名应派生路径");
    let expected = foundation::layout::resolve(env.ws_root.path())
        .explores_root
        .join("foo.md");

    assert_eq!(
        std::path::Path::new(&via_command)
            .components()
            .collect::<Vec<_>>(),
        expected.components().collect::<Vec<_>>(),
        "路径与 Layout::explores_root 派生结果逐段相等"
    );
}

#[test]
fn explore_doc_path穿越名与blank_root返回none() {
    let env = Env::new("doc-path-miss");
    let _app = app_with_stores(&env);

    assert_eq!(
        explore_doc_path(env.root(), "a/b".to_owned()),
        None,
        "穿越名拒绝（防穿越）"
    );
    assert_eq!(
        explore_doc_path(String::new(), "foo".to_owned()),
        None,
        "blank root 拒绝"
    );
}

// ---------------------------------------------------------------------------
// scan_explores：blank root + 求差组装（绑定过滤读 for_root 库）
// ---------------------------------------------------------------------------

#[test]
fn scan_explores_blank_root返回空数组不进入查询链路() {
    let env = Env::new("scan-blank");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    assert_eq!(
        scan_explores("   ".to_owned(), state).expect("blank root 空结果"),
        Vec::new(),
        "blank root 纪律：Ok(vec![])"
    );
}

#[test]
fn scan_explores求差结果与workflow扫描减workspace库绑定手工求差一致() {
    let env = Env::new("scan-diff");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    env.note("a", "# a");
    env.note("b", "# b");
    env.note("c", "# c");
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "b")
        .expect("建档应成功");

    let via_command = scan_explores(env.root(), state.clone()).expect("scan 应成功");

    let bound: Vec<String> = state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .list_explore_records(&env.root())
        .expect("store 清单应成功")
        .into_iter()
        .map(|record| record.name)
        .collect();
    let expected: Vec<String> =
        queries_lib::scan_explores(&foundation::layout::resolve(env.ws_root.path()))
            .into_iter()
            .filter(|entry| !bound.iter().any(|name| name == &entry.name))
            .map(|entry| entry.name)
            .collect();
    let actual: Vec<String> = via_command.into_iter().map(|entry| entry.name).collect();
    assert_eq!(
        actual, expected,
        "命令求差与「workflow 扫描 − workspace 库已绑定 stem」手工求差一致"
    );
}

// ---------------------------------------------------------------------------
// 集成关系 R1：workflow 扫描 → workspace 库清单求差 → scan_explores 命令
// ---------------------------------------------------------------------------

#[test]
fn 三文件一经绑定b后导入候选恰为未绑定的a与c() {
    let env = Env::new("r1-diff");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    env.note("a", "# a");
    env.note("b", "# b");
    env.note("c", "# c");
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "b")
        .expect("建档应成功");

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    let names: Vec<String> = scanned.into_iter().map(|entry| entry.name).collect();
    assert_eq!(names, vec!["a", "c"], "已绑定 stem 不再出现在导入清单");
}

#[test]
fn 全部文件已绑定时导入候选为空数组() {
    let env = Env::new("r1-all-bound");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    env.note("a", "# a");
    env.note("b", "# b");
    let ws = state.for_root(&env.root()).expect("for_root 应成功");
    ws.create_explore_record(&env.root(), "a")
        .expect("建档应成功");
    ws.create_explore_record(&env.root(), "b")
        .expect("建档应成功");

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    assert!(scanned.is_empty(), "全绑定 → 两集求差为空");
}

#[test]
fn 目录缺失且无绑定时导入候选为空数组两空集求差() {
    let env = Env::new("r1-both-empty");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    assert!(
        scanned.is_empty(),
        "目录缺失（扫描空）+ 无绑定（清单空）→ 空数组"
    );
}

#[test]
fn 绑定记录的文件已删孤儿仍被滤除不回入候选() {
    let env = Env::new("r1-orphan");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    env.note("a", "# a");
    let orphan_note = env.note("b", "# b");
    env.note("c", "# c");
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "b")
        .expect("建档应成功");
    fs::remove_file(&orphan_note).expect("删除磁盘文件失败（模拟孤儿）");

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    let names: Vec<String> = scanned.into_iter().map(|entry| entry.name).collect();
    assert_eq!(
        names,
        vec!["a", "c"],
        "求差以记录 name（非磁盘现状）为准：孤儿 b 仍被滤除、不回入候选"
    );
}

#[test]
fn 大小写差异文件按记录name精确求差不引入文件系统折叠() {
    let env = Env::new("r1-case");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    // 磁盘实际大小写 case-doc.md，记录名 casedoc（Windows 盘上不区分大小写
    // 但 fs 返回实际大小写）：精确求差不得引入大小写折叠
    env.note("case-doc", "# case");
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "casedoc")
        .expect("建档应成功");

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    let names: Vec<String> = scanned.into_iter().map(|entry| entry.name).collect();
    assert_eq!(
        names,
        vec!["case-doc"],
        "记录 casedoc ≠ stem case-doc：精确比对不滤除（不折叠）"
    );
}

// ---------------------------------------------------------------------------
// scan_explores 非 kebab 过滤（explore-name-file-binding）：命令层在未绑定过滤
// 之外再滤非 kebab stem（对齐 store 建档口径，避免「点击后报错」）
// ---------------------------------------------------------------------------

/// scan_explores 过滤非 kebab（边界）：笔记目录含非 kebab stem（大写 /
/// 下划线 / 前导数字）→ 不列入可绑定清单；已绑定过滤照常并存。
#[test]
fn scan_explores过滤非kebab_stem与已绑定过滤并存() {
    let env = Env::new("scan-kebab");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    env.note("valid-topic", "# 合法");
    env.note("BadCase", "# 大写");
    env.note("bad_case", "# 下划线");
    env.note("1bad", "# 前导数字");
    env.note("also-valid", "# 合法二");
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "also-valid")
        .expect("建档应成功");

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    let names: Vec<String> = scanned.into_iter().map(|entry| entry.name).collect();
    assert_eq!(
        names,
        vec!["valid-topic"],
        "非 kebab stem 不列入 + 已绑定 also-valid 滤除"
    );
}

// ---------------------------------------------------------------------------
// 记录面经 for_root 路由至所属 workspace 库（AC-2 / AC-5 回溯半边）
// ---------------------------------------------------------------------------

#[test]
fn create_explore_record携root经for_root落该workspace库() {
    let env = Env::new("route-create");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let record = create_explore_record(state.clone(), env.root(), "new-topic".to_owned())
        .expect("建档应成功");

    assert_eq!(record.root, env.root(), "归属 root 以入参落库");
    // 命令写后经 WorkspaceStores::for_root(root) 读回同记录（落该 workspace 库）
    let read_back = state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .list_explore_records(&env.root())
        .expect("读回应成功");
    assert_eq!(
        read_back,
        vec![record],
        "命令写经 for_root 落该 workspace 库"
    );
}

#[test]
fn rename保主键且delete级联名下runs与events在同一workspace库内收敛() {
    let env = Env::new("route-cascade");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let original =
        create_explore_record(state.clone(), env.root(), "topic".to_owned()).expect("建档应成功");
    // 名下归属会话（source=explore，source_ref=记录 id 十进制串）落同一 workspace 库
    {
        let ws = state.for_root(&env.root()).expect("for_root 应成功");
        let session_id =
            seed_chat_session(&ws, "ses-route-cascade", Some(&original.id.to_string()));
        ws.append_session_events(&session_id, &[raw_event(0)])
            .expect("append 应成功");
    }

    // rename：in-place 保主键（保 source_ref 链绑定）
    let renamed = rename_explore_record(
        state.clone(),
        env.root(),
        "topic".to_owned(),
        "renamed-topic".to_owned(),
    )
    .expect("改名应成功");
    assert_eq!(renamed.id, original.id, "in-place 改名保主键");

    // delete：级联名下 runs+events 在同一 workspace 库内收敛
    assert!(
        delete_explore_record(state.clone(), env.root(), "renamed-topic".to_owned())
            .expect("删除应成功"),
        "命中删除返回 true"
    );
    let ws = state.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        ws.list_session_events("ses-route-cascade")
            .unwrap()
            .is_empty(),
        "名下转录随记录级联删除（同库收敛）"
    );
    assert!(
        ws.find_session("ses-route-cascade").unwrap().is_none(),
        "归属会话随记录级联删除（同库收敛）"
    );
    assert!(
        ws.list_explore_records(&env.root()).unwrap().is_empty(),
        "记录已删除"
    );
}

#[test]
fn 两workspace同名建档各自独立_甲库建档后乙库清单为空() {
    let env = Env::new("route-cross-db");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root_a = env.root();
    let root_b = env.other_root();

    let a = create_explore_record(state.clone(), root_a.clone(), "same-topic".to_owned())
        .expect("A 库建档应成功");
    assert_eq!(
        list_explore_records(state.clone(), root_b.clone()).expect("B 库清单应成功"),
        Vec::new(),
        "A 库建档后 B 库清单为空（分库天然隔离）"
    );

    let b = create_explore_record(state.clone(), root_b.clone(), "same-topic".to_owned())
        .expect("B 库同名建档不冲突");
    assert_eq!(a.id, b.id, "两库同 id 并行（库域内自增）");
    let list_a = list_explore_records(state.clone(), root_a).expect("A 库清单应成功");
    let list_b = list_explore_records(state.clone(), root_b).expect("B 库清单应成功");
    assert_eq!(list_a, vec![a], "A 库恰本库一条");
    assert_eq!(list_b, vec![b], "B 库恰本库一条");
}

// ---------------------------------------------------------------------------
// list / create / rename / delete：store 薄包装 + blank root 纪律
// ---------------------------------------------------------------------------

#[test]
fn list_explore_records有两条记录返回与for_root直连serde等值的两条dto() {
    let env = Env::new("list-passthrough");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let ws = state.for_root(&env.root()).expect("for_root 应成功");
    let first = ws
        .create_explore_record(&env.root(), "alpha")
        .expect("建档应成功");
    let second = ws
        .create_explore_record(&env.root(), "beta")
        .expect("建档应成功");

    let via_command = list_explore_records(state.clone(), env.root()).expect("list 应成功");

    let via_store = state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .list_explore_records(&env.root())
        .expect("store 直连应成功");
    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与 store 直连 serde 等值"
    );
    let ids: Vec<i64> = via_command.iter().map(|record| record.id).collect();
    assert_eq!(ids, vec![first.id, second.id], "id 升序");
}

#[test]
fn list_explore_records_blank_root返回空数组() {
    let env = Env::new("list-blank");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "alpha")
        .expect("建档应成功");

    assert_eq!(
        list_explore_records(state, String::new()).expect("blank root 空结果"),
        Vec::new(),
        "blank root 纪律"
    );
}

#[test]
fn create_explore_record建档返回记录dto且list可见() {
    let env = Env::new("create-hit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let record = create_explore_record(state.clone(), env.root(), "new-topic".to_owned())
        .expect("建档应成功");

    assert_eq!(record.name, "new-topic");
    assert_eq!(
        record.root,
        env.root(),
        "归属 root 以入参（canonical root）落库"
    );
    let listed = list_explore_records(state.clone(), env.root()).expect("list 应成功");
    assert_eq!(listed, vec![record], "建档后 list 可见");
}

#[test]
fn create_explore_record重复名store错误透传为命令err字符串() {
    let env = Env::new("create-dup");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    create_explore_record(state.clone(), env.root(), "dup".to_owned()).expect("首次建档应成功");

    let err = create_explore_record(state, env.root(), "dup".to_owned())
        .expect_err("重复建档应 Err(String)");

    assert!(
        !err.is_empty(),
        "错误串可直抵前端（store 错误透传），实际: {err}"
    );
}

#[test]
fn 非法记录名建档在命令边界返回err() {
    let env = Env::new("create-invalid");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    for name in ["", "a/b", "a\\b"] {
        assert!(
            create_explore_record(state.clone(), env.root(), name.to_owned()).is_err(),
            "非法名 {name:?} 应 Err（单分量校验）"
        );
    }
    assert!(
        list_explore_records(state.clone(), env.root())
            .expect("list 应成功")
            .is_empty(),
        "全部拒绝：不产生任何记录"
    );
}

#[test]
fn blank_root纪律矩阵_list与scan空结果_create与rename与delete为err() {
    let env = Env::new("blank-matrix");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // 查询空结果语义
    assert_eq!(
        list_explore_records(state.clone(), String::new()).expect("list blank 空结果"),
        Vec::new()
    );
    assert_eq!(
        scan_explores(String::new(), state.clone()).expect("scan blank 空结果"),
        Vec::new()
    );
    // 写命令 Err 语义（写无空结果语义，无 panic 无错误弹窗）
    assert!(create_explore_record(state.clone(), String::new(), "x".to_owned()).is_err());
    assert!(
        rename_explore_record(
            state.clone(),
            "   ".to_owned(),
            "old".to_owned(),
            "new".to_owned()
        )
        .is_err(),
        "空白 root rename Err"
    );
    assert!(
        delete_explore_record(state.clone(), String::new(), "x".to_owned()).is_err(),
        "blank root delete Err"
    );
}

#[test]
fn rename_explore_record改名后返回更新记录且新名可寻址() {
    let env = Env::new("rename-hit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let original =
        create_explore_record(state.clone(), env.root(), "old".to_owned()).expect("建档应成功");

    let renamed = rename_explore_record(
        state.clone(),
        env.root(),
        "old".to_owned(),
        "new".to_owned(),
    )
    .expect("改名应成功");

    assert_eq!(renamed.id, original.id, "in-place 改名保主键（保链前提）");
    assert_eq!(renamed.name, "new");
    assert_eq!(
        state
            .for_root(&env.root())
            .expect("for_root 应成功")
            .find_explore_record(&env.root(), "new")
            .expect("find 应成功"),
        Some(renamed),
        "新名可寻址"
    );
}

#[test]
fn delete_explore_record删除已绑定记录返回true再删返回false幂等() {
    let env = Env::new("delete-hit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let note = env.note("gone", "# 内容");
    create_explore_record(state.clone(), env.root(), "gone".to_owned()).expect("建档应成功");

    let first =
        delete_explore_record(state.clone(), env.root(), "gone".to_owned()).expect("删除应成功");
    let second =
        delete_explore_record(state.clone(), env.root(), "gone".to_owned()).expect("再删应成功");

    assert!(first, "首次删除返回 true");
    assert!(!second, "miss 幂等返回 false");
    assert!(note.exists(), "删除记录不动磁盘文件（store 不触磁盘）");
}

/// 错误透传：root 不可寻址（`for_root` canonicalize 失败）→ 五命令显式
/// `Err` 记因（不 panic、不产半截结果）——命令层 `map_err` 透传半边（读 /
/// 写命令同口径）。
#[test]
fn root不可寻址_五命令err透传不panic() {
    let env = Env::new("for-root-err");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    // 数据根下不存在的 workspace 目录（canonicalize 失败 → for_root Err）
    let missing = env
        .data_dir
        .path()
        .join("absent-ws")
        .to_string_lossy()
        .into_owned();

    for err in [
        scan_explores(missing.clone(), state.clone()).expect_err("scan 应 Err"),
        list_explore_records(state.clone(), missing.clone()).expect_err("清单应 Err"),
        create_explore_record(state.clone(), missing.clone(), "topic".to_owned())
            .expect_err("建档应 Err"),
        rename_explore_record(
            state.clone(),
            missing.clone(),
            "a".to_owned(),
            "b".to_owned(),
        )
        .expect_err("改名应 Err"),
        delete_explore_record(state.clone(), missing.clone(), "a".to_owned())
            .expect_err("删除应 Err"),
    ] {
        assert!(
            err.contains("canonicalize") && err.contains("absent-ws"),
            "Err 记因 canonicalize 失败与 root 线索，实际: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// update_explore_title（explore-name-file-binding AC-5 命令半边）：title 回填
// 唯一写口，blank root / 空白 title 显式 Err
// ---------------------------------------------------------------------------

/// update_explore_title 正向（AC-5）：经 store.set_explore_title 回填 title，
/// 返回更新后记录（主键 / name 不变）。
#[test]
fn update_explore_title正向回填并返回更新记录() {
    let env = Env::new("update-title-ok");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let original =
        create_explore_record(state.clone(), env.root(), "topic-a".to_owned()).expect("建档应成功");

    let updated = update_explore_title(
        state.clone(),
        env.root(),
        "topic-a".to_owned(),
        "人类可读标题".to_owned(),
    )
    .expect("回填应成功");

    assert_eq!(updated.title, "人类可读标题", "返回更新后 title");
    assert_eq!(updated.id, original.id, "主键不变（保会话链）");
    assert_eq!(updated.name, original.name, "name 不变");
    assert_eq!(
        state
            .for_root(&env.root())
            .expect("for_root 应成功")
            .find_explore_record(&env.root(), "topic-a")
            .expect("find 应成功")
            .map(|record| record.title),
        Some("人类可读标题".to_owned()),
        "落库可寻址读出"
    );
}

/// update_explore_title 空白拒绝（异常）：blank root / 空白 title → 显式
/// `Err`（title 恒非空），记录零改动。
#[test]
fn update_explore_title空白root与空白title显式err() {
    let env = Env::new("update-title-blank");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let original =
        create_explore_record(state.clone(), env.root(), "topic-a".to_owned()).expect("建档应成功");

    for root in [String::new(), "   ".to_owned()] {
        let err =
            update_explore_title(state.clone(), root, "topic-a".to_owned(), "标题".to_owned())
                .expect_err("blank root 应 Err");
        assert_eq!(err, "非法 root: 不得为空白");
    }
    for title in ["", "   ", "\n\t"] {
        let err = update_explore_title(
            state.clone(),
            env.root(),
            "topic-a".to_owned(),
            title.to_owned(),
        )
        .expect_err("空白 title 应 Err");
        assert_eq!(err, "非法 title: 不得为空白", "title 恒非空（命令层拦截）");
    }
    assert_eq!(
        state
            .for_root(&env.root())
            .expect("for_root 应成功")
            .find_explore_record(&env.root(), "topic-a")
            .expect("find 应成功"),
        Some(original),
        "拒绝面零改动"
    );
}

// ---------------------------------------------------------------------------
// promote_explore（explore-name-file-binding AC-6 / AC-7 命令半边）：读笔记 →
// 读记录 → 写面 move → 打标四步；前置拒绝面显式 Err 且零副作用
//
// 覆盖边界（设计表「promote_explore 打标失败——写面成功后 mark_explore_promoted
// 注入 Err」一行）：④ 打标在进程内无确定性注入缝——`promote_explore_with` 的
// store 取自 `app.state::<WorkspaceStores>()`（具体类型，非 trait 缝），且 ② 与
// ④ 同源、同实例、同实参（root / name 逐字相同，`find_explore_record` 按
// (root, name) 幂等寻址），故 ② 通过后 ④ 的两条记录面拒绝（已 promoted / 记录
// miss）不可达；DB 层故障的可构造形态（文件 / 格式损坏）在链路起点 `for_root`
// 即短路，而④ 独有的写事务故障在单进程内无确定性诱发法。④ 的输入前置（写面成功
// 后记录仍在且未打标）与「笔记全文已在 change explore.md 留底」不变量由上方四步
// 成功用例断言，记录面两条拒绝由 store_test（mark_explore_promoted 已 promoted /
// miss）覆盖——不造假竞态用例。
// ---------------------------------------------------------------------------

/// promote_explore 四步成功（AC-6 命令层半边）：预置笔记与记录 → 读笔记命中
/// → 读记录未 promoted → 写面 move → `mark_explore_promoted(change_id)`；
/// 主仓笔记已删、worktree 内 explore.md = 笔记全文、记录保留且
/// `promoted_to == change.id`。
#[tokio::test]
async fn promote_explore四步成功_笔记移走记录保留打标() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("promote-ok");
    env.as_git_repo();
    let app = app_with_data(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let note = "# 重试策略\n\n- 线索一\n- 线索二";
    let note_path = env.note("api-retry", note);
    create_explore_record(state.clone(), root.clone(), "api-retry".to_owned()).expect("建档应成功");

    let outcome = promote_explore_with(app.handle().clone(), root.clone(), "api-retry".to_owned())
        .await
        .expect("promote 应成功");

    // ① 主仓笔记已删（move 半边）
    assert!(!note_path.exists(), "主仓 explores/api-retry.md 已删");
    // ② worktree 内 change explore.md = 笔记全文
    let worktree = vcs_runtime::worktree_dir(env.data_dir.path(), &root, "api-retry");
    let landed = foundation::layout::resolve(&worktree)
        .changes_root
        .join("api-retry")
        .join("explore.md");
    assert_eq!(
        fs::read_to_string(&landed).expect("读 worktree 内 explore.md 失败"),
        note,
        "worktree 内 explore.md 为笔记全文"
    );
    // ③ 记录保留且打标（MUST NOT 删记录——会话链保留）
    let record = state
        .for_root(&root)
        .expect("for_root 应成功")
        .find_explore_record(&root, "api-retry")
        .expect("find 应成功")
        .expect("记录保留（promote 是 move 而非登记）");
    assert_eq!(
        record.promoted_to.as_deref(),
        Some(outcome.change_id.as_str()),
        "promoted_to == change.id"
    );
    assert_eq!(record.title, "api-retry", "title 随记录保留（默认 = name）");
    assert_eq!(
        outcome.change_name, "api-retry",
        "change_name = explore.name"
    );
    // ④ change 建档在案（title 继承 explore.title）
    let change = state
        .for_root(&root)
        .expect("for_root 应成功")
        .find_change_record(&outcome.change_id)
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(change.name, "api-retry");
    assert_eq!(change.title, record.title, "title 继承 explore.title");
}

/// promote_explore blank root / 非 kebab 拒绝（异常）：显式 `Err` 且零读写
/// （笔记仍在、无 change 建档）。
#[tokio::test]
async fn promote_explore_blank_root与非kebab显式err零读写() {
    let env = Env::new("promote-reject");
    let app = app_with_data(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let note_path = env.note("valid-topic", "# 标题\n正文");
    create_explore_record(state.clone(), root.clone(), "valid-topic".to_owned())
        .expect("建档应成功");

    for root_arg in [String::new(), "   ".to_owned()] {
        let err = promote_explore_with(app.handle().clone(), root_arg, "valid-topic".to_owned())
            .await
            .expect_err("blank root 应 Err");
        assert_eq!(err, "非法 root: 不得为空白");
    }
    for name in ["BadCase", "bad_case", "1bad", "bad-name-"] {
        let err = promote_explore_with(app.handle().clone(), root.clone(), name.to_owned())
            .await
            .expect_err("非 kebab name 应 Err");
        assert!(
            err.contains("kebab-case"),
            "Err 归因 kebab 口径，实际: {err}"
        );
    }
    assert!(note_path.is_file(), "拒绝面零副作用（笔记仍在）");
    assert!(
        state
            .for_root(&root)
            .expect("for_root 应成功")
            .list_change_records()
            .expect("清单应成功")
            .is_empty(),
        "零 change 建档"
    );
}

/// promote_explore 笔记未落盘 / 空白拒绝（异常）：`read_explore` 返回 None 或
/// 空白 → 显式 `Err`，记录零改动。
#[tokio::test]
async fn promote_explore笔记未落盘或空白拒绝() {
    let env = Env::new("promote-no-note");
    let app = app_with_data(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    create_explore_record(state.clone(), root.clone(), "no-note".to_owned()).expect("建档应成功");

    // 未落盘
    let err = promote_explore_with(app.handle().clone(), root.clone(), "no-note".to_owned())
        .await
        .expect_err("未落盘应 Err");
    assert!(
        err.contains("尚未落盘") && err.contains("no-note"),
        "Err 携引导文案，实际: {err}"
    );

    // 落盘但内容空白
    env.note("no-note", "   \n\t");
    let err = promote_explore_with(app.handle().clone(), root.clone(), "no-note".to_owned())
        .await
        .expect_err("空白内容应 Err");
    assert!(err.contains("尚未落盘或内容为空白"), "实际: {err}");

    let record = state
        .for_root(&root)
        .expect("for_root 应成功")
        .find_explore_record(&root, "no-note")
        .expect("find 应成功")
        .expect("记录保留");
    assert_eq!(record.promoted_to, None, "记录零改动（未打标）");
    assert!(
        state
            .for_root(&root)
            .expect("for_root 应成功")
            .list_change_records()
            .expect("清单应成功")
            .is_empty(),
        "零 change 建档"
    );
}

/// promote_explore 已 promoted 拒绝（异常）：记录 promoted_to 非空 → 显式
/// `Err`（零重复搬移）。
#[tokio::test]
async fn promote_explore已promoted拒绝() {
    let env = Env::new("promote-again");
    let app = app_with_data(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let note_path = env.note("done-topic", "# 标题\n正文");
    create_explore_record(state.clone(), root.clone(), "done-topic".to_owned())
        .expect("建档应成功");
    state
        .for_root(&root)
        .expect("for_root 应成功")
        .mark_explore_promoted(&root, "done-topic", "chg-already")
        .expect("预置打标应成功");

    let err = promote_explore_with(app.handle().clone(), root.clone(), "done-topic".to_owned())
        .await
        .expect_err("已 promoted 应 Err");

    assert!(
        err.contains("已转变更") && err.contains("chg-already"),
        "Err 携既有 change id，实际: {err}"
    );
    assert!(note_path.is_file(), "笔记零改动");
    assert_eq!(
        state
            .for_root(&root)
            .expect("for_root 应成功")
            .find_explore_record(&root, "done-topic")
            .expect("find 应成功")
            .and_then(|record| record.promoted_to),
        Some("chg-already".to_owned()),
        "原打标值零改动"
    );
}
