use crate::models::{FilterMode, ItemType, LauncherItem, SearchResult};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::sync::Arc;

pub struct SearchEngine {
    items: Vec<LauncherItem>,
    matcher: Nucleo<LauncherItem>,
}

impl SearchEngine {
    pub fn new() -> Self {
        // 2 columns: title + subtitle
        let matcher = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), None, 2);
        Self {
            items: Vec::new(),
            matcher,
        }
    }

    pub fn set_items(&mut self, items: Vec<LauncherItem>) {
        self.items = items;
        self.matcher.restart(true);

        let injector = self.matcher.injector();
        for item in &self.items {
            injector.push(item.clone(), |item, cols| {
                cols[0] = item.title.clone().into();
                if let Some(sub) = &item.subtitle {
                    cols[1] = sub.clone().into();
                }
            });
        }
    }

    pub fn search(&mut self, query: &str) -> Vec<SearchResult> {
        if query.trim().is_empty() {
            return self
                .items
                .iter()
                .map(|item| SearchResult {
                    item: item.clone(),
                    score: 0.0,
                })
                .collect();
        }

        self.matcher
            .pattern
            .reparse(0, query, CaseMatching::Smart, Normalization::Smart, false);

        self.matcher.tick(10);

        let snapshot = self.matcher.snapshot();
        snapshot
            .matched_items(..)
            .map(|item| SearchResult {
                item: item.data.clone(),
                score: 0.0 as f32,
            })
            .collect()
    }

    pub fn search_filtered(&mut self, query: &str, filter: &FilterMode) -> Vec<SearchResult> {
        let mut results = self.search(query);

        if *filter == FilterMode::All {
            return results;
        }

        results.retain(|r| match filter {
            FilterMode::All => true,
            FilterMode::Applications => matches!(r.item.item_type, ItemType::Application),
            FilterMode::Files => matches!(r.item.item_type, ItemType::File),
            FilterMode::Shortcuts => r.item.id.starts_with("shortcut:"),
        });

        results
    }

    pub fn get_all_items(&self) -> &[LauncherItem] {
        &self.items
    }
}
