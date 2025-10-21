use image::ImageOutputFormat;
use once_cell::sync::Lazy;
use pdfium_render::prelude::*;
use std::rc::Rc;

use std::collections::HashMap;

use gloo::file::callbacks::FileReader;
use humansize::format_size;
use web_sys::{DragEvent, Event, HtmlInputElement};
use yew::html::TargetCast;
use yew::{Callback, Component, Context, Html, html};

pub struct RenderedImage {
    name: String,
    human_size: String,
    png_data: Vec<u8>,
    jpg_data: Vec<u8>,
}

pub enum Msg {
    Loaded(RenderedImage),
    Files(Option<web_sys::FileList>),
}

pub struct App {
    readers: HashMap<String, FileReader>,
    files: Vec<RenderedImage>,
    pdfium: Rc<Pdfium>,
}

static RENDER_CONFIG: Lazy<PdfRenderConfig> = Lazy::new(|| {
    PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(2000)
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
            Msg::Loaded(file) => {
                self.readers.remove(&file.name);
                self.files.push(file);
                true
            }
            Msg::Files(files) => {
                for file in gloo::file::FileList::from(files.expect("files")).iter() {
                    let link = ctx.link().clone();
                    let name = file.name().clone();
                    let file_type = file.raw_mime_type();
                    if file_type != "application/pdf" {
                        continue;
                    }
                    let human_size = format_size(file.size(), humansize::BINARY);

                    let pdfium = self.pdfium.clone();
                    let task = {
                        gloo::file::callbacks::read_as_bytes(file, move |res| {
                            let data = res.expect("failed to read file");
                            let document = pdfium.load_pdf_from_byte_vec(data, None).unwrap();
                            let first_page = document
                                .pages()
                                .first()
                                .expect("document does not have first page.");
                            let rendered_first_page =
                                first_page.render_with_config(&RENDER_CONFIG).unwrap();

                            let rendered_image = rendered_first_page.as_image();
                            let mut png_buf = Vec::new();
                            rendered_image
                                .write_to(&mut png_buf, ImageOutputFormat::Png)
                                .unwrap();

                            link.send_message(Msg::Loaded(RenderedImage { name, human_size }))
                        })
                    };
                    self.readers.insert(file.name(), task);
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let noop_drag = Callback::from(|e: DragEvent| {
            e.prevent_default();
        });

        html! {
            <>
                <main>
                    <h1>{ "pdf-to-whatever" }</h1>
                    <label>
                        <div
                            id="drop-container"
                            ondrop={ctx.link().callback(|event: DragEvent| {
                                event.prevent_default();
                                Msg::Files(event.data_transfer().unwrap().files())
                            })}
                            ondragover={&noop_drag}
                            ondragenter={&noop_drag}
                        >
                            <p>{"Drop your images here or click to select"}</p>
                        </div>
                        <input
                            type="file"
                            accept="application/pdf"
                            multiple={true}
                            onchange={ctx.link().callback(move |e: Event| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                Msg::Files(input.files())
                            })}
                        />
                    </label>
                    <div id="processed">
                        <div>{ "2025-10-17 närvarolista_2025-10-15.pdf"}</div>
                        <div>{"2.05 MiB"}</div>
                        <a class="download">
                            <img src="static/download-1-svgrepo-com.svg" width="10" height="15" />
                            {"PNG"}
                        </a>
                        <a class="download">
                            <img src="static/download-1-svgrepo-com.svg" width="10" height="15" />
                            {"JPG"}
                        </a>
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
        html! {
            <>
                <div>{ &file.name }</div>
                <div>{ &file.human_size }</div>
                <a class="download">
                    <img src="static/download-1-svgrepo-com.svg" width="10" height="15" />
                    {"PNG"}
                </a>
                <a class="download">
                    <img src="static/download-1-svgrepo-com.svg" width="10" height="15" />
                    {"JPG"}
                </a>
            </>
        }
    }
}
fn main() {
    yew::Renderer::<App>::new().render();
}
