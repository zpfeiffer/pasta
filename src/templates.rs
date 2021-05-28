use maud::{html, DOCTYPE, Markup};
use crate::{BASE_DOMAIN, kv::StoredPaste};

const STYLESHEET_PATH: &str = "/style.css";
// TODO: unpkg checksums
const PURECSS_PATH: &str = "https://unpkg.com/purecss@2.0.3/build/pure-min.css";
const PURECSS_GRIDS_PATH: &str = "https://unpkg.com/purecss@2.0.3/build/grids-responsive-min.css";

fn page(page_title: &str, body_content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (page_title) }
                link rel="stylesheet" href=(PURECSS_PATH) crossorigin="anonymous";
                link rel="stylesheet" href=(PURECSS_GRIDS_PATH) crossorigin="anonymous";
                link rel="stylesheet" type="text/css" href=(STYLESHEET_PATH);
            }
            body {
                (body_content)
            }
        }
    }
}

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
    page(
        &title,
        html! {
            h1 { (title) }
            div class="pure-g" {
                div class="pure-u-1-3" { p { "Author: " (author) } }
                div class="pure-u-1-3" { p { "Privacy: " (privacy) } }
                div class="pure-u-1-3" { p { "Expires: " (exp) } }
            }
            pre class="paste-content" { (paste.content) }
        }
    )
}

pub(crate) fn paste_created(paste: StoredPaste, path: &str) -> Markup {
    let name = paste.get_title();
    page(
        &name,
        html! {
            h1 { (name) " created!" }
            p {
                "Your paste can be accessed at "
                a href=(path) {
                    (BASE_DOMAIN)
                    (path)
                }
            }
        }
    )
}
