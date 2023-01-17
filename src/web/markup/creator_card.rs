use dioxus::prelude::*;

use crate::{model::Creator, web::markup::date::locale_date};

use self::streaming_service::streaming_service;

mod streaming_service;

#[derive(Debug, Props)]
pub struct Props<'c> {
    pub creator: &'c Creator,
}

pub fn creator_card<'s>(cx: Scope<'s, Props<'s>>) -> Element<'s> {
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
                class: "icon",
                draggable: false,
                src: "{creator.icon_url}",
                alt: "profile picture for {creator.display_name}",
                "loading": "lazy"
            }
            h3 {
                class: "display_name",
                a {
                    href: "{creator.href}",
                    target: "_blank",
                    "{creator.display_name}"
                }
            }
            {
                creator.stream.as_ref().map(|stream| {
                    rsx! {
                        div {
                            class: "stream",
                            h4 { "Stream" }
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
            streaming_service {
                service: creator.service
            }
        }
    })
}
