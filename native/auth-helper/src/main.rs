use anyhow::{Context, Result, anyhow, bail};
use std::env;
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

const EXIT_SUCCESS: i32 = 0;
const EXIT_CANCELLED: i32 = 2;
const EXIT_FAILURE: i32 = 3;

fn main() {
    if let Err(error) = try_main() {
        eprintln!("{error:#}");
        std::process::exit(EXIT_FAILURE);
    }
}

fn try_main() -> Result<()> {
    let options = Options::from_args(env::args().skip(1))?;
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    let mut window_builder = WindowBuilder::new()
        .with_title(options.title.clone())
        .with_inner_size(LogicalSize::new(
            f64::from(options.width),
            f64::from(options.height),
        ))
        .with_resizable(false);

    if let Some(position) = centered_position(&event_loop, options.width, options.height) {
        window_builder = window_builder.with_position(position);
    }

    let window = window_builder
        .build(&event_loop)
        .context("Failed to create auth helper window")?;

    let success_url = options.success_url.clone();
    let proxy = event_loop.create_proxy();

    let _webview = WebViewBuilder::new()
        .with_url(&options.url)
        .with_navigation_handler(move |url| {
            if url.starts_with(&success_url) {
                let _ = proxy.send_event(UserEvent::AuthCompleted);
            }
            true
        })
        .build(&window)
        .context("Failed to create native webview")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::AuthCompleted) => {
                std::process::exit(EXIT_SUCCESS);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                std::process::exit(EXIT_CANCELLED);
            }
            _ => {}
        }
    });

    #[allow(unreachable_code)]
    Ok(())
}

fn centered_position<T>(
    event_loop: &tao::event_loop::EventLoopWindowTarget<T>,
    width: u32,
    height: u32,
) -> Option<LogicalPosition<f64>> {
    let monitor = event_loop.primary_monitor()?;
    let scale_factor = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale_factor);
    let x = ((size.width - f64::from(width)) / 2.0).max(0.0);
    let y = ((size.height - f64::from(height)) / 2.0).max(0.0);

    Some(LogicalPosition::new(x, y))
}

#[derive(Debug)]
struct Options {
    url: String,
    success_url: String,
    title: String,
    width: u32,
    height: u32,
}

impl Options {
    fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut url = None;
        let mut success_url = None;
        let mut title = String::from("Docmost Sign In");
        let mut width = 500;
        let mut height = 680;
        let arguments = args.into_iter().collect::<Vec<_>>();
        let mut index = 0usize;

        while index < arguments.len() {
            let argument = arguments[index].as_str();
            let next_value = || {
                arguments
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| anyhow!("Missing value for {argument}"))
            };

            match argument {
                "--url" => {
                    url = Some(next_value()?);
                    index += 2;
                }
                "--success-url" => {
                    success_url = Some(next_value()?);
                    index += 2;
                }
                "--title" => {
                    title = next_value()?;
                    index += 2;
                }
                "--width" => {
                    width = next_value()?
                        .parse()
                        .context("Invalid integer value for --width")?;
                    index += 2;
                }
                "--height" => {
                    height = next_value()?
                        .parse()
                        .context("Invalid integer value for --height")?;
                    index += 2;
                }
                _ => {
                    bail!("Unknown argument: {argument}");
                }
            }
        }

        Ok(Self {
            url: url.context("Missing --url")?,
            success_url: success_url.context("Missing --success-url")?,
            title,
            width,
            height,
        })
    }
}

enum UserEvent {
    AuthCompleted,
}
