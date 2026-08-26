//! Version/type classification of main-namespace pages (WIKI_CORPUS_PLAN §5).
//!
//! Signal precedence: S0 redirect flag → S1 disambiguation template →
//! S2 infobox/talk template game parameter (`dst` vs `ds`) → S3 version
//! categories → fallback `Unknown`. Nothing is guessed silently: pages with
//! no matching signal end up `Unknown` and belong to the manual triage queue.

use crate::corpus::model::{ClassConfidence, GameClass};

/// Template fragments that self-declare the game version. Needle design
/// matters: every ambiguous prefix carries its trailing delimiter so that
/// `|ds|` cannot match inside `|dst|`, and `{{Pic` cannot match `{{DSPic`
/// (the `{` prefix fails on `S`). The `…台词` / `…信息框` families therefore
/// need both `|x|` and `|x}}` forms.
const DST_MARKERS: &[&str] = &[
    "{{实体信息框/自动|dst|",
    "{{实体信息框|dst|",
    "|dst}}",
    "{{全角色台词|dst",
    "{{DST}}",
    "{{Pic|",
];

const DS_MARKERS: &[&str] = &[
    "{{实体信息框/自动|ds|",
    "{{实体信息框|ds|",
    "|ds}}",
    "{{全角色台词|ds|",
    "{{全角色台词|ds}}",
    "{{DS}}",
    "{{DSPic",
];

/// Category titles (as returned by `prop=categories`, i.e. with the
/// `分类:` namespace prefix) that indicate a version.
const DST_CATEGORIES: &[&str] = &["分类:联机版"];

const DS_CATEGORIES: &[&str] = &["分类:单机版", "分类:海难", "分类:巨人国", "分类:猪镇"];

const DISAMBIG_MARKER: &str = "{{消歧义";

/// This wiki's single-player subpage convention: `<实体>/单机版`（如
/// `熟芦笋/单机版`，实测 577 页；不存在 `/联机版` 对应形式）。
const DS_TITLE_SUFFIX: &str = "/单机版";

/// Outcome of classifying one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub game_class: GameClass,
    pub signals: Vec<String>,
    pub confidence: ClassConfidence,
}

impl Classification {
    fn new(game_class: GameClass, confidence: ClassConfidence, signals: &[&str]) -> Self {
        Self {
            game_class,
            signals: signals.iter().map(|s| (*s).to_string()).collect(),
            confidence,
        }
    }
}

/// Classifies one page from its title, enumeration flags, wikitext and
/// categories.
///
/// `wikitext` is `None` for pages that were enumerated but not (yet) fetched;
/// those can only be classified from categories, the redirect flag or the
/// title convention.
pub fn classify(
    title: &str,
    redirect: bool,
    wikitext: Option<&str>,
    categories: &[String],
) -> Classification {
    // S0: redirects are known from enumeration alone; their content is just
    // `#重定向 [[…]]` and carries no version markers worth trusting.
    if redirect {
        return Classification::new(
            GameClass::Redirect,
            ClassConfidence::High,
            &["flag:redirect"],
        );
    }

    let cat_dst = categories
        .iter()
        .any(|c| DST_CATEGORIES.contains(&c.as_str()));
    let cat_ds = categories
        .iter()
        .any(|c| DS_CATEGORIES.contains(&c.as_str()));

    let Some(text) = wikitext else {
        return classify_without_templates(title, cat_dst, cat_ds);
    };

    // S1: disambiguation pages link to both versions by design.
    if text.contains(DISAMBIG_MARKER) {
        return Classification::new(GameClass::Disambig, ClassConfidence::High, &["tpl:消歧义"]);
    }

    let tpl_dst = DST_MARKERS.iter().any(|m| text.contains(m));
    let tpl_ds = DS_MARKERS.iter().any(|m| text.contains(m));

    match (tpl_dst, tpl_ds) {
        (true, true) => {
            return Classification::new(
                GameClass::Mixed,
                ClassConfidence::High,
                &["tpl_param:mixed"],
            );
        }
        (true, false) => {
            let mut signals = vec!["tpl_param:dst"];
            if cat_dst {
                signals.push("cat:联机版");
            }
            return Classification::new(GameClass::Dst, ClassConfidence::High, &signals);
        }
        (false, true) => {
            let mut signals = vec!["tpl_param:ds"];
            if cat_ds {
                signals.push("cat:单机版系");
            }
            return Classification::new(GameClass::Ds, ClassConfidence::High, &signals);
        }
        (false, false) => {}
    }

    classify_without_templates(title, cat_dst, cat_ds)
}

