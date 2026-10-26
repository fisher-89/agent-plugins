use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use store::{GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME};

use super::{
    append_boot_error, append_ignored_env, mode, prod_root, render_root_error, resolve_dev_root,
    resolve_root, DataRoot, RootError, RootSource, BOOT_ERROR_LOG_NAME, DATA_ROOT_ENV, DEV_SUBDIR,
    IGNORED_ENV_LOG_NAME, LOGS_DIR_NAME, PROD_DIR,
};

// ---------------------------------------------------------------------------
// 装置：临时目录（RAII）+ 进程环境变量窗口（串行 + 恢复原值）
// ---------------------------------------------------------------------------

/// 数据根临时环境（`tempfile` RAII，测试结束自动清理）。
fn tmp(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("env-test-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败")
}

/// 进程环境变量窗口的串行锁（本模块唯一读写 `DATA_ROOT_ENV` 的位置）。
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 环境变量写入守卫：进入即持锁改值（`None` = 删除），`Drop` 恢复原值。
struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(value: Option<&OsStr>) -> Self {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os(DATA_ROOT_ENV);
        match value {
            Some(value) => std::env::set_var(DATA_ROOT_ENV, value),
            None => std::env::remove_var(DATA_ROOT_ENV),
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(previous) => std::env::set_var(DATA_ROOT_ENV, previous),
            None => std::env::remove_var(DATA_ROOT_ENV),
        }
    }
}

/// `RootError` 携带的原值（五个变体同名同形字段）。
fn raw_of(err: &RootError) -> String {
    match err {
        RootError::Blank { raw }
        | RootError::Relative { raw }
        | RootError::EqualsProd { raw }
        | RootError::AncestorOfProd { raw }
        | RootError::NotDirectory { raw } => raw.clone(),
    }
}

/// 日志落点（`<home>/.dev-team/logs/<file_name>`，由公共常量复算）。
fn log_path(home: &Path, file_name: &str) -> PathBuf {
    home.join(PROD_DIR).join(LOGS_DIR_NAME).join(file_name)
}

/// 读日志文本（文件必在场，否则测试装置自身失败）。
fn read_log(home: &Path, file_name: &str) -> String {
    fs::read_to_string(log_path(home, file_name)).expect("读取日志文件应成功")
}

/// 断言行首 `[13 位 unix 毫秒] ` 形态，返回去掉前缀后的正文。
fn strip_stamp(line: &str) -> &str {
    assert!(line.starts_with('['), "行首应有毫秒时间戳前缀：{line}");
    let end = line.find("] ").expect("时间戳前缀应以 `] ` 收尾");
    let digits = &line[1..end];
    assert_eq!(digits.len(), 13, "时间戳为 13 位毫秒：{line}");
    assert!(
        digits.chars().all(|c| c.is_ascii_digit()),
        "时间戳为全数字：{line}"
    );
    &line[end + 2..]
}

/// 日志文件的行清单（末尾换行不产生空行）。
fn log_lines(content: &str) -> Vec<&str> {
    content.lines().collect()
}

// ---------------------------------------------------------------------------
// 六个常量：字面量单点 / 形态守卫 / 嵌套关系（AC-5 / AC-8 / AC-9 / AC-15）
// ---------------------------------------------------------------------------

#[test]
fn 六个常量字面量单点且与设计口径逐字一致() {
    let cases: [(&str, &str); 6] = [
        (PROD_DIR, ".dev-team"),
        (DEV_SUBDIR, "dev"),
        (DATA_ROOT_ENV, "DEV_TEAM_DATA_ROOT"),
        (LOGS_DIR_NAME, "logs"),
        (BOOT_ERROR_LOG_NAME, "boot-error.log"),
        (IGNORED_ENV_LOG_NAME, "ignored-env.log"),
    ];
    for (actual, want) in cases {
        assert_eq!(actual, want, "常量值必须与 spec 口径逐字一致");
    }
}

#[test]
fn 六个常量形态守卫均非空无空白无分隔符无上级段() {
    let cases = [
        PROD_DIR,
        DEV_SUBDIR,
        DATA_ROOT_ENV,
        LOGS_DIR_NAME,
        BOOT_ERROR_LOG_NAME,
        IGNORED_ENV_LOG_NAME,
    ];
    for value in cases {
        assert!(!value.is_empty(), "常量不得为空串");
        assert_eq!(value.trim(), value, "常量不得含前后空白：{value}");
        assert!(
            !value.contains('/') && !value.contains('\\'),
            "常量不得含路径分隔符（防路径逃逸）：{value}"
        );
        assert!(
            !value.split(['/', '\\']).any(|segment| segment == ".."),
            "常量不得含 `..` 段：{value}"
        );
        assert!(
            !value.contains(':'),
            "常量不得含盘符冒号（跨平台文件名非法）：{value}"
        );
    }
}

#[test]
fn 常量嵌套关系落位与启动类日志分文件约束成立() {
    let dir = tmp("nesting");
    let home = dir.path();

    let resolved = resolve_dev_root(home, None).expect("无 env 应落 dev 默认根");
    assert_eq!(
        resolved.path,
        home.join(PROD_DIR).join(DEV_SUBDIR),
        "dev 默认根 = home/PROD_DIR/DEV_SUBDIR（嵌套同屋檐）"
    );

    let logs = home.join(PROD_DIR).join(LOGS_DIR_NAME);
    for file_name in [BOOT_ERROR_LOG_NAME, IGNORED_ENV_LOG_NAME] {
        assert_eq!(
            logs.join(file_name),
            log_path(home, file_name),
            "启动类日志落 home/PROD_DIR/LOGS_DIR_NAME（与数据根无关）"
        );
    }

    assert_ne!(
        BOOT_ERROR_LOG_NAME, IGNORED_ENV_LOG_NAME,
        "启动失败日志与留痕日志分文件"
    );
    for file_name in [BOOT_ERROR_LOG_NAME, IGNORED_ENV_LOG_NAME] {
        assert_ne!(file_name, "engine.log", "启动类日志与引擎日志分文件");
    }
}

