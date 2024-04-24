
.PHONY: copy-to-desktop
copy-to-desktop:
	cargo build --release -j3
	cp target/release/face ~/Desktop


.PHONY: watch
watch:
	cargo watch -x "run -- -d"
