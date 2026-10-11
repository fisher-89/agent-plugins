p = 'crates/core/orchestration/src/state_test.rs'
s = open(p, encoding='utf-8', newline='').read()


def rep(old, new, cnt=1):
    global s
    o = old.replace('\n', '\r\n')
    nn = new.replace('\n', '\r\n')
    assert s.count(o) == cnt, (s.count(o), old[:80])
    s = s.replace(o, nn)


rep("""/// ConfirmWait 载荷形态守卫（异常）：旧 `{ phase }` 单字段载荷反序列化拒绝
/// （`archive` 为必填键——缺键即线面漂移）；六变体 `RunNotice` 不变（零新
/// kind，AC-7 / D2）。
#[test]
fn confirm_wait旧载荷缺archive键拒绝且notice六变体不变() {
    let legacy = json!({"ipc": "confirmWait", "phase": "test-gen"});
    let back: Result<RunUpdate, _> = serde_json::from_value(legacy);
    assert!(
        back.is_err(),
        "旧单字段 ConfirmWait 载荷应拒绝（archive 必填键）"
    );

    // 六变体 kind 词封闭集不变（Step / Edge / SessionEvent / Ask /
    // ConfirmWait / Finished）
    use crate::state::RunNotice;""",
    """/// ConfirmWait 载荷形态守卫（异常 / 边界）：`archive` 键恒随行出线（缺键即
/// 线面漂移——进程内 seam 载荷无入站反序列化面，契约以出线键集与文档面钉
/// 死）；六变体 `RunNotice` 不变（零新 kind，AC-7 / D2）。
#[test]
fn confirm_wait载荷archive键恒在场且notice六变体不变() {
    let legacy = RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
        archive: None,
    };
    let value = serde_json::to_value(&legacy).expect("出线应成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("ConfirmWait 为对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["ipc", "phase", "archive"],
        "archive 为恒在场键（零键省形态）"
    );
    assert_eq!(
        value["archive"],
        json!(null),
        "缺省 archive 出线 null（留位不省键）"
    );

    // 六变体 kind 词封闭集不变（Step / Edge / SessionEvent / Ask /
    // ConfirmWait / Finished）
    use crate::state::RunNotice;""")

with open(p, 'w', encoding='utf-8', newline='') as f:
    f.write(s)
print('ok')
