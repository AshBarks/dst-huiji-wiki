mod client;

pub use client::{
    EditResult, PageBrief, PageInfo, PageListingEntry, PageRevisionContent, RateLimitCfg,
    RecentChange, WikiClient, WikiConfig, TITLES_PER_QUERY,
};
