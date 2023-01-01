use dioxus::prelude::*;
use time::OffsetDateTime;

use crate::model::javascript_unix_timestamp;

pub static DATE_RENDERER: &str = include_str!("date_renderer.js");

#[derive(Debug, Props)]
pub struct CreatorCardProps<'c> {
    pub date: &'c OffsetDateTime,
}

pub fn locale_date<'s>(cx: Scope<'s, CreatorCardProps<'s>>) -> Element<'s> {
    let js_timestamp = javascript_unix_timestamp::date_time_to_js_timestamp(cx.props.date);

    cx.render(rsx! {
        span {
            class: "date",
            "data-unix-timestamp": "{js_timestamp}",

            "{cx.props.date}"
        }
    })
}
