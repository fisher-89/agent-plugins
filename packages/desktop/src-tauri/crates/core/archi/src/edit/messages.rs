//! 写面文案族：判据句（[`EditErrorKind::criterion`]）+ 事实段（[`EditError::facts`]）。
//!
//! 判据句是**操作中立**的封闭词表（同一族里元素写回与关系写回共用这些句子，不出现
//! 「本操作专属」的词），事实段承载「当前事实 + 下一步」（缺席清单、回读到的值、阻塞
//! 删除的子元素 / 关系对、期望与实际摘要）。
//!
//! 错误渲染只有 [`EditError::message`] 一个出口：`EditError` MUST NOT 实现 `Display` /
//! `Error`——第二条渲染路径会让「变体集合 == 文案集合」不可对拍。构造同理只有私有
//! [`error`] 一个入口。
//!
//! 机械判据：[`EditErrorKind::ALL`] 与 `criterion()` 的穷尽 `match` 分支集合逐值相等
//! （新增变体未同批补 `ALL` 即数字面不符），`criterion()` 逐值非空且互异。

/// 写面错误：失败的判据（封闭枚举）+ 当前事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError {
    /// 失败的是哪条判据
    pub kind: EditErrorKind,
    /// 当前事实 + 下一步
    pub facts: String,
}

impl EditError {
    /// 唯一渲染出口，恒为「判据句：事实」。
    pub fn message(&self) -> String {
        format!("{}：{}", self.kind.criterion(), self.facts)
    }
}

/// 写面判据（封闭二十三值：前置组二十一 + 自证组二）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditErrorKind {
    /// 模型结构诊断非空（写坏模型 = 读出错）
    ModelHasDiagnostics,
    /// 目标元素不在案
    ElementNotFound,
    /// 元素声明不是表形态
    ElementNotTable,
    /// 新建元素的父元素未声明（祖先须显式声明）
    ParentMissing,
    /// 关系端点元素未声明
    RelationEndpointMissing,
    /// 落点非法（缺 `file` / 逃出模型声明目录一级 / 目录缺席 / 改既有元素落点）
    TargetInvalid,
    /// 新建元素缺 `kind`
    ElementKindMissing,
    /// 元素已在案（给 `kind` 即新建意图，与在案冲突）
    ElementAlreadyExists,
    /// 新建元素的 FQN 形态非法（含空白 / 引号 / 方括号 / `#` / 换行）
    FqnInvalid,
    /// 模型文件读取失败
    SourceReadFailed,
    /// 模型源不可解析
    SourceParseFailed,
    /// 模型源缺少 `element` 表
    ElementTableMissing,
    /// 目标文件已变（`expect` 乐观锁不匹配）
    ExpectMismatch,
    /// 元素仍有已声明子元素，拒绝删除
    DeleteHasChildren,
    /// 元素仍被关系引用，拒绝删除
    DeleteHasRelations,
    /// 目标关系不在案
    RelationNotFound,
    /// 同一 `(from, to)` 多处声明（模型面跨文件 / 文件面同源，fail-closed）
    RelationDuplicated,
    /// 内联删边项带了字段（删除动作与字段赋值互斥）
    RelationDeleteHasFields,
    /// 同一批 `relations` 重复声明同一目标（先 upsert 后 delete 取决于数组序）
    RelationDuplicateInBatch,
    /// 级联闭包跨模型文件（一次计划只承载一个文件）
    CascadeCrossFile,
    /// 内联边已声明在别的模型文件（一次计划只写一个文件）
    RelationOwnedElsewhere,
    /// 渲染后重解析出诊断（自证失败，零文本产出）
    RenderSelfCheck,
    /// 回读值与规格不一致（零文本产出）
    ReadBackMismatch,
}

impl EditErrorKind {
    /// 判据句（操作中立、逐值非空且互异）。
    pub fn criterion(self) -> &'static str {
        match self {
            EditErrorKind::ModelHasDiagnostics => "模型存在结构诊断，拒绝写回",
            EditErrorKind::ElementNotFound => "元素不存在",
            EditErrorKind::ElementNotTable => "元素声明不是表形态",
            EditErrorKind::ParentMissing => "父元素不在案",
            EditErrorKind::RelationEndpointMissing => "关系端点元素不存在",
            EditErrorKind::TargetInvalid => "落点非法",
            EditErrorKind::ElementKindMissing => "新建元素缺 kind",
            EditErrorKind::ElementAlreadyExists => "元素已在案",
            EditErrorKind::FqnInvalid => "元素 FQN 形态非法",
            EditErrorKind::SourceReadFailed => "模型文件读取失败",
            EditErrorKind::SourceParseFailed => "模型源解析失败",
            EditErrorKind::ElementTableMissing => "模型源缺少 element 表",
            EditErrorKind::ExpectMismatch => "目标文件已变",
            EditErrorKind::DeleteHasChildren => "元素仍有已声明子元素，拒绝删除",
            EditErrorKind::DeleteHasRelations => "元素仍被关系引用，拒绝删除",
            EditErrorKind::RelationNotFound => "关系不存在",
            EditErrorKind::RelationDuplicated => "关系多处声明",
            EditErrorKind::RelationDeleteHasFields => "内联删边项带了字段",
            EditErrorKind::RelationDuplicateInBatch => "同一批 relations 重复声明同一目标",
            EditErrorKind::CascadeCrossFile => "级联闭包跨模型文件",
            EditErrorKind::RelationOwnedElsewhere => "内联边声明在别的模型文件",
            EditErrorKind::RenderSelfCheck => "写回渲染自证失败",
            EditErrorKind::ReadBackMismatch => "回读不一致",
        }
    }

    /// 全部判据（与 `criterion()` 的分支集合逐值相等——长度即对拍判据）。
    pub const ALL: [EditErrorKind; 23] = [
        EditErrorKind::ModelHasDiagnostics,
        EditErrorKind::ElementNotFound,
        EditErrorKind::ElementNotTable,
        EditErrorKind::ParentMissing,
        EditErrorKind::RelationEndpointMissing,
        EditErrorKind::TargetInvalid,
        EditErrorKind::ElementKindMissing,
        EditErrorKind::ElementAlreadyExists,
        EditErrorKind::FqnInvalid,
        EditErrorKind::SourceReadFailed,
        EditErrorKind::SourceParseFailed,
        EditErrorKind::ElementTableMissing,
        EditErrorKind::ExpectMismatch,
        EditErrorKind::DeleteHasChildren,
        EditErrorKind::DeleteHasRelations,
        EditErrorKind::RelationNotFound,
        EditErrorKind::RelationDuplicated,
        EditErrorKind::RelationDeleteHasFields,
        EditErrorKind::RelationDuplicateInBatch,
        EditErrorKind::CascadeCrossFile,
        EditErrorKind::RelationOwnedElsewhere,
        EditErrorKind::RenderSelfCheck,
        EditErrorKind::ReadBackMismatch,
    ];
}

/// 全 crate 的 `EditError` 构造单点（MUST NOT 在别处另有第二种拼装）。
pub(super) fn error(kind: EditErrorKind, facts: impl Into<String>) -> EditError {
    EditError {
        kind,
        facts: facts.into(),
    }
}
