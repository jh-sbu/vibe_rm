//! Ruffle's network side, serving the menus' loads (imports of font
//! libraries and shared parts) from the Interface files.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use ruffle_core::backend::navigator::{
    ErrorResponse, NavigationMethod, NavigatorBackend, NullSpawner, OwnedFuture, Request,
    SuccessResponse,
};
use ruffle_core::indexmap::IndexMap;
use ruffle_core::loader::Error;
use ruffle_core::socket::{ConnectionState, SocketAction, SocketHandle};
use url::{ParseError, Url};

use crate::files::InterfaceFiles;

/// Where the menus are, as URLs: imports resolve against it.
pub const BASE_URL: &str = "file:///interface/";

pub struct Navigator {
    files: Arc<InterfaceFiles>,
    spawner: NullSpawner,
}

impl Navigator {
    pub fn new(files: Arc<InterfaceFiles>, spawner: NullSpawner) -> Self {
        Navigator { files, spawner }
    }
}

struct Response {
    url: String,
    body: Arc<[u8]>,
    sent: bool,
}

impl SuccessResponse for Response {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.url)
    }

    fn set_url(&mut self, url: String) {
        self.url = url;
    }

    fn body(self: Box<Self>) -> OwnedFuture<Vec<u8>, Error> {
        Box::pin(async move { Ok(self.body.to_vec()) })
    }

    fn text_encoding(&self) -> Option<&'static swf::Encoding> {
        None
    }

    fn status(&self) -> u16 {
        0
    }

    fn redirected(&self) -> bool {
        false
    }

    fn next_chunk(&mut self) -> OwnedFuture<Option<Vec<u8>>, Error> {
        let chunk = (!std::mem::replace(&mut self.sent, true)).then(|| self.body.to_vec());
        Box::pin(async move { Ok(chunk) })
    }

    fn expected_length(&self) -> Result<Option<u64>, Error> {
        Ok(Some(self.body.len() as u64))
    }
}

impl NavigatorBackend for Navigator {
    fn navigate_to_url(
        &self,
        url: &str,
        _target: &str,
        _vars_method: Option<(NavigationMethod, IndexMap<String, String>)>,
    ) {
        log::debug!("menu navigates to {url} (ignored)");
    }

    fn fetch(&self, request: Request) -> OwnedFuture<Box<dyn SuccessResponse>, ErrorResponse> {
        let url = request.url().to_string();
        let found = self
            .resolve_url(&url)
            .ok()
            .and_then(|u| percent_decode(u.path()))
            .and_then(|path| self.files.get(&path));
        Box::pin(async move {
            match found {
                Some(body) => Ok(Box::new(Response {
                    url,
                    body,
                    sent: false,
                }) as Box<dyn SuccessResponse>),
                None => Err(ErrorResponse {
                    error: Error::FetchError(format!("no interface file {url}")),
                    url,
                }),
            }
        })
    }

    fn resolve_url(&self, url: &str) -> Result<Url, ParseError> {
        Url::parse(BASE_URL)?.join(&url.replace('\\', "/"))
    }

    fn spawn_future(&mut self, future: OwnedFuture<(), Error>) {
        self.spawner.spawn_local(future);
    }

    fn pre_process_url(&self, url: Url) -> Url {
        url
    }

    fn connect_socket(
        &mut self,
        _host: String,
        _port: u16,
        _timeout: Duration,
        handle: SocketHandle,
        _receiver: Receiver<Vec<u8>>,
        sender: Sender<SocketAction>,
    ) {
        let _ = sender.try_send(SocketAction::Connect(handle, ConnectionState::Failed));
    }
}

/// A URL path with its %XX escapes decoded (`Inventory%20components/...`).
fn percent_decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
