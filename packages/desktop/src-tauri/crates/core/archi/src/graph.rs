//! 图单点：`ModelGraph` 是唯一建图点（持有模型本体 + petgraph 邻接），四类自
//! 实现结构诊断（成环 / 依赖祖先 / 未定义引用 / 孤儿层级）与邻域查询的单点。
//!
//! 消费面（进程内校验 / agent 工具 / IPC / 画布）MUST NOT 各自再建图、再算环、
//! 再推祖先；1-hop 邻域由本模块三项查询组合，MUST NOT 在投影层做二次图遍历。

use std::collections::BTreeMap;

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::dto::DiagnosticKind;
use crate::model::{Diagnostic, Model, Relation};

/// 内存架构图（唯一构建点产物；非 `Clone`——单点持有语义）。
pub struct ModelGraph {
    /// 模型本体（图与模型同体：拿不到独立副本即无从再建一份图）
    model: Model,
    /// FQN → 节点下标
    node_of: BTreeMap<String, NodeIndex>,
    /// 节点下标 → FQN
    fqn_of: Vec<String>,
    /// 关系边（仅两端皆已声明者入图）
    graph: DiGraph<(), ()>,
    /// FQN → 出边在 `model.relations` 内的下标
    out_edges: BTreeMap<String, Vec<usize>>,
    /// FQN → 入边在 `model.relations` 内的下标
    in_edges: BTreeMap<String, Vec<usize>>,
}

/// 1-hop 邻域三项（出边 / 入边 / 直接子元素）。
pub struct Neighborhood<'a> {
    /// 出边（按 `(from, to)` 字典序）
    pub depends_on: Vec<&'a Relation>,
    /// 入边（按 `(from, to)` 字典序）
    pub depended_by: Vec<&'a Relation>,
    /// 直接子元素 FQN（字典序）
    pub children: Vec<&'a str>,
}

