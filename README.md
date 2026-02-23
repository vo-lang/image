# vo-lang/image

Vo wrapper for Rust `image` with mutable image handles.

## Module

```vo
import "github.com/vo-lang/image"
```

## Implemented API

- `Open(path)`
- `NewRGBA(width, height)`
- `Image.Resize(width, height)`
- `Image.Thumbnail(width, height)`
- `Image.Save(path)`
- `Image.EncodePNG()`
- `Image.Size()`
- `Image.Dimensions()`
- `Image.Close()`

## Build

```bash
cargo check --manifest-path rust/Cargo.toml
```
