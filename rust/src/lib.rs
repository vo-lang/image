use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use image::{DynamicImage, ImageFormat};
use lazy_static::lazy_static;

#[cfg(feature = "native")]
mod native {
    use super::*;
    use vo_ext::prelude::*;
    use vo_runtime::builtins::error_helper::{write_error_to, write_nil_error};

    lazy_static! {
        static ref IMAGES: Mutex<HashMap<u32, DynamicImage>> = Mutex::new(HashMap::new());
    }

    static NEXT_ID: AtomicU32 = AtomicU32::new(1);

    fn get_image<'a>(
        map: &'a HashMap<u32, DynamicImage>,
        id: u32,
    ) -> Result<&'a DynamicImage, String> {
        map.get(&id)
            .ok_or_else(|| format!("invalid image id {}", id))
    }

    fn get_image_mut<'a>(
        map: &'a mut HashMap<u32, DynamicImage>,
        id: u32,
    ) -> Result<&'a mut DynamicImage, String> {
        map.get_mut(&id)
            .ok_or_else(|| format!("invalid image id {}", id))
    }

    fn insert_image(img: DynamicImage) -> Result<u32, String> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        map.insert(id, img);
        Ok(id)
    }

    fn to_u32(v: i64, name: &str) -> Result<u32, String> {
        u32::try_from(v).map_err(|_| format!("{name} out of range: {v}"))
    }

    fn open_impl(path: &str) -> Result<u32, String> {
        let img = image::open(path).map_err(|e| e.to_string())?;
        insert_image(img)
    }

    fn open_from_bytes_impl(data: &[u8]) -> Result<u32, String> {
        let img = image::load_from_memory(data).map_err(|e| e.to_string())?;
        insert_image(img)
    }

    fn save_to_bytes_impl(id: u64, ext: &str) -> Result<Vec<u8>, String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let fmt = match ext.to_lowercase().trim_start_matches('.') {
            "png"        => ImageFormat::Png,
            "jpg"|"jpeg" => ImageFormat::Jpeg,
            "gif"        => ImageFormat::Gif,
            "bmp"        => ImageFormat::Bmp,
            "webp"       => ImageFormat::WebP,
            other => return Err(format!("unsupported image format: {}", other)),
        };
        let map = IMAGES.lock().map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, id)?;
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, fmt).map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }

    fn new_rgba_impl(width: i64, height: i64) -> Result<u32, String> {
        let width = to_u32(width, "width")?;
        let height = to_u32(height, "height")?;
        let img = DynamicImage::new_rgba8(width, height);
        insert_image(img)
    }

    fn resize_impl(id: u64, width: i64, height: i64) -> Result<(), String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let width = to_u32(width, "width")?;
        let height = to_u32(height, "height")?;
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let current = get_image_mut(&mut map, id)?;
        let resized = current.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
        *current = resized;
        Ok(())
    }

    fn thumbnail_impl(id: u64, width: i64, height: i64) -> Result<(), String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let width = to_u32(width, "width")?;
        let height = to_u32(height, "height")?;
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let current = get_image_mut(&mut map, id)?;
        let thumb = current.thumbnail(width, height);
        *current = thumb;
        Ok(())
    }

    fn save_impl(id: u64, path: &str) -> Result<(), String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, id)?;
        img.save(path).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn encode_png_impl(id: u64) -> Result<Vec<u8>, String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, id)?;

        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }

    fn size_impl(id: u64) -> Result<(u32, u32), String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, id)?;
        Ok((img.width(), img.height()))
    }

    fn close_impl(id: u64) -> Result<(), String> {
        let id = u32::try_from(id).map_err(|_| format!("id out of range: {id}"))?;
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        map.remove(&id)
            .ok_or_else(|| format!("invalid image id {}", id))?;
        Ok(())
    }

    #[vo_fn("github.com/vo-lang/image", "nativeOpen")]
    pub fn native_open(call: &mut ExternCallContext) -> ExternResult {
        let path = call.arg_str(0);
        match open_impl(path) {
            Ok(id) => {
                call.ret_u64(0, id as u64);
                write_nil_error(call, 1);
            }
            Err(msg) => {
                call.ret_u64(0, 0);
                write_error_to(call, 1, &msg);
            }
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeOpenFromBytes")]
    pub fn native_open_from_bytes(call: &mut ExternCallContext) -> ExternResult {
        let data = call.arg_bytes(0);
        match open_from_bytes_impl(data) {
            Ok(id) => {
                call.ret_u64(0, id as u64);
                write_nil_error(call, 1);
            }
            Err(msg) => {
                call.ret_u64(0, 0);
                write_error_to(call, 1, &msg);
            }
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeSaveToBytes")]
    pub fn native_save_to_bytes(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        let ext = call.arg_str(1);
        match save_to_bytes_impl(id, ext) {
            Ok(b) => {
                let r = call.alloc_bytes(&b);
                call.ret_ref(0, r);
                write_nil_error(call, 1);
            }
            Err(msg) => {
                call.ret_nil(0);
                write_error_to(call, 1, &msg);
            }
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeNewRGBA")]
    pub fn native_new_rgba(call: &mut ExternCallContext) -> ExternResult {
        let width = call.arg_i64(0);
        let height = call.arg_i64(1);
        match new_rgba_impl(width, height) {
            Ok(id) => {
                call.ret_u64(0, id as u64);
                write_nil_error(call, 1);
            }
            Err(msg) => {
                call.ret_u64(0, 0);
                write_error_to(call, 1, &msg);
            }
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeResize")]
    pub fn native_resize(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        let width = call.arg_i64(1);
        let height = call.arg_i64(2);
        match resize_impl(id, width, height) {
            Ok(()) => write_nil_error(call, 0),
            Err(msg) => write_error_to(call, 0, &msg),
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeThumbnail")]
    pub fn native_thumbnail(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        let width = call.arg_i64(1);
        let height = call.arg_i64(2);
        match thumbnail_impl(id, width, height) {
            Ok(()) => write_nil_error(call, 0),
            Err(msg) => write_error_to(call, 0, &msg),
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeSave")]
    pub fn native_save(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        let path = call.arg_str(1);
        match save_impl(id, path) {
            Ok(()) => write_nil_error(call, 0),
            Err(msg) => write_error_to(call, 0, &msg),
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeEncodePNG")]
    pub fn native_encode_png(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        match encode_png_impl(id) {
            Ok(bytes) => {
                let out_ref = call.alloc_bytes(&bytes);
                call.ret_ref(0, out_ref);
                write_nil_error(call, 1);
            }
            Err(msg) => {
                call.ret_nil(0);
                write_error_to(call, 1, &msg);
            }
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeSize")]
    pub fn native_size(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        match size_impl(id) {
            Ok((width, height)) => {
                call.ret_i64(0, width as i64);
                call.ret_i64(1, height as i64);
                write_nil_error(call, 2);
            }
            Err(msg) => {
                call.ret_i64(0, 0);
                call.ret_i64(1, 0);
                write_error_to(call, 2, &msg);
            }
        }
        ExternResult::Ok
    }

    #[vo_fn("github.com/vo-lang/image", "nativeClose")]
    pub fn native_close(call: &mut ExternCallContext) -> ExternResult {
        let id = call.arg_u64(0);
        match close_impl(id) {
            Ok(()) => write_nil_error(call, 0),
            Err(msg) => write_error_to(call, 0, &msg),
        }
        ExternResult::Ok
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn temp_file(name: &str) -> std::path::PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("vo_image_{name}_{nanos}.png"))
        }

        #[test]
        fn image_lifecycle_and_transform_paths() {
            let id = new_rgba_impl(64, 32).expect("new_rgba should succeed");

            let (w0, h0) = size_impl(id as u64).expect("size should succeed");
            assert_eq!(w0, 64, "initial width should match creation width");
            assert_eq!(h0, 32, "initial height should match creation height");

            resize_impl(id as u64, 20, 10).expect("resize should succeed");
            let (w1, h1) = size_impl(id as u64).expect("size after resize should succeed");
            assert_eq!(w1, 20, "width should be updated by resize");
            assert_eq!(h1, 10, "height should be updated by resize");

            thumbnail_impl(id as u64, 8, 8).expect("thumbnail should succeed");
            let (w2, h2) = size_impl(id as u64).expect("size after thumbnail should succeed");
            assert!(w2 <= 8, "thumbnail width should be bounded by requested max");
            assert!(h2 <= 8, "thumbnail height should be bounded by requested max");

            let encoded = encode_png_impl(id as u64).expect("encode_png should succeed");
            assert!(!encoded.is_empty(), "encoded png bytes should not be empty");

            let out_path = temp_file("save");
            save_impl(id as u64, &out_path.to_string_lossy()).expect("save should succeed");
            assert!(out_path.exists(), "saved output file should exist");

            let reopened = open_impl(&out_path.to_string_lossy()).expect("open should succeed");
            let (rw, rh) = size_impl(reopened as u64).expect("reopened image size should succeed");
            assert!(rw > 0 && rh > 0, "reopened image dimensions must be positive");

            close_impl(reopened as u64).expect("close reopened image should succeed");
            close_impl(id as u64).expect("close original image should succeed");
            fs::remove_file(&out_path).expect("cleanup saved file should succeed");
        }

        #[test]
        fn invalid_image_id_paths_fail() {
            let invalid = 9_999_999u64;
            assert!(size_impl(invalid).is_err(), "size should fail for invalid id");
            assert!(
                encode_png_impl(invalid).is_err(),
                "encode_png should fail for invalid id"
            );
            assert!(close_impl(invalid).is_err(), "close should fail for invalid id");
        }
    }
}

#[cfg(feature = "native")]
vo_ext::export_extensions!();

// ── Standalone C-ABI WASM exports ────────────────────────────────────────────
//
// Uses ext_bridge v2 tagged binary protocol:
//   Input:  one entry per param slot — Value=[u64 LE 8B], Bytes=[u32 len][bytes]
//   Output: self-describing tagged stream — see tag constants below.

#[cfg(feature = "wasm-standalone")]
mod standalone {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;
    use image::{DynamicImage, ImageFormat};
    use lazy_static::lazy_static;

    // v2 tagged protocol output tags (mirrors ext_bridge.rs constants)
    const TAG_NIL_ERROR: u8 = 0xE0;
    const TAG_ERROR_STR: u8 = 0xE1;
    const TAG_VALUE:     u8 = 0xE2;
    const TAG_BYTES:     u8 = 0xE3;
    const TAG_NIL_REF:   u8 = 0xE4;

    lazy_static! {
        static ref IMAGES: Mutex<HashMap<u32, DynamicImage>> = Mutex::new(HashMap::new());
    }
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);

    // ── Memory management ─────────────────────────────────────────────────────

    #[no_mangle]
    pub extern "C" fn vo_alloc(size: u32) -> *mut u8 {
        let mut buf = Vec::<u8>::with_capacity(size as usize);
        let ptr = buf.as_mut_ptr();
        std::mem::forget(buf);
        ptr
    }

    #[no_mangle]
    pub extern "C" fn vo_dealloc(ptr: *mut u8, size: u32) {
        unsafe { drop(Vec::from_raw_parts(ptr, 0, size as usize)) };
    }

    // ── Input / output helpers ────────────────────────────────────────────────

    fn alloc_output(data: &[u8], out_len: *mut u32) -> *mut u8 {
        unsafe { *out_len = data.len() as u32; }
        let ptr = vo_alloc(data.len() as u32);
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len()); }
        ptr
    }

    struct Input<'a> { buf: &'a [u8], pos: usize }
    impl<'a> Input<'a> {
        unsafe fn new(ptr: *const u8, len: u32) -> Self {
            Self { buf: std::slice::from_raw_parts(ptr, len as usize), pos: 0 }
        }
        fn read_u64(&mut self) -> u64 {
            if self.pos + 8 > self.buf.len() { return 0; }
            let v = u64::from_le_bytes(self.buf[self.pos..self.pos + 8].try_into().unwrap());
            self.pos += 8;
            v
        }
        fn read_bytes(&mut self) -> &[u8] {
            if self.pos + 4 > self.buf.len() { return &[]; }
            let len = u32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap()) as usize;
            self.pos += 4;
            if self.pos + len > self.buf.len() { return &self.buf[self.pos..]; }
            let data = &self.buf[self.pos..self.pos + len];
            self.pos += len;
            data
        }
        fn read_str(&mut self) -> &str {
            std::str::from_utf8(self.read_bytes()).unwrap_or("")
        }
    }

    fn write_u64_ok(v: u64, out_len: *mut u32) -> *mut u8 {
        // [TAG_VALUE][u64 LE][TAG_NIL_ERROR]
        let mut buf = Vec::with_capacity(11);
        buf.push(TAG_VALUE);
        buf.extend_from_slice(&v.to_le_bytes());
        buf.push(TAG_NIL_ERROR);
        alloc_output(&buf, out_len)
    }

    fn write_u64_err(msg: &str, out_len: *mut u32) -> *mut u8 {
        // [TAG_VALUE][u64 LE 0][TAG_ERROR_STR][u16 len][msg]
        let mb = msg.as_bytes();
        let mlen = mb.len().min(0xFFFF) as u16;
        let mut buf = Vec::with_capacity(11 + mlen as usize);
        buf.push(TAG_VALUE);
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.push(TAG_ERROR_STR);
        buf.extend_from_slice(&mlen.to_le_bytes());
        buf.extend_from_slice(&mb[..mlen as usize]);
        alloc_output(&buf, out_len)
    }

    fn write_nil_error(out_len: *mut u32) -> *mut u8 {
        alloc_output(&[TAG_NIL_ERROR], out_len)
    }

    fn write_error(msg: &str, out_len: *mut u32) -> *mut u8 {
        let mb = msg.as_bytes();
        let mlen = mb.len().min(0xFFFF) as u16;
        let mut buf = Vec::with_capacity(3 + mlen as usize);
        buf.push(TAG_ERROR_STR);
        buf.extend_from_slice(&mlen.to_le_bytes());
        buf.extend_from_slice(&mb[..mlen as usize]);
        alloc_output(&buf, out_len)
    }

    fn write_bytes_ok(data: &[u8], out_len: *mut u32) -> *mut u8 {
        // [TAG_BYTES][u32 len][bytes][TAG_NIL_ERROR]
        let mut buf = Vec::with_capacity(5 + data.len() + 1);
        buf.push(TAG_BYTES);
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(data);
        buf.push(TAG_NIL_ERROR);
        alloc_output(&buf, out_len)
    }

    fn write_bytes_err(msg: &str, out_len: *mut u32) -> *mut u8 {
        // [TAG_NIL_REF][TAG_ERROR_STR][u16 len][msg]
        let mb = msg.as_bytes();
        let mlen = mb.len().min(0xFFFF) as u16;
        let mut buf = Vec::with_capacity(4 + mlen as usize);
        buf.push(TAG_NIL_REF);
        buf.push(TAG_ERROR_STR);
        buf.extend_from_slice(&mlen.to_le_bytes());
        buf.extend_from_slice(&mb[..mlen as usize]);
        alloc_output(&buf, out_len)
    }

    fn write_two_ints_ok(a: i64, b: i64, out_len: *mut u32) -> *mut u8 {
        // [TAG_VALUE][u64 LE a][TAG_VALUE][u64 LE b][TAG_NIL_ERROR]
        let mut buf = Vec::with_capacity(19);
        buf.push(TAG_VALUE); buf.extend_from_slice(&(a as u64).to_le_bytes());
        buf.push(TAG_VALUE); buf.extend_from_slice(&(b as u64).to_le_bytes());
        buf.push(TAG_NIL_ERROR);
        alloc_output(&buf, out_len)
    }

    fn write_two_ints_err(msg: &str, out_len: *mut u32) -> *mut u8 {
        // [TAG_VALUE][0][TAG_VALUE][0][TAG_ERROR_STR][u16 len][msg]
        let mb = msg.as_bytes();
        let mlen = mb.len().min(0xFFFF) as u16;
        let mut buf = Vec::with_capacity(19 + mlen as usize);
        buf.push(TAG_VALUE); buf.extend_from_slice(&0u64.to_le_bytes());
        buf.push(TAG_VALUE); buf.extend_from_slice(&0u64.to_le_bytes());
        buf.push(TAG_ERROR_STR);
        buf.extend_from_slice(&mlen.to_le_bytes());
        buf.extend_from_slice(&mb[..mlen as usize]);
        alloc_output(&buf, out_len)
    }

    // ── Image operations ──────────────────────────────────────────────────────

    fn insert_image(img: DynamicImage) -> Result<u32, String> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        IMAGES.lock().map_err(|_| "image lock poisoned".to_string())?.insert(id, img);
        Ok(id)
    }

    fn format_from_ext(ext: &str) -> Result<ImageFormat, String> {
        match ext.to_lowercase().trim_start_matches('.') {
            "png"  => Ok(ImageFormat::Png),
            "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
            "gif"  => Ok(ImageFormat::Gif),
            "bmp"  => Ok(ImageFormat::Bmp),
            "webp" => Ok(ImageFormat::WebP),
            other  => Err(format!("unsupported image format: {}", other)),
        }
    }

    // ── WASM exports ──────────────────────────────────────────────────────────

    // Input: [u32 len][data bytes]  → (uint32, error)
    #[no_mangle]
    pub extern "C" fn nativeOpenFromBytes(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let data = input.read_bytes();
        match image::load_from_memory(data) {
            Ok(img) => match insert_image(img) {
                Ok(id) => write_u64_ok(id as u64, out_len),
                Err(e) => write_u64_err(&e, out_len),
            },
            Err(e) => write_u64_err(&e.to_string(), out_len),
        }
    }

    // Input: [u64 LE w][u64 LE h]  → (uint32, error)
    #[no_mangle]
    pub extern "C" fn nativeNewRGBA(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let w = input.read_u64() as u32;
        let h = input.read_u64() as u32;
        match insert_image(DynamicImage::new_rgba8(w, h)) {
            Ok(id) => write_u64_ok(id as u64, out_len),
            Err(e) => write_u64_err(&e, out_len),
        }
    }

    // Input: [u64 LE id][u64 LE w][u64 LE h]  → error
    #[no_mangle]
    pub extern "C" fn nativeResize(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let id = input.read_u64() as u32;
        let w  = input.read_u64() as u32;
        let h  = input.read_u64() as u32;
        match IMAGES.lock() {
            Err(_) => write_error("image lock poisoned", out_len),
            Ok(mut map) => match map.get_mut(&id) {
                None => write_error(&format!("invalid image id {}", id), out_len),
                Some(img) => {
                    *img = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
                    write_nil_error(out_len)
                }
            }
        }
    }

    // Input: [u64 LE id][u64 LE w][u64 LE h]  → error
    #[no_mangle]
    pub extern "C" fn nativeThumbnail(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let id = input.read_u64() as u32;
        let w  = input.read_u64() as u32;
        let h  = input.read_u64() as u32;
        match IMAGES.lock() {
            Err(_) => write_error("image lock poisoned", out_len),
            Ok(mut map) => match map.get_mut(&id) {
                None => write_error(&format!("invalid image id {}", id), out_len),
                Some(img) => {
                    *img = img.thumbnail(w, h);
                    write_nil_error(out_len)
                }
            }
        }
    }

    // Input: [u64 LE id][u32 LE len][ext bytes]  → ([]byte, error)
    #[no_mangle]
    pub extern "C" fn nativeSaveToBytes(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let id  = input.read_u64() as u32;
        let ext = input.read_str().to_string();
        let fmt = match format_from_ext(&ext) {
            Ok(f)  => f,
            Err(e) => return write_bytes_err(&e, out_len),
        };
        match IMAGES.lock() {
            Err(_) => write_bytes_err("image lock poisoned", out_len),
            Ok(map) => match map.get(&id) {
                None => write_bytes_err(&format!("invalid image id {}", id), out_len),
                Some(img) => {
                    let mut out = Cursor::new(Vec::new());
                    match img.write_to(&mut out, fmt) {
                        Ok(())  => write_bytes_ok(&out.into_inner(), out_len),
                        Err(e) => write_bytes_err(&e.to_string(), out_len),
                    }
                }
            }
        }
    }

    // Input: [u64 LE id]  → ([]byte, error)
    #[no_mangle]
    pub extern "C" fn nativeEncodePNG(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let id = input.read_u64() as u32;
        match IMAGES.lock() {
            Err(_) => write_bytes_err("image lock poisoned", out_len),
            Ok(map) => match map.get(&id) {
                None => write_bytes_err(&format!("invalid image id {}", id), out_len),
                Some(img) => {
                    let mut out = Cursor::new(Vec::new());
                    match img.write_to(&mut out, ImageFormat::Png) {
                        Ok(())  => write_bytes_ok(&out.into_inner(), out_len),
                        Err(e) => write_bytes_err(&e.to_string(), out_len),
                    }
                }
            }
        }
    }

    // Input: [u64 LE id]  → (int, int, error)
    #[no_mangle]
    pub extern "C" fn nativeSize(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let id = input.read_u64() as u32;
        match IMAGES.lock() {
            Err(_) => write_two_ints_err("image lock poisoned", out_len),
            Ok(map) => match map.get(&id) {
                None => write_two_ints_err(&format!("invalid image id {}", id), out_len),
                Some(img) => write_two_ints_ok(img.width() as i64, img.height() as i64, out_len),
            }
        }
    }

    // Input: [u64 LE id]  → error
    #[no_mangle]
    pub extern "C" fn nativeClose(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
        let mut input = unsafe { Input::new(ptr, len) };
        let id = input.read_u64() as u32;
        match IMAGES.lock() {
            Err(_) => write_error("image lock poisoned", out_len),
            Ok(mut map) => match map.remove(&id) {
                None    => write_error(&format!("invalid image id {}", id), out_len),
                Some(_) => write_nil_error(out_len),
            }
        }
    }

    // nativeOpen / nativeSave: file system not available in standalone WASM.
    // image.vo's Open() uses os.ReadFile + nativeOpenFromBytes instead.
    // image.vo's Save() uses nativeSaveToBytes + os.WriteFile instead.
    #[no_mangle]
    pub extern "C" fn nativeOpen(_ptr: *const u8, _len: u32, out_len: *mut u32) -> *mut u8 {
        write_u64_err("nativeOpen: not supported in WASM standalone", out_len)
    }

    #[no_mangle]
    pub extern "C" fn nativeSave(_ptr: *const u8, _len: u32, out_len: *mut u32) -> *mut u8 {
        write_error("nativeSave: not supported in WASM standalone", out_len)
    }
}
