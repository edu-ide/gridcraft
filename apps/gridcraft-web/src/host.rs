//! The `postMessage` bridge to a same-origin parent page (`?host=parent`), for pages that embed
//! GridCraft in an `<iframe>` (such as a Nextcloud app). Messages are plain objects with a `type`:
//!
//! - `gridcraft:ready` (to the parent): `{ version }`, the app is listening; send a file now.
//! - `gridcraft:open` (from the parent): `{ name, bytes }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a workbook picked with File › Open or dropped on the window.
//! - `gridcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`) for every
//!   file GridCraft would otherwise download: Save and Save As (an .xlsx workbook) and Export
//!   as PDF. The parent stores it and reports failures itself.
//! - `gridcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `gridcraft:failed` (to the parent): `{ error }` when the app couldn't start.
//! - `gridcraft:command` (from the parent): `{ request, id, params }` runs a registry command
//!   (the ids the CLI, the control channel and MCP use, such as `cell.set` or `selection.set`)
//!   through the app, as the control channel's `engine.execute` does.
//! - `gridcraft:result` (to the parent): `{ request, ok, result }` or `{ request, ok: false,
//!   error }` for each command, `request` echoed as it came.
//!
//! Only the parent window on the page's own origin is heard or answered. In host mode the page
//! starts with an empty workbook instead of a sample, and the parent guards leaving the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gridcraft_ui_egui::{Inbox, Services, SheetApp};
use serde_json::Value as Json;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;

use crate::host_protocol::{self as protocol, Inbound};

/// Saving one of these hands the whole workbook to the parent, which stores it: the workbook
/// counts as saved under that name, as it would after Save on the desktop.
const WORKBOOK_EXTS: &[&str] = &["xlsx"];

/// The longest user name taken from `?author=`.
const MAX_AUTHOR_CHARS: usize = 200;

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// Names of the files handed to the parent since the last frame.
    saved: Rc<RefCell<Vec<String>>>,
    /// The unsaved-changes state last reported to the parent.
    reported: Rc<Cell<Option<bool>>>,
    /// Commands from the parent, run at the next frame (they need the app).
    commands: Rc<RefCell<Vec<Inbound>>>,
}

impl Bridge {
    /// `None` when the page isn't framed (there is no parent to talk to).
    pub fn new() -> Option<Self> {
        let window = web_sys::window()?;
        let parent = window.parent().ok().flatten()?;
        if js_sys::Object::is(&parent, &window) {
            return None;
        }
        let origin = window.location().origin().ok()?;
        Some(Self { window, parent, origin, saved: Rc::default(), reported: Rc::default(), commands: Rc::default() })
    }

