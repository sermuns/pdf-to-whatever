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
use std::io::{Cursor, Write};
use std::sync::Arc;
use web_sys::{DragEvent, Event, HtmlElement, HtmlInputElement, HtmlScriptElement, Url};
use web_time::Instant;
use yew::html::TargetCast;
use yew::{Callback, Component, Context, Html, html};
use zip::ZipWriter;
use zip::write::{ExtendedFileOptions, FileOptions, SimpleFileOptions};

const CRATE_NAME: &str = env!("CARGO_BIN_NAME");
const CARGO_PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

static INTERPRETER_SETTINGS: Lazy<InterpreterSettings> = Lazy::new(InterpreterSettings::default);
static RENDER_SETTINGS: Lazy<RenderSettings> = Lazy::new(RenderSettings::default);
static ZIP_FILE_OPTIONS: Lazy<SimpleFileOptions> =
    Lazy::new(|| SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored));

pub struct RenderedImage {
    stem: String,
    pdf_human_size: String,
    png_zip: Vec<u8>,
    jpeg_zip: Vec<u8>,
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

                            let mut now = Instant::now();
                            let mut png_zip_writer = ZipWriter::new(Cursor::new(Vec::new()));
                            let mut jpeg_zip_writer = ZipWriter::new(Cursor::new(Vec::new()));
                            for (page_num, page) in pdf.pages().iter().enumerate() {
                                let page_num = page_num + 1; // 1-indexed!
                                let page_pixmap =
                                    render(page, &INTERPRETER_SETTINGS, &RENDER_SETTINGS);

                                let png_bytes = page_pixmap.take_png();
                                let png_filename =
                                    format!("{}-page-{:0>3}.png", stem.clone(), page_num);
                                png_zip_writer
                                    .start_file(png_filename, *ZIP_FILE_OPTIONS)
                                    .map_err(|e| panic!("{:?}", e))
                                    .unwrap();
                                png_zip_writer.write_all(&png_bytes).unwrap_or_else(|_| {
                                    panic!("failed to write png in zip {}", &stem)
                                });

                                let rgba_reader = ImageReader::with_format(
                                    Cursor::new(&png_bytes),
                                    ImageFormat::Png,
                                )
                                .decode()
                                .unwrap();
                                let mut jpeg_bytes: Vec<u8> = Vec::new();
                                rgba_reader
                                    .write_to(
                                        &mut Cursor::new(&mut jpeg_bytes),
                                        image::ImageFormat::Jpeg,
                                    )
                                    .map_err(|e| panic!("fuck"))
                                    .unwrap();
                                let jpeg_filename =
                                    format!("{}-page-{:0>3}.jpeg", stem.clone(), page_num);
                                jpeg_zip_writer
                                    .start_file(jpeg_filename, *ZIP_FILE_OPTIONS)
                                    .map_err(|e| panic!("FUCK: {:?}", e))
                                    .unwrap();
                                jpeg_zip_writer.write_all(&jpeg_bytes).unwrap_or_else(|_| {
                                    panic!("failed to write jpeg in zip {}", &stem)
                                });

                                log!("processed page", page_num, &stem);
                            }
                            log!(
                                "processed all pages for",
                                &stem,
                                now.elapsed().as_secs_f32(),
                                "s"
                            );

                            link.send_message(Msg::Render(RenderedImage {
                                stem,
                                pdf_human_size,
                                png_zip: png_zip_writer
                                    .finish()
                                    .expect("failed finishing png zip")
                                    .into_inner(),
                                jpeg_zip: jpeg_zip_writer
                                    .finish()
                                    .expect("failed finishing jpeg zip")
                                    .into_inner(),
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
            <p>{CARGO_PKG_DESCRIPTION}</p>
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
                <div style="margin-bottom: .5em">{"Drop your documents here or click to select"}</div>
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
        let png_zip_blob = Blob::new::<&[u8]>(&file.png_zip);
        let png_zip_url = Url::create_object_url_with_blob(&png_zip_blob.into())
            .expect("failed creating url for png");
        let jpeg_zip_blob = Blob::new::<&[u8]>(&file.jpeg_zip);
        let jpeg_zip_url = Url::create_object_url_with_blob(&jpeg_zip_blob.into())
            .expect("failed creating url for png");
        html! {
            <>
                <div>{ &file.stem }</div>
                <div>{ &file.pdf_human_size }</div>
                <a class="download" href={png_zip_url} target="_blank" download={file.stem.clone() + ".zip"}>
                    <img src="download-1-svgrepo-com.svg" width="10" height="15" />
                    {"PNG"}
                </a>
                <a class="download" href={jpeg_zip_url} target="_blank" download={file.stem.clone() + ".zip"}>
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
