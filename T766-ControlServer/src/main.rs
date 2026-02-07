#![allow(non_snake_case)]
use dioxus::prelude::*;
mod server;
use chrono::{Local, Timelike};
use server::*;

fn main() {
    launch(App);
}

#[derive(Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    Home {},
    #[route("/logs/:time/:hostname")]
    Logs { time: String, hostname: String },
}
#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/daisyui@4.12.14/dist/full.min.css" }
        script { src: "https://cdn.tailwindcss.com" }
        Router::<Route> {}
    }
}

#[component]
fn Home() -> Element {
    let mut sync_data = use_resource(move || async move { get_sync_table().await.ok() });

    rsx! {
        div { class: "min-h-screen bg-neutral p-6",
            div { class: "max-w-6xl mx-auto",
                div { class: "flex justify-between items-center mb-8 pb-4 border-b border-neutral-content/10",
                    h1 { class: "text-2xl font-light tracking-wide text-neutral-content",
                        "Control Node"
                    }
                    button {
                        class: "text-xs text-neutral-content/60 hover:text-neutral-content transition-colors uppercase tracking-wider",
                        onclick: move |_| sync_data.restart(),
                        "Refresh"
                    }
                }

                match &*sync_data.read_unchecked() {
                    Some(Some(data)) => rsx! {
                        div { class: "overflow-x-auto",
                            table { class: "w-full border-collapse",
                                thead {
                                    tr { class: "border-b border-neutral-content/10",
                                        th { class: "text-left py-3 px-4 text-xs font-light text-neutral-content/50 uppercase tracking-wider",
                                            "Time"
                                        }
                                        for hostname in &data.hostnames {
                                            th { class: "text-center py-3 px-4 text-xs font-light text-neutral-content/50 uppercase tracking-wider",
                                                "{hostname}"
                                            }
                                        }
                                    }
                                }
                                tbody {
                                    for time in &data.times {
                                        tr { class: "border-b border-neutral-content/5 hover:bg-neutral-content/5 transition-colors",
                                            td { class: "py-3 px-4 text-sm font-mono text-neutral-content/70",
                                                "{time}"
                                            }
                                            for hostname in &data.hostnames {
                                                td { class: "text-center py-3 px-4",
                                                    if let Some(hosts) = data.syncs.get(time) {
                                                        if let Some(status) = hosts.get(hostname) {
                                                            Link {
                                                                to: Route::Logs {
                                                                    time: time.clone(),
                                                                    hostname: hostname.clone()
                                                                },
                                                                div {
                                                                    class: if status == "success" {
                                                                        "w-2 h-2 rounded-full bg-success mx-auto cursor-pointer hover:scale-150 transition-transform"
                                                                    } else {
                                                                        "w-2 h-2 rounded-full bg-error mx-auto cursor-pointer hover:scale-150 transition-transform"
                                                                    },
                                                                }
                                                            }
                                                        } else {
                                                            div { class: "w-2 h-2 rounded-full bg-neutral-content/10 mx-auto" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(None) => rsx! {
                        div { class: "text-center py-12 text-neutral-content/40 text-sm",
                            "No sync data"
                        }
                    },
                    None => rsx! {
                        div { class: "text-center py-12 text-neutral-content/40 text-sm",
                            "Loading..."
                        }
                    },
                }
            }
            div {
                class: "fixed bottom-6 right-6 z-50 text-neutral-content/80",
                SyncCountdown {}
            }
        }
    }
}

#[component]
fn SyncCountdown() -> Element {
    let mut countdown = use_signal(|| String::new());

    use_future(move || async move {
        loop {
            let now = Local::now();
            let minute = now.minute();
            let mins_to_next = if minute < 30 {
                30 - minute
            } else {
                60 - minute
            };
            if mins_to_next == 1 {
                countdown.set(format!("{} minutes to sync", mins_to_next));
            } else {
                countdown.set(format!("{} minutes to sync", mins_to_next));
            }
            gloo_timers::future::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    rsx! {
        span { class: "text-2xl text-neutral-content/100",
            "{countdown}"
        }
    }
}

#[component]
fn Logs(time: String, hostname: String) -> Element {
    let time_display = time.clone();
    let hostname_display = hostname.clone();

    let log_data = use_resource(move || {
        let time = time.clone();
        let hostname = hostname.clone();
        async move { get_logs_for_interval(time, hostname).await.ok() }
    });

    rsx! {
        div { class: "min-h-screen bg-neutral p-6",
            div { class: "max-w-4xl mx-auto",
                div { class: "flex justify-between items-center mb-8 pb-4 border-b border-neutral-content/10",
                    h1 { class: "text-2xl font-light tracking-wide text-neutral-content",
                        "Logs: {hostname_display} @ {time_display}"
                    }
                    Link {
                        to: Route::Home {},
                        class: "text-xs text-neutral-content/60 hover:text-neutral-content transition-colors uppercase tracking-wider",
                        "Home"
                    }
                }

                match &*log_data.read_unchecked() {
                    Some(Some(logs)) if !logs.is_empty() => rsx! {
                        div { class: "space-y-4",
                            for (i, log) in logs.iter().enumerate() {
                                LogEntry { log: log.clone(), index: i }
                            }
                        }
                    },
                    Some(Some(_)) => rsx! {
                        div { class: "text-center py-12 text-neutral-content/40 text-sm",
                            "No logs found for this interval"
                        }
                    },
                    Some(None) => rsx! {
                        div { class: "text-center py-12 text-neutral-content/40 text-sm",
                            "No logs found"
                        }
                    },
                    None => rsx! {
                        div { class: "text-center py-12 text-neutral-content/40 text-sm",
                            "Loading..."
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn LogEntry(log: PuppetStatus, index: usize) -> Element {
    let mut is_open = use_signal(|| false);

    rsx! {
        div { class: "border border-neutral-content/10 rounded-lg overflow-hidden",
            button {
                class: "w-full px-4 py-3 flex justify-between items-center hover:bg-neutral-content/5 transition-colors",
                onclick: move |_| is_open.set(!is_open()),

                div { class: "flex items-center gap-4",
                    div {
                        class: if log.status == "success" {
                            "w-3 h-3 rounded-full bg-success"
                        } else {
                            "w-3 h-3 rounded-full bg-error"
                        },
                    }
                    span { class: "text-sm font-mono text-neutral-content/70",
                        "{log.display_timestamp}"
                    }
                    span { class: "text-xs text-neutral-content/50",
                        "Exit code: {log.exit_code}"
                    }
                    if log.total_manifests > 0 {
                        span { class: "text-xs text-neutral-content/50",
                            "Manifests: {log.total_manifests}"
                        }
                    }
                }

                span { class: "text-neutral-content/40",
                    if is_open() { "-" } else { "+" }
                }
            }

            if is_open() {
                div { class: "px-4 py-3 bg-neutral-content/5 border-t border-neutral-content/10",
                    if !log.manifests_applied.is_empty() {
                        div { class: "mb-3",
                            div { class: "text-xs text-success/70 font-semibold mb-1", "Applied Manifests:" }
                            for manifest in &log.manifests_applied {
                                div { class: "text-xs text-neutral-content/60 ml-2", "APPLIED {manifest}" }
                            }
                        }
                    }

                    if !log.manifests_failed.is_empty() {
                        div { class: "mb-3",
                            div { class: "text-xs text-error/70 font-semibold mb-1", "Failed Manifests:" }
                            for manifest in &log.manifests_failed {
                                div { class: "text-xs text-neutral-content/60 ml-2", "FAILED {manifest}" }
                            }
                        }
                    }

                    if !log.logs.is_empty() {
                        div {
                            div { class: "text-xs text-neutral-content/50 font-semibold mb-2", "Logs:" }
                            pre {
                                class: "text-xs text-neutral-content/70 bg-black/20 p-3 rounded overflow-x-auto font-mono whitespace-pre-wrap",
                                "{log.logs}"
                            }
                        }
                    }
                }
            }
        }
    }
}
