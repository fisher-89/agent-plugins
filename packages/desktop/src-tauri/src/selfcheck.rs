use std::io::Write;
use std::path::{Path, PathBuf};

use store::{WorkspaceStores, GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME};
use tauri::Manager;

use crate::env::{self, DataRoot};

/// 自检参数名单点（`main` 的 args 判定引用）。
pub const SELF_CHECK_ARG: &str = "--self-check";

/// prod 侧拒绝文案单点（`main` 输出到 stderr 后 `exit 2`）。
pub const PROD_REJECT_MESSAGE: &str = "--self-check 仅在 dev 构建可用（当前为生产构建）。";

/// 探查事实（`exit_code` 入参）：未知字段以 `None` 表达，首错即停（`error=`
/// 唯一且可定位，不拼装多条原因）。
#[derive(Debug, Clone)]
pub struct Probe {
    /// 全局库是否打开成功
    pub global_db_open: bool,
    /// workspace 清单条目数（清单读取失败为 `None`）
    pub workspace_count: Option<u64>,
    /// 全局库已注册模型名清单（读取失败为 `None`）
    pub models: Option<Vec<String>>,
    /// `workspaces/` 子树是否存在（只开全局库时通常不存在，属常态）
    pub workspaces_dir_exists: bool,
    /// 首个失败原因（全通为 `None`）
    pub error: Option<String>,
}

/// 摘要渲染视图（字段序即渲染键序，字段与 [`Probe`] 是「视图 ← 事实」投影）。
#[derive(Debug, Clone)]
pub struct Summary {
    /// 构建模式线值（`env::mode()`）
    pub mode: &'static str,
    /// home 目录（`app.path().home_dir()`，解析失败退化为空路径）
    pub home: PathBuf,
    /// 数据根归一后的绝对路径
    pub data_root: PathBuf,
    /// 数据根来源线值（`RootSource::wire()`）
    pub source: &'static str,
    /// 全局库文件路径（`<数据根>/<GLOBAL_DB_FILE_NAME>`）
    pub global_db: PathBuf,
    /// 全局库是否打开成功
    pub global_db_open: bool,
    /// workspace 库子树路径（`<数据根>/<WORKSPACES_DIR_NAME>`）
    pub workspaces_dir: PathBuf,
    /// `workspaces/` 子树是否存在
    pub workspaces_dir_exists: bool,
    /// workspace 清单条目数（未知渲染空值）
    pub workspace_count: Option<u64>,
    /// 全局库已注册模型名清单（未知渲染空值）
    pub models: Option<Vec<String>>,
    /// 首个失败原因（全通时不出线）
    pub error: Option<String>,
}

/// 摘要渲染（纯函数）：十键固定键序，未知值渲染为空串，`error` 有值才追加末行；
/// 末尾恰一个换行。MUST NOT 另加横幅 / 缩进 / 空行（agent 逐行解析）。
pub fn render(summary: &Summary) -> String {
    let mut out = String::new();
    out.push_str("mode=");
    out.push_str(summary.mode);
    out.push('\n');
    out.push_str("home=");
    out.push_str(&summary.home.display().to_string());
    out.push('\n');
    out.push_str("data_root=");
    out.push_str(&summary.data_root.display().to_string());
    out.push('\n');
    out.push_str("data_root_source=");
    out.push_str(summary.source);
    out.push('\n');
    out.push_str("global_db=");
    out.push_str(&summary.global_db.display().to_string());
    out.push('\n');
    out.push_str("global_db_open=");
    out.push_str(if summary.global_db_open { "ok" } else { "err" });
    out.push('\n');
    out.push_str("workspaces_dir=");
    out.push_str(&summary.workspaces_dir.display().to_string());
    out.push('\n');
    out.push_str("workspaces_dir_exists=");
    out.push_str(if summary.workspaces_dir_exists {
        "true"
    } else {
        "false"
    });
    out.push('\n');
    out.push_str("workspace_count=");
    if let Some(count) = summary.workspace_count {
        out.push_str(&count.to_string());
    }
    out.push('\n');
    out.push_str("models=");
    if let Some(models) = &summary.models {
        out.push_str(&models.join(","));
    }
    out.push('\n');
    if let Some(error) = &summary.error {
        out.push_str("error=");
        out.push_str(error);
        out.push('\n');
    }
    out
}

/// 退出码映射：全通 0、首错 1（2 = 用法或环境非法，由 `main` 产出——env 校验
/// 失败与 prod 侧拒绝自检）。
pub fn exit_code(probe: &Probe) -> i32 {
    if probe.error.is_none() {
        0
    } else {
        1
    }
}

/// 自检主路径：探库 → 摘要 → 渲染 → flush → 退出码。`&App` 的唯一用途是取
/// `home=` 行（不触碰管理态、不建窗）；`home_dir` 失败退化为空串，不影响退出码。
pub fn run(app: &tauri::App, data_root: &DataRoot) -> i32 {
    let home = app.path().home_dir().unwrap_or_default();
    let probe = probe(&data_root.path);
    let summary = summary_of(data_root, &home, &probe);
    print!("{}", render(&summary));
    let _ = std::io::stdout().flush();
    exit_code(&probe)
}

/// 探库（真开 `WorkspaceStores`：对不存在的数据根会一并建出全局库，与 GUI 首次
/// 启动同源同副作用，不做 dry-run 变体）：首个失败即记 `error=` 并停步，后续
/// 字段留空值。
fn probe(data_root: &Path) -> Probe {
    let workspaces_dir_exists = data_root.join(WORKSPACES_DIR_NAME).is_dir();
    let mut probe = Probe {
        global_db_open: false,
        workspace_count: None,
        models: None,
        workspaces_dir_exists,
        error: None,
    };
    let stores = match WorkspaceStores::open(data_root) {
        Ok(stores) => stores,
        Err(err) => {
            probe.error = Some(format!("全局库打开失败：{err}"));
            return probe;
        }
    };
    probe.global_db_open = true;
    let models = match stores.global().list_models() {
        Ok(models) => models,
        Err(err) => {
            probe.error = Some(format!("模型清单读取失败：{err}"));
            return probe;
        }
    };
    let workspaces = match stores.global().list_workspaces() {
        Ok(workspaces) => workspaces,
        Err(err) => {
            probe.error = Some(format!("workspace 清单读取失败：{err}"));
            return probe;
        }
    };
    probe.models = Some(models.into_iter().map(|info| info.name).collect());
    probe.workspace_count = Some(workspaces.len() as u64);
    probe
}

/// 摘要组装（纯函数）：模式 / 来源 / 两条路径由 `env` 与 store 常量单点产出，
/// 其余字段由 [`Probe`] 直接平移（命令层零加工）。
fn summary_of(data_root: &DataRoot, home: &Path, probe: &Probe) -> Summary {
    Summary {
        mode: env::mode(),
        home: home.to_path_buf(),
        data_root: data_root.path.clone(),
        source: data_root.source.wire(),
        global_db: data_root.path.join(GLOBAL_DB_FILE_NAME),
        global_db_open: probe.global_db_open,
        workspaces_dir: data_root.path.join(WORKSPACES_DIR_NAME),
        workspaces_dir_exists: probe.workspaces_dir_exists,
        workspace_count: probe.workspace_count,
        models: probe.models.clone(),
        error: probe.error.clone(),
    }
}

#[cfg(test)]
#[path = "selfcheck_test.rs"]
mod selfcheck_test;
