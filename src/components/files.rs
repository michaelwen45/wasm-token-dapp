use crate::store::{reducer, Action, Files, FilesVec};
use crate::transaction::{merklize, ToItems, Transaction};
use sycamore::futures::ScopeSpawnLocal;
use sycamore::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlInputElement};

pub async fn create_transaction(file: gloo_file::File) -> Result<Transaction, crate::error::Error> {
    let bytes = gloo_file::futures::read_as_bytes(&file).await.unwrap();
    merklize(bytes)
}

pub fn handle_click(ctx: ScopeRef<'_>, name: String) {
    let files = ctx.use_context::<Signal<Files>>();
    let file = files.get().get(&name).unwrap().clone();
    log::debug!("{:?} start", &name);
    ctx.spawn_local(async move {
        let tx = create_transaction(file).await.unwrap();
        reducer(ctx, Action::TransactionSet(tx));
        log::debug!("{:?} done", &name)
    });
}

#[component]
pub fn FilesSelector<G: Html>(ctx: ScopeRef) -> View<G> {
    let files_vec = ctx.use_context::<Signal<FilesVec>>();
    let tx = ctx.use_context::<Signal<Transaction>>();
    ctx.create_effect(|| {
        let trans = tx.get();
        if trans.format == 2 {
            let deep_hash_item = trans.to_deep_hash_item().unwrap();
            log::debug!("{:?}", deep_hash_item);
        }
    });

    view! {ctx,
        div(class="space-y-4") {
            label(for="file-upload", class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
            focus:outline-none text-sm text-slate-200 uppercase tracking-wider
            font-semibold sm:text-base"){
                    "Select Files..."
                    input(id="file-upload", class="hidden", type="file", multiple=true, on:change={
                        |event: Event| {
                            let target: HtmlInputElement = event.target().unwrap().unchecked_into();
                            if let Some(file_list) = target.files() {
                                reducer(ctx, Action::FilesSet(file_list));
                            }
                        }
                    }) {
                    }

                }
            }
            div(class="overflow-hidden rounded-lg min-w-full") {
                table(class="min-w-full") {
                    thead(class="bg-slate-700") {
                        tr {
                            th(scope="col", class="py-3 px-6 font-semibold tracking-wider text-left text-slate-100 uppercase") {"Name"}
                            th(scope="col", class="py-3 px-6 font-semibold tracking-wider text-left text-slate-100 uppercase") {"Size"}
                            th(scope="col", class="py-3 px-6 font-semibold tracking-wider text-left text-slate-100 uppercase") {"Actions"}
                        }
                    }
                    tbody {
                        Keyed {
                            iterable: files_vec,
                            view: |ctx, (name, size)| {
                                view! {ctx,
                                    tr(class="bg-slate-600 border-slate-700") {
                                        td(class="py-4 px-6 font-medium whitespace-nowrap text-white") {(name)}
                                        td(class="py-4 px-6 text-slate-200") {(size)}
                                        td(class="py-4 px-6 text-slate-200") {button(class="px-5 py-3 rounded-lg shadow-lg bg-indigo-700 hover:bg-indigo-600 active:bg-indigo-800
                                        focus:outline-none text-sm text-slate-200 uppercase tracking-wider
                                        font-semibold sm:text-base",on:click=move |_| handle_click(ctx, name.clone())){"Merklize"}}
                                    }
                                }
                            },
                            key: |(name, _) | name.clone()
                        }
                    }
                }
            }
    }
}
