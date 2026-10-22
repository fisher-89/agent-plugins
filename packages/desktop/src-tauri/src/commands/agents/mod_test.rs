//! `commands::agents` 七管理命令的单元测试 + 「命令面 → 全局库持久化」组合
//! 关系（AC-2 / AC-3 承载；AC-4 / AC-5 / AC-9 命令面半边回溯）。
//!
//! 装置沿 workspaces/mod_test.rs 既有先例：`#[tauri::command]` 保留原函数可
//! 直调，以 `tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）manage
//! 真实 `WorkspaceStores`（tempdir 真开全局库）后经 `app.state()` 取 State，
//! 不启动真实 Tauri runtime。命令体恒经 `WorkspaceStores::global()` 真实组合
//! store crate（文件系统 / db 不 mock，引用阻止场景以真实 agent 记录引用
//! 构造）；组合链（save → list → set_default → default 解析 → 重开一致）
//! 在本文件承载，不设独立组合测试区。

use tauri::{App, Manager};

use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, WorkspaceStores, CLI_TENANT_RETIRED,
};

use super::{
    delete_agent_instance, delete_agent_provider, list_agent_instances, list_agent_providers,
    save_agent_instance, save_agent_provider, set_default_agent_instance,
};

// ---------------------------------------------------------------------------
// 装置：tempdir 数据根 + mock app 托管真实 WorkspaceStores
// ---------------------------------------------------------------------------

/// 数据根临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    data_dir: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("agents-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        Self { data_dir }
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库——agent 管理七命令的数据面）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

/// 三档模型 fixture（三档可区分值，供逐字段断言）。
fn fixture_tiers() -> AgentModelTiers {
    AgentModelTiers {
        high: "m-high".to_owned(),
        medium: "m-medium".to_owned(),
        low: "m-low".to_owned(),
    }
}

/// 摘 Err(String)（`AgentProviderRecord` 刻意不派生 Debug，expect_err 不可用，
/// 统一以 match 摘取）。
fn err_of<T>(result: Result<T, String>) -> String {
    match result {
        Err(message) => message,
        Ok(_) => panic!("期待 Err，实际 Ok"),
    }
}

/// save_agent_provider 新建形态直调（id None）。
fn save_new_provider(
    state: &tauri::State<'_, WorkspaceStores>,
    name: &str,
    api_key: &str,
) -> Result<AgentProviderRecord, String> {
    save_agent_provider(
        state.clone(),
        None,
        name.to_owned(),
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        api_key.to_owned(),
        fixture_tiers(),
        None,
        None,
    )
}

/// save_agent_provider 更新形态直调（id Some，保留 name / base_url）。
fn save_provider_with_key(
    state: &tauri::State<'_, WorkspaceStores>,
    id: i64,
    name: &str,
    api_key: &str,
) -> Result<AgentProviderRecord, String> {
    save_agent_provider(
        state.clone(),
        Some(id),
        name.to_owned(),
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        api_key.to_owned(),
        fixture_tiers(),
        None,
        None,
    )
}

/// save_agent_instance 直调（provider_id 直通）。
fn save_agent(
    state: &tauri::State<'_, WorkspaceStores>,
    id: Option<i64>,
    name: &str,
    engine: AgentEngineKind,
    provider_id: Option<i64>,
) -> Result<AgentInstanceRecord, String> {
    save_agent_instance(state.clone(), id, name.to_owned(), engine, provider_id)
}

// ---------------------------------------------------------------------------
// 管理命令 → 全局库持久化（AC-2 组合链）：save → list → save(Some) → delete
// → WorkspaceStores 重开后状态一致
// ---------------------------------------------------------------------------

