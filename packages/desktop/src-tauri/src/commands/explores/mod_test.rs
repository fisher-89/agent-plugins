//! explore 命令轨道七命令的单元测试 + 「workflow 扫描 → store 清单求差 →
//! scan_explores 命令」集成关系（R1：求差 / 孤儿滤除 / 大小写精确比对）+
//! 记录面经 `for_root` 路由至所属 workspace 库的半边（AC-2 / AC-5 回溯：命令
//! 写读经 for_root 落该 workspace 库、跨库隔离、级联同库收敛）。
//!
//! `#[tauri::command]` 保留原函数可直调：以 `tauri::test::mock_app()`
//! （MockRuntime）manage 真实 `WorkspaceStores`（tempdir 真开全局库与各
//! workspace 库）后经 `app.state::<WorkspaceStores>()` 取 State；workspace
//! 磁盘为 tempfile 真实文件树。本轨道命令零子进程（无 PATH 隔离面）；
//! blank root 纪律与防穿越在命令边界断言。

use std::fs;
use std::path::PathBuf;

use agent::{AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus};
use tauri::{App, Manager};

use store::{AgentRunRecord, WorkspaceStores};
use workflow::queries as queries_lib;

use super::{
    create_explore_record, delete_explore_record, explore_doc_path, list_explore_records,
    read_explore, rename_explore_record, scan_explores,
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
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库；各 workspace 库经 for_root 惰性开）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

/// 带来源三元组的 running 形态 run 记录（id 由 begin 分配）。
fn chain_run(
    root: &str,
    prompt: &str,
    started_at: i64,
    source_ref: Option<&str>,
) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: prompt.to_owned(),
        cwd: root.to_owned(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Running,
        started_at,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: "explore".to_owned(),
        source_ref: source_ref.map(str::to_owned),
        parent_run_id: None,
    }
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
            "敌意名 {name:?} 在命令边界拒绝（同 commands/queries 敌意 source 口径）"
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
    // 磁盘实际大小写 CaseDoc.md，记录名小写 casedoc（Windows 盘上不区分大小写
    // 但 fs 返回实际大小写）：精确求差不得引入大小写折叠
    env.note("CaseDoc", "# case");
    state
        .for_root(&env.root())
        .expect("for_root 应成功")
        .create_explore_record(&env.root(), "casedoc")
        .expect("建档应成功");

    let scanned = scan_explores(env.root(), state).expect("scan 应成功");

    let names: Vec<String> = scanned.into_iter().map(|entry| entry.name).collect();
    assert_eq!(
        names,
        vec!["CaseDoc"],
        "记录 casedoc ≠ stem CaseDoc：精确比对不滤除（不折叠）"
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
        create_explore_record(state.clone(), env.root(), "话题".to_owned()).expect("建档应成功");
    // 名下会话链（source=explore，source_ref=记录 id 十进制串）落同一 workspace 库
    let run = {
        let ws = state.for_root(&env.root()).expect("for_root 应成功");
        let run = ws
            .begin_agent_run(&chain_run(
                &env.root(),
                "链上首轮",
                100,
                Some(&original.id.to_string()),
            ))
            .expect("begin 应成功");
        ws.append_agent_run_events(run.id, &[raw_event(0)])
            .expect("append 应成功");
        run
    };

    // rename：in-place 保主键（保 source_ref 链绑定）
    let renamed = rename_explore_record(
        state.clone(),
        env.root(),
        "话题".to_owned(),
        "改名话题".to_owned(),
    )
    .expect("改名应成功");
    assert_eq!(renamed.id, original.id, "in-place 改名保主键");

    // delete：级联名下 runs+events 在同一 workspace 库内收敛
    assert!(
        delete_explore_record(state.clone(), env.root(), "改名话题".to_owned())
            .expect("删除应成功"),
        "命中删除返回 true"
    );
    let ws = state.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        ws.list_agent_run_events(run.id).unwrap().is_empty(),
        "名下事件随记录级联删除（同库收敛）"
    );
    assert!(
        !ws.list_agent_runs()
            .unwrap()
            .iter()
            .any(|left| left.id == run.id),
        "名下 run 随记录级联删除（同库收敛）"
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

    let a = create_explore_record(state.clone(), root_a.clone(), "同名话题".to_owned())
        .expect("A 库建档应成功");
    assert_eq!(
        list_explore_records(state.clone(), root_b.clone()).expect("B 库清单应成功"),
        Vec::new(),
        "A 库建档后 B 库清单为空（分库天然隔离）"
    );

    let b = create_explore_record(state.clone(), root_b.clone(), "同名话题".to_owned())
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
