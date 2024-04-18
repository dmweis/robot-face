
.PHONY: copy-to-desktop
copy-to-desktop:
	cargo build --release -j2
	cp target/release/face ~/Desktop


.PHONY: watch
watch:
	cargo watch -x "run -- -d"
