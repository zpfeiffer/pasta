extern crate cfg_if;
extern crate wasm_bindgen;

//#[macro_use]
//extern crate serde_derive;'

mod utils;

use cfg_if::cfg_if;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use js_sys::{Array, Error, Function, Promise, Reflect, JsString};
use arrayvec::ArrayString;
// use std::time::SystemTime;
use wasm_bindgen_futures::JsFuture;
use serde::{Serialize, Deserialize};

// TODO: Use array buffers for keys instead of strings

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen]
extern "C" {
    type PasteNs;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn get(key: &str, data_type: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn put(key: &str, val: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn delete(key: &str) -> Promise;
}

#[wasm_bindgen]
pub fn greet() -> String {
    "Hello, wasm-worker!".to_string()
}

fn render_paste_html(paste: Paste) -> String {
    format!(
        "<html>
        <h1>{0}</h1>
        <p>author: {1}</p>
        <pre>{2}</pre>
        </html>",
        paste.title,
        paste.author,
        paste.content
    )
}

/* 
 * Request:
 * POST /new
 *
 * Response:
 * 201 (Created) w/ Location header
 * Content TBD possibly Refresh header or HTML redirect or JS redirect to new resource
 */

//type PasteId = ArrayString::<8>;
type PasteId = String;

#[derive(Serialize, Deserialize, Debug)]
struct Paste {
    id: PasteId,
    title: String,
    content: String,
    author: String,
}

impl Paste {
    async fn get(paste_id: PasteId) -> Result<Paste, KvError> {
        let promise = PasteNs::get(paste_id.as_str(), "text");
        JsFuture::from(promise)
            .await
            .map_err(KvError::from)?
            //.dyn_into::<JsString>(); // if this breaks try as_string which is slower but might work
            // .as_string()
            // .ok_or(KvError::ResultNotString)?
            .into_serde::<Paste>()
            .map_err(KvError::from)
    }

    #[inline]
    fn id_str(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Error)]
enum KvError {
    #[error("js error: {0:?}")]
    JsError(JsValue),

    // #[error("expected kv get result to be a string")]
    // ResultNotString,

    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::error::Error),
}

impl From<JsValue> for KvError {
    fn from(value: JsValue) -> Self {
        Self::JsError(value)
    }
}
