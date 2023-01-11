use dioxus::prelude::*;

use crate::{model::Creator, web::markup::date::locale_date};

#[derive(Debug, Props)]
pub struct CreatorCardProps<'c> {
    pub creator: &'c Creator,
}

pub fn creator_card<'s>(cx: Scope<'s, CreatorCardProps<'s>>) -> Element<'s> {
    let creator = cx.props.creator;

    let class = if creator.stream.is_some() {
        "creator live"
    } else {
        "creator"
    };

    cx.render(rsx! {
        div {
            class: class,
            img {
                src: "{creator.icon_url}",
                alt: "Profile Picture",
                "loading": "lazy",
            }
            h4 {
                class: "display_name",
                a {
                    href: "{creator.href}",
                    "{creator.display_name}"
                }
            }
            {
                creator.stream.as_ref().map(|stream| {
                    rsx! {
                        div {
                            class: "stream",
                            h5 { "Stream" }
                            a {
                                class: "title",
                                href: "{stream.href}",
                                target: "_blank",
                                title: "{stream.title}",
                                "{stream.title}"
                            }
                            p {
                                "Started: "
                                locale_date { date: &stream.start_time }
                            }
                            p {
                                "Viewers: "
                                if let Some(viewers) = stream.viewers {
                                    rsx! { "{viewers}" }
                                } else {
                                    rsx! { "Hidden By Creator" }
                                }
                            }
                        }
                    }
                })
            }
        }
    })
}
