//! `create_explore_note` 专用工具执行体：占位记录两步原子定名 / 已定名记录
//! 同名补建与异名拦截。无路径参数——服务端按**调用时**（非轮建立快照）以
//! 会话指针（记录 `session_id` == 本会话 id）圈定当前 explore 记录（per-call
//! 现查，改名后续轮零漂移）；文件与记录自诞生即最终名，agent 手里不存在旧
//! 路径。执行上下文由组合根 per-turn 铸造（store 句柄 + 轮 root + 会话指针，
//! 经 `LoopTurn` 携带），`tools::execute` 纯函数签名零侵入。
//!
//! 前置校验必须先于落盘（fs + db 跨存储非真原子，校验收窄残留窗口——promote
//! 先例）：非法 kebab / 目标名已存在 / 文件已存在均显式 `Err` 零半边残留；
//! ①落盘②改名两步间失败以 `Err` 附残留与恢复指引收口。

use std::path::PathBuf;
use std::sync::Arc;

use foundation::layout::resolve;
use serde_json::Value;
use store::{ExploreRecord, Store, EXPLORE_PLACEHOLDER_PREFIX};

/// explore 名称长度上限（与 store `check_explore_name` 同宽）。
const MAX_NOTE_NAME_LENGTH: usize = 128;

/// 占位名前缀（消费 store 导出 [`EXPLORE_PLACEHOLDER_PREFIX`]——同口径单点
/// 本地复制，promote 门禁 / 前端 stance 分形与此前缀判别互链）。
const DRAFT_PREFIX: &str = EXPLORE_PLACEHOLDER_PREFIX;

