use dioxus::prelude::*;

use crate::model::{Creator, CreatorsList};

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
                // loading: "lazy",
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
                            p {
                                "Title: "
                                a {
                                    href: "{stream.href}",
                                    target: "_blank",
                                    "{stream.title}"
                                }
                            }
                            p { "Start Time: {stream.start_time}" }
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

#[derive(Debug, Props, PartialEq, Eq)]
pub struct DashboardProps {
    pub twitch: CreatorsList,
    pub youtube: CreatorsList,
}

#[tracing::instrument(skip_all)]
pub fn dashboard<'s>(cx: Scope<'s, DashboardProps>) -> Element<'s> {
    let funds = 0;

    cx.render(rsx! {
        main {
            h1 { "Creators for a Cause" }
            section {
                h2 { "Fundraiser" }
                p { "Together we have raised ${funds}"}
            }
            section {
                h2 { "Participating Streamers" }
                section {
                    h3 { "Twitch" }
                    pre { "{cx.props.twitch.updated}" }
                    div {
                        class: "creators",
                        {
                            cx.props.twitch.creators.iter().map(|creator| {
                                cx.render(rsx! {
                                    creator_card { creator: creator, }
                                })
                            })
                        }
                    }
                }
                section {
                    h3 { "Youtube" }
                    pre { "{cx.props.youtube.updated}" }
                    div {
                        class: "creators",
                        {
                            cx.props.youtube.creators.iter().map(|creator| {
                                cx.render(rsx! {
                                    creator_card { creator: creator, }
                                })
                            })
                        }
                    }
                }
            }
        }
    })
}
