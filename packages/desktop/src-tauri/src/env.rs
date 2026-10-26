use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// prod 数据目录名（`home_dir()` 根下；prod 构建的数据根 = `<home>/.dev-team`）。
pub const PROD_DIR: &str = ".dev-team";

/// dev 子根名（dev 构建默认数据根 = `<home>/.dev-team/dev`，与 prod 同屋檐）。
pub const DEV_SUBDIR: &str = "dev";

/// 数据根覆盖环境变量名（仅 dev 构建消费；提示文案 / 留痕文本 / 探测共用）。
pub const DATA_ROOT_ENV: &str = "DEV_TEAM_DATA_ROOT";

/// 数据根下日志子树名（引擎异常日志与启动失败日志共用目录、分文件）。
pub const LOGS_DIR_NAME: &str = "logs";

/// 启动失败日志文件名（数据根校验失败双写落点，锚 prod 根）。
pub const BOOT_ERROR_LOG_NAME: &str = "boot-error.log";

/// prod 侧 env 留痕日志文件名（锚 prod 根）。
pub const IGNORED_ENV_LOG_NAME: &str = "ignored-env.log";

/// 数据根单值（Tauri 管理态承载）：`main` 单点解析一次后 `manage`，`setup`
/// 钩子与 `--self-check` 共用同一份值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot {
    /// 归一后的数据根绝对路径
    pub path: PathBuf,
    /// 根的来源（线值经 [`RootSource::wire`] 单点产出）
    pub source: RootSource,
}

/// 数据根来源：值域与线值一对一（`prod` / `dev-default` / `env`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootSource {
    /// prod 构建：`<home>/.dev-team`
    Prod,
    /// dev 构建未设 env：`<home>/.dev-team/dev`
    DevDefault,
    /// dev 构建经 `DEV_TEAM_DATA_ROOT` 覆盖
    DevEnv,
}

impl RootSource {
    /// 线值单点（自检摘要 `data_root_source=` 与 IPC `source` 同名同形）：
    /// prod 构建 MUST NOT 再拼第二套字符串。
    pub fn wire(self) -> &'static str {
        match self {
            RootSource::Prod => "prod",
            RootSource::DevDefault => "dev-default",
            RootSource::DevEnv => "env",
        }
    }
}

/// 数据根校验失败（每变体自带收到的原值：提示三要素之二由类型携带，
/// 渲染侧不二次读取 env）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootError {
    /// 空白值（trim 后为空 / 纯空白）
    Blank { raw: String },
    /// 相对路径（含 Windows 的「仅根」形态；MUST NOT 按 cwd 解析）
    Relative { raw: String },
    /// 归一后等于 prod 根（会污染生产数据根）
    EqualsProd { raw: String },
    /// 归一后是 prod 根的严格祖先（会污染生产数据根）
    AncestorOfProd { raw: String },
    /// 已存在但是普通文件（数据根需要目录）
    NotDirectory { raw: String },
}

/// 构建模式线值：`cfg!(debug_assertions)` 三档全对（`tauri dev` 与
/// `tauri build --debug` 同为 dev ⇒「dev 构建与 `pnpm dev` 共库」是免费推论），
/// 是 Rust 侧唯一判别表达式。
pub fn mode() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "prod"
    }
}

/// prod 数据根：**签名不接收 env** ⇒「prod 不读 `DEV_TEAM_DATA_ROOT`」由结构
/// 保证，MUST NOT 靠调用点纪律。
pub fn prod_root(home: &Path) -> DataRoot {
    DataRoot {
        path: normalize(&prod_dir(home)),
        source: RootSource::Prod,
    }
}

/// dev 数据根三分优先级（纯函数，零 env 读取、零 fs 写入）：
///
/// - `env` 为 `None` → `<home>/.dev-team/dev`（[`RootSource::DevDefault`]）；
/// - `env` 为 `Some` → 五项校验（空白 / 相对 / 等于 prod 根 / prod 根祖先 /
///   目标是文件）全过才接受，值为逻辑归一后的路径（[`RootSource::DevEnv`]），
///   任一不过即 `Err`（硬失败，MUST NOT 降级到默认根）。
pub fn resolve_dev_root(home: &Path, env: Option<&OsStr>) -> Result<DataRoot, RootError> {
    let Some(raw_os) = env else {
        return Ok(DataRoot {
            path: normalize(&dev_default_dir(home)),
            source: RootSource::DevDefault,
        });
    };
    let raw = raw_os.to_string_lossy().into_owned();
    // ① 空白（含纯空白串）：空值不能表达「未设」
    if raw.trim().is_empty() {
        return Err(RootError::Blank { raw });
    }
    // ② 相对路径：MUST NOT 按 cwd 解析成随启动目录漂移的根
    let candidate = Path::new(raw_os);
    if !candidate.is_absolute() {
        return Err(RootError::Relative { raw });
    }
    let prod = compare_key(&prod_dir(home));
    let key = compare_key(candidate);
    // ③ 归一后等于 prod 根
    if key == prod {
        return Err(RootError::EqualsProd { raw });
    }
    // ④ 归一后是 prod 根的严格祖先（家目录 / 盘符根 / 任一层上级）
    if prod.starts_with(&key) {
        return Err(RootError::AncestorOfProd { raw });
    }
    // ⑤ 已存在但是普通文件（数据根需要目录）
    if candidate.is_file() {
        return Err(RootError::NotDirectory { raw });
    }
    Ok(DataRoot {
        path: normalize(candidate),
        source: RootSource::DevEnv,
    })
}