// ---------------------------------------------------------------------------
// mode()：判别源单点（AC-1 / AC-2）
// ---------------------------------------------------------------------------

#[test]
fn mode与判别源同源且值域闭集() {
    let expected = if cfg!(debug_assertions) {
        "dev"
    } else {
        "prod"
    };
    assert_eq!(mode(), expected, "mode 与 cfg!(debug_assertions) 同源");
    assert_eq!(mode(), mode(), "同进程两次调用恒同值（无隐藏状态）");
    assert!(matches!(mode(), "dev" | "prod"), "值域闭集，无第三种可能");
}

#[test]
fn mode的prod字面量与root_source线值同形() {
    let prod_wire = RootSource::Prod.wire();
    assert_eq!(prod_wire, "prod", "prod 来源的线值单点为 `prod`");
    if cfg!(debug_assertions) {
        // dev 构建下 mode 恒 dev，prod 字面量由 RootSource::wire 单独钉住
        assert_eq!(mode(), "dev");
    } else {
        assert_eq!(
            mode(),
            prod_wire,
            "两处产出的 prod 线值同形（不各拼字符串）"
        );
    }
}

#[test]
fn mode不受外部环境变量污染() {
    let expected = if cfg!(debug_assertions) {
        "dev"
    } else {
        "prod"
    };
    for value in ["/tmp/env-test-independent-root", "   "] {
        let _guard = EnvGuard::set(Some(OsStr::new(value)));
        assert_eq!(
            mode(),
            expected,
            "判别源不读 env，MUST NOT 被外部变量污染（值：{value}）"
        );
    }
}

// ---------------------------------------------------------------------------
// prod_root(home)：落位 / home 边界 / env 零影响（AC-3 / AC-5 / AC-10）
// ---------------------------------------------------------------------------

#[test]
fn prod_root落位与来源且无隐藏状态() {
    let dir = tmp("prod-root");
    let home = dir.path();

    let root = prod_root(home);
    assert_eq!(root.path, home.join(PROD_DIR), "prod 根 = home/PROD_DIR");
    assert_eq!(root.source, RootSource::Prod);
    assert_eq!(prod_root(home), root, "第二次调用逐字段相等");
}

#[test]
fn prod_root_home边界不查盘不panic() {
    let dir = tmp("prod-home-edge");
    let home = dir.path();

    let long = home.join("中文目录🙂").join("x".repeat(300));
    let trailing = PathBuf::from(format!("{}/", home.display()));
    let dotted = home.join(".").join("nested");

    // 结构断言（不做归一、不查盘、不 panic）
    for case in [
        PathBuf::new(),
        PathBuf::from("rel"),
        long.clone(),
        trailing.clone(),
    ] {
        let root = prod_root(&case);
        assert_eq!(root.source, RootSource::Prod);
        assert_eq!(
            root.path,
            case.join(PROD_DIR),
            "路径结构恒为 home.join(PROD_DIR)：{}",
            case.display()
        );
    }

    // 含 `.` 段的 home 按逻辑归一落位（`prod_root` 归一后拼接）
    let dotted_root = prod_root(&dotted);
    assert_eq!(dotted_root.path, home.join("nested").join(PROD_DIR));
    assert!(dotted_root.path.ends_with(PROD_DIR));
}

#[test]
fn prod_root对env零影响且永不因非法env失败() {
    let dir = tmp("prod-env");
    let home = dir.path();
    let baseline = prod_root(home);

    for value in ["/tmp/env-test-elsewhere-root", "   "] {
        let _guard = EnvGuard::set(Some(OsStr::new(value)));
        assert_eq!(
            prod_root(home),
            baseline,
            "签名不接收 env ⇒ 合法与非法 env 均零影响、均不产生 Err"
        );
    }
}

// ---------------------------------------------------------------------------
// resolve_dev_root(home, env)：三分优先级（AC-5）
// ---------------------------------------------------------------------------

#[test]
fn resolve_dev_root无env落dev默认根() {
    let dir = tmp("dev-default");
    let home = dir.path();

    let root = resolve_dev_root(home, None).expect("无 env 即默认根，不失败");
    assert_eq!(
        root.path,
        home.join(PROD_DIR).join(DEV_SUBDIR),
        "默认根 <home>/.dev-team/dev"
    );
    assert_eq!(root.source, RootSource::DevDefault);
}

#[test]
fn resolve_dev_root_env合法独立目录即覆盖为env来源() {
    let home_dir = tmp("dev-env-home");
    let base = tmp("dev-env-accept");
    let target = base.path().join("accept-1");

    let root =
        resolve_dev_root(home_dir.path(), Some(target.as_os_str())).expect("合法 env 应接受");
    assert_eq!(root.source, RootSource::DevEnv);
    assert_eq!(root.path, target, "逻辑归一路径与输入等价");
    assert!(
        !target.exists(),
        "目标不存在不构成失败（不查盘、不 NotFound）"
    );
}

