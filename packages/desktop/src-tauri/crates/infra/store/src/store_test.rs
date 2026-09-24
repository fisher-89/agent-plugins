//! `store` 的单元测试：open 打开流程 + workspace 三操作 + agent run
//! begin / finish / list 存量回归 + 事件类型化（`append_agent_run_events` /
//! `list_agent_run_events` 经 `AgentEvent` 构造）+ 信封 API（`list_models` /
//! `scan`）。tempdir 真开 db 文件（存储层不 mock）；全部断言经 `Store` 公共
//! API，内部协作（模型编解码、canonical 口径）由此间接覆盖。系统时钟不
//! mock：`added_at` / `started_at` 仅记录入库值，清单排序与获取时间无关。
//!
//! 存量 schema_version 轮账与 `user_*` 表名前缀用例随 META 轮账退役而废弃；
//! legacy 迁移全链路用例在 `migrate_test.rs`（集成关系 R1）。

use std::fs;
use std::path::{Path, PathBuf};

use agent::{AgentEvent, AgentEventKind};

use crate::{AgentRunRecord, Store, StoreError, WorkspaceRecord};

/// db 文件 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    db_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("store-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("store-test-{tag}-ws-"))
            .tempdir()
            .expect("创建 workspace 临时目录失败");
        Self { db_dir, ws_root }
    }

    /// 默认 db 文件路径（固定文件名，多次调用同值，支撑重开场景）。
    fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("test.redb")
    }

    /// 在 workspace 根下创建一个真实目录并返回路径（add 的入参目录）。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }
}

fn open_ok(path: &Path) -> Store {
    Store::open(path).unwrap_or_else(|e| panic!("open 应成功: {e}"))
}

fn add_ok(store: &Store, dir: &Path) -> WorkspaceRecord {
    store
        .add_workspace(dir)
        .unwrap_or_else(|e| panic!("add_workspace 应成功: {e}"))
}

// ---------------------------------------------------------------------------
// Store::open
// ---------------------------------------------------------------------------

#[test]
fn open对不存在的路径返回ok并创建db文件与父目录() {
    let env = Env::new("open-create");
    let db_path = env.db_dir.path().join("nested/sub/test.redb");
    assert!(!db_path.exists());

    let store = open_ok(&db_path);

    assert!(db_path.exists(), "db 文件被创建");
    assert!(db_path.parent().unwrap().is_dir(), "父目录被创建");
    // 空库可正常 list
    assert!(store.list_workspaces().unwrap().is_empty());
}

#[test]
fn 已有db文件再次open返回ok且此前写入的记录完整读回() {
    let env = Env::new("reopen");
    let dir = env.ws("persisted");

    let first = open_ok(&env.db_path());
    let record = add_ok(&first, &dir);
    drop(first);

    let second = open_ok(&env.db_path());
    let list = second.list_workspaces().unwrap();
    assert_eq!(list, vec![record], "重开同一 db 文件后记录仍在");
}

#[test]
fn 父路径被同名普通文件占据时open返回db错误不panic() {
    let env = Env::new("open-blocked");
    let blocker = env.db_dir.path().join("blocker");
    fs::write(&blocker, "普通文件占位").expect("写占位文件失败");

    let result = Store::open(&blocker.join("test.redb"));

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("create_dir_all 无法建目录应失败"),
    };
    assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
}

#[test]
fn 目标为损坏文件时open返回err不静默降级为空库() {
    let env = Env::new("open-corrupt");
    let corrupt = env.db_dir.path().join("corrupt.redb");
    let garbage = "这不是一个合法的 db 数据库文件。".repeat(32);
    fs::write(&corrupt, garbage).expect("写损坏文件失败");

    let result = Store::open(&corrupt);

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("非合法 db 文件必须报错，不得静默降级为空库"),
    };
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
}

// ---------------------------------------------------------------------------
// Store::add_workspace
// ---------------------------------------------------------------------------

#[test]
fn add新目录返回canonical完整路径name为目录名末段() {
    let env = Env::new("add-basic");
    let store = open_ok(&env.db_path());
    let dir = env.ws("alpha");

    let record = add_ok(&store, &dir);

    assert_eq!(
        record.root,
        dunce::canonicalize(&dir)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        "root 为 canonical 完整路径"
    );
    assert_eq!(record.name, "alpha", "name 为目录名最后一段");
    assert_eq!(store.list_workspaces().unwrap(), vec![record]);
}

