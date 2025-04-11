run:
	cargo run $(path)
build:
	cargo build
test:
	cargo test
clean:
	rm -r ./target
2048:
	cargo run ./binaries/2048.obj
rogue:
	cargo run ./binaries/rogue.obj