// ---------------------------------------------------------------------------
// 五项校验：①空白 ②相对 ③等于 prod 根 ④prod 根严格祖先 ⑤目标是文件（AC-6）
// ---------------------------------------------------------------------------

#[test]
fn resolve_dev_root空白值一律拒绝且回显原值() {
    let dir = tmp("dev-blank");
    let home = dir.path();

    for value in ["", "   ", "\t\n ", "\u{3000}"] {
        let err = resolve_dev_root(home, Some(OsStr::new(value))).expect_err("空白值应拒绝");
        match err {
            RootError::Blank { raw } => assert_eq!(raw, value, "原值逐字回显（含空白原样）"),
            other => panic!("期待 Blank，实际 {other:?}"),
        }
    }
}

#[test]
fn resolve_dev_root相对路径一律拒绝且不按cwd解析() {
    let dir = tmp("dev-relative");
    let home = dir.path();

    for value in ["dev-data", "./data", "..\\data"] {
        let err = resolve_dev_root(home, Some(OsStr::new(value))).expect_err("相对路径应拒绝");
        match err {
            RootError::Relative { raw } => assert_eq!(raw, value, "原值逐字回显（不按 cwd 解析）"),
            other => panic!("期待 Relative，实际 {other:?}"),
        }
    }
}

#[test]
fn resolve_dev_root等于prod根的四种写法均拒绝() {
    let dir = tmp("dev-equals-prod");
    let home = dir.path();
    let prod = home.join(PROD_DIR);

    let cases = [
        prod.display().to_string(),
        format!("{}/", prod.display()),
        format!("{}/.", prod.display()),
        format!("{}//{}", home.display(), PROD_DIR),
    ];
    for value in cases {
        let err = resolve_dev_root(home, Some(OsStr::new(&value))).expect_err("等于 prod 根应拒绝");
        match err {
            RootError::EqualsProd { raw } => assert_eq!(raw, value, "原值逐字回显"),
            other => panic!("期待 EqualsProd，实际 {other:?}（值：{value}）"),
        }
    }
}

#[test]
fn resolve_dev_root_prod根的严格祖先拒绝而后代接受() {
    let dir = tmp("dev-ancestor");
    let home = dir.path();
    let prod = home.join(PROD_DIR);
    let parent = home.parent().expect("home 应有父目录");
    let top = home.ancestors().last().expect("路径至少有一个祖先");

    let cases = [
        home.display().to_string(),
        parent.display().to_string(),
        top.display().to_string(),
        format!("{}/..", prod.display()),
    ];
    for value in cases {
        let err = resolve_dev_root(home, Some(OsStr::new(&value))).expect_err("prod 根祖先应拒绝");
        match err {
            RootError::AncestorOfProd { raw } => assert_eq!(raw, value, "原值逐字回显"),
            other => panic!("期待 AncestorOfProd，实际 {other:?}（值：{value}）"),
        }
    }

    // 后代（严格位于 prod 根之下）不误拒
    let descendant = prod.join(DEV_SUBDIR).join("accept");
    let root = resolve_dev_root(home, Some(descendant.as_os_str())).expect("后代目录应接受");
    assert_eq!(root.source, RootSource::DevEnv);
    assert_eq!(root.path, descendant);
}

#[test]
fn resolve_dev_root目标是已存在普通文件即拒绝而同目录目录接受() {
    let home_dir = tmp("dev-file-home");
    let dir = tmp("dev-file-target");
    let file = dir.path().join("data-file");
    fs::write(&file, b"occupied").expect("创建普通文件失败");

    let err = resolve_dev_root(home_dir.path(), Some(file.as_os_str())).expect_err("文件应拒绝");
    match err {
        RootError::NotDirectory { raw } => {
            assert_eq!(raw, file.to_string_lossy().into_owned(), "原值逐字回显");
        }
        other => panic!("期待 NotDirectory，实际 {other:?}"),
    }

    let ok_dir = dir.path().join("data-dir");
    fs::create_dir_all(&ok_dir).expect("创建对照目录失败");
    let root = resolve_dev_root(home_dir.path(), Some(ok_dir.as_os_str())).expect("目录应接受");
    assert_eq!(root.source, RootSource::DevEnv);
}

