module github.com/vo-lang/image

vo ^0.1.0

[extension]
name = "image"

[extension.native]
path = "rust/target/{profile}/libvo_image"

[[extension.native.targets]]
target = "aarch64-apple-darwin"
library = "libvo_image.dylib"

[[extension.native.targets]]
target = "x86_64-unknown-linux-gnu"
library = "libvo_image.so"

[extension.wasm]
type = "standalone"
wasm = "image.wasm"
