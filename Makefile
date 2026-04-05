.PHONY: test-e2e-local

test-e2e-local:
	cargo test --test download_test
	cargo test --test upload_test
	cargo test --test chunk_retry_test
	cargo test --test callback_panic_test
	cargo test --test download_protocol_validation_test
