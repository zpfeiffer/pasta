use maud::{html, DOCTYPE, Markup};
use crate::kv::StoredPaste;

const STYLESHEET_PATH: &str = "/style.css";
// TODO: unpkg checksums
const PURECSS_PATH: &str = "https://unpkg.com/purecss@2.0.6/build/pure-min.css";
const PURECSS_INTEGRITY: &str = "sha384-Uu6IeWbM+gzNVXJcM9XV3SohHtmWE+3VGi496jvgX1jyvDTXfdK+rfZc8C1Aehk5";
const PURECSS_GRIDS_PATH: &str = "https://unpkg.com/purecss@2.0.6/build/grids-responsive-min.css";
const PURECSS_GRIDS_INTEGRITY: &str = "sha384-TxqXEM39LKAlr6mwXYlM8+n31/tjeQXzvjbORoLHWeLhkNhWoa9WkMJO/IIghaek";

pub(crate) fn paste(paste: StoredPaste) -> Markup {
    let title = paste.get_title();
    let exp = match paste.exp {
        Some(datetime) => datetime.to_string(),
        None => "never".to_string(),
    };
    let author = match &paste.author {
        Some(name) => html! { span { (name) } },
        None => html! { span class="anon-name" { "anonymous" } },
    };
    let privacy = if paste.unlisted {
        "unlisted"
    } else {
        "public"
    };
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) }
                link rel="stylesheet" href=(PURECSS_PATH) integrity=(PURECSS_INTEGRITY) crossorigin="anonymous";
                link rel="stylesheet" href=(PURECSS_GRIDS_PATH) integrity=(PURECSS_GRIDS_INTEGRITY) crossorigin="anonymous";
                link rel="stylesheet" type="text/css" href=(STYLESHEET_PATH);
            }
            body {
                h1 { (title) }
                div class="pure-g" {
                    div class="pure-u-1-3" { p { "Author: " (author) } }
                    div class="pure-u-1-3" { p { "Privacy: " (privacy) } }
                    div class="pure-u-1-3" { p { "Expires: " (exp) } }
                }
                pre class="paste-content" { (paste.content) }
            }
        }
    }
}
