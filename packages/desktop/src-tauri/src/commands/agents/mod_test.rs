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
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, WorkspaceStores,
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
        "https://api.example.com/v1".to_owned(),
        api_key.to_owned(),
        fixture_tiers(),
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
        "https://api.example.com/v1".to_owned(),
        api_key.to_owned(),
        fixture_tiers(),
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
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        fixture_tiers(),
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
// agent 命令 engine 约束（AC-3）：sdk 必填校验单点在 store，命令层透传
// ---------------------------------------------------------------------------

#[test]
fn agent命令_cli新建provider可空ok_sdk选存量provider_ok() {
    let env = Env::new("agent-engine-ok");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // cli 引擎 provider 可空
    let cli =
        save_agent(&state, None, "cli实例", AgentEngineKind::Cli, None).expect("cli 新建应成功");
    assert_eq!(cli.provider_id, None);
    assert!(!cli.is_default, "新建恒非默认（默认标记唯一写口）");

    // sdk 引擎选存量 provider
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
    assert_eq!(list_agent_instances(state.clone()).unwrap(), vec![cli, sdk]);
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
    let first = save_agent(&state, None, "甲实例", AgentEngineKind::Cli, None).expect("新建应成功");
    let second =
        save_agent(&state, None, "乙实例", AgentEngineKind::Cli, None).expect("第二实例新建应成功");
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
fn agent命令_set_default_miss_id_err_删除miss幂等okfalse() {
    let env = Env::new("agent-cmd-miss");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let agent =
        save_agent(&state, None, "在库实例", AgentEngineKind::Cli, None).expect("新建应成功");

    // set_default miss id：Err
    let err = set_default_agent_instance(state.clone(), 999).expect_err("miss id 应 Err");
    assert!(err.contains("999"), "错误串含 id 语境，实际: {err}");

    // delete miss 幂等 Ok(false)
    assert!(
        !delete_agent_instance(state.clone(), 999).unwrap(),
        "删除 miss 幂等返回 false"
    );

    // 既有默认标记不受 miss 影响（miss 前 set_default 一次复核）
    set_default_agent_instance(state.clone(), agent.id).expect("标记应成功");
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
            "https://api.example.com/v1".to_owned(),
            "sk-live-1234567890".to_owned(),
            fixture_tiers(),
            Some(200_000),
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
        "https://api.example.com/v1".to_owned(),
        "".to_owned(),
        fixture_tiers(),
        Some(131_072),
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
        "https://api.example.com/v1".to_owned(),
        "".to_owned(),
        fixture_tiers(),
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
        "https://api.example.com/v1".to_owned(),
        "".to_owned(),
        fixture_tiers(),
        Some(200_000),
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