#[test]
fn 管理命令组合链save新建_list呈现_save更新_delete删除_重开后状态一致() {
    let env = Env::new("combined-chain");
    let list_snapshot;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();

        // save_agent_provider(None) 新建
        let created =
            save_new_provider(&state, "组合端点", "sk-live-1234567890").expect("新建应成功");
        assert_eq!(created.id, 1, "命令面新建经 global() 写事务内 max+1 分配");
        // list_agent_providers 清单呈现
        let listed = list_agent_providers(state.clone()).expect("list 应成功");
        assert_eq!(listed, vec![created.clone()], "清单呈现落库记录");

        // save_agent_provider(Some) 更新整行替换
        let updated = save_provider_with_key(&state, created.id, "组合端点", "sk-updated-999")
            .expect("更新应成功");
        assert_eq!(updated.id, created.id, "更新 id 不变");
        assert_eq!(updated.api_key, "sk-updated-999", "整行替换生效");

        // delete_agent_provider 删除
        let hit = delete_agent_provider(state.clone(), created.id).expect("删除应成功");
        assert!(hit, "命中删除返回 true");
        list_snapshot = list_agent_providers(state.clone()).unwrap();
        assert!(list_snapshot.is_empty(), "删除后清单为空");

        // list_agent_instances 同 list 模板（空库空数组）
        assert!(list_agent_instances(state.clone()).unwrap().is_empty());

        // tauri 托管值生命周期长于 App（redb 文件锁不随 App drop 释放）：
        // 显式取回注册表销毁，重开同一数据根才能拿到文件锁
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
    }

    // WorkspaceStores 重开后状态一致（命令面经 global() 真实组合 store crate）
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    assert_eq!(
        list_agent_providers(state.clone()).unwrap(),
        list_snapshot,
        "重开后 provider 清单与删除后状态一致"
    );
    assert!(list_agent_instances(state.clone()).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// save_agent_provider 留空 key 回填（AC-2）：id 存在且入参 api_key 空串 →
// 读存量记录回填原值（留空 = 保持原值）
// ---------------------------------------------------------------------------

#[test]
fn save_agent_provider_id存在且api_key空串_读存量回填原值() {
    let env = Env::new("keep-key");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let created =
        save_new_provider(&state, "保原值端点", "sk-live-1234567890").expect("新建应成功");

    // 编辑态 api_key 恒空提交：落库 key 与存量逐字段一致
    let saved =
        save_provider_with_key(&state, created.id, "保原值端点", "").expect("留空 key 提交应成功");

    assert_eq!(saved.id, created.id);
    assert_eq!(
        saved.api_key, "sk-live-1234567890",
        "留空 = 保持原值（命令层读存量回填，store 恒收全字段）"
    );
    // 落库复核
    assert_eq!(
        list_agent_providers(state.clone()).unwrap()[0].api_key,
        "sk-live-1234567890"
    );
}

#[test]
fn save_agent_provider_id存在且api_key非空_直接落新值_新建传空key空值落库() {
    let env = Env::new("key-boundary");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let created = save_new_provider(&state, "边界端点", "sk-live-1234567890").expect("新建应成功");

    // id 存在且 api_key 非空：显式换 key 不误回填
    let replaced = save_provider_with_key(&state, created.id, "边界端点", "sk-new-777")
        .expect("显式换 key 应成功");
    assert_eq!(replaced.api_key, "sk-new-777", "非空直接落新值");

    // id=None 新建传空 key：空值落库（新建语义无原值可保）
    let blank_new = save_new_provider(&state, "空key端点", "").expect("空 key 新建应成功");
    assert_eq!(blank_new.api_key, "", "新建空 key 原样落库");
}

// ---------------------------------------------------------------------------
// provider 命令校验与删除语义（AC-2 / AC-5）
// ---------------------------------------------------------------------------

#[test]
fn provider命令_重名保存err清单不变_更新miss_id_err_name空白err() {
    let env = Env::new("provider-reject");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let created = save_new_provider(&state, "占名端点", "sk-live-1234567890").expect("新建应成功");

    // 重名保存 reject：Err 且清单不变
    let dup = save_new_provider(&state, "占名端点", "sk-live-1234567890");
    let err = err_of(dup);
    assert!(err.contains("占名端点"), "错误串含 name 语境，实际: {err}");
    let listed = list_agent_providers(state.clone()).unwrap();
    assert_eq!(listed, vec![created], "清单不变（失败无副作用）");

    // 更新 miss id：Err（store 行存在校验统一报错）
    let miss = save_provider_with_key(&state, 999, "幽灵端点", "sk-live-1234567890");
    let err = err_of(miss);
    assert!(err.contains("999"), "错误串含 id 语境，实际: {err}");

    // name 空白：Err
    let blank = save_agent_provider(
        state.clone(),
        None,
        "   ".to_owned(),
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        fixture_tiers(),
        None,
        None,
    );
    assert!(blank.is_err(), "name 空白应 Err");
}

#[test]
fn provider删除语义_命中oktrue_被引用err透传_miss幂等okfalse_空库list空数组() {
    let env = Env::new("provider-delete");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // 空库 list 空数组
    assert!(
        list_agent_providers(state.clone()).unwrap().is_empty(),
        "list_agent_providers 空库空数组"
    );

    // miss 幂等 Ok(false)
    assert!(
        !delete_agent_provider(state.clone(), 999).unwrap(),
        "delete miss 幂等返回 false"
    );

    // 命中 Ok(true)
    let provider = save_new_provider(&state, "可删端点", "sk-live-1234567890").expect("新建应成功");
    assert!(delete_agent_provider(state.clone(), provider.id).unwrap());

    // 被引用 Err 透传（引用阻止场景以真实 agent 记录引用构造）
    let referenced =
        save_new_provider(&state, "被引用端点", "sk-live-1234567890").expect("新建应成功");
    let agent = save_agent(
        &state,
        None,
        "引用方实例",
        AgentEngineKind::Sdk,
        Some(referenced.id),
    )
    .expect("sdk agent 新建应成功");
    let err = delete_agent_provider(state.clone(), referenced.id).expect_err("被引用删除应 Err");
    assert!(
        err.contains("引用方实例"),
        "引用阻止 Err 含引用方 name 提示，实际: {err}"
    );
    // 两类记录均原样（不级联不静默删除）
    assert_eq!(list_agent_providers(state.clone()).unwrap().len(), 1);
    assert_eq!(list_agent_instances(state.clone()).unwrap(), vec![agent]);
}

// ---------------------------------------------------------------------------
// agent 命令 engine 约束（AC-3 / AC-5）：sdk 必填与拒 Cli 校验单点在 store，
// 命令层零校验逻辑、Err(String) 透传
// ---------------------------------------------------------------------------

#[test]
fn agent命令_sdk新建携provider_ok_薄包装透传store落库记录() {
    let env = Env::new("agent-engine-ok");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // sdk 引擎选存量 provider：命令体平参直入 store（薄包装不加工）
    let provider = save_new_provider(&state, "引擎端点", "sk-live-1234567890").expect("新建应成功");
    let sdk = save_agent(
        &state,
        None,
        "sdk实例",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("sdk 新建应成功");
    assert_eq!(sdk.provider_id, Some(provider.id));
    assert!(!sdk.is_default, "新建恒非默认（默认标记唯一写口）");
    assert_eq!(
        list_agent_instances(state.clone()).unwrap(),
        vec![sdk],
        "清单透传落库记录（薄包装不加工）"
    );
}

/// 预置存量 cli 实例行（native_db 裸构造绕过 store 写入口拒面——命令面「存量
/// 行更新拒」透传链的被测前提数据形态；须在 `app_with_stores` 之前落盘）。
fn preset_cli_row(env: &Env, id: i64, name: &str, is_default: bool) {
    let path = env.data_dir.path().join("desktop-global.redb");
    let mut models = native_db::Models::new();
    models
        .define::<AgentInstanceRecord>()
        .expect("定义 AgentInstanceRecord 失败");
    let db = native_db::Builder::new()
        .create(&models, &path)
        .expect("预置存量全局库失败");
    let rw = db.rw_transaction().expect("开启写事务失败");
    let mut instance = AgentInstanceRecord::new(name.to_owned(), AgentEngineKind::Cli, None);
    instance.id = id;
    instance.is_default = is_default;
    rw.insert(instance).expect("写入存量 cli 行失败");
    rw.commit().expect("提交预置事务失败");
}

#[test]
fn save_agent_instance拒cli透传_新建与更新均逐字退役文案() {
    let env = Env::new("agent-cmd-reject-cli");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let provider = save_new_provider(&state, "拒面端点", "sk-live-1234567890").expect("新建应成功");

    // engine: Cli 新建 → Err（store 单点校验透传不吞不改写；除 StoreError::Db
    // 通用前缀外逐字 CLI_TENANT_RETIRED，DD3）
    let err = err_of(save_agent(
        &state,
        None,
        "cli新实例",
        AgentEngineKind::Cli,
        None,
    ));
    assert_eq!(
        err,
        format!("db: {CLI_TENANT_RETIRED}"),
        "命令面 Err 为 store 错误串原样透传，实际: {err}"
    );

    // engine: Cli 更新形态同拒（携存量 id 同样先过入参口）
    let created = save_agent(
        &state,
        None,
        "sdk在库",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("sdk 新建应成功");
    let err = err_of(save_agent(
        &state,
        Some(created.id),
        "更新为cli",
        AgentEngineKind::Cli,
        None,
    ));
    assert_eq!(err, format!("db: {CLI_TENANT_RETIRED}"), "更新臂同文案透传");

    // 拒后零写入：清单仅 sdk 在库行
    assert_eq!(
        list_agent_instances(state.clone()).unwrap(),
        vec![created],
        "失败无副作用：清单不变"
    );
}

#[test]
fn save_agent_instance存量cli行更新拒_含改engine整行透传store单点() {
    let env = Env::new("agent-cmd-reject-stored");
    preset_cli_row(&env, 7, "存量cli", false);
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let provider = save_new_provider(&state, "整行端点", "sk-live-1234567890").expect("新建应成功");

    // 存量 cli 行整行更新（含「改 engine 为 sdk」）→ 拒——命令面 → store
    // 双拒透传组合链贯通（AC-5 全链锚）
    let err = err_of(save_agent(
        &state,
        Some(7),
        "存量cli改sdk",
        AgentEngineKind::Sdk,
        Some(provider.id),
    ));
    assert_eq!(
        err,
        format!("db: {CLI_TENANT_RETIRED}"),
        "存量行口透传，实际: {err}"
    );

    // 拒后零写入：存量行原样可读（list 呈现），并可删（可读可删不可改）
    let listed = list_agent_instances(state.clone()).unwrap();
    assert_eq!(listed.len(), 1, "拒后零写入");
    assert_eq!(listed[0].engine, AgentEngineKind::Cli, "存量行原样");
    assert_eq!(listed[0].name, "存量cli", "存量行整行不可改");
    assert!(
        delete_agent_instance(state.clone(), 7).unwrap(),
        "存量 cli 行可删（可读可删不可改的删半边）"
    );
}

#[test]
fn set_default_agent_instance拒cli目标_逐字透传且零写入() {
    let env = Env::new("agent-cmd-reject-default");
    preset_cli_row(&env, 9, "默认cli", true);
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // Cli 目标 id → Err（store 单点校验透传），除 StoreError::Db 通用前缀外
    // 逐字 CLI_TENANT_RETIRED（DD3）
    let err = err_of(set_default_agent_instance(state.clone(), 9));
    assert_eq!(err, format!("db: {CLI_TENANT_RETIRED}"), "实际: {err}");

    // 零数据写入：存量默认 cli 不被顺手清（is_default 仍 true）
    let default = state
        .inner()
        .global()
        .default_agent_instance()
        .unwrap()
        .expect("默认解析应命中存量行");
    assert_eq!(default.id, 9, "存量默认 cli 实例原样");
    assert!(default.is_default, "不清 is_default（零数据写入）");
}

#[test]
fn save_agent边界_id零为新建_负数与i64max更新miss记因_name怪值透传() {
    let env = Env::new("agent-cmd-edge");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let provider = save_new_provider(&state, "边界端点", "sk-live-1234567890").expect("新建应成功");

    // id Some(0) 等价新建（命令体 id.unwrap_or(0) 平参口径零改动）
    let created = save_agent(
        &state,
        Some(0),
        "零号新建",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("Some(0) 新建应成功");
    assert!(created.id > 0, "Some(0) 走新建臂分配 id");

    // name 超长（>1000）/ 特殊字符（`\n` `\0` emoji）平参透传不加工
    for name in [
        format!("超长名{}", "名".repeat(1100)),
        "特殊\n名\u{0}😀".to_owned(),
    ] {
        let saved = save_agent(&state, None, &name, AgentEngineKind::Sdk, Some(provider.id))
            .expect("怪值 name 应成功");
        assert_eq!(saved.name, name, "name 平参透传不加工");
    }

    // 更新 miss（-1 / i64::MAX）→ 既有「agent 不存在」Err 透传携 id 语境
    for id in [-1_i64, i64::MAX] {
        let err = err_of(save_agent(
            &state,
            Some(id),
            "幽灵更新",
            AgentEngineKind::Sdk,
            Some(provider.id),
        ));
        assert!(err.contains(&id.to_string()), "Err 携 id 语境，实际: {err}");
    }
}

#[test]
fn agent命令_sdk缺provider与悬空引用reject_store校验单点透传() {
    let env = Env::new("agent-engine-reject");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // sdk 缺 provider：reject（store 校验单点透传，命令层不加重复校验）
    let missing = save_agent(&state, None, "sdk缺provider", AgentEngineKind::Sdk, None);
    let err = missing.expect_err("sdk 缺 provider 应 reject");
    assert!(
        err.contains("sdk缺provider"),
        "错误串含 name 语境，实际: {err}"
    );

    // 悬空引用（provider_id 指向不存在的 provider）：reject
    let dangling = save_agent(&state, None, "sdk悬空", AgentEngineKind::Sdk, Some(999));
    let err = dangling.expect_err("悬空引用应 reject");
    assert!(
        err.contains("999"),
        "错误串含 provider id 语境，实际: {err}"
    );

    assert!(
        list_agent_instances(state.clone()).unwrap().is_empty(),
        "失败无副作用：清单不变"
    );
}

// ---------------------------------------------------------------------------
// agent 命令删除与默认（AC-4 / AC-5）
// ---------------------------------------------------------------------------

#[test]
fn agent命令_删默认agent后default解析none_set_default标记即切换返回更新后记录_list恰一默认() {
    let env = Env::new("agent-default-cmd");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let provider = save_new_provider(&state, "默认端点", "sk-live-1234567890").expect("新建应成功");
    let first = save_agent(
        &state,
        None,
        "甲实例",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("新建应成功");
    let second = save_agent(
        &state,
        None,
        "乙实例",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("第二实例新建应成功");
    assert_ne!(first.id, second.id);

    // 标记默认 → 删除默认 agent → default 解析 None（store 同事务清标记）
    set_default_agent_instance(state.clone(), first.id).expect("标记默认应成功");
    assert!(delete_agent_instance(state.clone(), first.id).unwrap());
    let stores = state.inner();
    assert_eq!(
        stores.global().default_agent_instance().unwrap(),
        None,
        "删默认 agent 后 default_agent_instance 返回 None"
    );

    // set_default 标记即切换：返回更新后记录，list 复核全局恰一默认
    let switched = set_default_agent_instance(state.clone(), second.id).expect("标记即切换应成功");
    assert_eq!(switched.id, second.id);
    assert!(switched.is_default, "返回更新后记录");
    let listed = list_agent_instances(state.clone()).unwrap();
    let defaults: Vec<i64> = listed
        .iter()
        .filter(|record| record.is_default)
        .map(|record| record.id)
        .collect();
    assert_eq!(defaults, vec![second.id], "list 复核全局恰一默认");
}

#[test]
fn agent命令_set_default_miss与拒cli透传_删除miss幂等okfalse() {
    let env = Env::new("agent-cmd-miss");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let provider = save_new_provider(&state, "miss端点", "sk-live-1234567890").expect("新建应成功");
    let agent = save_agent(
        &state,
        None,
        "在库实例",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("新建应成功");

    // set_default miss id（0 / -1 / i64::MAX）：既有「agent 不存在」Err 透传
    for id in [0_i64, -1, i64::MAX] {
        let err = set_default_agent_instance(state.clone(), id).expect_err("miss id 应 Err");
        assert!(err.contains(&id.to_string()), "Err 携 id 语境，实际: {err}");
    }

    // delete miss 幂等 Ok(false)
    assert!(
        !delete_agent_instance(state.clone(), 999).unwrap(),
        "删除 miss 幂等返回 false"
    );

    // 既有默认标记不受 miss 影响（miss 前 set_default 一次复核）+ 已默认幂等透传
    set_default_agent_instance(state.clone(), agent.id).expect("标记应成功");
    let again = set_default_agent_instance(state.clone(), agent.id).expect("已默认幂等透传");
    assert!(again.is_default, "重复标记幂等仍为默认");
    let _ = set_default_agent_instance(state.clone(), 999);
    assert!(
        state
            .inner()
            .global()
            .default_agent_instance()
            .unwrap()
            .is_some(),
        "miss Err 后既有默认标记不变"
    );
}

// ---------------------------------------------------------------------------
// 错误串机密边界（AC-9）：全部 Err(String) 错误串不含 api_key 明文
// ---------------------------------------------------------------------------

#[test]
fn 全部命令面err错误串不含api_key明文_错误只含name与id语境() {
    let env = Env::new("err-secret");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    const SECRET: &str = "sk-live-secret-1234567890";

    let provider = save_new_provider(&state, "机密端点", SECRET).expect("新建应成功");
    save_agent(
        &state,
        None,
        "机密引用方",
        AgentEngineKind::Sdk,
        Some(provider.id),
    )
    .expect("sdk agent 新建应成功");

    // 收集全部 Err(String) 语境：重名 / 引用阻止 / 回填 miss / sdk 校验 / 默认 miss
    let errors: Vec<String> = vec![
        err_of(save_new_provider(&state, "机密端点", SECRET)),
        err_of(delete_agent_provider(state.clone(), provider.id)),
        err_of(save_provider_with_key(&state, 999, "机密端点", "")),
        err_of(save_agent(
            &state,
            None,
            "sdk缺provider",
            AgentEngineKind::Sdk,
            None,
        )),
        err_of(save_agent(
            &state,
            None,
            "sdk悬空",
            AgentEngineKind::Sdk,
            Some(999),
        )),
        err_of(set_default_agent_instance(state.clone(), 999)),
    ];
    assert_eq!(errors.len(), 6, "六类 Err 场景全部在收集面");

    for err in &errors {
        assert!(
            !err.contains(SECRET),
            "错误串不得含 api_key 明文，实际: {err}"
        );
    }
    // 错误只含 name / id / 计数语境（非空可读）
    assert!(errors.iter().all(|err| !err.is_empty()));
}

// ---------------------------------------------------------------------------
// save_agent_provider context_length 平参（AC-9 命令面半边）：留空 = None
// 未配置语义（MUST NOT 落 0 / 128000 字面）；与 api_key 回填语义正交
// ---------------------------------------------------------------------------

#[test]
fn save携context_length落库且重开一致() {
    let env = Env::new("ctx-window");
    let list_snapshot;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();

        // 新建携 Some(200_000)：命令面平参直入记录构造
        let created = save_agent_provider(
            state.clone(),
            None,
            "窗长端点".to_owned(),
            AgentProviderProtocol::Openai,
            "https://api.example.com/v1".to_owned(),
            "sk-live-1234567890".to_owned(),
            fixture_tiers(),
            Some(200_000),
            None,
        )
        .expect("新建应成功");
        assert_eq!(created.context_length, Some(200_000), "命令面平参落记录");

        // list 读回一致（组合链扩字段）
        let listed = list_agent_providers(state.clone()).expect("list 应成功");
        assert_eq!(listed[0].context_length, Some(200_000), "清单读回一致");

        // tauri 托管值生命周期：显式 unmanage 后重开
        list_snapshot = listed;
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
    }

    // WorkspaceStores 重开后一致（可空列落全局库持久化）
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    assert_eq!(
        list_agent_providers(state.clone()).unwrap(),
        list_snapshot,
        "重开后 context_length 与删除前状态一致"
    );
}

#[test]
fn save留空context_length为none不落0或128000字面() {
    let env = Env::new("ctx-blank");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // 留空提交 = None（未配置），非 0 非缺省字面
    let created = save_new_provider(&state, "留空端点", "sk-live-1234567890").expect("新建应成功");
    assert_eq!(created.context_length, None, "留空 = None 未配置语义");
    // 库中读回复核：无 0 / 128_000 字面
    let listed = list_agent_providers(state.clone()).unwrap();
    assert_eq!(listed[0].context_length, None, "库中无 0 / 128_000 字面");
}

#[test]
fn save编辑态context_length双向翻转按入参落库() {
    let env = Env::new("ctx-flip");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let created = save_new_provider(&state, "翻转端点", "sk-live-1234567890").expect("新建应成功");

    // None → Some(N)：编辑补窗长
    let with_window = save_agent_provider(
        state.clone(),
        Some(created.id),
        "翻转端点".to_owned(),
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        "".to_owned(),
        fixture_tiers(),
        Some(131_072),
        None,
    )
    .expect("补窗长编辑应成功");
    assert_eq!(
        with_window.context_length,
        Some(131_072),
        "None → Some(N) 按入参落库"
    );

    // Some(N) → None：编辑清窗长（留空 = 未配置）
    let cleared = save_agent_provider(
        state.clone(),
        Some(created.id),
        "翻转端点".to_owned(),
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        "".to_owned(),
        fixture_tiers(),
        None,
        None,
    )
    .expect("清窗长编辑应成功");
    assert_eq!(
        cleared.context_length, None,
        "Some(N) → None 按入参落库（可空列双向翻转）"
    );
}

#[test]
fn save_api_key回填与新context_length参正交() {
    let env = Env::new("ctx-key-orthogonal");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let created = save_new_provider(&state, "正交端点", "sk-live-1234567890").expect("新建应成功");

    // 编辑态 api_key 传空（回填原值）+ context_length 传 Some(N)：两语义互不干扰
    let saved = save_agent_provider(
        state.clone(),
        Some(created.id),
        "正交端点".to_owned(),
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        "".to_owned(),
        fixture_tiers(),
        Some(200_000),
        None,
    )
    .expect("编辑应成功");

    assert_eq!(
        saved.api_key, "sk-live-1234567890",
        "api_key 留空回填原值语义不变"
    );
    assert_eq!(
        saved.context_length,
        Some(200_000),
        "context_length 无回填语义（有值即落，与 api_key 区分）"
    );
}

/// 留空 api_key 更新：回填存量原值（不覆写为空串——密钥一次填写后，前端
/// 后续更新不回传原文的契约半边）。
#[test]
fn save_agent_provider留空key更新回填原值不落空串() {
    let env = Env::new("key-backfill");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let created = save_new_provider(&state, "backfill-p", "secret-1").expect("新建应成功");
    let updated =
        save_provider_with_key(&state, created.id, &created.name, "").expect("留空 key 更新应成功");

    assert_eq!(
        updated.api_key, "secret-1",
        "空 key 回填原值（读存量记录半边）"
    );
    assert_ne!(updated.api_key, "", "不落空串");
    assert_eq!(
        list_agent_providers(state.clone())
            .expect("清单应成功")
            .into_iter()
            .find(|record| record.id == created.id)
            .expect("落库记录在场")
            .api_key,
        "secret-1",
        "回填值持久化（非仅返回面）"
    );
}

// ---------------------------------------------------------------------------
// save_agent_provider 保存期校验单点（AC-10）：模型名后缀复用引擎侧同一
// parse_model_spec（high → medium → low）+ max_tokens 下限 4096，错误串不含
// api_key；通过后落库（含 [1m] 原文与两新列）
// ---------------------------------------------------------------------------

/// 全参数直调（保存期校验面用例底座：三档 / 协议 / 双可空列全部显式）。
#[allow(clippy::too_many_arguments)]
fn save_full(
    state: &tauri::State<'_, WorkspaceStores>,
    id: Option<i64>,
    name: &str,
    protocol: AgentProviderProtocol,
    api_key: &str,
    high: &str,
    medium: &str,
    low: &str,
    context_length: Option<u64>,
    max_tokens: Option<u64>,
) -> Result<AgentProviderRecord, String> {
    save_agent_provider(
        state.clone(),
        id,
        name.to_owned(),
        protocol,
        "https://api.example.com/v1".to_owned(),
        api_key.to_owned(),
        AgentModelTiers {
            high: high.to_owned(),
            medium: medium.to_owned(),
            low: low.to_owned(),
        },
        context_length,
        max_tokens,
    )
}

#[test]
fn save合法形态_anthropic携后缀与下限上限逐字段保真且后缀原文落库() {
    let env = Env::new("save-legal");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let saved = save_full(
        &state,
        None,
        "合法端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-legal",
        "claude-sonnet-4-5[1m]",
        "claude-medium",
        "claude-haiku-4-5",
        Some(200_000),
        Some(4_096),
    )
    .expect("合法形态应保存成功");

    assert_eq!(saved.protocol, AgentProviderProtocol::Anthropic);
    assert_eq!(
        saved.max_tokens,
        Some(4_096),
        "恰下限放行（>= MIN_OUTPUT_TOKENS）"
    );
    assert_eq!(saved.context_length, Some(200_000));
    assert_eq!(
        saved.models.high, "claude-sonnet-4-5[1m]",
        "[1m] 原文原样落库（后缀摘除在引擎侧，保存期不归一化）"
    );
    // 回读一致性（真库往返）
    let listed = list_agent_providers(state.clone()).expect("list 应成功");
    assert_eq!(listed, vec![saved], "落库记录与返回记录一致");
}

#[test]
fn save未配置上限_两种协议均ok且不落缺省字面() {
    let env = Env::new("save-no-max-tokens");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let anthropic = save_full(
        &state,
        None,
        "未配置上限",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        "claude-x",
        "claude-m",
        "claude-y",
        Some(200_000),
        None,
    )
    .expect("未配置上限应保存成功");
    assert_eq!(
        anthropic.max_tokens, None,
        "留空 = 未配置（存 null，不落 32000）"
    );

    // openai 协议同样不因协议差异拒收（字段照记，R10 边界）
    let openai = save_full(
        &state,
        None,
        "openai 未配置上限",
        AgentProviderProtocol::Openai,
        "sk-live-1234567890",
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4o-mini",
        None,
        None,
    )
    .expect("openai 协议未配置上限应保存成功");
    assert_eq!(openai.max_tokens, None);
    assert_eq!(openai.context_length, None);
}

#[test]
fn save非法后缀按档报错_high_medium_low三档均被校验() {
    let env = Env::new("save-illegal-suffix-tiers");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let cases: [(&str, &str, &str, &str); 3] = [
        ("x[2m]", "m", "l", "high"),
        ("h", "m[2m]", "l", "medium"),
        ("h", "m", "l[2m]", "low"),
    ];
    for (high, medium, low, tier) in cases {
        let error = err_of(save_full(
            &state,
            None,
            "非法后缀端点",
            AgentProviderProtocol::Anthropic,
            "sk-ant-x",
            high,
            medium,
            low,
            None,
            None,
        ));
        assert!(
            error.contains(tier),
            "{tier} 档非法应点名档位，实际: {error}"
        );
        assert!(error.contains("[2m]"), "错误串含模型名原文，实际: {error}");
        assert!(
            error.contains("模型名后缀非法"),
            "复用引擎侧 ModelSpecError 原文（无第二套规则），实际: {error}"
        );
    }

    // 失败无副作用：清单仍空
    assert!(
        list_agent_providers(state.clone()).unwrap().is_empty(),
        "校验失败零落库"
    );
}

#[test]
fn save非法后缀形态表复用引擎侧同一规则() {
    let env = Env::new("save-illegal-suffix-forms");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    for raw in ["[8k]", "[]", "[ 1m ]", "x[1m]y", "[1m]", "[2m]"] {
        let error = err_of(save_full(
            &state,
            None,
            &format!("非法形态 {raw}"),
            AgentProviderProtocol::Anthropic,
            "sk-ant-x",
            raw,
            "m",
            "l",
            None,
            None,
        ));
        assert!(
            error.contains(raw),
            "{raw} 的 Err 串与原串逐字对应，实际: {error}"
        );
    }

    // 与引擎侧同一函数同一结论（无第二套规则）
    for raw in ["[8k]", "x[1m]y", "[]"] {
        assert!(
            matches!(
                agent_runtime::parse_model_spec(raw),
                Err(agent_runtime::ModelSpecError { .. })
            ),
            "引擎侧同一函数对 {raw} 同判 Err"
        );
    }
}

#[test]
fn save上限下限_低于4096拒收且恰下限放行() {
    let env = Env::new("save-min-output");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    for tokens in [4_095_u64, 1, 0] {
        let error = err_of(save_full(
            &state,
            None,
            &format!("下限端点 {tokens}"),
            AgentProviderProtocol::Anthropic,
            "sk-ant-x",
            "claude-x",
            "m",
            "l",
            None,
            Some(tokens),
        ));
        assert!(
            error.contains("4096"),
            "下限 {tokens} 拒收且文案含 4096，实际: {error}"
        );
        assert!(
            error.contains(&tokens.to_string()),
            "文案含被拒取值，实际: {error}"
        );
    }

    // 恰下限放行
    let saved = save_full(
        &state,
        None,
        "下限放行端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        "claude-x",
        "m",
        "l",
        None,
        Some(4_096),
    )
    .expect("恰 4096 应保存成功");
    assert_eq!(saved.max_tokens, Some(4_096));
}

#[test]
fn save错误串机密边界_任一err不含api_key明文() {
    let env = Env::new("save-secret-boundary");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    const CANARY: &str = "sk-ant-UNIQUE-LEAK-CANARY";

    let errors = vec![
        // 非法后缀（high 档）
        err_of(save_full(
            &state,
            None,
            "机密端点甲",
            AgentProviderProtocol::Anthropic,
            CANARY,
            "x[2m]",
            "m",
            "l",
            None,
            None,
        )),
        // 非法后缀（low 档）
        err_of(save_full(
            &state,
            None,
            "机密端点乙",
            AgentProviderProtocol::Anthropic,
            CANARY,
            "h",
            "m",
            "[1m]",
            None,
            None,
        )),
        // 上限过小
        err_of(save_full(
            &state,
            None,
            "机密端点丙",
            AgentProviderProtocol::Anthropic,
            CANARY,
            "h",
            "m",
            "l",
            None,
            Some(1),
        )),
        // store 侧拒绝（重名）
        {
            save_full(
                &state,
                None,
                "机密端点丁",
                AgentProviderProtocol::Anthropic,
                CANARY,
                "h",
                "m",
                "l",
                None,
                None,
            )
            .expect("首次保存应成功");
            err_of(save_full(
                &state,
                None,
                "机密端点丁",
                AgentProviderProtocol::Anthropic,
                CANARY,
                "h",
                "m",
                "l",
                None,
                None,
            ))
        },
    ];

    for error in errors {
        assert!(
            !error.contains(CANARY),
            "错误串不得含 api_key 明文，实际: {error}"
        );
        assert!(
            !error.contains("api_key"),
            "错误面只含档位名 / 模型名原文 / 数字下限，实际: {error}"
        );
    }
}

#[test]
fn save校验顺序_模型名先于上限且档序high先于low() {
    let env = Env::new("save-validation-order");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // ① 模型名非法 + 上限过小同时成立 → 返回模型名错误（① 先于 ②）
    let error = err_of(save_full(
        &state,
        None,
        "双非法端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        "x[2m]",
        "m",
        "l",
        None,
        Some(1),
    ));
    assert!(
        error.contains("模型名后缀非法") && !error.contains("4096"),
        "模型名校验先于上限下限，实际: {error}"
    );

    // ② high 与 low 双非法 → 先报 high（档序 high → medium → low）
    let error = err_of(save_full(
        &state,
        None,
        "双档非法端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        "high[2m]",
        "m",
        "low[2m]",
        None,
        None,
    ));
    assert!(
        error.contains("high") && error.contains("high[2m]") && !error.contains("low[2m]"),
        "档序 high 先行，实际: {error}"
    );

    // ③ 校验前置不产生库 IO 副作用
    assert!(
        list_agent_providers(state.clone()).unwrap().is_empty(),
        "失败后清单计数恒 0（无部分写入）"
    );
}

#[test]
fn save空模型名不新增拒收面_齐备性归启动期() {
    let env = Env::new("save-empty-model");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // 三档全空：Ok（齐备性由启动期 ConfigMissing 承担）
    let all_blank = save_full(
        &state,
        None,
        "全空档端点",
        AgentProviderProtocol::Openai,
        "sk-live-1234567890",
        "",
        "",
        "",
        None,
        None,
    )
    .expect("三档全空应保存成功（不新增拒收面）");
    assert_eq!(
        all_blank.models,
        AgentModelTiers {
            high: String::new(),
            medium: String::new(),
            low: String::new(),
        }
    );

    // 单档空 + 其余带后缀：Ok
    let single_blank = save_full(
        &state,
        None,
        "单空档端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        "",
        "claude-m[1m]",
        "claude-y",
        None,
        None,
    )
    .expect("单档空应保存成功");
    assert_eq!(single_blank.models.high, "");
    assert_eq!(single_blank.models.medium, "claude-m[1m]");
}

#[test]
fn save重复同id同载荷幂等_行数不增且后缀不叠加() {
    let env = Env::new("save-upsert-idempotent");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let first = save_full(
        &state,
        None,
        "幂等端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-original",
        "claude-sonnet-4-5[1m]",
        "claude-m",
        "claude-haiku-4-5",
        Some(200_000),
        Some(8_192),
    )
    .expect("首次保存应成功");
    let count_after_first = list_agent_providers(state.clone()).unwrap().len();
    assert_eq!(count_after_first, 1, "首次保存落一行");

    // 同 id 同载荷再存（写面为 upsert 整行替换，非 append）
    let second = save_full(
        &state,
        Some(first.id),
        "幂等端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-original",
        "claude-sonnet-4-5[1m]",
        "claude-m",
        "claude-haiku-4-5",
        Some(200_000),
        Some(8_192),
    )
    .expect("二次保存应成功");
    assert_eq!(second.id, first.id, "id 逐字相同（无新 id）");
    assert_eq!(second.protocol, first.protocol);
    assert_eq!(second.max_tokens, first.max_tokens);
    assert_eq!(
        second.models.high, "claude-sonnet-4-5[1m]",
        "后缀原文不叠加成 [1m][1m]"
    );
    assert_eq!(second.base_url, first.base_url);
    assert_eq!(second.name, first.name);
    assert_eq!(second.context_length, first.context_length);
    let listed = list_agent_providers(state.clone()).unwrap();
    assert_eq!(listed.len(), 1, "行数不增（无重复行）");
    assert_eq!(listed[0].id, first.id, "无新 id 分配");

    // 第三次把 api_key 留空：回填原值，仍一行
    let third = save_full(
        &state,
        Some(first.id),
        "幂等端点",
        AgentProviderProtocol::Anthropic,
        "",
        "claude-sonnet-4-5[1m]",
        "claude-m",
        "claude-haiku-4-5",
        Some(200_000),
        Some(8_192),
    )
    .expect("留空 key 再存应成功");
    assert_eq!(third.api_key, first.api_key, "回填原值后与首次逐字相同");
    assert_eq!(list_agent_providers(state.clone()).unwrap().len(), 1);
}

#[test]
fn save数值与字符串边界_上限不设拒收且双可空列独立组合() {
    let env = Env::new("save-numeric-boundary");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // max_tokens = u64::MAX：上限不设拒收
    let huge = save_full(
        &state,
        None,
        "极端上限端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        "claude-x",
        "m",
        "l",
        Some(u64::MAX),
        Some(u64::MAX),
    )
    .expect("上限极大应保存成功");
    assert_eq!(huge.max_tokens, Some(u64::MAX));
    assert_eq!(huge.context_length, Some(u64::MAX));

    // 双可空列四态独立组合（index 序号区分 name）
    for (index, (context_length, max_tokens)) in [
        (None, None),
        (Some(131_072), None),
        (None, Some(4_096)),
        (Some(200_000), Some(32_000)),
    ]
    .into_iter()
    .enumerate()
    {
        let saved = save_full(
            &state,
            None,
            &format!("双空列端点 {index}"),
            AgentProviderProtocol::Anthropic,
            "sk-ant-x",
            "claude-x",
            "m",
            "l",
            context_length,
            max_tokens,
        )
        .expect("双可空列组合应保存成功");
        assert_eq!(saved.context_length, context_length);
        assert_eq!(saved.max_tokens, max_tokens);
    }

    // 超长模型名（>1000 字符）带后缀 + emoji / \0 / 换行：Ok 且落库保真
    let long_model = format!("{}[1m]", "m".repeat(1_200));
    let special = save_full(
        &state,
        None,
        "怪值模型端点",
        AgentProviderProtocol::Anthropic,
        "sk-ant-x",
        &long_model,
        "claude-\u{0}-m",
        "claude\n-y🎉",
        None,
        Some(4_096),
    )
    .expect("怪值模型名应保存成功");
    assert_eq!(special.models.high, long_model, "超长模型名原文保真");
    assert_eq!(special.models.medium, "claude-\u{0}-m");
    assert_eq!(special.models.low, "claude\n-y🎉");
}

#[test]
fn 组合保存清单重开库_协议与上限与后缀原文三面一致() {
    let env = Env::new("save-list-reopen");
    let saved;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();
        saved = save_full(
            &state,
            None,
            "重开端点",
            AgentProviderProtocol::Anthropic,
            "sk-ant-reopen",
            "claude-sonnet-4-5[1m]",
            "claude-m[1m]",
            "claude-haiku-4-5",
            Some(200_000),
            Some(8_192),
        )
        .expect("保存应成功");

        // 清单回读逐字一致
        let listed = list_agent_providers(state.clone()).expect("list 应成功");
        assert_eq!(listed, vec![saved.clone()], "清单回读逐字一致");
        assert_eq!(listed[0].protocol, AgentProviderProtocol::Anthropic);
        assert_eq!(listed[0].max_tokens, Some(8_192));
        assert_eq!(listed[0].models.high, "claude-sonnet-4-5[1m]");

        // 显式取回托管值销毁（redb 文件锁不随 App drop 释放）
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
    }

    // WorkspaceStores 重开后三面仍一致（v3 版本链顺带落地）
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let reopened = list_agent_providers(state.clone()).expect("重开后 list 应成功");
    assert_eq!(reopened, vec![saved], "重开后记录逐字一致");
    assert_eq!(reopened[0].max_tokens, Some(8_192), "上限列持久");
    assert_eq!(
        reopened[0].models.medium, "claude-m[1m]",
        "后缀原文持久（不做归一化）"
    );
}