#[test]
fn add大小写不同的等价路径仅一条且原记录原样返回保留首添added_at() {
    let env = Env::new("case-dedup");
    let store = open_ok(&env.db_path());
    let dir = env.ws("DedupMe");
    let first = add_ok(&store, &dir);

    // 同一目录、大小写不同的书写形态（Windows 盘上真实大小写归一）
    let flipped = Path::new(&first.root).with_file_name("dEDUPmE");
    let second = add_ok(&store, &flipped);

    assert_eq!(
        second, first,
        "等价路径去重为同一 canonical key，原记录原样返回"
    );
    let list = store.list_workspaces().unwrap();
    assert_eq!(list.len(), 1, "清单仅一条记录");
    assert_eq!(list[0].added_at, first.added_at, "added_at 保留首添值");
}

#[test]
fn add尾分隔符与正反斜杠混写路径与已存key去重为一条() {
    let env = Env::new("slash-dedup");
    let store = open_ok(&env.db_path());
    let dir = env.ws("SlashDir");
    let first = add_ok(&store, &dir);

    let trailing = format!("{}\\", first.root);
    let second = add_ok(&store, Path::new(&trailing));
    assert_eq!(second.root, first.root, "尾分隔符书写差异去重");

    let forward = first.root.replace('\\', "/");
    let third = add_ok(&store, Path::new(&forward));
    assert_eq!(third.root, first.root, "正反斜杠混写去重");

    assert_eq!(store.list_workspaces().unwrap().len(), 1, "清单仅一条记录");
}

#[test]
fn add目录名含空格中文emoji的路径name提取正确且记录往返无损() {
    let env = Env::new("unicode");
    let store = open_ok(&env.db_path());
    let dir = env.ws("my 项目 📁");

    let record = add_ok(&store, &dir);

    assert_eq!(record.name, "my 项目 📁", "name 提取正确");
    assert_eq!(
        record.root,
        dunce::canonicalize(&dir)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    );
    // 记录经模型编解码落库：读回与返回记录逐字段相等（往返无损）
    assert_eq!(store.list_workspaces().unwrap(), vec![record]);
}

#[test]
fn add不存在的目录返回canonicalize错误() {
    let env = Env::new("add-missing");
    let store = open_ok(&env.db_path());
    let missing = env.ws_root.path().join("no-such-dir");

    let err = store
        .add_workspace(&missing)
        .expect_err("不存在的目录应失败");

    assert!(
        matches!(err, StoreError::Canonicalize(_)),
        "变体为 Canonicalize，实际: {err:?}"
    );
    assert!(
        err.to_string().starts_with("canonicalize:"),
        "错误串以 canonicalize: 前缀，实际: {err}"
    );
}

#[test]
fn add空路径返回canonicalize错误不panic() {
    let env = Env::new("add-empty");
    let store = open_ok(&env.db_path());

    let err = store
        .add_workspace(Path::new(""))
        .expect_err("空路径应失败");

    assert!(
        err.to_string().starts_with("canonicalize:"),
        "错误串以 canonicalize: 前缀，实际: {err}"
    );
    assert!(
        store.list_workspaces().unwrap().is_empty(),
        "失败不产生记录"
    );
}

// ---------------------------------------------------------------------------
// Store::list_workspaces
// ---------------------------------------------------------------------------

#[test]
fn list按canonical_root字典序升序与添加顺序无关() {
    let env = Env::new("ordering");
    let store = open_ok(&env.db_path());
    // 刻意乱序 add：默认序不随添加（或打开）时间变化
    let rec_gamma = add_ok(&store, &env.ws("gamma"));
    let rec_alpha = add_ok(&store, &env.ws("alpha"));
    let rec_beta = add_ok(&store, &env.ws("beta"));

    let roots: Vec<String> = store
        .list_workspaces()
        .unwrap()
        .into_iter()
        .map(|r| r.root)
        .collect();

    assert_eq!(
        roots,
        vec![rec_alpha.root, rec_beta.root, rec_gamma.root],
        "表主键（canonical root）自然序升序，与添加顺序无关"
    );
}

