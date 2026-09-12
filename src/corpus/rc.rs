//! 语料 recentchanges 增量通道(`corpus-fetch --rc`)。
//!
//! 按 [WIKI_CORPUS_PLAN.md](../../docs/WIKI_CORPUS_PLAN.md) §12 的设计实施:
//! 以 `rc_state.json` 检查点 + 10 分钟重叠窗拉取主命名空间事件流,追加
//! `events.jsonl`,按 §12.5 规则应用(零内容重抓/批量重抓/move/delete 归档),
//! 语料与事件全部落盘后才推进检查点(崩溃即重放,幂等兜底)。检查点缺失或
//! 超过 60 天保守保留期时自动回落枚举对账。

use crate::corpus::classify;
use crate::corpus::model::PageMeta;
use crate::corpus::store::CorpusStore;
use crate::error::{Error, Result};
use crate::platform::progress::Reporter;
use crate::wiki::{RecentChange, WikiClient};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// 重叠窗:自愈时钟边界与去重依赖重复投递。
const OVERLAP_SECS: i64 = 10 * 60;
/// 检查点保留期:超过即回落枚举对账(保守值,$wgRCMaxAge 默认 90 天)。
const RETENTION_SECS: i64 = 60 * 24 * 3600;
/// 已知 bot 用户名白名单(配置外置时可覆盖,见 rc_actors.toml)。
const DEFAULT_BOT_LULU: &str = "Mr鲁鲁";

// ---------------------------------------------------------------------------
// MediaWiki 时间戳 ↔ epoch(无 chrono 依赖,civil 算法)
// ---------------------------------------------------------------------------

/// 解析 `YYYY-MM-DDTHH:MM:SSZ` 为 epoch 秒。
pub fn parse_mw_ts(ts: &str) -> Option<i64> {
    let b = ts.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { ts.get(r)?.parse().ok() };
    let year = num(0..4)?;
    let month = num(5..7)?;
    let day = num(8..10)?;
    let hour = num(11..13)?;
    let min = num(14..16)?;
    let sec = num(17..19)?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day < 1 || day > month_days[(month - 1) as usize] {
        return None;
    }
    // days_from_civil(Howard Hinnant 算法)
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3600 + min * 60 + sec)
}

/// epoch 秒 → `YYYY-MM-DDTHH:MM:SSZ`。
pub fn format_mw_ts(epoch: i64) -> String {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    // civil_from_days(Howard Hinnant 算法)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let (h, mi, sec) = (secs / 3600, secs / 60 % 60, secs % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{sec:02}Z")
}

// ---------------------------------------------------------------------------
// 检查点 / 归因 / 事件模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RcState {
    pub last_ts: String,
    pub last_rcid: i64,
    pub updated_at_ms: u64,
}

/// §12.4 actor_class 判定:配置外置(rc_actors.toml),缺省仅 Mr鲁鲁。
fn actor_class(user: Option<&str>, bot_flag: bool, whitelist: &[String]) -> &'static str {
    let self_name = std::env::var("HUIJI__USERNAME").ok();
    if let Some(u) = user {
        if self_name.as_deref() == Some(u) {
            return "self";
        }
        if whitelist.iter().any(|w| w == u) {
            return "bot_lulu";
        }
    }
    if bot_flag {
        return "bot_flagged";
    }
    "human"
}

fn load_actor_whitelist(store: &CorpusStore) -> Vec<String> {
    let path = store.root().join("rc_actors.toml");
    #[derive(serde::Deserialize)]
    struct Actors {
        #[serde(default)]
        bot_lulu: Vec<String>,
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| toml::from_str::<Actors>(&raw).ok())
        .map(|a| a.bot_lulu)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec![DEFAULT_BOT_LULU.to_string()])
}

