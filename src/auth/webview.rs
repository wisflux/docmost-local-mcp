use std::{env, process::Stdio};

use anyhow::{Context, Result};
use tokio::process::{Child, Command};

use crate::{debug::debug_log, types::AuthWindowSession};

const EXIT_SUCCESS: i32 = 0;
const EXIT_CANCELLED: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthWindowMode {
    Native,
    Browser,
}

pub struct AuthWindowHandle {
    pub mode: AuthWindowMode,
    child: Option<Child>,
}

impl AuthWindowHandle {
    pub async fn wait_for_exit(&mut self) -> Result<Option<i32>> {
        match self.child.as_mut() {
            Some(child) => Ok(child.wait().await?.code()),
            None => Ok(None),
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill().await;
        }
        self.child = None;
        Ok(())
    }
}

pub async fn launch_auth_window(session: &AuthWindowSession) -> Result<AuthWindowHandle> {
    #[cfg(feature = "native-webview")]
    {
        match launch_native_window(session).await {
            Ok(child) => {
                return Ok(AuthWindowHandle {
                    mode: AuthWindowMode::Native,
                    child: Some(child),
                });
            }
            Err(error) => {
                debug_log(
                    "auth-helper",
                    "Native helper unavailable, falling back to browser",
                    Some(&serde_json::json!({
                        "error": error.to_string(),
                        "fallbackUrl": session.fallback_url
                    })),
                );
            }
        }
    }

    open::that(&session.fallback_url).context("Failed to open fallback browser window")?;
    Ok(AuthWindowHandle {
        mode: AuthWindowMode::Browser,
        child: None,
    })
}

#[cfg(feature = "native-webview")]
async fn launch_native_window(session: &AuthWindowSession) -> Result<Child> {
    let executable = env::current_exe().context("Failed to resolve current executable")?;
    debug_log(
        "auth-helper",
        "Launching native auth helper",
        Some(&serde_json::json!({
            "binaryPath": executable,
            "loginUrl": session.login_url
        })),
    );

    let child = Command::new(executable)
        .arg("auth-window")
        .arg("--url")
        .arg(&session.login_url)
        .arg("--success-url")
        .arg(&session.success_url)
        .arg("--title")
        .arg(&session.window_title)
        .arg("--width")
        .arg(session.window_width.to_string())
        .arg("--height")
        .arg(session.window_height.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn native auth window subprocess")?;

    Ok(child)
}

pub async fn run_auth_window(
    url: String,
    success_url: String,
    title: String,
    width: u32,
    height: u32,
) -> Result<()> {
    #[cfg(not(feature = "native-webview"))]
    {
        let _ = (url, success_url, title, width, height);
        return Err(anyhow::anyhow!(
            "This binary was built without the native-webview feature."
        ));
    }

    #[cfg(feature = "native-webview")]
    {
        run_native_webview(url, success_url, title, width, height)
    }
}

#[cfg(feature = "native-webview")]
fn run_native_webview(
    url: String,
    success_url: String,
    title: String,
    width: u32,
    height: u32,
) -> Result<()> {
    use tao::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
        window::WindowBuilder,
    };
    use wry::WebViewBuilder;

    enum UserEvent {
        AuthCompleted,
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    let mut window_builder = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(f64::from(width), f64::from(height)))
        .with_resizable(false);

    if let Some(position) = centered_position(&event_loop, width, height) {
        window_builder = window_builder.with_position(position);
    }

    let window = window_builder
        .build(&event_loop)
        .context("Failed to create auth helper window")?;
    let proxy = event_loop.create_proxy();

    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .with_navigation_handler(move |candidate| {
            if candidate.starts_with(&success_url) {
                let _ = proxy.send_event(UserEvent::AuthCompleted);
            }
            true
        })
        .build(&window)
        .context("Failed to create native webview")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::AuthCompleted) => std::process::exit(EXIT_SUCCESS),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => std::process::exit(EXIT_CANCELLED),
            _ => {}
        }
    });

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(feature = "native-webview")]
fn centered_position<T>(
    event_loop: &tao::event_loop::EventLoopWindowTarget<T>,
    width: u32,
    height: u32,
) -> Option<tao::dpi::LogicalPosition<f64>> {
    let monitor = event_loop.primary_monitor()?;
    let scale_factor = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale_factor);
    let x = ((size.width - f64::from(width)) / 2.0).max(0.0);
    let y = ((size.height - f64::from(height)) / 2.0).max(0.0);
    Some(tao::dpi::LogicalPosition::new(x, y))
}

pub fn helper_exit_success(code: Option<i32>) -> bool {
    code == Some(EXIT_SUCCESS)
}

pub fn helper_exit_cancelled(code: Option<i32>) -> bool {
    code == Some(EXIT_CANCELLED)
}

pub fn helper_exit_error_message(code: Option<i32>) -> String {
    if helper_exit_cancelled(code) {
        return "The Docmost sign-in window was closed before authentication completed."
            .to_string();
    }

    if let Some(code) = code {
        return format!("The Docmost sign-in window exited unexpectedly (code {code}).");
    }

    "The Docmost sign-in window exited unexpectedly.".to_string()
}

#[cfg(test)]
impl AuthWindowHandle {
    pub(crate) fn test_handle(mode: AuthWindowMode, child: Child) -> Self {
        Self {
            mode,
            child: Some(child),
        }
    }
}