    /// Send saved and exported files to the parent instead of downloading them, and hear files
    /// from it.
    pub fn connect(&self, services: &mut Services, inbox: Inbox, ctx: egui::Context) {
        self.listen(inbox, ctx);
        let bridge = self.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| {
            if let Err(e) = bridge.post_save(name, bytes) {
                log::error!("couldn't hand {name} to the page: {e}");
            }
        }));
    }

    /// `?author=`: the host's name for the user signs new notes and comments.
    pub fn set_author(app: &mut SheetApp, author: &str) {
        let author: String = author.trim().chars().take(MAX_AUTHOR_CHARS).collect();
        if !author.is_empty() {
            app.session.prefs.user_name = author;
        }
    }

    pub fn post_ready(&self) {
        self.send(&[("type", protocol::READY.into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", protocol::FAILED.into()), ("error", error.into())]);
    }

    /// Before the frame's layout, after the app has opened what arrived: run the parent's
    /// commands through the app and answer each one.
    pub fn run_commands(&self, app: &mut SheetApp) {
        let pending = std::mem::take(&mut *self.commands.borrow_mut());
        for inbound in pending {
            let answer = match inbound {
                Inbound::Command(command) => protocol::run(command, |id, params| app.run(id, params)),
                Inbound::BadCommand { request, error } => protocol::result(request, Err(error)),
                Inbound::Open { .. } => continue,
            };
            if let Err(e) = self.post_json(&answer) {
                log::error!("couldn't answer the page: {e}");
            }
        }
    }

    /// After each frame: a workbook handed to the parent this frame counts as saved under the
    /// name it was saved as, and the parent hears when unsaved changes appear or go away.
    pub fn sync(&self, app: &mut SheetApp) {
        let saved = std::mem::take(&mut *self.saved.borrow_mut());
        if let Some(name) = saved.iter().rev().find(|name| is_workbook(name))
            && let Some(doc) = app.session.active_mut()
        {
            doc.saved = doc.wb.clone();
            if doc.path.is_none() {
                doc.title = name.clone();
            }
        }
        let dirty = app.session.documents().iter().any(|d| d.is_dirty());
        if self.reported.get() != Some(dirty) && self.send(&[("type", protocol::DIRTY.into()), ("dirty", dirty.into())]) {
            self.reported.set(Some(dirty));
        }
    }

    fn post_save(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        let buffer = js_sys::Uint8Array::from(bytes).buffer();
        let message = message(&[("type", protocol::SAVE.into()), ("name", name.as_str().into()), ("bytes", buffer.clone().into())])
            .ok_or_else(|| "couldn't build the message".to_string())?;
        self.parent
            .post_message_with_transfer(&message, &self.origin, &js_sys::Array::of1(&buffer))
            .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
        self.saved.borrow_mut().push(name);
        Ok(())
    }

    fn listen(&self, inbox: Inbox, ctx: egui::Context) {
        let parent = self.parent.clone();
        let origin = self.origin.clone();
        let commands = self.commands.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let from_parent = event.source().is_some_and(|s| js_sys::Object::is(&s, &parent));
            if !protocol::accept(&event.origin(), &origin, from_parent) {
                return;
            }
            let data = event.data();
            // Only an open carries a file: don't copy bytes out of anything else.
            let file = if string(&data, "type").as_deref() == Some(protocol::OPEN) { bytes(&data) } else { None };
            match protocol::parse(&fields(&data), file) {
                Some(Inbound::Open { name, bytes }) => inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes)),
                Some(command) => commands.borrow_mut().push(command),
                None => return,
            }
            ctx.request_repaint();
        });
        if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
            // The listener lives as long as the page.
            on_message.forget();
        }
    }

    /// Post a JSON message to the parent (as a plain object).
    fn post_json(&self, message: &Json) -> Result<(), String> {
        let text = serde_json::to_string(message).map_err(|e| e.to_string())?;
        let value = js_sys::JSON::parse(&text).map_err(|e| format!("{e:?}"))?;
        self.parent.post_message(&value, &self.origin).map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))
    }

    /// Post a message to the parent; false if it couldn't be sent.
    fn send(&self, fields: &[(&str, JsValue)]) -> bool {
        message(fields).is_some_and(|m| self.parent.post_message(&m, &self.origin).is_ok())
    }
}

fn is_workbook(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| WORKBOOK_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

fn message(fields: &[(&str, JsValue)]) -> Option<js_sys::Object> {
    let m = js_sys::Object::new();
    for (key, value) in fields {
        js_sys::Reflect::set(&m, &JsValue::from_str(key), value).ok()?;
    }
    Some(m)
}

fn string(data: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok()?.as_string()
}

/// The message's [`protocol::FIELDS`] that are present, as JSON.
fn fields(data: &JsValue) -> Json {
    let mut out = serde_json::Map::new();
    for key in protocol::FIELDS {
        let Ok(value) = js_sys::Reflect::get(data, &JsValue::from_str(key)) else { continue };
        if value.is_undefined() {
            continue;
        }
        let text = js_sys::JSON::stringify(&value).ok().and_then(|t| t.as_string());
        if let Some(value) = text.and_then(|t| serde_json::from_str(&t).ok()) {
            out.insert((*key).to_string(), value);
        }
    }
    Json::Object(out)
}

/// `bytes` as an `ArrayBuffer` or `Uint8Array`.
fn bytes(data: &JsValue) -> Option<Vec<u8>> {
    let bytes = js_sys::Reflect::get(data, &JsValue::from_str("bytes")).ok()?;
    if let Some(array) = bytes.dyn_ref::<js_sys::Uint8Array>() {
        Some(array.to_vec())
    } else if bytes.is_instance_of::<js_sys::ArrayBuffer>() {
        Some(js_sys::Uint8Array::new(&bytes).to_vec())
    } else {
        None
    }
}
