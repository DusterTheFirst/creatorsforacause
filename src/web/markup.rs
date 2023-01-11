use dioxus::prelude::*;
use tokio::sync::watch;

use crate::watcher::{WatcherData, WatcherDataReceive};

use self::creator_card::creator_card;
use self::date::locale_date;

mod creator_card;
mod date;

#[derive(Debug)]
pub struct DashboardProps {
    pub watched_data: watch::Receiver<WatcherDataReceive>,
}

#[tracing::instrument(skip_all)]
pub fn dashboard<'s>(cx: Scope<'s, DashboardProps>) -> Element<'s> {
    let watched_data = use_state(cx, || cx.props.watched_data.borrow().as_ref().cloned());

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
        creators,
        tiltify,
    }) = watched_data.as_deref()
    {
        cx.render(rsx! {
            main {
                h1 {
                    class: "title",
                    "Creators for a Cause"
                }
                p {
                    class: "updated",
                    "Updated: "
                    locale_date { date: updated }
                }
                // p {
                //     script {
                //         "document.currentScript.parentElement.appendChild(document.createTextNode(new Date({js_timestamp}).toLocaleString()));"
                //     }
                // }
                section {
                    p { "Together we have raised ${tiltify.total_amount_raised} out of the ${tiltify.fundraiser_goal_amount} goal" }
                    pre { serde_json::to_string(tiltify).expect("tiltify should be serializable") }
                }
                section {
                    h2 { "Participating Streamers" }
                    div {
                        class: "creators",
                        creators.iter().map(|creator| {
                            cx.render(rsx! {
                                creator_card { key: "{creator.id}", creator: creator, }
                            })
                        })
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