#[test]
fn list顺序确定可复现不因读写抖动() {
    let env = Env::new("stable-order");
    let store = open_ok(&env.db_path());
    let rec_a = add_ok(&store, &env.ws("zzz-last"));
    let rec_b = add_ok(&store, &env.ws("aaa-first"));

    let first = store.list_workspaces().unwrap();
    let second = store.list_workspaces().unwrap();
    assert_eq!(first, second, "两次 list 顺序确定可复现");
    assert_eq!(
        first.iter().map(|r| r.root.clone()).collect::<Vec<_>>(),
        vec![rec_b.root, rec_a.root],
        "主键自然序稳定，先添的 zzz 不因晚读而置顶"
    );
}

#[test]
fn 空库list返回空向量不报错() {
    let env = Env::new("list-empty");
    let store = open_ok(&env.db_path());

    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Store::remove_workspace
// ---------------------------------------------------------------------------

#[test]
fn remove已存在key返回true且list不再含该项() {
    let env = Env::new("remove-hit");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("alpha"));

    let hit = store.remove_workspace(Path::new(&record.root)).unwrap();

    assert!(hit);
    assert!(
        store.list_workspaces().unwrap().is_empty(),
        "list 不再含该项"
    );
}

#[test]
fn remove未注册路径返回false且库内容不变() {
    let env = Env::new("remove-miss");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("registered"));
    let ghost = env.ws("ghost");

    let hit = store.remove_workspace(&ghost).unwrap();

    assert!(!hit, "未注册路径 remove 幂等 miss");
    assert_eq!(store.list_workspaces().unwrap(), vec![record], "库内容不变");
}

#[test]
fn remove大小写不同等价路径命中删除同一条无孤儿条目() {
    let env = Env::new("remove-case");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("CaseKey"));

    let flipped = Path::new(&record.root).with_file_name("cASEkEY");
    let hit = store.remove_workspace(&flipped).unwrap();

    assert!(hit, "等价路径命中同一条");
    assert!(store.list_workspaces().unwrap().is_empty(), "无孤儿条目");
}

#[test]
fn remove目录消失后回退匹配命中删除() {
    let env = Env::new("remove-vanish");
    let store = open_ok(&env.db_path());
    let dir = env.ws("gone");
    let record = add_ok(&store, &dir);
    fs::remove_dir_all(&dir).expect("删除目录失败");

    let hit = store.remove_workspace(Path::new(&record.root)).unwrap();

    assert!(hit, "残留清单项可被清理（D1 用户救济路径）");
    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// StoreError 与全链路
// ---------------------------------------------------------------------------

#[test]
fn store_error两变体display携带db与canonicalize前缀() {
    let db = StoreError::Db("打开失败".to_string());
    let canonicalize = StoreError::Canonicalize("无效路径".to_string());

    assert_eq!(db.to_string(), "db: 打开失败");
    assert_eq!(canonicalize.to_string(), "canonicalize: 无效路径");
}

#[test]
fn 全链路add_list_remove后重开同一db文件清单状态与各操作返回一致() {
    let env = Env::new("full-cycle");
    let (rec_a_root, snapshot) = {
        let store = open_ok(&env.db_path());
        let rec_a = add_ok(&store, &env.ws("alpha"));
        let rec_b = add_ok(&store, &env.ws("beta"));
        let list = store.list_workspaces().unwrap();
        let roots: Vec<String> = list.iter().map(|r| r.root.clone()).collect();
        assert_eq!(
            roots,
            vec![rec_a.root.clone(), rec_b.root],
            "默认序：alpha 字典序在前，与添加顺序无关（此处恰同序）"
        );
        (rec_a.root, list)
    };

    // drop 并重开同一 db 文件：清单状态与操作序列的最终状态一致
    let reopened = open_ok(&env.db_path());
    let after_reopen = reopened.list_workspaces().unwrap();
    assert_eq!(after_reopen, snapshot);

    // 再经一次 remove + 重开：删除同样持久化，仅剩 beta
    assert!(reopened.remove_workspace(Path::new(&rec_a_root)).unwrap());
    drop(reopened);
    let again = open_ok(&env.db_path());
    let remaining = again.list_workspaces().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].root, snapshot[1].root);
}