impl ModelGraph {
    /// 唯一建图点：元素建节点、关系建边（仅两端皆已声明者入图；未定义引用的边
    /// 不入图——留给 `unknownReference` 诊断）。
    pub fn build(model: Model) -> ModelGraph {
        let mut graph: DiGraph<(), ()> = DiGraph::new();
        let mut node_of: BTreeMap<String, NodeIndex> = BTreeMap::new();
        let mut fqn_of: Vec<String> = Vec::with_capacity(model.elements.len());
        for element in &model.elements {
            let index = graph.add_node(());
            node_of.insert(element.fqn.clone(), index);
            fqn_of.push(element.fqn.clone());
        }
        let mut out_edges: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut in_edges: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, relation) in model.relations.iter().enumerate() {
            let (Some(from), Some(to)) = (
                node_of.get(&relation.from).copied(),
                node_of.get(&relation.to).copied(),
            ) else {
                continue;
            };
            graph.add_edge(from, to, ());
            out_edges
                .entry(relation.from.clone())
                .or_default()
                .push(index);
            in_edges.entry(relation.to.clone()).or_default().push(index);
        }
        ModelGraph {
            model,
            node_of,
            fqn_of,
            graph,
            out_edges,
            in_edges,
        }
    }

    /// 模型只读访问（避免 `(model, graph)` 双参数形态）。
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// 出边（按 `(from, to)` 字典序）。
    pub fn depends_on(&self, fqn: &str) -> Vec<&Relation> {
        self.relations_of(&self.out_edges, fqn)
    }

    /// 入边（按 `(from, to)` 字典序）。
    pub fn depended_by(&self, fqn: &str) -> Vec<&Relation> {
        self.relations_of(&self.in_edges, fqn)
    }

    /// 下标表 → 关系引用清单（稳定序单点）。
    fn relations_of(&self, table: &BTreeMap<String, Vec<usize>>, fqn: &str) -> Vec<&Relation> {
        let mut relations: Vec<&Relation> = table
            .get(fqn)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|index| self.model.relations.get(*index))
                    .collect()
            })
            .unwrap_or_default();
        relations.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then_with(|| left.to.cmp(&right.to))
        });
        relations
    }

    /// 直接子元素（FQN 前缀一级，字典序）。
    pub fn children(&self, fqn: &str) -> Vec<&str> {
        let prefix = format!("{fqn}.");
        let mut children: Vec<&str> = self
            .model
            .elements
            .iter()
            .filter(|element| {
                element
                    .fqn
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('.'))
            })
            .map(|element| element.fqn.as_str())
            .collect();
        children.sort_unstable();
        children
    }

    /// 1-hop 邻域（三项组合，零二次图遍历）。
    pub fn neighborhood(&self, fqn: &str) -> Neighborhood<'_> {
        Neighborhood {
            depends_on: self.depends_on(fqn),
            depended_by: self.depended_by(fqn),
            children: self.children(fqn),
        }
    }

    /// 四类自实现结构诊断（一次列全、顺序固定：孤儿层级 → 成环 → 依赖祖先 →
    /// 未定义引用；各带 `file` + `line`）。
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        diagnostics.extend(self.orphan_hierarchy());
        diagnostics.extend(self.cycles());
        diagnostics.extend(self.depends_on_ancestor());
        diagnostics.extend(self.unknown_reference());
        diagnostics
    }

    /// 子元素已声明而祖先未声明（隐式祖先 MUST NOT 被自动创建）。
    fn orphan_hierarchy(&self) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        for element in &self.model.elements {
            let mut segments: Vec<&str> = element.fqn.split('.').collect();
            while segments.len() > 1 {
                segments.pop();
                let ancestor = segments.join(".");
                if !self.node_of.contains_key(&ancestor) {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::OrphanHierarchy,
                        message: format!(
                            "孤儿层级：{} 已声明，其祖先 {} 未声明（祖先须显式声明）",
                            element.fqn, ancestor
                        ),
                        file: element.file.clone(),
                        line: element.line,
                        column: None,
                    });
                    break;
                }
            }
        }
        diagnostics
    }

    /// 成环（SCC：多节点分量或单节点自环；`message` 含环上 FQN 路径）。
    fn cycles(&self) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        for component in tarjan_scc(&self.graph) {
            let self_loop = component.len() == 1
                && self
                    .graph
                    .neighbors_directed(component[0], petgraph::Direction::Outgoing)
                    .any(|target| target == component[0]);
            if component.len() < 2 && !self_loop {
                continue;
            }
            let mut fqns: Vec<&str> = component
                .iter()
                .map(|index| self.fqn_of[index.index()].as_str())
                .collect();
            fqns.sort_unstable();
            let Some(first) = self.model.element(fqns[0]) else {
                continue;
            };
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Cycle,
                message: format!("依赖成环：{} -> {}", fqns.join(" -> "), fqns[0]),
                file: first.file.clone(),
                line: first.line,
                column: None,
            });
        }
        diagnostics
    }

    /// 元素依赖自己的祖先。
    fn depends_on_ancestor(&self) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        for relation in &self.model.relations {
            let ancestor_prefix = format!("{}.", relation.to);
            if relation.from.starts_with(&ancestor_prefix) {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::DependsOnAncestor,
                    message: format!(
                        "依赖祖先：{} 依赖其祖先 {}（祖先不参与实现依赖）",
                        relation.from, relation.to
                    ),
                    file: relation.file.clone(),
                    line: relation.line,
                    column: None,
                });
            }
        }
        diagnostics
    }

    /// 关系引用不存在的元素（指明引用方与缺失 FQN）。
    fn unknown_reference(&self) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        for relation in &self.model.relations {
            let missing: Vec<&str> = [relation.from.as_str(), relation.to.as_str()]
                .into_iter()
                .filter(|fqn| !self.node_of.contains_key(*fqn))
                .collect();
            if missing.is_empty() {
                continue;
            }
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::UnknownReference,
                message: format!(
                    "未定义引用：{} -> {}（{} 未声明）",
                    relation.from,
                    relation.to,
                    missing.join(" / ")
                ),
                file: relation.file.clone(),
                line: relation.line,
                column: None,
            });
        }
        diagnostics
    }
}
