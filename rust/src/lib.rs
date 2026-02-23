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
}

#[cfg(feature = "native")]
vo_ext::export_extensions!();
