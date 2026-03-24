RS := $(shell find . -name '*.rs')

WASM  := target/wasm32-unknown-emscripten/release/examples/index.html
SDL2  := target/release/examples/demo-sdl
ARMv7 := target/armv7-unknown-linux-gnueabihf/release/examples/demo-armv7
FB32  := target/release/examples/demo-fb32

.PHONY: all clean

all: $(WASM) $(SDL2) $(ARMv7) $(FB32)

$(WASM): src/sim.html $(RS)
	CARGO_ENCODED_RUSTFLAGS="$(shell printf '-C\x1flink-args=-sALLOW_MEMORY_GROWTH=1 --embed-file fonts --js-library=src/web.js')" \
	cargo build --release --example demo --features web,bpp32rgba --target wasm32-unknown-emscripten
	cp src/sim.html $(WASM)

$(SDL2): $(RS)
	cargo clippy --features sdl,bpp16 --example demo -- -D warnings
	cargo build --release --example demo --features sdl,bpp16
	cp target/release/examples/demo $(SDL2)

$(FB32): $(RS)
	cargo build --release --example demo --features fb0,bpp32
	cp target/release/examples/demo $(FB32)

$(ARMv7): $(RS)
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS='-C target-feature=+crt-static' \
	cargo build --release --example demo --features fb0,bpp16 --target armv7-unknown-linux-gnueabihf
	cp target/armv7-unknown-linux-gnueabihf/release/examples/demo $(ARMv7)

clean:
	rm -rf target
