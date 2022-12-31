use dioxus::prelude::*;
use tokio::sync::watch;

use crate::{
    model::{Creator, javascript_unix_timestamp},
    watcher::{WatcherData, WatcherDataReceive},
};

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

#[derive(Debug)]
pub struct DashboardProps {
    pub watched_data: watch::Receiver<WatcherDataReceive>,
}

#[tracing::instrument(skip_all)]
pub fn dashboard<'s>(cx: Scope<'s, DashboardProps>) -> Element<'s> {
    let watched_data = use_state(cx, || None);

    use_coroutine(cx, {
        let schedule_update = cx.schedule_update_any();
        let scope_id = cx.scope_id();
        let mut watched_data_rx = cx.props.watched_data.clone();
        let watched_data = watched_data.clone();

        move |_: UnboundedReceiver<()>| async move {
            loop {
                *watched_data.make_mut() = watched_data_rx.borrow().as_ref().cloned();

                match watched_data_rx.changed().await {
                    Ok(()) => {}
                    // Channel closed
                    Err(_err) => break,
                };

                schedule_update(scope_id);
            }
        }
    });

    if let Some(WatcherData {
        updated,
        twitch,
        youtube,
        tiltify,
    }) = watched_data.as_deref()
    {
        let js_timestamp = javascript_unix_timestamp::date_time_to_js_timestamp(updated);

        cx.render(rsx! {
            main {
                pre { "{updated}" }
                p {
                    script {
                        "document.currentScript.parentElement.appendChild(document.createTextNode(new Date({js_timestamp}).toLocaleString()));"
                    }
                }
                h1 { "Creators for a Cause" }
                section {
                    h2 { "Fundraiser" }
                    p { "Together we have raised $TODO:"}
                    pre { serde_json::to_string(tiltify).expect("tiltify should be serializable") }
                }
                section {
                    h2 { "Participating Streamers" }
                    section {
                        h3 { "Twitch" }
                        div {
                            class: "creators",
                            twitch.iter().map(|creator| {
                                cx.render(rsx! {
                                    creator_card { key: "{creator.id}", creator: creator, }
                                })
                            })
                        }
                    }
                    section {
                        h3 { "Youtube" }
                        div {
                            class: "creators",
                            youtube.iter().map(|creator| {
                                cx.render(rsx! {
                                    creator_card { key: "{creator.id}", creator: creator, }
                                })
                            })
                        }
                    }
                }
            }
        })
    } else {
        cx.render(rsx! {
            main { "Please wait... the backend has not populated the scraping data" }
        })
    }
}