fn load_state(store: &CorpusStore) -> Option<RcState> {
    let raw = std::fs::read_to_string(store.root().join("rc_state.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_state(store: &CorpusStore, state: &RcState) -> Result<()> {
    let path = store.root().join("rc_state.json");
    std::fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

/// §12.4 事件行(追加式 events.jsonl)。
#[derive(Debug, Clone, Serialize)]
pub struct RcEvent {
    pub rcid: i64,
    pub ts: String,
    pub pageid: Option<i64>,
    pub title: Option<String>,
    pub actor: Option<String>,
    pub actor_class: &'static str,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_revid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldlen: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newlen: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub bot_flag: bool,
    pub in_bot_window: bool,
    pub applied: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_params: Option<serde_json::Value>,
}

fn event_from_rc(rc: &RecentChange, whitelist: &[String]) -> RcEvent {
    let class = actor_class(rc.user.as_deref(), rc.bot_flag, whitelist);
    let action = match (&rc.rc_type[..], rc.log_type.as_deref()) {
        ("log", Some(lt)) => format!("log/{lt}/{}", rc.log_action.as_deref().unwrap_or("-")),
        (t, _) => t.to_string(),
    };
    RcEvent {
        rcid: rc.rcid,
        ts: rc.timestamp.clone(),
        pageid: rc.pageid,
        title: rc.title.clone(),
        actor: rc.user.clone(),
        actor_class: class,
        action,
        revid: rc.revid,
        old_revid: rc.old_revid,
        sha1: rc.sha1.clone(),
        oldlen: rc.oldlen,
        newlen: rc.newlen,
        comment: rc.comment.clone(),
        bot_flag: rc.bot_flag,
        in_bot_window: class == "bot_lulu",
        applied: "recorded",
        log_params: rc.log_params.clone(),
    }
}

/// 事件按 §12.5 归类为本地动作(纯函数,便于测试)。
#[derive(Debug, Default)]
pub struct RcPlan {
    pub refetch: BTreeMap<i64, String>,
    pub touched_bump: BTreeMap<i64, String>,
    pub moves: BTreeMap<i64, String>,
    pub deletes: BTreeSet<i64>,
}

pub fn plan_applies(events: &[RcEvent], meta: &BTreeMap<i64, PageMeta>) -> RcPlan {
    let mut plan = RcPlan::default();
    // rcid 升序重放,后发事件覆盖先发(同页多事件合并为最终态)
    let mut ordered: Vec<&RcEvent> = events.iter().collect();
    ordered.sort_by_key(|e| e.rcid);
    for e in ordered {
        let Some(pageid) = e.pageid else { continue };
        match e.action.as_str() {
            "log/delete" => {
                plan.refetch.remove(&pageid);
                plan.touched_bump.remove(&pageid);
                plan.moves.remove(&pageid);
                plan.deletes.insert(pageid);
            }
            "log/restore" => {
                plan.deletes.remove(&pageid);
                plan.refetch
                    .insert(pageid, e.title.clone().unwrap_or_default());
            }
            "log/move" => {
                if let Some(new_title) = move_target(e) {
                    plan.deletes.remove(&pageid);
                    plan.moves.insert(pageid, new_title);
                    plan.refetch
                        .insert(pageid, e.title.clone().unwrap_or_default());
                }
            }
            "edit" | "new" => {
                if plan.deletes.contains(&pageid) {
                    continue;
                }
                let local_sha1 = meta.get(&pageid).and_then(|m| m.rev_sha1.as_deref());
                let local_exists = meta.contains_key(&pageid);
                let same = local_exists && local_sha1.is_some() && local_sha1 == e.sha1.as_deref();
                if same {
                    plan.touched_bump.insert(pageid, e.ts.clone());
                } else {
                    plan.touched_bump.remove(&pageid);
                    plan.refetch
                        .insert(pageid, e.title.clone().unwrap_or_default());
                }
            }
            _ => {}
        }
    }
    plan
}

/// move 事件的新标题:logparams.target_title(本站扁平化在 rc 行上)。
fn move_target(e: &RcEvent) -> Option<String> {
    e.log_params
        .as_ref()?
        .get("target_title")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

pub async fn sync_rc(
    client: &WikiClient,
    base_dir: &Path,
    dry_run: bool,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let store = CorpusStore::new(base_dir, client.config().host());

    // 回退判定:检查点缺失或超保留期 → 枚举对账
    let state = load_state(&store);
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let stale = state
        .as_ref()
        .and_then(|s| parse_mw_ts(&s.last_ts))
        .is_none_or(|t| now_epoch - t > RETENTION_SECS);
    if state.is_none() || stale {
        reporter.log("RC 检查点缺失或超过保留期(60 天),回落枚举对账".to_string());
        let out = crate::corpus::sync(client, base_dir, false, dry_run, reporter).await?;
        // 播种检查点:枚举对账后从当下开始增量,否则每次 --rc 都会永远回落
        if !dry_run {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            save_state(
                &store,
                &RcState {
                    last_ts: format_mw_ts(now_epoch),
                    last_rcid: 0,
                    updated_at_ms: now_ms,
                },
            )?;
            reporter.log(format!("RC 检查点已播种:{}", format_mw_ts(now_epoch)));
        }
        return Ok(
            serde_json::json!({"fallback": true, "reason": "checkpoint_missing_or_stale", "enumeration": out}),
        );
    }
    let state = state.expect("state checked above");

    let whitelist = load_actor_whitelist(&store);
    let start_epoch = parse_mw_ts(&state.last_ts)
        .ok_or_else(|| Error::Config(format!("检查点时间戳非法: {}", state.last_ts)))?
        - OVERLAP_SECS;
    let start_ts = format_mw_ts(start_epoch);

    reporter.stage("拉取 recentchanges");
    let changes = client.recentchanges(&start_ts).await?;
    // 去重 by rcid(重叠窗重复投递)
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    let mut events: Vec<RcEvent> = Vec::new();
    for rc in &changes {
        if seen.insert(rc.rcid) {
            events.push(event_from_rc(rc, &whitelist));
        }
    }
    reporter.log(format!(
        "窗口 {} 起,事件 {} 条(去重后),涉及页面 {} 个",
        start_ts,
        events.len(),
        events
            .iter()
            .filter_map(|e| e.pageid)
            .collect::<BTreeSet<_>>()
            .len(),
    ));

    let meta = store.load_meta()?;
    let plan = plan_applies(&events, &meta);
    // 每页最后事件 ts(refetch 页 touched 用)
    let mut page_ts: BTreeMap<i64, String> = BTreeMap::new();
    for e in &events {
        if let Some(id) = e.pageid {
            page_ts.insert(id, e.ts.clone());
        }
    }
    reporter.log(format!(
        "计划:重抓 {} 页 / touched 刷新 {} 页 / move {} 页 / delete {} 页",
        plan.refetch.len(),
        plan.touched_bump.len(),
        plan.moves.len(),
        plan.deletes.len(),
    ));

    if dry_run {
        return Ok(serde_json::json!({
            "dry_run": true,
            "window_start": start_ts,
            "events": events,
            "plan": {
                "refetch": plan.refetch.len(),
                "touched_bump": plan.touched_bump.len(),
                "moves": plan.moves.len(),
                "deletes": plan.deletes.len(),
            },
        }));
    }

    let mut new_state = state.clone();
    if events.is_empty() {
        reporter.log("无事件,检查点不推进".to_string());
        return Ok(serde_json::json!({"events": 0, "checkpoint_advanced": false}));
    }

    let mut meta = meta;
    // delete:归档 + meta 移除
    if !plan.deletes.is_empty() {
        let tag = crate::corpus::now_compact_tag();
        for id in &plan.deletes {
            store.archive_removed(*id, &tag)?;
            meta.remove(id);
        }
        reporter.log(format!("已归档 {} 页到 _removed/{tag}", plan.deletes.len()));
    }

    // 重抓(edit/new sha1 变化、restore、move 确认;move 以新标题抓取)
    let mut fetched = 0usize;
    let mut missing = 0usize;
    let mut refetch_titles: Vec<(i64, String)> = plan
        .refetch
        .iter()
        .map(|(id, title)| {
            let title = plan.moves.get(id).cloned().unwrap_or_else(|| title.clone());
            (*id, title)
        })
        .collect();
    refetch_titles.sort();
    for chunk in refetch_titles.chunks(50) {
        let titles: Vec<&str> = chunk.iter().map(|(_, t)| t.as_str()).collect();
        let contents = client.get_pages_wikitext(&titles).await?;
        let by_title: BTreeMap<&str, &crate::wiki::PageRevisionContent> =
            contents.iter().map(|c| (c.title.as_str(), c)).collect();
        for (pageid, title) in chunk {
            let Some(content) = by_title.get(title.as_str()) else {
                missing += 1;
                continue;
            };
            if content.missing {
                missing += 1;
                continue;
            }
            let Some(text) = &content.wikitext else {
                missing += 1;
                continue;
            };
            store.write_page(*pageid, text)?;
            let redirect = crate::corpus::store::CorpusStore::redirect_target(text).is_some();
            let outcome = classify::classify(title, redirect, Some(text), &content.categories);
            let entry = PageMeta {
                pageid: *pageid,
                title: title.clone(),
                touched: page_ts
                    .get(pageid)
                    .cloned()
                    .or_else(|| Some(new_state.last_ts.clone())),
                len: Some(text.len() as u64),
                redirect,
                rev_sha1: content.sha1.clone(),
                categories: content.categories.clone(),
                game_class: outcome.game_class,
                class_signals: outcome.signals,
                class_confidence: outcome.confidence,
            };
            meta.insert(*pageid, entry);
            fetched += 1;
        }
    }
    // touched 刷新(零内容请求)
    for (id, ts) in &plan.touched_bump {
        if let Some(m) = meta.get_mut(id) {
            m.touched = Some(ts.clone());
        }
    }
    store.save_meta(&meta)?;

    // events 追加落盘
    let events_path: PathBuf = store.root().join("events.jsonl");
    let mut sink = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)?;
    let mut max_ts = parse_mw_ts(&new_state.last_ts).unwrap_or(0);
    let mut max_rcid = new_state.last_rcid;
    for e in &events {
        writeln!(sink, "{}", serde_json::to_string(e)?)?;
        let t = parse_mw_ts(&e.ts).unwrap_or(0);
        if t > max_ts || (t == max_ts && e.rcid > max_rcid) {
            max_ts = t;
            max_rcid = e.rcid;
        }
    }
    new_state.last_ts = format_mw_ts(max_ts);
    new_state.last_rcid = max_rcid;
    new_state.updated_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    save_state(&store, &new_state)?;

    reporter.log(format!(
        "RC 应用完成:重抓 {fetched}(缺失 {missing}),events 追加 {},检查点 {}|{}",
        events.len(),
        new_state.last_ts,
        new_state.last_rcid
    ));
    Ok(serde_json::json!({
        "fallback": false,
        "window_start": start_ts,
        "events": events.len(),
        "refetched": fetched,
        "missing": missing,
        "touched_bump": plan.touched_bump.len(),
        "moves": plan.moves.len(),
        "deletes": plan.deletes.len(),
        "checkpoint": {"last_ts": new_state.last_ts, "last_rcid": new_state.last_rcid},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mw_ts_roundtrip() {
        let ts = "2026-08-31T12:34:56Z";
        assert_eq!(format_mw_ts(parse_mw_ts(ts).unwrap()), ts);
        assert_eq!(parse_mw_ts("2026-02-29T00:00:00Z"), None, "非闰年拒绝");
        assert!(parse_mw_ts("2024-02-29T00:00:00Z").is_some());
        assert!(parse_mw_ts("bad").is_none());
    }

    #[test]
    fn overlap_and_retention_math() {
        let base = parse_mw_ts("2026-08-31T12:00:00Z").unwrap();
        assert_eq!(format_mw_ts(base - OVERLAP_SECS), "2026-08-31T11:50:00Z");
        let stale = base - RETENTION_SECS - 1;
        assert_eq!(format_mw_ts(stale), "2026-07-02T11:59:59Z");
    }

    fn ev(rcid: i64, pageid: i64, action: &str, sha1: Option<&str>, title: &str) -> RcEvent {
        RcEvent {
            rcid,
            ts: "2026-08-31T12:00:00Z".into(),
            pageid: Some(pageid),
            title: Some(title.into()),
            actor: None,
            actor_class: "human",
            action: action.into(),
            revid: None,
            old_revid: None,
            sha1: sha1.map(str::to_string),
            oldlen: None,
            newlen: None,
            comment: None,
            bot_flag: false,
            in_bot_window: false,
            applied: "recorded",
            log_params: None,
        }
    }

    fn meta_with(pageid: i64, sha1: &str) -> BTreeMap<i64, PageMeta> {
        let mut m = BTreeMap::new();
        m.insert(
            pageid,
            PageMeta {
                pageid,
                title: format!("p{pageid}"),
                touched: None,
                len: None,
                redirect: false,
                rev_sha1: Some(sha1.into()),
                categories: vec![],
                game_class: crate::corpus::model::GameClass::Dst,
                class_signals: vec![],
                class_confidence: crate::corpus::model::ClassConfidence::High,
            },
        );
        m
    }

    #[test]
    fn plan_edit_same_sha_bumps_touched_only() {
        let events = vec![ev(1, 42, "edit", Some("aa"), "p42")];
        let meta = meta_with(42, "aa");
        let plan = plan_applies(&events, &meta);
        assert!(plan.refetch.is_empty());
        assert_eq!(
            plan.touched_bump.get(&42),
            Some(&"2026-08-31T12:00:00Z".to_string())
        );
    }

    #[test]
    fn plan_edit_new_sha_refetches_and_new_page_refetches() {
        let events = vec![
            ev(1, 42, "edit", Some("bb"), "p42"),
            ev(2, 99, "new", Some("cc"), "p99"),
        ];
        let meta = meta_with(42, "aa");
        let plan = plan_applies(&events, &meta);
        assert_eq!(plan.refetch.len(), 2);
        assert!(plan.touched_bump.is_empty());
    }

    #[test]
    fn plan_delete_wins_and_move_refetches() {
        let events = vec![
            ev(1, 42, "edit", Some("bb"), "p42"),
            ev(2, 42, "log/delete", None, "p42"),
        ];
        let meta = meta_with(42, "aa");
        let plan = plan_applies(&events, &meta);
        assert!(plan.refetch.is_empty());
        assert!(plan.deletes.contains(&42));

        let move_ev = RcEvent {
            rcid: 3,
            action: "log/move".into(),
            log_params: Some(serde_json::json!({"target_title": "新标题"})),
            ..ev(3, 7, "log/move", None, "旧标题")
        };
        let plan = plan_applies(&[move_ev], &BTreeMap::new());
        assert_eq!(
            plan.moves.get(&7),
            Some(&"新标题".to_string()),
            "logparams target_title 进 moves"
        );
        assert!(plan.refetch.contains_key(&7), "move 需重抓确认");
    }

    #[test]
    fn actor_classification() {
        std::env::remove_var("HUIJI__USERNAME");
        let wl = vec!["Mr鲁鲁".to_string()];
        assert_eq!(actor_class(Some("Mr鲁鲁"), false, &wl), "bot_lulu");
        assert_eq!(actor_class(Some("张三"), false, &wl), "human");
        assert_eq!(actor_class(Some("张三"), true, &wl), "bot_flagged");
        std::env::set_var("HUIJI__USERNAME", "wikibot");
        assert_eq!(actor_class(Some("wikibot"), false, &wl), "self");
        std::env::remove_var("HUIJI__USERNAME");
    }
}
