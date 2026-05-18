## 1. 模板汉化

- [x] 1.1 翻译 `templates/artifacts/proposal.md.template` —— 章节标题、表格列头、标签文字为中文
- [x] 1.2 翻译 `templates/artifacts/test-design.md.template` —— 章节标题、表格列头、标签文字为中文
- [x] 1.3 翻译 `templates/artifacts/design.md.template` —— 章节标题、表格列头、标签文字为中文

## 2. Planner agent prompt 增加中文约束

- [x] 2.1 在 `requirements-planner.md` 中增加 `## Language` 约束段，要求正文中文、代码标识符/路径/术语保持英文
- [x] 2.2 在 `test-design-planner.md` 中增加 `## Language` 约束段，要求正文中文、代码标识符/路径/术语保持英文
- [x] 2.3 在 `dev-proposal-planner.md` 中增加 `## Language` 约束段，要求正文中文、代码标识符/路径/术语保持英文

## 3. 验证

- [x] 3.1 在 demo-project 中创建测试 change，运行 `phase-requirements` 验证 proposal.md 为中文输出
- [x] 3.2 运行 `phase-test-design` 验证 test-design.md 为中文输出
- [x] 3.3 运行 `phase-dev-proposal` 验证 design.md 和 tasks.md 为中文输出
