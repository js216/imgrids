RS  := $(shell find . -name '*.rs')
LUA := scripts/layout.lua examples/ui.lua

EXAMPLES = raw app

SDL2_BINS  = $(EXAMPLES:%=target/release/examples/%-sdl)
FB32_BINS  = $(EXAMPLES:%=target/release/examples/%-fb32)
ARMv7_BINS = $(EXAMPLES:%=target/armv7-unknown-linux-gnueabihf/release/examples/%-armv7)
WASM_HTMLS = $(EXAMPLES:%=target/wasm32-unknown-emscripten/release/examples/%.html)

.PHONY: all run clean

all: $(SDL2_BINS) $(FB32_BINS) $(ARMv7_BINS) $(WASM_HTMLS)

examples/app/ui.rs: $(LUA)
	lua scripts/layout.lua < examples/ui.lua > $@

run: target/release/examples/raw-sdl
	target/release/examples/raw-sdl

target/release/examples/%-sdl: $(RS)
	cargo clippy --features sdl,bpp16 --example $* -- -D warnings
	cargo build --release --example $* --features sdl,bpp16
	cp target/release/examples/$* $@

target/release/examples/%-fb32: $(RS)
	cargo build --release --example $* --features fb0,bpp32
	cp target/release/examples/$* $@

target/armv7-unknown-linux-gnueabihf/release/examples/%-armv7: $(RS)
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS='-C target-feature=+crt-static' \
	cargo build --release --example $* --features fb0,bpp16 --target armv7-unknown-linux-gnueabihf
	cp target/armv7-unknown-linux-gnueabihf/release/examples/$* $@

target/wasm32-unknown-emscripten/release/examples/%.html: src/sim.html $(RS)
	CARGO_ENCODED_RUSTFLAGS="$(shell printf '-C\x1flink-args=-sALLOW_MEMORY_GROWTH=1 -sASYNCIFY --embed-file fonts --js-library=src/web.js')" \
	cargo build --release --example $* --features web,bpp32rgba --target wasm32-unknown-emscripten
	cp src/sim.html $@

clean:
	rm -rf target
