//! 夹具生成器：造一个 native_db 布局的 redb 文件，用来验证 redb-peek 的各条路径。
//!
//! 布局与 native_db 0.8 一致：主表 `Table<DatabaseInnerKeyValue, &[u8]>`，
//! 二级索引表 `MultimapTable<DatabaseInnerKeyValue, DatabaseInnerKeyValue>`，
//! 外加它自己的 `metadata` 表 `Table<&str, &str>`。行值 = native_model 头
//! （u32 LE id + u32 LE version）+ 载荷。
//!
//! 运行：`cargo test --test fixture -- --nocapture`，夹具写进临时目录并打印路径。

use std::path::PathBuf;

use redb::{
    MultimapTableDefinition, MultimapTableHandle, TableDefinition, TableHandle, TypeName, Value,
};

/// 与 native_db::db_type::Key 同身份：自定义类型名 `DatabaseInnerKeyValue`。
#[derive(Debug)]
struct NativeKey;

impl Value for NativeKey {
    type SelfType<'a> = &'a [u8];
    type AsBytes<'a> = &'a [u8];
    fn fixed_width() -> Option<usize> {
        None
    }
    fn from_bytes<'a>(data: &'a [u8]) -> &'a [u8]
    where
        Self: 'a,
    {
        data
    }
    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8]
    where
        Self: 'b,
    {
        value
    }
    fn type_name() -> TypeName {
        TypeName::new("DatabaseInnerKeyValue")
    }
}

impl redb::Key for NativeKey {
    fn compare(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        a.cmp(b)
    }
}

fn wrapped(id: u32, version: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(body);
    out
}

#[test]
fn build_fixture() {
    let dir = std::env::temp_dir().join("redb-peek-fixture");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join("fixture.redb");

    let db = redb::Database::create(&path).unwrap();
    let txn = db.begin_write().unwrap();
    {
        // 主表：7_2_id —— 三行会话，载荷是 JSON（模拟 SerdeJsonCodec）
        let mut t = txn
            .open_table(TableDefinition::<NativeKey, &'static [u8]>::new("7_2_id"))
            .unwrap();
        for (i, name) in ["ses-alpha", "ses-beta", "ses-gamma"].iter().enumerate() {
            let body = format!(
                r#"{{"id":"{name}","createdAt":{},"model":"m-high"}}"#,
                1_700_000_000_000i64 + i as i64
            );
            let v = wrapped(7, 2, body.as_bytes());
            t.insert(name.as_bytes(), v.as_slice()).unwrap();
        }
        drop(t);

        // 主表：2_4_id —— bincode 风格载荷（无可读 JSON），验证 --strings
        let mut t2 = txn
            .open_table(TableDefinition::<NativeKey, &'static [u8]>::new("2_4_id"))
            .unwrap();
        let mut body = Vec::new();
        body.extend_from_slice(&1i64.to_le_bytes()); // id
        let s1 = b"ses-alpha-0001";
        body.extend_from_slice(&(s1.len() as u64).to_le_bytes());
        body.extend_from_slice(s1);
        body.extend_from_slice(&2i32.to_le_bytes()); // 枚举序号
        let s2 = "轮结束但未产出收敛事件";
        body.extend_from_slice(&(s2.len() as u64).to_le_bytes());
        body.extend_from_slice(s2.as_bytes());
        let v = wrapped(2, 4, &body);
        t2.insert(1i64.to_be_bytes().as_slice(), v.as_slice()).unwrap();
        drop(t2);

        // 二级索引表：8_1_session_id —— multimap，验证索引 dump 分支
        let mut mm = txn
            .open_multimap_table(MultimapTableDefinition::<NativeKey, NativeKey>::new(
                "8_1_session_id",
            ))
            .unwrap();
        for (session, seqs) in [
            ("ses-alpha", vec![0u64, 1, 2]),
            ("ses-beta", vec![0u64]),
        ] {
            for seq in seqs {
                let mut key = Vec::new();
                key.extend_from_slice(&seq.to_be_bytes());
                mm.insert(session.as_bytes(), key.as_slice()).unwrap();
            }
        }
        drop(mm);

        // 转录事件表：8_1_event_key —— 真实形态是 SerdeJsonCodec，载荷为
        // AgentEvent JSON（`{"event":{"kind":"message","blocks":[{"kind":"toolUse","name":"Bash"}]}}`）。
        // 键是复合打包 `(hash64(session)<<64) | seq`。
        let mut ev = txn
            .open_table(TableDefinition::<NativeKey, &'static [u8]>::new("8_1_event_key"))
            .unwrap();
        let transcripts = [
            (0xc0ffeeu64, 0u64, r#"{"seq":0,"kind":"runStarted"}"#),
            (
                0xc0ffeeu64,
                1u64,
                r#"{"seq":1,"kind":"message","role":"assistant","blocks":[{"kind":"toolUse","name":"Bash","input":{"command":"ls"}}]}"#,
            ),
            (
                0xc0ffeeu64,
                2u64,
                r#"{"seq":2,"kind":"message","role":"tool","blocks":[{"kind":"toolResult","content":"a.txt b.txt"}]}"#,
            ),
        ];
        for (session_hash, seq, body) in transcripts {
            let key = ((session_hash as u128) << 64) | seq as u128;
            let v = wrapped(8, 1, body.as_bytes());
            ev.insert(key.to_be_bytes().as_slice(), v.as_slice()).unwrap();
        }
        drop(ev);

        // native_db 自己的 metadata 表
        let mut meta = txn
            .open_table(TableDefinition::<&str, &str>::new("metadata"))
            .unwrap();
        meta.insert("version_native_db", "0.8.2").unwrap();
        meta.insert("version_native_model", "0.4.20").unwrap();
        drop(meta);
    }
    txn.commit().unwrap();
    drop(db);

    // 断言：自造的表能被正确重新打开（类型名与 native_db 对齐）
    let db = redb::Database::open(&path).unwrap();
    let r = db.begin_read().unwrap();
    let names: Vec<String> = r.list_tables().unwrap().map(|h| h.name().to_string()).collect();
    assert!(names.contains(&"7_2_id".to_string()));
    let mm: Vec<String> = r
        .list_multimap_tables()
        .unwrap()
        .map(|h| h.name().to_string())
        .collect();
    assert!(mm.contains(&"8_1_session_id".to_string()));

    println!("FIXTURE={}", path.display());
}