/// explore 名称 kebab-case 判定（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`，不引
/// regex）：与 store `is_kebab_case` / 命令层 `is_kebab_case` 同口径三处互链
/// ——前置校验先于落盘，防半边残留。
fn is_kebab_case(name: &str) -> bool {
    let segment_ok = |first: bool, segment: &str| {
        let mut chars = segment.chars();
        match chars.next() {
            // 首段必须字母开头；后续段可数字开头
            Some(c) if c.is_ascii_lowercase() || (!first && c.is_ascii_digit()) => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    let mut segments = name.split('-');
    match segments.next() {
        Some(first) => {
            if !segment_ok(true, first) {
                return false;
            }
        }
        // split 至少返回一段，防御式拒绝
        None => return false,
    }
    segments.all(|segment| segment_ok(false, segment))
}

/// `create_explore_note` 专用工具执行上下文（per-turn 铸造：组合根 store 句柄
/// + 轮 root + 会话指针）。
pub(crate) struct ExploreNoteContext {
    store: Arc<Store>,
    root: PathBuf,
    session_id: String,
}

impl ExploreNoteContext {
    /// 上下文构造（pump 侧 `toolset == Explore` 且句柄与 Continue id 在场时铸造）。
    pub(crate) fn new(store: Arc<Store>, root: PathBuf, session_id: String) -> Self {
        Self {
            store,
            root,
            session_id,
        }
    }

    /// 执行体：① 会话指针圈定当前 explore 记录（miss 显式 `Err`）；② 占位
    /// 记录 → 前置校验 + create-only 落盘 + in-place 改名（原子两步，返回
    /// repo 相对路径）；③ 已定名记录 → 同名且文件缺失时补建（孤儿重建），
    /// 否则 `Err` 附现有名与笔记路径（文案即恢复通道——agent 转用 edit）。
    pub(crate) async fn execute(&self, input: &Value) -> Result<String, String> {
        let name = input
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "入参缺失或非字符串字段: name".to_owned())?;
        let content = input
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| "入参缺失或非字符串字段: content".to_owned())?;
        let record = self.locate_record()?;
        if record.name.starts_with(DRAFT_PREFIX) {
            self.finalize_placeholder(&record, name, content).await
        } else {
            self.ensure_named(&record, name, content).await
        }
    }

    /// ① 会话指针圈定记录：记录 `session_id` == 本会话 id（explore 建档预建
    /// 回写，改名 in-place 保指针）；miss 显式 `Err` 附会话 id。
    fn locate_record(&self) -> Result<ExploreRecord, String> {
        let root = self.root.to_string_lossy();
        self.store
            .list_explore_records(&root)
            .map_err(|e| format!("探索记录读取失败: {e}"))?
            .into_iter()
            .find(|record| record.session_id.as_deref() == Some(self.session_id.as_str()))
            .ok_or_else(|| {
                format!(
                    "未找到归属当前会话的探索记录: session_id={:?}",
                    self.session_id
                )
            })
    }

    /// ② 占位记录定名臂：前置校验（本地 kebab 校验 + 长度 ≤128 / 目标名无既
    /// 有记录 / 目标文件不存在）→ ① `create_new(true)` 落盘（create-only 硬
    /// 语义，缺失父目录补建）→ ② store in-place 改名（用记录自身 `root` 寻
    /// 址）→ 返回 repo 相对路径。①后②失败 `Err` 附已落盘路径与恢复指引。
    async fn finalize_placeholder(
        &self,
        record: &ExploreRecord,
        name: &str,
        content: &str,
    ) -> Result<String, String> {
        // 前置校验（先于任何写动作——零半边残留）
        if !is_kebab_case(name) {
            return Err(format!(
                "非法笔记名: {name:?}（须为 kebab-case（小写字母/数字，可用 `-` 连接））——请换一个最终名重试"
            ));
        }
        if name.len() > MAX_NOTE_NAME_LENGTH {
            return Err(format!(
                "非法笔记名: {name:?}（长度超过 {MAX_NOTE_NAME_LENGTH} 字符限制）——请换一个更短的最终名重试"
            ));
        }
        let record_root = record.root.clone();
        if self
            .store
            .find_explore_record(&record_root, name)
            .map_err(|e| format!("探索记录读取失败: {e}"))?
            .is_some()
        {
            return Err(format!(
                "目标名已被其他探索记录占用: {name:?}——请换一个最终名重试"
            ));
        }
        let path = resolve(&self.root).explores_root.join(format!("{name}.md"));
        if tokio::fs::metadata(&path).await.is_ok() {
            return Err(format!(
                "笔记文件已存在: {}——请换一个最终名重试，或删除该残留文件后再试",
                display_path(&self.root, &path)
            ));
        }
        // ① create-only 落盘（并发窗口归零——`create_new` 缺席即失败）
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("创建目录失败: {e}"))?;
        }
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(|e| format!("创建笔记文件失败: {e}"))?;
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("写入笔记失败: {e}"))?;
        // ② store in-place 改名（保主键保会话链）；失败残留窗口收口：
        // 文件在而记录未改——附残留路径与恢复指引
        self.store
            .rename_explore_record(&record_root, &record.name, name)
            .map_err(|e| {
                format!(
                    "笔记文件已落盘（{}）但记录定名失败: {e}；请重跑 create_explore_note（将命中\
                     「文件已存在」附路径），按指引在会话外对齐记录或删除残留文件后重试",
                    display_path(&self.root, &path)
                )
            })?;
        Ok(display_path(&self.root, &path))
    }

    /// ③ 已定名记录臂：`name` == 记录名且文件缺失时同名补建（会话外删除的
    /// 孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道）。
    async fn ensure_named(
        &self,
        record: &ExploreRecord,
        name: &str,
        content: &str,
    ) -> Result<String, String> {
        let path = resolve(&self.root)
            .explores_root
            .join(format!("{}.md", record.name));
        if name == record.name {
            if tokio::fs::metadata(&path).await.is_err() {
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("创建目录失败: {e}"))?;
                }
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .await
                    .map_err(|e| format!("创建笔记文件失败: {e}"))?;
                tokio::fs::write(&path, content)
                    .await
                    .map_err(|e| format!("写入笔记失败: {e}"))?;
                return Ok(display_path(&self.root, &path));
            }
            return Err(format!(
                "本次探索已定名且笔记在盘: {name:?} → {}（请转用 edit 修改笔记）",
                display_path(&self.root, &path)
            ));
        }
        Err(format!(
            "本次探索已定名为 {:?}（笔记路径 {}），create_explore_note 不接受异名 {name:?}——修改笔记请转用 edit",
            record.name,
            display_path(&self.root, &path)
        ))
    }
}

/// repo 相对路径（`strip_prefix` root + `\`→`/` 归一；strip 失败退全路径）。
fn display_path(root: &std::path::Path, path: &std::path::Path) -> String {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    relative
}
