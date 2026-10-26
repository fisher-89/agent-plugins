use std::path::PathBuf;

use tauri::Manager;

use crate::env::{DataRoot, RootSource};

use super::{app_environment, app_environment_inner, AppEnvironment};

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

/// 数据根 fixture（临时目录真实路径 + 指定来源）。
fn data_root(path: PathBuf, source: RootSource) -> DataRoot {
    DataRoot { path, source }
}

/// 以 MockRuntime 建测用 app 并托管数据根（命令层薄包装取值装置）。
fn app_with_data_root(root: DataRoot) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    app.manage(root);
    app
}

// ---------------------------------------------------------------------------
// app_environment_inner：三字段平移（AC-1 / AC-14）
// ---------------------------------------------------------------------------

#[test]
fn app_environment_inner三字段各取单点且无空值() {
    let dir = tempfile::Builder::new()
        .prefix("commands-env-test-")
        .tempdir()
        .expect("创建数据根临时目录失败");
    let root = data_root(dir.path().to_path_buf(), RootSource::DevEnv);

    let environment = app_environment_inner(&root);

    assert_eq!(
        environment.data_root,
        dir.path().to_string_lossy().into_owned(),
        "data_root 为 path 的 lossy 串逐字"
    );
    assert_eq!(environment.source, "env", "source = RootSource::wire()");
    assert_eq!(
        environment.mode,
        crate::env::mode(),
        "mode = env::mode()（单点产出）"
    );
    for field in [
        &environment.mode,
        &environment.data_root,
        &environment.source,
    ] {
        assert!(!field.is_empty(), "三字段均无空值：{field}");
    }
}

#[test]
fn app_environment_inner_source三值遍历与wire逐字相等() {
    for (source, want) in [
        (RootSource::Prod, "prod"),
        (RootSource::DevDefault, "dev-default"),
        (RootSource::DevEnv, "env"),
    ] {
        let environment = app_environment_inner(&data_root(PathBuf::from("root"), source));
        assert_eq!(environment.source, want, "命令层不另拼字符串");
        assert_eq!(environment.source, source.wire(), "与 wire() 逐字相等");
    }
}

#[test]
fn app_environment_inner_mode与env_mode同源且值域闭集() {
    let environment = app_environment_inner(&data_root(PathBuf::from("root"), RootSource::Prod));
    assert_eq!(environment.mode, crate::env::mode());
    assert!(
        matches!(environment.mode.as_str(), "dev" | "prod"),
        "mode 值域闭集（与自检摘要 mode= 同形）"
    );
}

#[test]
fn app_environment_inner无err通道且任意数据根均返回完整结构() {
    for (path, source) in [
        (PathBuf::new(), RootSource::Prod),
        (PathBuf::from("root"), RootSource::DevDefault),
    ] {
        let environment = app_environment_inner(&data_root(path, source));
        // 签名为 -> AppEnvironment（非 Result，管理态恒在）：不 panic、不返回错误态
        assert_eq!(environment.source, source.wire());
        assert_eq!(environment.mode, crate::env::mode());
    }
}

#[test]
fn app_environment_inner路径边界原样透传不归一不查盘() {
    let long = format!("中文目录🙂/{}", "x".repeat(300));

    // 空路径 → 空串（不 panic）
    assert_eq!(
        app_environment_inner(&data_root(PathBuf::new(), RootSource::DevEnv)).data_root,
        "",
        "空 path 平移为空串"
    );

    // 超长含中文与 emoji → 逐字透传
    let long_path = PathBuf::from(&long);
    assert_eq!(
        app_environment_inner(&data_root(long_path.clone(), RootSource::DevEnv)).data_root,
        long_path.to_string_lossy().into_owned(),
        "超长路径逐字透传"
    );

    // 尾分隔符 / 空格 / `..` 段 → 原样透传（平移语义）
    for raw in ["root/", "root with space", "root/../other"] {
        let path = PathBuf::from(raw);
        assert_eq!(
            app_environment_inner(&data_root(path.clone(), RootSource::DevEnv)).data_root,
            path.to_string_lossy().into_owned(),
            "原样透传：{raw}"
        );
    }

    // 非 UTF-8 路径 → lossy 不 panic
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let raw_os = OsStr::from_bytes(b"/tmp/\xff\xfe-non-utf8");
        let path = PathBuf::from(raw_os);
        let environment = app_environment_inner(&data_root(path, RootSource::DevEnv));
        assert_eq!(environment.data_root, path.to_string_lossy().into_owned());
    }
}

// ---------------------------------------------------------------------------
// app_environment：薄包装（MockRuntime + 管理态直取，AC-1 / AC-14）
// ---------------------------------------------------------------------------

#[test]
fn app_environment薄包装与inner逐字段全等() {
    let root = data_root(PathBuf::from("data-root"), RootSource::DevEnv);
    let app = app_with_data_root(root.clone());

    let wrapped = app_environment(app.state::<DataRoot>());
    let inner = app_environment_inner(&root);

    assert_eq!(wrapped.mode, inner.mode, "命令层零加工（mode）");
    assert_eq!(
        wrapped.data_root, inner.data_root,
        "命令层零加工（dataRoot）"
    );
    assert_eq!(wrapped.source, inner.source, "命令层零加工（source）");
}

#[test]
fn app_environment不依赖其它管理态() {
    // 未 manage 任何其它注册表（无 WorkspaceStores / WatchRegistry）也能取值
    let app = app_with_data_root(data_root(
        PathBuf::from("data-root"),
        RootSource::DevDefault,
    ));

    let wrapped = app_environment(app.state::<DataRoot>());
    assert_eq!(wrapped.source, "dev-default");
    assert_eq!(wrapped.data_root, "data-root");
}

// ---------------------------------------------------------------------------
// AppEnvironment：线面 camelCase 形态与类型面（AC-14 / AC-16）
// ---------------------------------------------------------------------------

#[test]
fn app_environment线面恰三键且为camel_case() {
    let environment = AppEnvironment {
        mode: "dev".to_owned(),
        data_root: "/data/root".to_owned(),
        source: "dev-default".to_owned(),
    };

    let value = serde_json::to_value(&environment).expect("序列化应成功");
    let object = value.as_object().expect("线面为对象形态");
    assert_eq!(object.len(), 3, "恰三个键（无多余字段、无嵌套对象）");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["dataRoot", "mode", "source"], "camelCase 三键");

    assert_eq!(value["mode"], "dev");
    assert_eq!(value["dataRoot"], "/data/root");
    assert_eq!(value["source"], "dev-default");
    assert!(
        !serde_json::to_string(&environment)
            .expect("序列化应成功")
            .contains("data_root"),
        "蛇形键零命中"
    );
}

#[test]
fn app_environment类型面clone与debug与三字段均为字符串() {
    let environment = AppEnvironment {
        mode: "prod".to_owned(),
        data_root: "/data/root".to_owned(),
        source: "prod".to_owned(),
    };
    let cloned = environment.clone();

    assert_eq!(cloned.mode, environment.mode);
    assert_eq!(cloned.data_root, environment.data_root);
    assert_eq!(cloned.source, environment.source);

    let debug = format!("{environment:?}");
    for field in ["mode", "data_root", "source"] {
        assert!(debug.contains(field), "Debug 渲染含字段名 {field}");
    }
}
