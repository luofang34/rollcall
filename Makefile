.PHONY: ci

# The full local gate — run before pushing. Mirrors .github/workflows/ci.yml.
ci:
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --all-targets
	RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links -D rustdoc::invalid_html_tags" cargo doc --no-deps
	cargo build --release
