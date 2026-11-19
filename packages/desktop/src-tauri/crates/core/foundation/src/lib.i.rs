// 生成于 archi-api 0.1.0 · common.json@sha256:1f8f0ec14abb —— 勿手改
// 手改将在下次 generate 时被覆盖，且 api:check 零 diff 门会失败。
// 类型定义与自动构造器由契约派发；impl 块写在本模块任意手写文件。
// 本文件由元素的契约 markdown 生成，勿手改：手改将在下次 generate 时被覆盖，且 api:check 零 diff 门会失败

// 签名见证：契约与实现的编译期绑定（参数数或类型不符即 E0308）
const _: fn(&str) -> bool = super::layout::is_archi_rel_path;
const _: fn() -> String = super::layout::archi_rel_dir;
const _: fn() -> String = super::layout::archi_models_rel_dir;
const _: fn() -> String = super::layout::archi_graph_rel_dir;
const _: fn() -> String = super::layout::archi_graph_model_rel_dir;
const _: fn(&str) -> String = super::identity::workspace_identity_segment;
const _: fn(&str) -> bool = super::prune::is_pruned_dir_name;
const _: fn(&str) -> bool = super::windows_name::is_reserved_path;
const _: fn() -> bool = super::CancelFlag::is_set;