#[test]
fn resolve_dev_root多项命中时按一至五的顺序取首个() {
    let dir = tmp("dev-order");
    let home = dir.path();

    // ① 空白且相对 → Blank
    match resolve_dev_root(home, Some(OsStr::new("  "))).expect_err("应拒绝") {
        RootError::Blank { .. } => {}
        other => panic!("空白应先于相对命中，实际 {other:?}"),
    }
    // ② 相对（归一后等于 cwd）→ Relative，不落 EqualsProd / AncestorOfProd
    match resolve_dev_root(home, Some(OsStr::new("."))).expect_err("应拒绝") {
        RootError::Relative { raw } => assert_eq!(raw, "."),
        other => panic!("相对应先于等值判定，实际 {other:?}"),
    }
    // ② 先于 ⑤：相对形态即便同名文件存在也不进入文件判定
    match resolve_dev_root(home, Some(OsStr::new("Cargo.toml"))).expect_err("应拒绝") {
        RootError::Relative { raw } => assert_eq!(raw, "Cargo.toml"),
        other => panic!("相对应先于文件判定，实际 {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 逻辑归一：不依赖存在性 / 吃 . 与 .. 与重复与尾分隔符 / 大小写（AC-7）
// ---------------------------------------------------------------------------

#[test]
fn resolve_dev_root归一化不依赖目标存在性() {
    let home_dir = tmp("dev-normalize-home");
    let base = tmp("dev-normalize-base");
    let dotted = base
        .path()
        .join("x")
        .join(".")
        .join("y")
        .join("..")
        .join("z");
    let plain = base.path().join("x").join("z");

    let a = resolve_dev_root(home_dir.path(), Some(dotted.as_os_str())).expect("不存在路径应接受");
    let b = resolve_dev_root(home_dir.path(), Some(plain.as_os_str())).expect("不存在路径应接受");
    assert_eq!(a.path, b.path, "`x/./y/../z` 与 `x/z` 同判");
    assert!(!plain.exists(), "全程不查盘：目标不存在也不产生 NotFound");
}

#[test]
fn resolve_dev_root归一吃点段与上级段与重复与尾分隔符() {
    let home_dir = tmp("dev-normalize-forms-home");
    let base = tmp("dev-normalize-forms-base");
    let base_display = base.path().display().to_string();

    let cases = [
        format!("{base_display}/x/./z"),
        format!("{base_display}/x/y/../z"),
        format!("{base_display}//x///z"),
        format!("{base_display}/x/z/"),
    ];
    let mut paths = Vec::new();
    for value in cases {
        let root = resolve_dev_root(home_dir.path(), Some(OsStr::new(&value))).expect("应接受");
        assert_eq!(root.source, RootSource::DevEnv);
        paths.push(root.path);
    }
    for window in paths.windows(2) {
        assert_eq!(window[0], window[1], "四形态归一后两两相等");
    }

    // ParentDir 无可弹段时原样保留：绝对路径首段即 `..` → 不 panic，红线判定照常生效
    #[cfg(unix)]
    {
        let wild = Path::new("/../env-test-keep");
        let root = resolve_dev_root(home_dir.path(), Some(wild.as_os_str()))
            .expect("首段 .. 原样保留，不 panic 且不误判为祖先");
        assert_eq!(root.source, RootSource::DevEnv);
    }
}

#[test]
fn resolve_dev_root大小写等价按平台语义判定不绕过红线() {
    let dir = tmp("dev-case");
    let home = dir.path();
    let upper = format!("{}/.DEV-TEAM", home.display());
    let result = resolve_dev_root(home, Some(OsStr::new(&upper)));

    if cfg!(windows) {
        match result.expect_err("Windows 大小写不敏感，应判等于 prod 根") {
            RootError::EqualsProd { raw } => assert_eq!(raw, upper),
            other => panic!("期待 EqualsProd，实际 {other:?}"),
        }
    } else {
        let root = result.expect("大小写敏感平台视为不同路径");
        assert_eq!(root.source, RootSource::DevEnv);
        assert_eq!(root.path, PathBuf::from(&upper));
    }

    // 盘符 / 用户名家目录的大小写变形祖先：Windows 下仍判祖先（红线不被绕过），
    // 大小写敏感平台视为另一条路径而接受。
    let ancestor = home
        .parent()
        .expect("home 应有父目录")
        .display()
        .to_string()
        .to_uppercase();
    assert!(
        Path::new(&ancestor).is_absolute(),
        "前置：祖先为绝对路径（大写入参不改变绝对性）：{ancestor}"
    );
    let ancestor_result = resolve_dev_root(home, Some(OsStr::new(&ancestor)));

    if cfg!(windows) {
        match ancestor_result.expect_err("大小写变形祖先在 Windows 下应拒绝") {
            RootError::AncestorOfProd { raw } => assert_eq!(raw, ancestor, "原值逐字回显"),
            other => panic!("期待 AncestorOfProd，实际 {other:?}（值：{ancestor}）"),
        }
    } else {
        match ancestor_result {
            Ok(root) => assert_eq!(root.source, RootSource::DevEnv),
            // 兜底：若大写变形后的祖先恰为文件系统根（父目录即根），仍按祖先拒绝
            Err(RootError::AncestorOfProd { raw }) => assert_eq!(raw, ancestor),
            other => panic!("大小写敏感平台期待接受（或根祖先拒绝），实际 {other:?}"),
        }
    }
}

#[test]
fn resolve_dev_root五个变体均逐字回显原值() {
    let home_dir = tmp("dev-raw-home");
    let home = home_dir.path();
    let file_dir = tmp("dev-raw-file");
    let file = file_dir.path().join("中文文件🙂");
    fs::write(&file, b"occupied").expect("创建普通文件失败");

    // ① 空白变体：前后空白原样回显
    let blank_raw = "  \t  ";
    match resolve_dev_root(home, Some(OsStr::new(blank_raw))).expect_err("空白应拒绝") {
        RootError::Blank { raw } => assert_eq!(raw, blank_raw, "空白原值逐字节相等"),
        other => panic!("期待 Blank，实际 {other:?}"),
    }

    // ② 相对变体：超长（> 1000 字符）原值
    let long_relative = "长".repeat(1200);
    match resolve_dev_root(home, Some(OsStr::new(&long_relative))).expect_err("相对应拒绝") {
        RootError::Relative { raw } => assert_eq!(raw, long_relative, "超长原值逐字节相等"),
        other => panic!("期待 Relative，实际 {other:?}"),
    }

    // ③ 等于 prod 根变体：尾分隔符形态
    let trailing_sep = format!("{}/", home.join(PROD_DIR).display());
    match resolve_dev_root(home, Some(OsStr::new(&trailing_sep))).expect_err("应拒绝") {
        RootError::EqualsProd { raw } => assert_eq!(raw, trailing_sep, "尾分隔符原值回显"),
        other => panic!("期待 EqualsProd，实际 {other:?}"),
    }

    // ④ 祖先变体：`..` 形态
    let dotted_up = format!("{}/..", home.join(PROD_DIR).display());
    match resolve_dev_root(home, Some(OsStr::new(&dotted_up))).expect_err("应拒绝") {
        RootError::AncestorOfProd { raw } => assert_eq!(raw, dotted_up, "`..` 形态原值回显"),
        other => panic!("期待 AncestorOfProd，实际 {other:?}"),
    }

    // ⑤ 文件变体：中文与 emoji 原值
    let file_raw = file.to_string_lossy().into_owned();
    match resolve_dev_root(home, Some(file.as_os_str())).expect_err("文件应拒绝") {
        RootError::NotDirectory { raw } => {
            assert_eq!(
                raw, file_raw,
                "特殊字符原值逐字节相等（渲染侧不二次取 env）"
            );
        }
        other => panic!("期待 NotDirectory，实际 {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn resolve_dev_root非utf8路径值不panic且raw为lossy串() {
    use std::os::unix::ffi::OsStrExt;

    let dir = tmp("dev-non-utf8");
    let raw_os = OsStr::from_bytes(b"\xff\xfe-dev");
    let err = resolve_dev_root(dir.path(), Some(raw_os)).expect_err("非法 UTF-8 相对值应拒绝");
    match err {
        RootError::Relative { raw } => assert_eq!(
            raw,
            raw_os.to_string_lossy().into_owned(),
            "raw 为 lossy 字符串（判定链不中断）"
        ),
        other => panic!("期待 Relative，实际 {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// resolve_root(home, is_dev)：prod 侧不读 env / dev 分支透传 / 存在性双档
// （AC-3 / AC-5 / AC-9）
// ---------------------------------------------------------------------------

#[test]
fn resolve_root_prod侧不读env() {
    let dir = tmp("root-prod-home");
    let elsewhere = tmp("root-prod-elsewhere");
    let target = elsewhere.path().join("dev-only");
    let home = dir.path();
    let _guard = EnvGuard::set(Some(target.as_os_str()));

    let root = resolve_root(home, false).expect("prod 侧恒 Ok（不因残留 env 失败）");
    assert_eq!(root.source, RootSource::Prod);
    assert_eq!(root, prod_root(home), "env 零影响，与 prod_root 逐字段相等");
}

#[test]
fn resolve_root_dev分支逐档透传env() {
    let dir = tmp("root-dev-home");
    let elsewhere = tmp("root-dev-elsewhere");
    let home = dir.path();
    let accept = elsewhere.path().join("accept-2");

    {
        let _guard = EnvGuard::set(None);
        let root = resolve_root(home, true).expect("未设 env 落默认根");
        assert_eq!(root.source, RootSource::DevDefault);
        assert_eq!(root.path, home.join(PROD_DIR).join(DEV_SUBDIR));
    }
    {
        let _guard = EnvGuard::set(Some(accept.as_os_str()));
        let root = resolve_root(home, true).expect("合法 env 应接受");
        assert_eq!(root.source, RootSource::DevEnv);
        assert_eq!(root.path, accept, "值为逻辑归一路径");
    }
    {
        let prod = home.join(PROD_DIR);
        let _guard = EnvGuard::set(Some(prod.as_os_str()));
        assert!(
            matches!(resolve_root(home, true), Err(RootError::EqualsProd { .. })),
            "设为 prod 根即硬失败（不降级到默认根）"
        );
    }
}

#[test]
fn resolve_root区分未设与设了空串() {
    let dir = tmp("root-empty-var");
    let home = dir.path();

    {
        let _guard = EnvGuard::set(None);
        let root = resolve_root(home, true).expect("未设即默认根");
        assert_eq!(root.source, RootSource::DevDefault);
    }
    {
        let _guard = EnvGuard::set(Some(OsStr::new("")));
        let outcome = resolve_root(home, true);
        // 平台差异留痕：保留空串值的平台 ⇒ Blank 硬失败；把空值等同删除的平台
        // （部分 Windows 口径）⇒ 语义等价于「未设」，落默认根
        if std::env::var_os(DATA_ROOT_ENV).is_some() {
            assert!(
                matches!(outcome, Err(RootError::Blank { .. })),
                "设了空值 MUST NOT 与未设同判"
            );
        } else {
            let root = outcome.expect("平台将空值等同删除，视作未设");
            assert_eq!(root.source, RootSource::DevDefault);
        }
    }
}

// ---------------------------------------------------------------------------
// 组合锚：三分优先级全链 / 两模式数据面互不可见（AC-5 / AC-8 / AC-10 / AC-12）
// ---------------------------------------------------------------------------

#[test]
fn 三分优先级全链路径与线值逐档对齐() {
    let dir = tmp("chain-home");
    let elsewhere = tmp("chain-elsewhere");
    let home = dir.path();
    let accept = elsewhere.path().join("accept-1");

    {
        let _guard = EnvGuard::set(Some(OsStr::new("   ")));
        let root = resolve_root(home, false).expect("prod 侧不读 env ⇒ 非法 env 也无影响");
        assert_eq!(root.path, home.join(PROD_DIR));
        assert_eq!(root.source.wire(), "prod");
    }
    {
        let _guard = EnvGuard::set(None);
        let root = resolve_root(home, true).expect("dev 默认根");
        assert_eq!(root.path, home.join(PROD_DIR).join(DEV_SUBDIR));
        assert_eq!(root.source.wire(), "dev-default");
    }
    {
        let _guard = EnvGuard::set(Some(accept.as_os_str()));
        let root = resolve_root(home, true).expect("dev env 覆盖");
        assert_eq!(root.path, accept);
        assert_eq!(root.source.wire(), "env");
    }
}

#[test]
fn 两模式数据面互不可见同名条目不同父目录() {
    let dir = tmp("isolation");
    let home = dir.path();

    let prod = prod_root(home);
    let dev = resolve_dev_root(home, None).expect("dev 默认根");
    assert_ne!(prod.path, dev.path, "两模式数据根必须分裂");
    assert_eq!(
        dev.path.parent(),
        Some(prod.path.as_path()),
        "dev 根是 prod 根的直接子目录（同屋檐不同根）"
    );

    for name in [GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME] {
        let prod_path = prod.path.join(name);
        let dev_path = dev.path.join(name);
        assert_ne!(prod_path, dev_path, "同名条目在两模式下拼出不同绝对路径");
        assert_eq!(
            prod_path.file_name(),
            dev_path.file_name(),
            "文件名相同（{name}）"
        );
        assert_ne!(
            prod_path.parent(),
            dev_path.parent(),
            "父目录不同（redb 锁与 workspaces 天然隔离）"
        );
    }
}

// ---------------------------------------------------------------------------
// render_root_error：提示三要素 / 不退化 / home 边界 / 同文案复用（AC-6 / AC-8）
// ---------------------------------------------------------------------------

/// 五个变体的 fixture（`raw` 各带一种形态：空白 / 相对 / 尾分隔符 / 祖先 / 特殊字符）。
fn five_errors(home: &Path) -> Vec<RootError> {
    vec![
        RootError::Blank {
            raw: "  \t ".to_owned(),
        },
        RootError::Relative {
            raw: "dev-数据🙂".to_owned(),
        },
        RootError::EqualsProd {
            raw: format!("{}/", home.join(PROD_DIR).display()),
        },
        RootError::AncestorOfProd {
            raw: home.display().to_string(),
        },
        RootError::NotDirectory {
            raw: "/tmp/中文文件🙂".to_owned(),
        },
    ]
}

#[test]
fn render_root_error三要素齐备() {
    let dir = tmp("render-essentials");
    let home = dir.path();
    let default_root = home.join(PROD_DIR).join(DEV_SUBDIR).display().to_string();

    for err in five_errors(home) {
        let text = render_root_error(&err, home);
        assert!(
            text.contains(DATA_ROOT_ENV),
            "三要素之一：指名环境变量（{err:?}）"
        );
        assert!(
            text.contains(&raw_of(&err)),
            "三要素之二：回显收到的原值（{err:?}）"
        );
        assert!(
            text.contains(&default_root),
            "三要素之三：可照抄的绝对路径示例（默认根，{err:?}）"
        );
        assert!(
            text.matches(&home.display().to_string()).count() >= 2,
            "示例路径由 home 派生且不少于两处（默认根 + 独立目录示例，{err:?}）"
        );
        assert!(!text.contains('\n'), "单段自然语言（不换行，{err:?}）");
    }
}

#[test]
fn render_root_error不泄露枚举名且五段文案互不共用() {
    let dir = tmp("render-degrade");
    let home = dir.path();
    let errors = five_errors(home);
    let texts: Vec<String> = errors
        .iter()
        .map(|err| render_root_error(err, home))
        .collect();

    for text in &texts {
        assert!(!text.is_empty(), "文案不得为空");
        assert!(
            text.contains('。') || text.contains('：'),
            "含自然语言句读标点（非裸错误码）：{text}"
        );
    }
    for name in [
        "Blank",
        "Relative",
        "EqualsProd",
        "AncestorOfProd",
        "NotDirectory",
    ] {
        for text in &texts {
            assert!(!text.contains(name), "文案不得泄露 variant 标识符 {name}");
        }
    }
    for (index, text) in texts.iter().enumerate() {
        for other in texts.iter().skip(index + 1) {
            assert_ne!(text, other, "各 variant 文案不得共用同一段兜底串");
        }
    }
}

#[test]
fn render_root_error_home边界不panic且非空home仍给绝对示例() {
    let dir = tmp("render-home-edge");
    let base = dir.path();
    let long = base.join("中文🙂".repeat(50));
    let trailing = PathBuf::from(format!("{}/", base.display()));

    for home in [PathBuf::new(), long, trailing.clone()] {
        let text = render_root_error(
            &RootError::Blank {
                raw: " ".to_owned(),
            },
            &home,
        );
        assert!(!text.is_empty(), "home 边界下文案不得为空");
        assert!(text.contains(DATA_ROOT_ENV), "home 边界下仍指名变量");
    }

    // 非空 home：默认根示例保持绝对形态（以 home 起头、含分隔符）
    let text = render_root_error(
        &RootError::Blank {
            raw: " ".to_owned(),
        },
        &trailing,
    );
    let default_root = trailing.join(PROD_DIR).join(DEV_SUBDIR);
    assert!(
        text.contains(&default_root.display().to_string()),
        "默认根示例由 home 派生并保持绝对形态"
    );
}

#[test]
fn render_root_error纯函数可复用且落盘含全文() {
    let dir = tmp("render-reuse");
    let home = dir.path();
    let err = RootError::EqualsProd {
        raw: home.join(PROD_DIR).display().to_string(),
    };

    let first = render_root_error(&err, home);
    let second = render_root_error(&err, home);
    assert_eq!(first, second, "同一 (err, home) 两次渲染逐字相等（纯函数）");

    append_boot_error(home, &first);
    assert!(
        read_log(home, BOOT_ERROR_LOG_NAME).contains(&first),
        "GUI 与自检共用同一文案（只差往哪写）"
    );
}

// ---------------------------------------------------------------------------
// append_boot_error：落点 / append-only / 分文件 / IO 静默 / 文案边界
// （AC-8 / AC-15）
// ---------------------------------------------------------------------------

#[test]
fn append_boot_error自建父目录并落prod根日志行() {
    let dir = tmp("boot-error");
    let home = dir.path();
    assert!(
        !log_path(home, BOOT_ERROR_LOG_NAME).exists(),
        "前置：日志不存在"
    );

    let message = "数据根不可用：环境变量非法";
    append_boot_error(home, message);

    let content = read_log(home, BOOT_ERROR_LOG_NAME);
    let lines = log_lines(&content);
    assert_eq!(lines.len(), 1, "首次调用恰一行");
    assert_eq!(strip_stamp(lines[0]), message, "内容含 message 全文");
}

#[test]
fn append_boot_error多次调用追加不覆盖() {
    let dir = tmp("boot-append");
    let home = dir.path();

    append_boot_error(home, "第一次失败");
    append_boot_error(home, "第二次失败");
    let content = read_log(home, BOOT_ERROR_LOG_NAME);
    let lines = log_lines(&content);
    assert_eq!(lines.len(), 2, "第三次调用前恰两行");
    assert_eq!(strip_stamp(lines[0]), "第一次失败", "首次内容仍在");
    assert_eq!(strip_stamp(lines[1]), "第二次失败", "行序 == 调用序");

    append_boot_error(home, "第三次失败");
    let content = read_log(home, BOOT_ERROR_LOG_NAME);
    let lines = log_lines(&content);
    assert_eq!(lines.len(), 3, "继续追加，不清旧");
    assert_eq!(strip_stamp(lines[0]), "第一次失败");
}

#[test]
fn append_boot_error与引擎日志分文件互不混写() {
    let dir = tmp("boot-separate");
    let home = dir.path();
    let engine_prod = home.join(PROD_DIR).join(LOGS_DIR_NAME).join("engine.log");
    let engine_dev = home
        .join(PROD_DIR)
        .join(DEV_SUBDIR)
        .join(LOGS_DIR_NAME)
        .join("engine.log");
    for path in [&engine_prod, &engine_dev] {
        fs::create_dir_all(path.parent().expect("引擎日志应有父目录"))
            .expect("创建引擎日志目录失败");
        fs::write(path, b"engine-line\n").expect("预置引擎日志失败");
    }

    append_boot_error(home, "启动失败");

    assert_eq!(
        fs::read(&engine_prod).expect("读引擎日志失败"),
        b"engine-line\n",
        "prod 数据根引擎日志字节不变"
    );
    assert_eq!(
        fs::read(&engine_dev).expect("读引擎日志失败"),
        b"engine-line\n",
        "dev 数据根引擎日志字节不变"
    );
    assert!(
        log_path(home, BOOT_ERROR_LOG_NAME).is_file(),
        "仅 boot-error.log 变化"
    );
}

#[test]
fn append_boot_error_io失败静默不阻断调用方() {
    // ① prod 根位置被普通文件占据：父目录无法建
    let occupied = tmp("boot-io-file");
    fs::write(occupied.path().join(PROD_DIR), b"occupied").expect("创建占位文件失败");
    append_boot_error(occupied.path(), "启动失败");
    append_ignored_env(occupied.path());
    assert!(
        occupied.path().join(PROD_DIR).is_file(),
        "失败路径不得破坏既有文件（静默返回）"
    );

    // ② 目标日志位置为目录：打开失败静默
    let shadowed = tmp("boot-io-dir");
    let target = log_path(shadowed.path(), BOOT_ERROR_LOG_NAME);
    fs::create_dir_all(&target).expect("创建同名目录失败");
    append_boot_error(shadowed.path(), "启动失败");
    assert!(target.is_dir(), "失败路径不得掩盖原始失败原因（不 panic）");
}

#[test]
fn append_boot_error文案边界不panic且可读() {
    let dir = tmp("boot-message");
    let home = dir.path();
    let long = "长".repeat(1200);

    for message in ["", long.as_str(), "多行\n文案🙂"] {
        append_boot_error(home, message);
    }

    let content = read_log(home, BOOT_ERROR_LOG_NAME);
    // 空文案一行、超长文案一行、含 `\n` 文案两行（原样落盘）
    assert_eq!(log_lines(&content).len(), 4, "多行文案原样落盘（不清旧）");
    assert!(content.contains("文案🙂"), "特殊字符原样落盘");
    assert!(
        content.contains(&long),
        "超长文案全文落盘（不截断、不 panic）"
    );
    let first = log_lines(&content)[0];
    assert_eq!(strip_stamp(first), "", "空文案仍产出合法行（仅时间戳前缀）");
}

// ---------------------------------------------------------------------------
// append_ignored_env：行文本逐字 / 幂等 / 锚 prod 根 / IO 静默（AC-9）
// ---------------------------------------------------------------------------

/// 留痕行正文（由常量复算，测试侧不另写字面量）。
fn ignored_env_line() -> String {
    format!("ignored_env={DATA_ROOT_ENV} (release build ignores it)")
}

#[test]
fn append_ignored_env行文本逐字且自建日志目录() {
    let dir = tmp("ignored-line");
    let home = dir.path();

    append_ignored_env(home);

    let content = read_log(home, IGNORED_ENV_LOG_NAME);
    let lines = log_lines(&content);
    assert_eq!(lines.len(), 1, "一次调用恰一行");
    assert_eq!(
        strip_stamp(lines[0]),
        ignored_env_line(),
        "行文本逐字（变量名由常量复算）"
    );
    assert!(content.contains("release build ignores it"), "留痕语义在场");
}

#[test]
fn append_ignored_env多次调用幂等且不影响同目录其它文件() {
    let dir = tmp("ignored-idempotent");
    let home = dir.path();
    let neighbour = log_path(home, BOOT_ERROR_LOG_NAME);
    fs::create_dir_all(neighbour.parent().expect("日志目录应有父路径")).expect("创建日志目录失败");
    fs::write(&neighbour, b"existing\n").expect("预置邻居日志失败");

    append_ignored_env(home);
    append_ignored_env(home);

    let content = read_log(home, IGNORED_ENV_LOG_NAME);
    let lines = log_lines(&content);
    assert_eq!(lines.len(), 2, "两次调用两行");
    assert_eq!(strip_stamp(lines[0]), strip_stamp(lines[1]), "两行内容相同");
    assert_eq!(
        fs::read(&neighbour).expect("读邻居日志失败"),
        b"existing\n",
        "不影响同目录其它文件"
    );
}

#[test]
fn append_ignored_env恒锚prod根而非数据根() {
    let dir = tmp("ignored-anchor");
    let home = dir.path();
    let dev_logs = home.join(PROD_DIR).join(DEV_SUBDIR).join(LOGS_DIR_NAME);
    fs::create_dir_all(&dev_logs).expect("创建 dev 数据根日志目录失败");

    append_ignored_env(home);

    assert!(
        log_path(home, IGNORED_ENV_LOG_NAME).is_file(),
        "留痕只写 prod 根 logs/"
    );
    assert_eq!(
        fs::read_dir(&dev_logs)
            .expect("读 dev 日志目录失败")
            .count(),
        0,
        "日志不是数据：dev 数据根零写入"
    );
}

#[test]
fn append_ignored_env_io失败静默不阻断prod启动() {
    let dir = tmp("ignored-io");
    let home = dir.path();
    fs::write(home.join(PROD_DIR), b"occupied").expect("创建占位文件失败");

    append_ignored_env(home); // 不 panic、返回 ()
    assert!(
        home.join(PROD_DIR).is_file(),
        "留痕失败不得阻断调用方（prod 启动不中断）"
    );
}

// ---------------------------------------------------------------------------
// RootSource::wire / DataRoot 类型面（AC-5 / AC-11 / AC-14 / AC-15）
// ---------------------------------------------------------------------------

#[test]
fn root_source_wire三值逐值断言且两两不等() {
    let cases = [
        (RootSource::Prod, "prod"),
        (RootSource::DevDefault, "dev-default"),
        (RootSource::DevEnv, "env"),
    ];
    let mut values = Vec::new();
    for (source, want) in cases {
        assert_eq!(source.wire(), want, "线值单点");
        values.push(source.wire());
    }
    for (index, value) in values.iter().enumerate() {
        assert!(!value.is_empty(), "线值非空");
        for other in values.iter().skip(index + 1) {
            assert_ne!(value, other, "三值两两不等");
        }
    }
}

#[test]
fn root_source_wire线值形态守卫为小写kebab() {
    /// `^[a-z]+(-[a-z]+)*$`（无非 ASCII、无空格、无下划线、无前后 `-`）。
    fn is_lower_kebab(value: &str) -> bool {
        !value.is_empty()
            && value.split('-').all(|segment| {
                !segment.is_empty() && segment.chars().all(|c| c.is_ascii_lowercase())
            })
    }

    for source in [RootSource::Prod, RootSource::DevDefault, RootSource::DevEnv] {
        let wire = source.wire();
        assert!(is_lower_kebab(wire), "线值为小写 kebab 形态：{wire}");
        assert_eq!(wire.trim(), wire, "线值无前后空白");
    }
    assert_eq!(
        RootSource::Prod.wire(),
        "prod",
        "prod 线值与 env::mode 的 prod 字面量同形（一处拼串）"
    );
}

#[test]
fn root_source与data_root类型面copy与clone与debug可诊断() {
    let source = RootSource::Prod;
    let copied = source; // Copy：拷贝后原值仍可用
    assert_eq!(source, copied, "RootSource 为 Copy 且拷贝后相等");

    let root = DataRoot {
        path: PathBuf::from("rel/data"),
        source: RootSource::DevEnv,
    };
    let cloned = root.clone();
    assert_eq!(cloned, root, "DataRoot clone 后 PartialEq 相等");
    assert_eq!(root, root.clone(), "PartialEq 自反");

    let debug = format!("{root:?}");
    assert!(debug.contains("path"), "Debug 渲染含 path（可诊断）");
    assert!(debug.contains("source"), "Debug 渲染含 source（可诊断）");
    assert!(
        format!("{:?}", RootSource::DevDefault).contains("DevDefault"),
        "RootSource Debug 含变体名"
    );
}

#[test]
fn data_root字段公开可读且多次引用同一值() {
    let dir = tmp("dataroot-managed");
    let root = resolve_dev_root(dir.path(), None).expect("dev 默认根");

    let borrowed = &root;
    let cloned = root.clone();
    assert_eq!(borrowed.path, cloned.path, "字段公开可读（管理态承载）");
    assert_eq!(borrowed.source, cloned.source);
    assert!(
        matches!(root.source, RootSource::DevDefault),
        "同一 DataRoot 实例可被多次引用而不产生第二份解析"
    );
}