// ---------------------------------------------------------------------------
// agent 域：begin / finish / list_runs 存量回归（事件类型化用例由 test-gen
// 按 test-design 落位）
// ---------------------------------------------------------------------------

/// 构造一份 running 形态的 run 记录（id 由 begin 分配，入参不参与匹配）。
fn running_run(prompt: &str, started_at: i64) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: prompt.to_owned(),
        cwd: "C:\\ws\\demo".to_owned(),
        env: "default".to_owned(),
        permission_mode: "bypassPermissions".to_owned(),
        status: "running".to_owned(),
        started_at,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
    }
}

fn begin_ok(store: &Store, prompt: &str, started_at: i64) -> AgentRunRecord {
    store
        .begin_agent_run(&running_run(prompt, started_at))
        .unwrap_or_else(|e| panic!("begin_agent_run 应成功: {e}"))
}

#[test]
fn begin_agent_run空库首跑返回id为1且status为running且started_at落值() {
    let env = Env::new("agent-begin-first");
    let store = open_ok(&env.db_path());

    let record = begin_ok(&store, "首轮", 1727000000000);

    assert_eq!(record.id, 1, "空库首跑 max+1 分配 id=1");
    assert_eq!(record.status, "running", "落 running 行");
    assert_eq!(
        record.started_at, 1727000000000,
        "started_at 按调用方值落库"
    );
    assert_eq!(record.finished_at, None, "running 行无结束时间");
    // 清单可见 running 行
    assert_eq!(store.list_agent_runs().unwrap(), vec![record.clone()]);
}

#[test]
fn begin_agent_run连续begin时id严格递增() {
    let env = Env::new("agent-begin-incr");
    let store = open_ok(&env.db_path());

    let first = begin_ok(&store, "第一跑", 100);
    let second = begin_ok(&store, "第二跑", 200);
    let third = begin_ok(&store, "第三跑", 300);

    assert_eq!((first.id, second.id, third.id), (1, 2, 3), "max+1 严格递增");
}

#[test]
fn begin_agent_run传入记录的id字段不参与匹配以分配id落行为准() {
    let env = Env::new("agent-begin-id");
    let store = open_ok(&env.db_path());

    let mut requested = running_run("调用方自填 id", 1727000000000);
    requested.id = 999;
    let record = store.begin_agent_run(&requested).unwrap();

    assert_eq!(record.id, 1, "id 由写事务内 max+1 分配，入参 id 被覆盖");
    assert_eq!(store.list_agent_runs().unwrap()[0].id, 1, "以分配 id 落行");
}

#[test]
fn append空切片返回ok且不产生行() {
    let env = Env::new("agent-append-empty");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "空事件流", 1727000000000);

    store.append_agent_run_events(run.id, &[]).unwrap();

    assert!(store.list_agent_run_events(run.id).unwrap().is_empty());
}

#[test]
fn finish后整行替换为终态且list反映() {
    let env = Env::new("agent-finish");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "待收敛", 1727000000000);

    let mut finished = run.clone();
    finished.status = "completed".to_owned();
    finished.finished_at = Some(1727000001000);
    finished.num_turns = Some(4);
    finished.cost_usd = Some(0.5);
    finished.duration_ms = Some(999);
    finished.session_id = Some("s-1".to_owned());
    store
        .finish_agent_run(run.id, &finished)
        .unwrap_or_else(|e| panic!("finish 应成功: {e}"));

    let listed = store.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, "completed", "终态整行替换");
    assert_eq!(listed[0].num_turns, Some(4));
    assert_eq!(listed[0].cost_usd, Some(0.5));
    assert_eq!(listed[0].duration_ms, Some(999));
    assert_eq!(listed[0].session_id.as_deref(), Some("s-1"));
    assert_eq!(listed[0].finished_at, Some(1727000001000));
}

