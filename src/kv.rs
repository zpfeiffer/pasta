use wasm_bindgen::{JsValue, prelude::*};
use serde::{Serialize, Deserialize};
use js_sys::{ArrayBuffer, JSON, JsString, Object, Promise};
use wasm_bindgen_futures::JsFuture;
use thiserror::Error;
use chrono::{Duration, prelude::*};
use uuid::Uuid;
use web_sys::FormData;

use crate::{ALLOW_NEVER_EXPIRE, ResponseError, templates};

#[wasm_bindgen]
extern "C" {
    type PasteNs;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn get(key: &str, data_type: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn put(key: &str, val: &str, obj: Object) -> Promise;

    // #[wasm_bindgen(static_method_of = PasteNs)]
    // fn delete(key: &str) -> Promise;

    // #[wasm_bindgen(js_namespace = console)]
    // fn log(s: &str);
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
        let promise = PasteNs::get(key, "text");
        let retrieved = JsFuture::from(promise)
            .await
            .map_err(KvError::from)?
            .as_string();
        match retrieved.as_deref() {
            Some(string) => serde_json::from_str(string)
                .map(|stored_paste| Some(stored_paste))
                .map_err(KvError::from),
            None => Ok(None)
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

    pub(crate) fn from_form_data(
        form: FormData
    ) -> Result<NewPaste, ResponseError> {
        // TODO: See if we can avoid copying into linear memory just to check string equality
        let title = match form.get("paste-title").as_string() {
            Some(string) if string == "" => None,
            other => other,
        };
        let author: Option<String> = form.get("paste-author").as_string();
        let content = form.get("paste-content")
            .as_string()
            .ok_or(ResponseError::MissingFormValue)?;
        let ttl = match form.get("expiration").as_string().as_deref() {
            Some("1 hour") => Ok(Some(3600u32)),
            Some("24 hours") => Ok(Some(86400)),
            Some("Never") if ALLOW_NEVER_EXPIRE => Ok(None),
            Some(_) => Err(ResponseError::InvalidExpiration),
            None => Err(ResponseError::MissingFormValue),
        }?;
        let unlisted = match form.get("privacy").as_string().as_deref() {
            Some("Public") => Ok(false),
            Some("Unlisted") => Ok(true),
            Some(_) => Err(ResponseError::InvalidExpiration),
            None => Err(ResponseError::MissingFormValue),
        }?;
        let id = Uuid::new_v4();
        Ok(NewPaste { id, title, content, author, unlisted, ttl })
    }

    pub async fn put(self) -> Result<String, KvError> {
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
            Ok(js_sys::Object::new())
        }?;

        let id = self.id;
        let prepared = self.prepare();

        let paste_json = serde_json::to_string(&prepared)?;
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

        Ok(path)
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
