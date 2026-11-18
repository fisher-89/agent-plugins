# App.Backend.Infra.ArchiRuntime

## 职能

架构漂移门禁执行体：取输入 → 调判定 → 落报告 → 派生结论。

**边界**：零子进程（无外部命令、无 shell）、零版本控制（变更集经 port 注入）、零库句柄、
零工作区配置读取。判定本身在判定域（core）里，本模块只做执行面。

## 不变量

- 报告写盘失败 MUST NOT 返回 `Err`——审计面失败不阻断结论，主流程不中断。
- 取消以 `fault` 正常返回，MUST NOT 用 `Err` 表达。
- 门禁 `passed` = 零违例且零模型结构错；执行体 MUST NOT 自行改写该判据。

## 操作

### file_stem

时间戳 → 报告文件名主干：只保留 ASCII 数字与 `T` / `Z`（`2026-10-20T08:31:12Z` →
`20261020T083112Z`；冒号在 Windows 文件名上非法）。空输入退化为 `report`。

```json api-op
{
  "owner": "LocalArchiCheck",
  "params": [{ "name": "timestamp", "schema": { "type": "string" } }],
  "returns": { "type": "string" }
}
```

## 类型

```json api-types
{
  "LocalArchiCheck": {
    "type": "object",
    "properties": {},
    "x-common": { "note": "零字段门禁执行体：类型本身即名义性，构造器由生成器产出" }
  }
}
```