#[test]
fn list_agent_runs按started_at降序并列时按id降序() {
    let env = Env::new("agent-list-order");
    let store = open_ok(&env.db_path());
    // started_at 显式注入（排序依据由调用方落库值决定），无需 sleep
    let early = begin_ok(&store, "早", 100);
    let late = begin_ok(&store, "晚", 300);
    let middle = begin_ok(&store, "中", 200);

    let ids: Vec<i64> = store
        .list_agent_runs()
        .unwrap()
        .into_iter()
        .map(|record| record.id)
        .collect();
    assert_eq!(ids, vec![late.id, middle.id, early.id], "started_at 降序");

    // 并列时 id 降序（顺序确定）
    let tie_a = begin_ok(&store, "并列甲", 300);
    let tie_b = begin_ok(&store, "并列乙", 300);
    let top_two: Vec<i64> = store
        .list_agent_runs()
        .unwrap()
        .into_iter()
        .take(2)
        .map(|record| record.id)
        .collect();
    assert_eq!(top_two, vec![tie_b.id, tie_a.id], "并列按 id 降序");
}

#[test]
fn 空库list_agent_runs返回空向量不报错() {
    let env = Env::new("agent-list-empty");
    let store = open_ok(&env.db_path());

    assert!(store.list_agent_runs().unwrap().is_empty());
}

#[test]
fn 不存在run_id的list_agent_run_events返回空向量不报错() {
    let env = Env::new("agent-events-miss");
    let store = open_ok(&env.db_path());

    assert!(store.list_agent_run_events(42).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Store::open：信封 API 空库形态 + native 格式重开直通（迁移探测不误触发，
// legacy 迁移全链路见 migrate_test.rs 集成关系 R1）
// ---------------------------------------------------------------------------

#[test]
fn open全新路径后list_models列出全部注册模型且计数为0() {
    let env = Env::new("open-models-empty");

    let store = open_ok(&env.db_path());

    let models = store.list_models().unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace", "agent_run", "agent_event"],
        "注册表全量列出，计数 0 也列出"
    );
    assert!(
        models.iter().all(|model| model.count == 0),
        "空库三模型计数全 0: {models:?}"
    );
}

#[test]
fn native格式已有库重开直通此前写入的run与事件完整读回且不产生bak() {
    let env = Env::new("reopen-native");
    let db_path = env.db_path();

    let (workspace, run, events) = {
        let store = open_ok(&db_path);
        let workspace = add_ok(&store, &env.ws("native-persist"));
        let run = begin_ok(&store, "native 重开", 1727000000000);
        let events = vec![
            stamped(0, run_started_kind()),
            stamped(1, raw_kind("native")),
        ];
        store
            .append_agent_run_events(run.id, &events)
            .unwrap_or_else(|e| panic!("append 应成功: {e}"));
        (workspace, run, events)
    };

    // native 格式已有库：重开直通（legacy 探测不命中），全部记录完整读回
    let reopened = open_ok(&db_path);
    assert_eq!(reopened.list_workspaces().unwrap(), vec![workspace]);
    assert_eq!(reopened.list_agent_runs().unwrap(), vec![run.clone()]);
    assert_eq!(reopened.list_agent_run_events(run.id).unwrap(), events);
    assert!(
        !db_path.with_file_name("test.redb.bak").exists(),
        "native 重开不触发迁移：不产生 .bak 留档"
    );
}

// ---------------------------------------------------------------------------
// 事件类型化：append / list_agent_run_events（五变体、重复合成主键、千级 seq、
// 两 run 隔离）
// ---------------------------------------------------------------------------

/// 以指定 kind 构造盖戳事件（seq 由调用方给定，时间戳取当前钟面）。
fn stamped(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent::stamp(seq, kind)
}

fn run_started_kind() -> AgentEventKind {
    AgentEventKind::RunStarted {
        model: Some("claude-opus".to_owned()),
        session_id: Some("s-1".to_owned()),
        tools: vec!["Bash".to_owned()],
        mcp_servers: Vec::new(),
    }
}

fn message_kind(role: &str) -> AgentEventKind {
    AgentEventKind::Message {
        role: role.to_owned(),
        blocks: Vec::new(),
        parent_tool_use_id: None,
    }
}

fn system_notice_kind(subtype: &str) -> AgentEventKind {
    AgentEventKind::SystemNotice {
        subtype: subtype.to_owned(),
        payload: serde_json::json!({ "attempt": 2 }),
    }
}

fn run_result_kind(is_error: bool) -> AgentEventKind {
    AgentEventKind::RunResult {
        subtype: if is_error {
            "error_max_turns"
        } else {
            "success"
        }
        .to_owned(),
        is_error,
        num_turns: Some(1),
        duration_ms: Some(1234),
        cost_usd: Some(0.5),
        usage: serde_json::Value::Null,
        session_id: Some("s-1".to_owned()),
    }
}

/// Raw 变体：`tag` 进入原文载荷，供归属/保真断言提取。
fn raw_kind(tag: &str) -> AgentEventKind {
    AgentEventKind::Raw {
        event_type: "mystery".to_owned(),
        raw_json: format!(r#"{{"type":"mystery","tag":"{tag}"}}"#),
    }
}

/// 从 Raw 变体提取 tag（fixture 口径漂移即 panic）。
fn raw_tag(event: &AgentEvent) -> &str {
    match &event.kind {
        AgentEventKind::Raw { raw_json, .. } => raw_json
            .split(r#""tag":""#)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_else(|| panic!("raw fixture 无 tag: {event:?}")),
        other => panic!("期望 Raw 变体，实际: {other:?}"),
    }
}

#[test]
fn append五变体类型化批量追加后重放seq升序逐字段保真() {
    let env = Env::new("append-five");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "五变体", 1727000000000);
    let seeded = vec![
        stamped(0, run_started_kind()),
        stamped(1, message_kind("assistant")),
        stamped(2, system_notice_kind("api_retry")),
        stamped(3, run_result_kind(false)),
        stamped(4, raw_kind("mystery-tag")),
    ];

    store
        .append_agent_run_events(run.id, &seeded)
        .unwrap_or_else(|e| panic!("批量追加应成功: {e}"));

    let replayed = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(
        replayed, seeded,
        "类型化批量落库后重放逐字段保真（含 Raw 逃生舱）"
    );
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3, 4], "重放 seq 升序");
}

