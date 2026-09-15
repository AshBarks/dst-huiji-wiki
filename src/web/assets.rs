//! Embedded frontend assets (single-binary deployment).
//!
//! JS 以 ES modules 分发（无打包器）：`app.js` 为入口，`js/` 下是按页/域
//! 拆分的模块。`JS_MODULES` 是文件名 → 内容的静态表，由
//! `/static/js/{file}` 路由 serve（include_str 需要字面量路径）。

pub const INDEX_HTML: &str = include_str!("assets/index.html");
pub const APP_JS: &str = include_str!("assets/app.js");
pub const STYLE_CSS: &str = include_str!("assets/style.css");

pub const JS_MODULES: &[(&str, &str)] = &[
    ("util.js", include_str!("assets/js/util.js")),
    ("cooking_eval.js", include_str!("assets/js/cooking_eval.js")),
    ("jobs.js", include_str!("assets/js/jobs.js")),
    ("skills.js", include_str!("assets/js/skills.js")),
    ("icon_shared.js", include_str!("assets/js/icon_shared.js")),
    (
        "inventory_icons.js",
        include_str!("assets/js/inventory_icons.js"),
    ),
    (
        "crafting_icons.js",
        include_str!("assets/js/crafting_icons.js"),
    ),
    ("anim_assets.js", include_str!("assets/js/anim_assets.js")),
    (
        "skilltree_icons.js",
        include_str!("assets/js/skilltree_icons.js"),
    ),
    ("source_icons.js", include_str!("assets/js/source_icons.js")),
    (
        "pages/dashboard.js",
        include_str!("assets/js/pages/dashboard.js"),
    ),
    (
        "pages/recipes.js",
        include_str!("assets/js/pages/recipes.js"),
    ),
    (
        "pages/cooking.js",
        include_str!("assets/js/pages/cooking.js"),
    ),
    (
        "pages/translations.js",
        include_str!("assets/js/pages/translations.js"),
    ),
    (
        "pages/constants.js",
        include_str!("assets/js/pages/constants.js"),
    ),
    ("pages/anims.js", include_str!("assets/js/pages/anims.js")),
    (
        "pages/snapshots.js",
        include_str!("assets/js/pages/snapshots.js"),
    ),
];
