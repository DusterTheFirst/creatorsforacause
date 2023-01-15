use dioxus::prelude::*;

use crate::model::StreamingService;

#[derive(Debug, Props, PartialEq, Eq)]
pub struct Props {
    pub service: StreamingService,
}

pub fn streaming_service(cx: Scope<Props>) -> Element {
    let (class, label) = match cx.props.service {
        StreamingService::Twitch => ("twitch", "Twitch"),
        StreamingService::Youtube => ("youtube", "Youtube"),
    };

    cx.render(rsx! {
        div {
            class: "service {class}",
            "{label}"
        }
    })
}