#[test]
fn 同run重复seq二次追加由合成主键冲突拒绝且重放恰一条() {
    let env = Env::new("append-dup-seq");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "重复 seq", 1727000000000);
    let first = stamped(0, raw_kind("first"));

    store.append_agent_run_events(run.id, &[first]).unwrap();

    // 同 (run_id, seq) 再追加：合成主键冲突（native_db insert 语义），二次追加报错
    let second = stamped(0, raw_kind("second"));
    let result = store.append_agent_run_events(run.id, &[second]);
    assert!(
        result.is_err(),
        "同合成主键二次追加被拒绝，实际: {result:?}"
    );

    // 重放面恰一条：不产生重复行（与旧载体「重放不重」口径一致）
    let replayed = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(replayed.len(), 1, "重放恰一条不重复");
    assert_eq!(raw_tag(&replayed[0]), "first", "保留先写入的一条");
}

#[test]
fn 单run千级seq批量追加后重放序完整不回绕() {
    let env = Env::new("append-thousand");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "千级 seq", 1727000000000);
    let seeded: Vec<AgentEvent> = (0..1000u64)
        .map(|seq| stamped(seq, raw_kind(&seq.to_string())))
        .collect();

    store
        .append_agent_run_events(run.id, &seeded)
        .unwrap_or_else(|e| panic!("批量追加应成功: {e}"));

    let replayed = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(replayed.len(), 1000, "千级事件一条不丢");
    // u128 打包键在大 seq 下保序：重放序完整、严格单调不回绕
    for (index, event) in replayed.iter().enumerate() {
        assert_eq!(
            event.seq, index as u64,
            "重放第 {index} 条 seq 恰为 {index}"
        );
    }
}

