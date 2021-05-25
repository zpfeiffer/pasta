use maud::{html, DOCTYPE, Markup};
use crate::Paste;

const STYLESHEET_PATH: &str = "style.css";
// TODO: unpkg checksums
const PURECSS_PATH: &str = "https://unpkg.com/purecss@2.0.3/build/pure-min.css";
const PURECSS_GRIDS_PATH: &str = "https://unpkg.com/purecss@2.0.3/build/grids-responsive-min.css";

fn page(page_title: &str, body_content: Markup) -> Markup {
    html! {
        html lang="en" {
            (head(page_title))
            body {
                (body_content)
            }
        }
    }
}

fn head(page_title: &str) -> Markup {
    html! {
        (DOCTYPE)
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1.0";
            title { (page_title) }
            link rel="stylesheet" type="text/css" href=(STYLESHEET_PATH);
            link rel="stylesheet" href=(PURECSS_PATH) crossorigin="anonymous";
            link rel="stylesheet" href=(PURECSS_GRIDS_PATH) crossorigin="anonymous";
        }
    }
}

pub(crate) fn index() -> Markup {
    page(
        "Pasta",
        html! {
            h1 { "Pasta" }
            p { "A minimal pastebin on Cloudflare Workers" }
            form class="pure-form pure-form-stacked" method="POST" action="/paste" {
                fieldset {
                    legend { "Create new paste" }
                    input type="text" class="pure-input-1" placeholder="Paste title (optional)";
                    textarea class="pure-input-1" placeholder="Paste here..." required="" { }
                }
                fieldset class="options" {
                    div class="pure-g" {
                        div class="pure-u-1 pure-u-md-1-3" {
                            label for="stacked-expiration" { "Expiration" }
                            select id="stacked-expiration" class="pure-u-23-24" required {
                                option { "1 hour" }
                                option { "24 hours" }
                                option { "Never" }
                            };
                        }
                        div class="pure-u-1 pure-u-md-1-3" {
                            label for="stacked-privacy" { "Privacy" }
                            select id="stacked-privacy" class="pure-u-23-24" required {
                                option { "Public" }
                                option { "Unlisted" }
                            }
                        }
                        div class="pure-u-1 pure-u-md-1-3" {
                            label for="submit-paste" { "Submit paste" }
                            button type="submit" id="submit-paste" class="pure-button pure-button-primary pure-u-23-24" { "Paste" }
                        }
                    }
                }
            }
        }
    )
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