fn classify_without_templates(title: &str, cat_dst: bool, cat_ds: bool) -> Classification {
    match (cat_dst, cat_ds) {
        (true, true) => {
            Classification::new(GameClass::Mixed, ClassConfidence::Medium, &["cat:both"])
        }
        (true, false) => {
            Classification::new(GameClass::Dst, ClassConfidence::Medium, &["cat:联机版"])
        }
        (false, true) => {
            Classification::new(GameClass::Ds, ClassConfidence::Medium, &["cat:单机版系"])
        }
        // S4: 单机版子页后缀命名惯例（`熟芦笋/单机版` 等）。
        (false, false) if title.ends_with(DS_TITLE_SUFFIX) => {
            Classification::new(GameClass::Ds, ClassConfidence::Medium, &["title:/单机版"])
        }
        (false, false) => Classification::new(GameClass::Unknown, ClassConfidence::Low, &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    const HOUND_SNIPPET: &str = r#"{{RichTab/信息框
|猎犬|{{实体信息框/自动|dst|hound
|掉落 = {{Pic|32|怪物肉}}×1<br>{{Pic|32|犬牙}}×1（12.5%）
}}
{{全角色台词|dst|hound}}
'''猎犬（{{en|Hound}}）'''是[[饥荒：联机版]]{{DST}}中的一种[[生物]]。"#;

    #[test]
    fn dst_entity_page() {
        let c = classify("猎犬", false, Some(HOUND_SNIPPET), &cats(&["分类:联机版"]));
        assert_eq!(c.game_class, GameClass::Dst);
        assert_eq!(c.confidence, ClassConfidence::High);
        assert!(c.signals.contains(&"tpl_param:dst".to_string()));
    }

    #[test]
    fn ds_entity_page_via_dspic() {
        // 虎鲨-style page: ds infobox param + DSPic + 单机版/海难 categories.
        let text = r#"{{实体信息框/自动|ds|tigershark
|掉落 = {{DSPic|32|生鱼肉}}×8（50%）
}}
{{全角色台词|ds|tigershark|首选=wolfgang}}"#;
        let c = classify(
            "虎鲨",
            false,
            Some(text),
            &cats(&["分类:单机版", "分类:海难"]),
        );
        assert_eq!(c.game_class, GameClass::Ds);
        assert_eq!(c.confidence, ClassConfidence::High);
    }

    #[test]
    fn mixed_page_with_both_params() {
        let text = "{{实体信息框/自动|dst|a}}\n{{实体信息框/自动|ds|b}}";
        let c = classify("聚合页", false, Some(text), &[]);
        assert_eq!(c.game_class, GameClass::Mixed);
        assert_eq!(c.confidence, ClassConfidence::High);
    }

    #[test]
    fn dst_marker_does_not_match_inside_ds_marker() {
        // "{{DSPic" alone must not trip the dst side ("{{Pic" needle).
        let text = "正文引用 {{DSPic|32|x}} 而已";
        assert!(!DST_MARKERS.iter().any(|m| text.contains(m)));
        let c = classify("x", false, Some(text), &[]);
        assert_eq!(c.game_class, GameClass::Ds);
    }

    #[test]
    fn ds_param_needle_not_confused_by_dst() {
        // "|dst|" contains "|ds" but not "|ds|" — both marker sets coexist
        // safely; likewise "台词|dst" must not trip the "台词|ds|" needle.
        let text = "{{实体信息框/自动|dst|hound}}\n{{全角色台词|dst|hound}}";
        assert!(DST_MARKERS.iter().any(|m| text.contains(m)));
        assert!(!DS_MARKERS.iter().any(|m| text.contains(m)));
    }

    #[test]
    fn disambiguation_page_wins_over_templates() {
        let text = "{{消歧义}}\n本页列出：{{实体信息框/自动|dst|x}} 与 {{实体信息框/自动|ds|y}}";
        let c = classify("代码", false, Some(text), &[]);
        assert_eq!(c.game_class, GameClass::Disambig);
        assert_eq!(c.confidence, ClassConfidence::High);
    }

    #[test]
    fn redirect_flag_short_circuits() {
        let c = classify("X", true, Some("{{实体信息框/自动|dst|h}}"), &[]);
        assert_eq!(c.game_class, GameClass::Redirect);
        assert_eq!(c.confidence, ClassConfidence::High);
    }

    #[test]
    fn category_fallback_without_wikitext() {
        let c = classify("Y", false, None, &cats(&["分类:联机版"]));
        assert_eq!(c.game_class, GameClass::Dst);
        assert_eq!(c.confidence, ClassConfidence::Medium);

        let c = classify("Z", false, None, &cats(&["分类:海难"]));
        assert_eq!(c.game_class, GameClass::Ds);

        let c = classify("W", false, None, &[]);
        assert_eq!(c.game_class, GameClass::Unknown);
        assert_eq!(c.confidence, ClassConfidence::Low);
    }

    #[test]
    fn plain_prose_page_is_unknown() {
        let c = classify(
            "帮助",
            false,
            Some("这是一个普通页面，没有任何版本模板。"),
            &[],
        );
        assert_eq!(c.game_class, GameClass::Unknown);
    }

    #[test]
    fn ds_title_suffix_rescues_unmarked_pages() {
        let c = classify(
            "熟芦笋/单机版",
            false,
            Some("没有任何版本模板的单机版子页"),
            &[],
        );
        assert_eq!(c.game_class, GameClass::Ds);
        assert_eq!(c.confidence, ClassConfidence::Medium);
        assert!(c.signals.contains(&"title:/单机版".to_string()));
    }
}
