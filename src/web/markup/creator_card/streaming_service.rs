use dioxus::prelude::*;

use crate::model::StreamingService;

#[derive(Debug, Props, PartialEq, Eq)]
pub struct Props {
    pub service: StreamingService,
}

pub fn streaming_service(cx: Scope<Props>) -> Element {
    let (class, label, icon) = match cx.props.service {
        StreamingService::Twitch => ("twitch", "Twitch", "/logos/logo-twitch.svg"),
        StreamingService::Youtube => ("youtube", "Youtube", "/logos/logo-youtube.svg"),
    };

    cx.render(rsx! {
        span {
            class: "service {class}",
            dangerous_inner_html: icon,
            "{label}"
        }
    })
}
