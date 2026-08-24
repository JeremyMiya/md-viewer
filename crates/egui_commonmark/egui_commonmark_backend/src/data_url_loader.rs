use egui::load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError};
use egui::mutex::Mutex;

use std::collections::HashMap;
use std::sync::Arc;
use std::task::Poll;

pub fn install_loader(ctx: &egui::Context) {
    if !ctx.is_loader_installed(DataUrlLoader::ID) {
        ctx.add_bytes_loader(std::sync::Arc::new(DataUrlLoader::default()));
    }
}

#[derive(Clone)]
struct Data {
    bytes: Arc<[u8]>,
    mime: Option<String>,
}

type Entry = Poll<Result<Data, String>>;

const MAX_DATA_URL_ENCODED_BYTES: usize = 16 * 1024 * 1024;

fn data_url_too_large(uri: &str) -> bool {
    uri.len() > MAX_DATA_URL_ENCODED_BYTES
}

/// Match the scheme normalization used by `data_url::DataUrl::process`
/// without scanning the potentially large header or body.
fn has_data_url_scheme(uri: &str) -> bool {
    let mut bytes = uri
        .trim_start_matches(|ch| ch <= ' ')
        .bytes()
        .filter(|byte| !matches!(byte, b'\t' | b'\n' | b'\r'));
    b"data:".iter().all(|expected| {
        bytes
            .next()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
    })
}

#[derive(Default)]
pub struct DataUrlLoader {
    cache: Arc<Mutex<HashMap<String, Entry>>>,
}

impl DataUrlLoader {
    pub const ID: &'static str = egui::generate_loader_id!(DataUrlLoader);
}

impl BytesLoader for DataUrlLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if !has_data_url_scheme(uri) {
            return Err(LoadError::NotSupported);
        }
        if data_url_too_large(uri) {
            return Err(LoadError::Loading(format!(
                "data URL exceeds the {} MiB encoded-size limit",
                MAX_DATA_URL_ENCODED_BYTES / (1024 * 1024)
            )));
        }
        if data_url::DataUrl::process(uri).is_err() {
            return Err(LoadError::NotSupported);
        };

        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get(uri).cloned() {
            match entry {
                Poll::Ready(Ok(file)) => Ok(BytesPoll::Ready {
                    size: None,
                    bytes: Bytes::Shared(file.bytes),
                    mime: file.mime,
                }),
                Poll::Ready(Err(err)) => Err(LoadError::Loading(err)),
                Poll::Pending => Ok(BytesPoll::Pending { size: None }),
            }
        } else {
            cache.insert(uri.to_owned(), Poll::Pending);
            drop(cache);

            let cache = self.cache.clone();
            let uri = uri.to_owned();
            let ctx = ctx.clone();

            std::thread::Builder::new()
                .name("DataUrlLoader".to_owned())
                .spawn(move || {
                    // Must unfortuntely do the process step again
                    let url = data_url::DataUrl::process(&uri);
                    match url {
                        Ok(url) => {
                            let result = url
                                .decode_to_vec()
                                .map(|(decoded, _)| {
                                    let mime = url.mime_type().to_string();
                                    let mime = if mime.is_empty() { None } else { Some(mime) };

                                    Data {
                                        bytes: decoded.into(),
                                        mime,
                                    }
                                })
                                .map_err(|e| e.to_string());
                            cache.lock().insert(uri, Poll::Ready(result));
                        }
                        Err(e) => {
                            cache.lock().insert(uri, Poll::Ready(Err(e.to_string())));
                        }
                    }

                    ctx.request_repaint();
                })
                .expect("could not spawn thread");

            Ok(BytesPoll::Pending { size: None })
        }
    }

    fn forget(&self, uri: &str) {
        let _ = self.cache.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|entry| match entry {
                Poll::Ready(Ok(file)) => {
                    file.bytes.len() + file.mime.as_ref().map_or(0, |m| m.len())
                }
                Poll::Ready(Err(err)) => err.len(),
                _ => 0,
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_size_limit_is_strict() {
        assert!(!data_url_too_large(&"x".repeat(MAX_DATA_URL_ENCODED_BYTES)));
        assert!(data_url_too_large(
            &"x".repeat(MAX_DATA_URL_ENCODED_BYTES + 1)
        ));
    }

    #[test]
    fn scheme_check_matches_data_url_normalization() {
        assert!(has_data_url_scheme("data:,hello"));
        assert!(has_data_url_scheme("  DATA:,hello"));
        assert!(has_data_url_scheme("\ndata\t:\n,hello"));
        assert!(!has_data_url_scheme("https://example.com"));
        assert!(!has_data_url_scheme("database:value"));
    }
}