/// 数据根解析薄分派：`is_dev == false` **早退**到 [`prod_root`]（不读 env）；
/// dev 分支是本模块唯一 env 读取点。
pub fn resolve_root(home: &Path, is_dev: bool) -> Result<DataRoot, RootError> {
    if !is_dev {
        return Ok(prod_root(home));
    }
    let raw = std::env::var_os(DATA_ROOT_ENV);
    resolve_dev_root(home, raw.as_deref())
}

/// 校验失败提示渲染（自然语言，三要素齐备：指名 `DEV_TEAM_DATA_ROOT`、回显
/// 收到的原值、给可照抄的下一步）：GUI 启动与 `--self-check` 共用同一文案，
/// MUST NOT 各写一份。默认根与示例路径一律由 `home` 派生。
pub fn render_root_error(err: &RootError, home: &Path) -> String {
    let default_root = dev_default_dir(home);
    let example = home.join("dev-team-data").join("accept-1");
    let next_step = format!(
        "下一步二选一：删除环境变量 {DATA_ROOT_ENV}，改用默认数据根 `{}`；或把 {DATA_ROOT_ENV} 改成另一独立目录的绝对路径，例如 `{}`。",
        default_root.display(),
        example.display()
    );
    let prod = prod_dir(home);
    match err {
        RootError::Blank { raw } => format!(
            "环境变量 {DATA_ROOT_ENV} 的值是空白（收到值：「{raw}」），无法定位数据根。{next_step}"
        ),
        RootError::Relative { raw } => format!(
            "环境变量 {DATA_ROOT_ENV} 的值是相对路径（收到值：「{raw}」），相对路径会随启动目录漂移，不会按当前目录解析。{next_step}"
        ),
        RootError::EqualsProd { raw } => format!(
            "环境变量 {DATA_ROOT_ENV} 的值就是生产数据根 `{}`（收到值：「{raw}」），dev 构建会污染生产数据根（全局库 / workspaces / 引擎日志全落进生产目录）。{next_step}",
            prod.display()
        ),
        RootError::AncestorOfProd { raw } => format!(
            "环境变量 {DATA_ROOT_ENV} 的值是生产数据根 `{}` 的上级目录（收到值：「{raw}」），dev 构建会污染生产数据根（生产库就在该路径之下）。{next_step}",
            prod.display()
        ),
        RootError::NotDirectory { raw } => format!(
            "环境变量 {DATA_ROOT_ENV} 指向一个已存在的普通文件（收到值：「{raw}」），数据根必须是目录。{next_step}"
        ),
    }
}

/// 启动失败双写之二：追加到 `<home>/.dev-team/logs/boot-error.log`（自建父
/// 目录、`[unix 毫秒]` 前缀、append-only、IO 失败静默——不掩盖原始失败原因）。
pub fn append_boot_error(home: &Path, message: &str) {
    append_log(home, BOOT_ERROR_LOG_NAME, message);
}

/// prod 侧 env 检测留痕：追加 `<home>/.dev-team/logs/ignored-env.log` 一行
/// （release 构建不认该变量，但「设了 env 却误跑 build:prod」必须留痕，
/// 否则数据无声写进真实生产库）。
pub fn append_ignored_env(home: &Path) {
    append_log(
        home,
        IGNORED_ENV_LOG_NAME,
        &format!("ignored_env={DATA_ROOT_ENV} (release build ignores it)"),
    );
}

/// prod 目录路径（`PROD_DIR` 唯一落点）。
fn prod_dir(home: &Path) -> PathBuf {
    home.join(PROD_DIR)
}

/// dev 默认数据根路径（`<home>/.dev-team/dev`）。
fn dev_default_dir(home: &Path) -> PathBuf {
    prod_dir(home).join(DEV_SUBDIR)
}

/// 逻辑归一：按 `Path::components()` 逐段重拼——`CurDir` 丢弃、`ParentDir` 在
/// 无可弹段时原样保留、`Prefix` / `RootDir` / `Normal` 依次压入。吃 `.` / `..` /
/// 重复分隔符 / 尾分隔符，保留根与前缀；**不触碰 fs**（MUST NOT 用
/// `canonicalize`：目标目录通常尚不存在）。
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component.as_os_str());
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 「等于 / 祖先」判定专用比较键：同一份归一结果，Windows 下大小写不敏感
/// （盘符 / 用户名大小写差异 MUST NOT 绕过红线）。
#[cfg(windows)]
fn compare_key(path: &Path) -> PathBuf {
    PathBuf::from(normalize(path).to_string_lossy().to_lowercase())
}

#[cfg(not(windows))]
fn compare_key(path: &Path) -> PathBuf {
    normalize(path)
}

/// 当前 unix 毫秒（沿引擎日志既有行格式约定；时钟异常取 0）。
fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// 追加一行到 `<prod 根>/logs/<file_name>`：自建父目录 → append → `[毫秒] 行`；
/// 任一步 IO 失败静默返回（尽力而为，MUST NOT 阻断启动）。
fn append_log(home: &Path, file_name: &str, line: &str) {
    let dir = prod_dir(home).join(LOGS_DIR_NAME);
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(file_name))
        .and_then(|mut file| writeln!(file, "[{}] {line}", now_millis()));
}

#[cfg(test)]
#[path = "env_test.rs"]
mod env_test;
