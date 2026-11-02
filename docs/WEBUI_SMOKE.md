# WebUI 手工冒烟清单（Playwright）

CI 只做 Rust 测试/静态检查与前端 JS 语法检查（`node --input-type=module --check`）。
涉及表单渲染、下拉来源、必填校验这类浏览器行为时，在本机按本清单验证。

## 1. 启动测试服务器

```bash
cargo build            # 前端资产用 include_str! 嵌入，改 JS 后必须重新 build
target/debug/dst-huiji-wiki serve --port 3099
```

- 需要 `.env` 里的 `DST__ROOT`；不要占用正式端口（默认 3080）。
- 改完 `src/web/assets/` 后不重启进程不会生效（资产在编译期嵌入）。

## 2. 快速视觉检查

```bash
playwright screenshot --wait-for-timeout 2500 --viewport-size 1280,900 \
  'http://127.0.0.1:3099/#/jobs' /tmp/jobs.png
```

## 3. 交互检查脚本

需要本机有 Python Playwright（`pip install playwright` 或 `uv add playwright`）。

```python
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    b = p.chromium.launch()
    pg = b.new_page(viewport={"width": 1280, "height": 900})
    pg.goto("http://127.0.0.1:3099/#/jobs")
    pg.wait_for_timeout(1500)

    # 下拉来自 /api/jobs/specs，值必须是 serde tag（snake）
    opts = pg.eval_on_selector_all("#jobKind option", "els=>els.map(e=>e.value)")
    assert "update_local" in opts and "update_scan" in opts
    assert "parse-po" not in opts  # 已改为 parse_po；这里断言错误值不会出现

    # 无手写布局的作业：通用表单按 ParamSpec 生成，必填项带 required
    pg.select_option("#jobKind", "update_scan")
    pg.wait_for_timeout(300)
    required = pg.eval_on_selector_all(
        "#jobFields input[required]", "els=>els.map(e=>e.dataset.field)"
    )
    assert required == ["old", "new"]

    # 只读作业给出只读提示
    pg.select_option("#jobKind", "update_check")
    pg.wait_for_timeout(200)
    assert "只读" in pg.inner_text("#jobHint")

    # StrList 字段按逗号拆分
    pg.select_option("#jobKind", "maintain_wikitext")
    pg.wait_for_timeout(200)
    assert pg.get_attribute('#jobFields input[data-field="pages"]', "data-list") == "1"

    b.close()
```

## 4. 断言清单

- 下拉项数量 == `JOB_SPECS` 中 `web_ui = true` 的数量；改 `web_ui` 后无需改 JS。
- 提交 `kind` 是 serde tag（`update_local`），不是 CLI 的 kebab 名（`update-local`）。
- 无布局作业：Bool→勾选、Usize→数字、StrList→逗号分隔文本、其余→文本；必填有"（必填）"且空提交被拦下。
- 有布局作业：显示 `JOB_UI` 里的专用控件；`readonly` 作业不弹"将真实执行本地文件操作"。
- `/api/jobs/specs` 不可用时任务页显示加载失败，而不是静默空下拉。
