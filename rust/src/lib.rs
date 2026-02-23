use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use image::{DynamicImage, ImageFormat};
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_json::json;
use std::io::Cursor;

#[cfg(feature = "native")]
mod native {
    use super::*;
    use vo_ext::prelude::*;
    use vo_runtime::builtins::error_helper::{write_error_to, write_nil_error};

    lazy_static! {
        static ref IMAGES: Mutex<HashMap<u32, DynamicImage>> = Mutex::new(HashMap::new());
    }

    static NEXT_ID: AtomicU32 = AtomicU32::new(1);

    #[derive(Deserialize)]
    struct OpenReq {
        path: String,
    }

    #[derive(Deserialize)]
    struct NewReq {
        width: u32,
        height: u32,
    }

    #[derive(Deserialize)]
    struct ResizeReq {
        id: u32,
        width: u32,
        height: u32,
    }

    #[derive(Deserialize)]
    struct SaveReq {
        id: u32,
        path: String,
    }

    #[derive(Deserialize)]
    struct IdReq {
        id: u32,
    }

    fn empty_ok() -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }

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

    fn insert_image(img: DynamicImage) -> Result<Vec<u8>, String> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        map.insert(id, img);
        serde_json::to_vec(&json!({ "id": id })).map_err(|e| e.to_string())
    }

    fn handle_open(input: &str) -> Result<Vec<u8>, String> {
        let req: OpenReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let img = image::open(&req.path).map_err(|e| e.to_string())?;
        insert_image(img)
    }

    fn handle_new_rgba(input: &str) -> Result<Vec<u8>, String> {
        let req: NewReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let img = DynamicImage::new_rgba8(req.width, req.height);
        insert_image(img)
    }

    fn handle_resize(input: &str) -> Result<Vec<u8>, String> {
        let req: ResizeReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let current = get_image_mut(&mut map, req.id)?;
        let resized =
            current.resize_exact(req.width, req.height, image::imageops::FilterType::Lanczos3);
        *current = resized;
        empty_ok()
    }

    fn handle_thumbnail(input: &str) -> Result<Vec<u8>, String> {
        let req: ResizeReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let current = get_image_mut(&mut map, req.id)?;
        let thumb = current.thumbnail(req.width, req.height);
        *current = thumb;
        empty_ok()
    }

    fn handle_save(input: &str) -> Result<Vec<u8>, String> {
        let req: SaveReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, req.id)?;
        img.save(&req.path).map_err(|e| e.to_string())?;
        empty_ok()
    }

    fn handle_encode_png(input: &str) -> Result<Vec<u8>, String> {
        let req: IdReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, req.id)?;

        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }

    fn handle_dimensions(input: &str) -> Result<Vec<u8>, String> {
        let req: IdReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        let img = get_image(&map, req.id)?;
        serde_json::to_vec(&json!({ "width": img.width(), "height": img.height() }))
            .map_err(|e| e.to_string())
    }

    fn handle_close(input: &str) -> Result<Vec<u8>, String> {
        let req: IdReq = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let mut map = IMAGES
            .lock()
            .map_err(|_| "image lock poisoned".to_string())?;
        map.remove(&req.id)
            .ok_or_else(|| format!("invalid image id {}", req.id))?;
        empty_ok()
    }

    fn dispatch(op: &str, input: &str) -> Result<Vec<u8>, String> {
        match op {
            "open" => handle_open(input),
            "new_rgba" => handle_new_rgba(input),
            "resize" => handle_resize(input),
            "thumbnail" => handle_thumbnail(input),
            "save" => handle_save(input),
            "encode_png" => handle_encode_png(input),
            "dimensions" => handle_dimensions(input),
            "close" => handle_close(input),
            _ => Err(format!("unsupported operation: {op}")),
        }
    }

    #[vo_fn("github.com/vo-lang/image", "RawCall")]
    pub fn raw_call(call: &mut ExternCallContext) -> ExternResult {
        let op = call.arg_str(0);
        let input = call.arg_str(1);

        match dispatch(op, input) {
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
}

#[cfg(feature = "native")]
vo_ext::export_extensions!();
