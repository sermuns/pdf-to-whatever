use gloo::console::{error, log};
use gloo::file::callbacks::FileReader;
use gloo::file::{Blob, FileList};
use hayro::{Pdf, RenderSettings, render};
use hayro_interpret::InterpreterSettings;
use humansize::format_size;
use image::ImageFormat;
use image::ImageReader;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use web_sys::{DragEvent, Event, HtmlElement, HtmlInputElement, HtmlScriptElement, Url};
use web_time::Instant;
use yew::html::TargetCast;
use yew::{Callback, Component, Context, Html, html};

const CRATE_NAME: &str = env!("CARGO_BIN_NAME");
static INTERPRETER_SETTINGS: Lazy<InterpreterSettings> = Lazy::new(InterpreterSettings::default);
static RENDER_SETTINGS: Lazy<RenderSettings> = Lazy::new(RenderSettings::default);

pub struct RenderedImage {
    stem: String,
    pdf_human_size: String,
    png_data: Vec<u8>,
    jpeg_data: Vec<u8>,
}

pub enum Msg {
    Render(RenderedImage),
    Upload(web_sys::FileList),
}

pub struct App {
    readers: HashMap<String, FileReader>,
    files: Vec<RenderedImage>,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            readers: HashMap::default(),
            files: Vec::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Render(file) => {
                self.readers.remove(&file.stem);
                self.files.push(file);
                true
            }
            Msg::Upload(files) => {
                for file in FileList::from(files).iter() {
                    let mime_type = file.raw_mime_type();
                    if mime_type != "application/pdf" {
                        error!("tried to upload file with mime type", mime_type);
                        continue;
                    }
                    let stem = file.name().trim_end_matches(".pdf").to_string();
                    let pdf_human_size = format_size(file.size(), humansize::BINARY);

                    log!("creating task", &file.name());
                    let link = ctx.link().clone();
                    self.readers.insert(
                        stem.clone(),
                        gloo::file::callbacks::read_as_bytes(file, move |res| {
                            let data = res.expect("failed to read file");

                            let pdf = Pdf::new(Arc::new(data)).expect("failed reading document");

                            let first_page_pixmap = render(
                                pdf.pages().first().expect("has no pages"),
                                &INTERPRETER_SETTINGS,
                                &RENDER_SETTINGS,
                            );

                            let mut now = Instant::now();
                            let png_data = first_page_pixmap.take_png();
                            log!("render png", &stem, now.elapsed().as_secs_f32(), "s");

                            now = Instant::now();
                            let mut jpeg_data: Vec<u8> = Vec::new();
                            let rgba_reader =
                                ImageReader::with_format(Cursor::new(&png_data), ImageFormat::Png)
                                    .decode()
                                    .unwrap();
                            rgba_reader
                                .write_to(
                                    &mut Cursor::new(&mut jpeg_data),
                                    image::ImageFormat::Jpeg,
                                )
                                .unwrap();

                            log!("render jpeg", &stem, now.elapsed().as_secs_f32(), "s");

                            link.send_message(Msg::Render(RenderedImage {
                                stem,
                                pdf_human_size,
                                png_data,
                                jpeg_data,
                            }))
                        }),
                    );
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
        <>
        <main>
            <h1>{CRATE_NAME}</h1>
            <div
                id="file-pick"
                ondrop={ctx.link().callback(|e: DragEvent| {
                    e.prevent_default();
                    Msg::Upload(e.data_transfer().unwrap().files().expect("must be some files"))
                })}
                ondragenter={Callback::from(|e: DragEvent| {
                    e.prevent_default();
                    let element: HtmlElement = e.target_unchecked_into();
                    element.set_class_name("hovered");
                })}
                ondragover={Callback::from(|e: DragEvent| {
                    e.prevent_default();
                })}
                ondragleave={Callback::from(|e: DragEvent| {
                    e.prevent_default();
                    let element: HtmlElement = e.target_unchecked_into();
                    let _ = element.remove_attribute("class");
                })}
            >
                <p>{"Drop your documents here or click to select"}</p>
                <input
                    type="file"
                    accept="application/pdf"
                    multiple=true
                    onchange={ctx.link().callback(|e: Event| {
                        let input: HtmlInputElement = e.target_unchecked_into();
                        Msg::Upload(input.files().expect("must be some files"))
                    })}
                />
            </div>
            <div id="processed">
                { for self.files.iter().map(Self::view_file) }
            </div>
        </main>
        <footer>
            {"Created by "}
            <a href="https://samake.se" target="_blank">
                {"Samuel \"sermuns\" Åkesson"}
            </a>
        </footer>
        </>
        }
    }
}

impl App {
    fn view_file(file: &RenderedImage) -> Html {
        log!("viewing", &file.stem);

        let png_blob = Blob::new::<&[u8]>(&file.png_data);
        let png_url = Url::create_object_url_with_blob(&png_blob.into())
            .expect("failed creating url for png");
        let jpeg_blob = Blob::new::<&[u8]>(&file.jpeg_data);
        let jpeg_url = Url::create_object_url_with_blob(&jpeg_blob.into())
            .expect("failed creating url for png");
        html! {
            <>
                <div>{ &file.stem }</div>
                <div>{ &file.pdf_human_size }</div>
                <a class="download" href={png_url} target="_blank" download={file.stem.clone() + ".png"}>
                    <img src="download-1-svgrepo-com.svg" width="10" height="15" />
                    {"PNG"}
                </a>
                <a class="download" href={jpeg_url} target="_blank" download={file.stem.clone() + ".jpeg"}>
                    <img src="download-1-svgrepo-com.svg" width="10" height="15" />
                    {"JPEG"}
                </a>
            </>
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();

    yew::Renderer::<App>::new().render();
}
