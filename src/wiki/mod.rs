mod client;

pub use client::{
    file_title, EditResult, FileInfo, PageBrief, PageInfo, PageListingEntry, PageRevisionContent,
    RateLimitCfg, RecentChange, WikiClient, WikiConfig, TITLES_PER_QUERY,
};
