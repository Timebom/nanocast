#[cfg(test)]
mod tests {
	use engine::{Config, SearchEngine, IndexBuilder};

	#[test]
	fn test_search() {
		let mut search = SearchEngine::new();
		search.set_items(IndexBuilder::build_demo_index());

		let results = search.search("code");

		assert!(!results.is_empty());
		println!("Found {} results for 'code':", results.len());
		for r in results.iter().take(5) {
			println!("- {} (score: {})", r.item.title, r.score);
		}
	}

	#[test]
	fn test_index() {
	    let config = Config::load();
		let builder = IndexBuilder::new(config.expect("Should load Pre-Configuration"));

		println!("Building index...");
		let items = builder.build().unwrap();

		println!("Indexed {} items", items.len());
		for item in items.iter().take(10) {
		println!("* {} ({:?})", item.title, item.item_type);
		}
	}
}
