RS       := $(shell find . -name '*.rs')
LUA      := scripts/layout.lua examples/demo.lua
EXAMPLES  = raw app
EX_FLAGS  = $(EXAMPLES:%=--example %)

.PHONY: all run clean

all: target/sdl/.built target/fb32/.built target/armv7/.built target/wasm/.built

examples/app/ui.rs: $(LUA)
	lua scripts/layout.lua < examples/demo.lua > $@

run: target/sdl/.built
	target/release/examples/app-sdl

target/sdl/.built: $(RS)
	CARGO_TARGET_DIR=target/sdl cargo clippy --features sdl $(EX_FLAGS) -- -D warnings
	CARGO_TARGET_DIR=target/sdl cargo build --release --features sdl $(EX_FLAGS)
	mkdir -p target/release/examples
	$(foreach ex,$(EXAMPLES),cp target/sdl/release/examples/$(ex) target/release/examples/$(ex)-sdl;)
	touch $@

target/fb32/.built: $(RS)
	CARGO_TARGET_DIR=target/fb32 cargo build --release --features fb0 $(EX_FLAGS)
	mkdir -p target/release/examples
	$(foreach ex,$(EXAMPLES),cp target/fb32/release/examples/$(ex) target/release/examples/$(ex)-fb32;)
	touch $@

target/armv7/.built: $(RS)
	CARGO_TARGET_DIR=target/armv7 \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS='-C target-feature=+crt-static' \
	cargo build --release --features fb0 $(EX_FLAGS) --target armv7-unknown-linux-gnueabihf
	mkdir -p target/armv7-unknown-linux-gnueabihf/release/examples
	$(foreach ex,$(EXAMPLES),cp target/armv7/armv7-unknown-linux-gnueabihf/release/examples/$(ex) target/armv7-unknown-linux-gnueabihf/release/examples/$(ex)-armv7;)
	touch $@

# WASM: cargo + sed output live together in target/wasm/wasm32-.../release/examples/
target/wasm/.built: src/sim.html $(RS)
	CARGO_TARGET_DIR=target/wasm \
	CARGO_ENCODED_RUSTFLAGS="$(shell printf '-C\x1flink-args=-sALLOW_MEMORY_GROWTH=1 -sASYNCIFY --embed-file fonts --js-library=src/web.js')" \
	cargo build --release --features web $(EX_FLAGS) --target wasm32-unknown-emscripten
	$(foreach ex,$(EXAMPLES),sed 's/demo\.js/$(ex).js/' src/sim.html > target/wasm/wasm32-unknown-emscripten/release/examples/$(ex).html;)
	touch $@

clean:
	rm -rf target
