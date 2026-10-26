// 生成期自检桩（手写、非生成物、不被导出链写入）：只 import 纯 schema 产物，
// 让 `tsc --noEmit -p tsconfig.zod.json` 在**声明期**就能看住三件事：
//   1. struct 面 strict 化（`z.strictObject`）没有退化成 loose / 未知 API；
//   2. 递归引用已 getter 化（`z.lazy` 自引用初值会触发 TS7022 / TS7023）；
//   3. 产物零类型别名（`export type` 被删后若残留注释里的类型引用即悬空名）。
// 零 DOM / 零 Tauri / 零网络 / 零测试框架：整份 program 只有本桩 + 产物 + zod 类型。
import * as zodSchemas from './bindings.zod';

// 引用一次即让整份产物进入 program（`noUnusedLocals` 也就干净了）；不做任何断言或
// 类型注解——自检的判据是「能编译」，不是「能收窄」。
export const zodProduct = zodSchemas;
