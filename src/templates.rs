use maud::{html, DOCTYPE, Markup};
use crate::{Paste, BASE_URL};

const STYLESHEET_PATH: &str = "style.css";
// TODO: unpkg checksums
const PURECSS_PATH: &str = "https://unpkg.com/purecss@2.0.3/build/pure-min.css";
const PURECSS_GRIDS_PATH: &str = "https://unpkg.com/purecss@2.0.3/build/grids-responsive-min.css";

fn page(page_title: &str, body_content: Markup) -> Markup {
    html! {
        html lang="en" {
            (DOCTYPE)
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (page_title) }
                link rel="stylesheet" type="text/css" href=(STYLESHEET_PATH);
                link rel="stylesheet" href=(PURECSS_PATH) crossorigin="anonymous";
                link rel="stylesheet" href=(PURECSS_GRIDS_PATH) crossorigin="anonymous";
            }
            body {
                (body_content)
            }
        }
    }
}

pub(crate) fn paste(paste: Paste) -> Markup {
    let title = paste.title.unwrap_or(format!("Paste {}", paste.id));
    page(
        &title,
        html! {
            h1 { (title) }
            div class="pure-g" {
                div class="pure-u-1-3" { p { "Thirds" } }
                div class="pure-u-1-3" { p { "Thirds" } }
                div class="pure-u-1-3" { p { "Thirds" } }
            }
            pre class="content" { (paste.content) }
        }
    )
}

pub(crate) fn paste_created(paste: Paste) -> Markup {
    let path = paste.get_path();
    let name = paste.title.unwrap_or(format!("Paste {}", paste.id));
    page(
        &name,
        html! {
            h1 { (name) "created!" }
            p {
                "Your paste can be accessed at "
                a href=(path) {
                    (BASE_URL)
                    (path)
                }
            }
        }
    )
}
