use gloo::file::Blob;
use gloo::utils::document;
use image::ImageFormat;
use once_cell::sync::Lazy;
use pdfium_render::prelude::*;
use std::io::Cursor;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_time::Instant;

use std::collections::HashMap;

const CRATE_NAME: &str = env!("CARGO_BIN_NAME");

use gloo::console::log;
use gloo::file::callbacks::FileReader;
use humansize::format_size;
use web_sys::{DragEvent, Event, HtmlInputElement, Url};
use yew::html::TargetCast;
use yew::{Callback, Component, Context, Html, html};

pub struct RenderedImage {
    stem: String,
    original_human_size: String,
    png_data: Vec<u8>,
    jpeg_data: Vec<u8>,
}

pub enum Msg {
    Render(RenderedImage),
    Upload(Option<web_sys::FileList>),
}

pub struct App {
    readers: HashMap<String, FileReader>,
    files: Vec<RenderedImage>,
    pdfium: Rc<Pdfium>,
}

static RENDER_CONFIG: Lazy<PdfRenderConfig> = Lazy::new(|| {
    PdfRenderConfig::new()
        .set_target_width(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true)
});

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            readers: HashMap::default(),
            files: Vec::default(),
            pdfium: Rc::new(Pdfium::default()),
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
                for file in gloo::file::FileList::from(files.expect("no files uploaded??")).iter() {
                    let link = ctx.link().clone();
                    let file_type = file.raw_mime_type();
                    if file_type != "application/pdf" {
                        continue;
                    }
                    let stem = file.name().trim_end_matches(".pdf").to_string();
                    let document_human_size = format_size(file.size(), humansize::BINARY);

                    let pdfium = self.pdfium.clone();
                    log!("creating task", &file.name());
                    self.readers.insert(
                        stem.clone(),
                        gloo::file::callbacks::read_as_bytes(file, move |res| {
                            let data = res.expect("failed to read file");

                            let document = pdfium
                                .load_pdf_from_byte_vec(data, None)
                                .expect("failed to load document");

                            let first_page = document
                                .pages()
                                .first()
                                .expect("document does not have first page.");

                            let rendered_first_page =
                                first_page.render_with_config(&RENDER_CONFIG).unwrap();

                            let image = rendered_first_page.as_image();

                            // unnecessary micro-optimizations? Do we reall yneed to pre-allocate?
                            let mut png_buf = Cursor::new(Vec::with_capacity(
                                PdfBitmap::bytes_required_for_size(
                                    image.width().try_into().unwrap(),
                                    image.height().try_into().unwrap(),
                                ),
                            ));
                            let mut jpeg_buf = png_buf.clone();

                            let mut now = Instant::now();
                            image.write_to(&mut png_buf, ImageFormat::Png).unwrap();
                            log!("render png", &stem, now.elapsed().as_secs_f32(), "s");

                            now = Instant::now();
                            image.write_to(&mut jpeg_buf, ImageFormat::Jpeg).unwrap();

                            log!("render jpeg", &stem, now.elapsed().as_secs_f32(), "s");

                            link.send_message(Msg::Render(RenderedImage {
                                stem,
                                original_human_size: document_human_size,
                                png_data: png_buf.into_inner(),
                                jpeg_data: jpeg_buf.into_inner(),
                            }))
                        }),
                    );
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
        <main
            ondrop={ctx.link().callback(|e: DragEvent| {
                e.prevent_default();
                Msg::Upload(e.data_transfer().unwrap().files())
            })}
            ondragover={Callback::from(|e: DragEvent| {
                e.prevent_default();
            })}
            ondragenter={Callback::from(|e: DragEvent| {
                e.prevent_default();
            })}
        >
            <h1>{CRATE_NAME}</h1>
            <label>
                <p>{"Drop your documents here or click to select"}</p>
                <input
                    type="file"
                    accept="application/pdf"
                    multiple={true}
                    onchange={ctx.link().callback(move |e: Event| {
                        let input: HtmlInputElement = e.target_unchecked_into();
                        Msg::Upload(input.files())
                    })}
                />
            </label>
            <div id="processed">
                { for self.files.iter().map(Self::view_file) }
            </div>
        </main>
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
                <div>{ &file.original_human_size }</div>
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

#[wasm_bindgen]
pub fn begin_rendering() {
    console_error_panic_hook::set_once();

    let main_element = document()
        .query_selector("main")
        .unwrap()
        .expect("main element must exist!");
    yew::Renderer::<App>::with_root(main_element).render();
}

fn main() {}
