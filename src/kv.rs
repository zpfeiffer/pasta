use wasm_bindgen::{JsValue, prelude::*};
use serde::{Serialize, Deserialize};
use js_sys::{ArrayBuffer, JSON, JsString, Object, Promise};
use wasm_bindgen_futures::JsFuture;
use thiserror::Error;
use chrono::{Duration, prelude::*};
use uuid::Uuid;

use crate::{templates};

#[wasm_bindgen]
extern "C" {
    type PasteNs;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn get(key: &str, data_type: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn put(key: &str, val: &str, obj: Object) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn delete(key: &str) -> Promise;

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Debug, Error)]
pub enum KvError {
    #[error("js error: {0:?}")]
    JsError(JsValue),

    /// Key was not found in namespace.
    ///
    /// Constructed when the KV namespace returns a null JsValue
    #[error("key not found")]
    NotFound,

    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::error::Error),

    #[error("unsupported ttl: {0} (must be >= 60)")]
    UnsupportedTtl(u32),

    #[error("failed to create expiration setting object")]
    ExpReflectFailed,

    #[error("uuid error: {0}")]
    UuidError(#[from] uuid::Error),
}

impl From<JsValue> for KvError {
    #[inline]
    fn from(value: JsValue) -> Self {
        Self::JsError(value)
    }
}

// TODO: Could use an arraybuf for content as KV value then store the
// metadata in metadata

#[derive(Serialize, Deserialize, Debug)]
pub struct StoredPaste {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    pub content: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    pub unlisted: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<DateTime<Utc>>,
}

impl StoredPaste {
    #[inline]
    pub fn get_title<'a>(&'a self) -> &'a str {
        let name = self.title.as_ref();
        match name {
            Some(string) => string.as_str(),
            None => "Untitled paste"
        }
    }

    async fn get(uuid: Uuid) -> Result<Option<StoredPaste>, KvError> {
        let mut buf = Uuid::encode_buffer();
        let key = uuid.to_simple_ref().encode_lower(&mut buf);
        StoredPaste::get_from_exact_key(key).await
    }

    pub async fn get_from_uuid_str(
        uuid_str: &str
    ) -> Result<Option<StoredPaste>, KvError> {
        // If the input `uuid_str` cannot be parsed as a UUID, we're
        // done here as it must not exist. Otherwise, call `StoredPaste::get`.
        match Uuid::parse_str(uuid_str) {
            Ok(uuid) => StoredPaste::get(uuid).await,
            Err(_e) => Ok(None)
        }
    }

    async fn get_from_exact_key(key: &str) -> Result<Option<StoredPaste>, KvError> {
        let promise = PasteNs::get(key, "json");
        let retrieved = JsFuture::from(promise)
            .await
            .map_err(KvError::from)?;
        if !retrieved.is_null() {
            // FIXME: I'm honestly not sure how the JSON.stringify invocation
            // in `into_serde` isn't breaking this. Shouldn't `retrieved`
            // already be a JSON string? Seems wasteful either way.
            // Actually it looks like Cloudflare will do the JSON -> JS object
            // converion with the "json" type set:
            // https://developers.cloudflare.com/workers/runtime-apis/kv#reading-key-value-pairs
            // so there could be better way to do this
            retrieved.into_serde::<StoredPaste>()
                .map(|stored_paste| Some(stored_paste))
                .map_err(KvError::from)
        } else {
            Ok(None)
        }
    }

    pub fn render_html(self) -> String {
        templates::paste(self).into_string()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewPaste {
    id:  Uuid,
    title: Option<String>,
    content: String,
    author: Option<String>,
    unlisted: bool,
    ttl: Option<u32>,
}

impl NewPaste {
    // TODO: Maybe a builder type? Async?
    #[inline]
    pub fn new(
        title: Option<String>,
        content: String,
        author: Option<String>,
        unlisted: bool,
        ttl: Option<u32>
    ) -> NewPaste {
        // TODO: Should invalid TTLs be rejected here?
        let id = Uuid::new_v4();
        NewPaste { id, title, content, author, unlisted, ttl }
    }

    pub async fn put(self) -> Result<(StoredPaste, String), KvError> {
        let ttl = if let Some(ttl) = self.ttl {
            if ttl < 60 {
                Err(KvError::UnsupportedTtl(ttl))
            } else {
                // Create the expiration configuration object
                let obj = js_sys::Object::new();
                let reflect_success  = js_sys::Reflect::set(
                    &obj,
                    &"expirationTtl".into(),
                    &ttl.into()
                )?;
                if reflect_success {
                    Ok(obj)
                } else {
                    Err(KvError::ExpReflectFailed)
                }
            }
        } else {
            //Ok(JsValue::null())
            Ok(js_sys::Object::new())
        }?;

        let id = self.id;
        let stored = self.prepare();

        let paste_json = serde_json::to_string(&stored)?;
        let mut id_str_buf = Uuid::encode_buffer();
        let id_str = id.to_simple_ref().encode_lower(&mut id_str_buf);


        // Insert into KV.
        let promise = PasteNs::put(&id_str, &paste_json, ttl);
        let future = JsFuture::from(promise);

        // While we wait on the put operation, create a string for the path
        let path = format!("/paste/{}", id_str);

        // We must await on the promise to ensure it's inserted but we can
        // discard the (`undefined`) result.
        future.await?;

        Ok((stored, path))
    }

    #[inline]
    pub fn simple_id_string(&self) -> String {
        let mut buf = Uuid::encode_buffer();
        self.id.to_simple_ref().encode_lower(&mut buf).to_string()
    }

    fn prepare(self) -> StoredPaste {
        let exp = match self.ttl {
            Some(ttl) => {
                let now: DateTime<Utc> = Utc::now();
                let ttl_duration = Duration::seconds(ttl.into());

                // Note: TTL values that would cause an overflow are silently
                // removed here.
                now.checked_add_signed(ttl_duration)
            }
            None => None
        };
        StoredPaste {
            title: self.title,
            content: self.content,
            author: self.author,
            unlisted: self.unlisted,
            exp
        }
    }
}
