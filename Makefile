RS  := $(shell find . -name '*.rs')
LUA := scripts/layout.lua examples/demo.lua

EXAMPLES = raw app

SDL2_BINS  = $(EXAMPLES:%=target/release/examples/%-sdl)
FB32_BINS  = $(EXAMPLES:%=target/release/examples/%-fb32)
ARMv7_BINS = $(EXAMPLES:%=target/armv7-unknown-linux-gnueabihf/release/examples/%-armv7)
WASM_HTMLS = $(EXAMPLES:%=target/wasm/wasm32-unknown-emscripten/release/examples/%.html)

.PHONY: all run clean

all: $(SDL2_BINS) $(FB32_BINS) $(ARMv7_BINS) $(WASM_HTMLS)

examples/app/ui.rs: $(LUA)
	lua scripts/layout.lua < examples/demo.lua > $@

run: target/release/examples/raw-sdl
	target/release/examples/raw-sdl

target/release/examples/%-sdl: $(RS)
	CARGO_TARGET_DIR=target/sdl cargo clippy --features sdl,bpp16 --example $* -- -D warnings
	CARGO_TARGET_DIR=target/sdl cargo build --release --example $* --features sdl,bpp16
	cp target/sdl/release/examples/$* $@

target/release/examples/%-fb32: $(RS)
	CARGO_TARGET_DIR=target/fb32 cargo build --release --example $* --features fb0,bpp32
	cp target/fb32/release/examples/$* $@

target/armv7-unknown-linux-gnueabihf/release/examples/%-armv7: $(RS)
	CARGO_TARGET_DIR=target/armv7 \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS='-C target-feature=+crt-static' \
	cargo build --release --example $* --features fb0,bpp16 --target armv7-unknown-linux-gnueabihf
	cp target/armv7/armv7-unknown-linux-gnueabihf/release/examples/$* $@

# WASM: cargo + sed output live together in target/wasm/wasm32-.../release/examples/
target/wasm/wasm32-unknown-emscripten/release/examples/%.html: src/sim.html $(RS)
	CARGO_TARGET_DIR=target/wasm \
	CARGO_ENCODED_RUSTFLAGS="$(shell printf '-C\x1flink-args=-sALLOW_MEMORY_GROWTH=1 -sASYNCIFY --embed-file fonts --js-library=src/web.js')" \
	cargo build --release --example $* --features web,bpp32rgba --target wasm32-unknown-emscripten
	sed 's/demo\.js/$*.js/' src/sim.html > $@

clean:
	rm -rf target
