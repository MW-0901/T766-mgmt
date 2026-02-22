#![allow(non_snake_case)]
use dioxus::prelude::*;
mod server;
use chrono::{Local, Timelike};
use server::*;
use urlencoding::{encode, decode};

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    use axum::extract::Path;
    use axum::response::IntoResponse;
    use axum::http::StatusCode;
    use dioxus_server::DioxusRouterExt;
    use tower_http::services::ServeDir;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    async fn hash_handler(Path(filename): Path<String>) -> impl IntoResponse {
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') || filename.starts_with('.') {
            return (StatusCode::BAD_REQUEST, String::from("Invalid filename"));
        }
        let file_path = format!("/puppet/{}", filename);
        let mut file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => return (StatusCode::NOT_FOUND, format!("File not found: {}", e)),
        };
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 8192];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {}", e)),
            }
        }
        (StatusCode::OK, format!("{:x}", hasher.finalize()))
    }

    let address = dioxus::cli_config::fullstack_address_or_localhost();

    let router = axum::Router::new()
        .nest_service("/data", ServeDir::new("/puppet"))
        .route("/data/hashes/{filename}", axum::routing::get(hash_handler))
        .serve_dioxus_application(dioxus_server::ServeConfig::new(), App);

    let router = router.into_make_service();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(App);
}

#[derive(Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    Home {},
    #[route("/logs/:time/:hostname")]
    Logs { time: String, hostname: String },
    #[route("/checkins")]
    Checkins {},
    #[route("/checkin/:hostname/:log_text")]
    CheckinLog { hostname: String, log_text: String },
}

#[component]
fn Checkins() -> Element {
    let mut search_query = use_signal(|| String::new());
    let checkin_data = use_resource(move || async move { get_all_checkin_logs().await.ok() });

    let filtered_logs = use_memo(move || {
        let query = search_query().to_lowercase();
        if let Some(Some(logs)) = checkin_data.read().as_ref() {
            let mut filtered = if query.is_empty() {
                logs.clone()
            } else {
                logs.iter()
                    .filter(|log| {
                        log.log.to_lowercase().contains(&query)
                            || log.hostname.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect()
            };

            filtered.sort_by(|a, b| {
                let a_time = a.log.split(" - ").next().unwrap_or("");
                let b_time = b.log.split(" - ").next().unwrap_or("");
                b_time.cmp(a_time)
            });

            filtered
        } else {
            Vec::new()
        }
    });

    rsx! {
        div { class: "min-h-screen bg-neutral p-6",
              div { class: "max-w-6xl mx-auto",
                    div { class: "flex justify-between items-center mb-8 pb-4 border-b border-neutral-content/10",
                          h1 { class: "text-2xl font-light tracking-wide text-neutral-content",
                               "Laptop Checkins"
                          }
                          Link {
                              to: Route::Home {},
                              class: "text-xs text-neutral-content/60 hover:text-neutral-content transition-colors uppercase tracking-wider",
                              "Home"
                          }
                    }

                    div { class: "mb-6",
                          input {
                              class: "w-full px-4 py-3 bg-neutral-content/5 border border-neutral-content/10 rounded-lg text-neutral-content placeholder-neutral-content/40 focus:outline-none focus:border-neutral-content/30 transition-colors",
                              r#type: "text",
                              placeholder: "Search logs...",
                              value: "{search_query}",
                              oninput: move |evt| search_query.set(evt.value().clone())
                          }
                    }

                    match &*checkin_data.read_unchecked() {
                        Some(Some(_)) => rsx! {
                            div { class: "space-y-2",
                                  if filtered_logs.read().is_empty() {
                                      div { class: "text-center py-12 text-neutral-content/40 text-sm",
                                            "No matching logs found"
                                      }
                                  } else {
                                      for log in filtered_logs.read().iter() {
                                          Link {
                                              to: Route::CheckinLog {
                                                  hostname: log.hostname.clone(),
                                                  log_text: encode(&log.log).to_string(),
                                              },
                                              div {
                                                  class: "px-4 py-3 bg-neutral-content/5 border border-neutral-content/10 rounded-lg hover:bg-neutral-content/10 transition-colors cursor-pointer",
                                                  div { class: "mb-2",
                                                        span { class: "text-xs font-mono text-neutral-content/50",
                                                               "{log.hostname}"
                                                        }
                                                  }
                                                  pre {
                                                      class: "text-sm text-neutral-content/70 font-mono whitespace-pre-wrap break-words",
                                                      "{log.log}"
                                                  }
                                              }
                                          }
                                      }
                                  }
                            }
                        },
                        Some(None) => rsx! {
                            div { class: "text-center py-12 text-neutral-content/40 text-sm",
                                  "No checkin logs"
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
fn CheckinLog(hostname: String, log_text: String) -> Element {
    let decoded_log_text = decode(&log_text).unwrap_or_default().to_string();

    let log_data = use_resource(move || {
        let hostname = hostname.clone();
        let log_text = decoded_log_text.clone();
        async move { get_checkin_log(hostname, log_text).await.ok() }
    });

    rsx! {
        div { class: "min-h-screen bg-neutral p-6",
              div { class: "max-w-4xl mx-auto",
                    div { class: "flex justify-between items-center mb-8 pb-4 border-b border-neutral-content/10",
                          h1 { class: "text-2xl font-light tracking-wide text-neutral-content",
                               "Checkin Log"
                          }
                          Link {
                              to: Route::Checkins {},
                              class: "text-xs text-neutral-content/60 hover:text-neutral-content transition-colors uppercase tracking-wider",
                              "Back to Checkins"
                          }
                    }

                    match &*log_data.read_unchecked() {
                        Some(Some(Some(log))) => rsx! {
                            div { class: "space-y-4",
                                  div { class: "px-4 py-3 bg-neutral-content/5 border border-neutral-content/10 rounded-lg",
                                        div { class: "mb-4 pb-3 border-b border-neutral-content/10",
                                              span { class: "text-sm font-mono text-neutral-content/70",
                                                     "{log.hostname}"
                                              }
                                        }
                                        pre {
                                            class: "text-base text-neutral-content font-mono whitespace-pre-wrap break-words leading-relaxed",
                                            "{log.log}"
                                        }
                                  }
                            }
                        },
                        Some(Some(None)) | Some(None) => rsx! {
                            div { class: "text-center py-12 text-neutral-content/40 text-sm",
                                  "Log not found"
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
                          Link {
                              to: Route::Checkins {},
                              class: "text-xs text-neutral-content/60 hover:text-neutral-content transition-colors uppercase tracking-wider",
                              "Laptop Checkins"
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
            countdown.set(format!("{} minutes to sync", mins_to_next));
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
                                  for log in logs.iter() {
                                      LogEntry { log: log.clone() }
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
fn LogEntry(log: PuppetStatus) -> Element {
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
                        span { class: "text-xs text-neutral-content/50",
                               "Exit code: {log.exit_code}"
                        }
                  }

                  span { class: "text-neutral-content/40",
                         if is_open() { "−" } else { "+" }
                  }
              }

              if is_open() {
                  div { class: "px-4 py-3 bg-neutral-content/5 border-t border-neutral-content/10",
                        if !log.logs.is_empty() {
                            pre {
                                class: "text-xs text-neutral-content/70 bg-black/20 p-3 rounded overflow-x-auto font-mono whitespace-pre-wrap",
                                "{log.logs}"
                            }
                        } else {
                            div { class: "text-xs text-neutral-content/40 italic",
                                  "No logs available"
                            }
                        }
                  }
              }
        }
    }
}