#[test]
fn 两run同seq区间互不串扰经run_id二级索引隔离() {
    let env = Env::new("append-isolation");
    let store = open_ok(&env.db_path());
    let run_a = begin_ok(&store, "run A", 100);
    let run_b = begin_ok(&store, "run B", 200);
    assert_ne!(run_a.id, run_b.id);
    let events_a: Vec<AgentEvent> = (0..4u64).map(|seq| stamped(seq, raw_kind("A"))).collect();
    let events_b: Vec<AgentEvent> = (0..4u64).map(|seq| stamped(seq, raw_kind("B"))).collect();
    // 两 run 落完全重叠的 seq 区间：run_id 高位隔离缺失即串扰
    store.append_agent_run_events(run_a.id, &events_a).unwrap();
    store.append_agent_run_events(run_b.id, &events_b).unwrap();

    let replay_a = store.list_agent_run_events(run_a.id).unwrap();
    assert_eq!(replay_a, events_a, "run A 重放恰为自己 seq 区间的四条");
    assert!(replay_a.iter().all(|event| raw_tag(event) == "A"));
    let replay_b = store.list_agent_run_events(run_b.id).unwrap();
    assert_eq!(replay_b, events_b, "run B 重放恰为自己 seq 区间的四条");
    assert!(replay_b.iter().all(|event| raw_tag(event) == "B"));
}

// ---------------------------------------------------------------------------
// 信封 API：list_models 计数一致性 + scan 分页（分页边界与信封形态矩阵见
// envelope_test.rs，此处为 Store 公共 API 的正向往返）
// ---------------------------------------------------------------------------

#[test]
fn list_models写入三模型数据后计数与各模型实有记录数一致() {
    let env = Env::new("models-counts");
    let store = open_ok(&env.db_path());
    add_ok(&store, &env.ws("alpha"));
    add_ok(&store, &env.ws("beta"));
    let run = begin_ok(&store, "计数复核", 1727000000000);
    let events: Vec<AgentEvent> = (0..3u64).map(|seq| stamped(seq, raw_kind("c"))).collect();
    store.append_agent_run_events(run.id, &events).unwrap();

    let models = store.list_models().unwrap();
    let count_of = |name: &str| {
        models
            .iter()
            .find(|model| model.name == name)
            .unwrap_or_else(|| panic!("模型 {name} 应在清单中"))
            .count
    };
    assert_eq!(count_of("workspace"), 2, "workspace 计数与实有记录数一致");
    assert_eq!(count_of("agent_run"), 1, "agent_run 计数与实有记录数一致");
    assert_eq!(
        count_of("agent_event"),
        3,
        "agent_event 计数与实有记录数一致"
    );
}

#[test]
fn scan分页按offset_limit返回主键自然序翻页拼接不重不漏() {
    let env = Env::new("scan-paging");
    let store = open_ok(&env.db_path());
    let seeded: Vec<WorkspaceRecord> = ["alpha", "beta", "gamma", "delta", "epsilon"]
        .iter()
        .map(|name| add_ok(&store, &env.ws(name)))
        .collect();

    let all_keys = |offset: u32, limit: u32| -> Vec<String> {
        store
            .scan("workspace", offset, limit)
            .unwrap_or_else(|e| panic!("scan 应成功: {e}"))
            .into_iter()
            .map(|envelope| {
                envelope
                    .key
                    .as_str()
                    .expect("workspace key 为字符串")
                    .to_owned()
            })
            .collect()
    };

    let page1 = all_keys(0, 2);
    let page2 = all_keys(2, 2);
    let page3 = all_keys(4, 2);
    let mut union = page1;
    union.extend(page2);
    union.extend(page3);
    let mut sorted = union.clone();
    sorted.sort();
    assert_eq!(sorted, union, "翻页拼接不重不漏且全局唯一");
    let mut expected: Vec<String> = seeded.iter().map(|record| record.root.clone()).collect();
    expected.sort();
    assert_eq!(union, expected, "拼接结果恰为全部记录的主键自然序");
}

#[test]
fn scan未知模型名与空串返回err不panic且错误串可读() {
    let env = Env::new("scan-unknown");
    let store = open_ok(&env.db_path());

    for name in ["nope", ""] {
        let result = store.scan(name, 0, 10);
        let err = result.expect_err(&format!("未知模型 {name:?} 应 Err"));
        assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
        assert!(
            err.to_string().contains("未知模型"),
            "错误串含「未知模型」语境便于排查，实际: {err}"
        );
    }
    // 失败不产生任何副作用：库仍可正常读写
    assert!(store.list_workspaces().unwrap().is_empty());
}
