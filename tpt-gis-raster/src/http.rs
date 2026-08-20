//! Async, HTTP Range-request streaming reader for Cloud Optimized GeoTIFFs.
//!
//! A COG is read without downloading the whole file: the header and IFD are
//! fetched first (a small prefix), then only the tiles overlapping the requested
//! region are fetched on demand. The crate's synchronous [`geotiff`] decoder is
//! driven through a `fetch` closure built from the bytes gathered so far.
//!
//! This module is only available with the `http` feature, which pulls in
//! `reqwest` (rustls, no OpenSSL FFI).
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "http")]
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use tpt_gis_raster::http::CogRangeReader;
//!
//! let reader = CogRangeReader::new("https://example.com/coverage.tif");
//! // Only the tiles covering the top-left 1024x1024 window are downloaded.
//! let band = reader.read_region(0, 0, 1024, 1024).await?;
//! println!("fetched a {}x{} window", band.width(), band.height());
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::band::AnyBand;
use crate::geotiff::{GeoTiff, GeoTiffError, ImageLayout};

/// How many leading bytes of a COG are fetched up front. This covers the header,
/// the IFD, and (in a well-formed COG) the tile offset/byte-count arrays, so the
/// layout can be parsed without a second round trip.
const PREFIX: usize = 1 << 20; // 1 MiB

/// Per-request deadlines for the HTTP transport. A Range-unaware or slow-loris
/// origin must not be able to hang a streaming reader indefinitely.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Errors from the HTTP COG reader.
#[derive(Debug)]
pub enum CogHttpError {
    /// A parse or decode error from the underlying TIFF reader.
    Tiff(GeoTiffError),
    /// An HTTP / transport error.
    Http(String),
}

impl std::fmt::Display for CogHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CogHttpError::Tiff(e) => write!(f, "tiff error: {e}"),
            CogHttpError::Http(e) => write!(f, "http error: {e}"),
        }
    }
}

impl std::error::Error for CogHttpError {}

impl From<GeoTiffError> for CogHttpError {
    fn from(e: GeoTiffError) -> Self {
        CogHttpError::Tiff(e)
    }
}

/// The future returned by [`RangeTransport::fetch_range`].
#[allow(clippy::type_complexity)]
pub type RangeFetch = Pin<Box<dyn Future<Output = Result<Vec<u8>, CogHttpError>> + Send>>;

/// The future returned by [`RangeTransport::content_length`].
#[allow(clippy::type_complexity)]
pub type RangeSize = Pin<Box<dyn Future<Output = Option<u64>> + Send>>;

/// A byte-range source for a (remote) COG: the single seam a
/// [`CogRangeReader`] plugs a transport into. The default [`CogRangeReader::new`]
/// uses [`reqwest`] over HTTP Range requests, but any source that can satisfy
/// "give me `[start, start + len)`" implements this — including an in-memory
/// buffer (see [`MemoryTransport`]), a local file, or an object store.
pub trait RangeTransport: Send + Sync {
    /// Fetches `[start, start + len)`.
    fn fetch_range(&self, start: u64, len: usize) -> RangeFetch;
    /// The total size of the source, if known (used to size the prefix fetch and to
    /// bound the decoder's view of the file).
    fn content_length(&self) -> RangeSize;
}

/// A [`RangeTransport`] backed by `reqwest`, issuing HTTP `Range` requests.
struct HttpTransport {
    client: reqwest::Client,
    url: String,
}

/// Validates an HTTP Range response before its body is buffered.
///
/// A COG reader must stream only the requested bytes, so a Range-unaware origin
/// that answers `200 OK` with the *entire* file would silently defeat that
/// promise (the reader would download and then truncate the whole file). We
/// therefore require the `206 Partial Content` status exactly. As defense-in-depth
/// we also reject a `206` whose declared `Content-Length` is larger than the range
/// we asked for.
fn validate_range_response(
    status: reqwest::StatusCode,
    content_length: Option<u64>,
    len: usize,
) -> Result<(), CogHttpError> {
    if status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(CogHttpError::Http(format!(
            "expected 206 Partial Content for range request, got {status}"
        )));
    }
    if let Some(cl) = content_length {
        if cl > len as u64 {
            return Err(CogHttpError::Http(format!(
                "range response body {cl} bytes exceeds requested {len}"
            )));
        }
    }
    Ok(())
}

impl RangeTransport for HttpTransport {
    fn fetch_range(&self, start: u64, len: usize) -> RangeFetch {
        let client = self.client.clone();
        let url = self.url.clone();
        Box::pin(async move {
            // A zero-length range would make the `end` computation below underflow
            // (`start + len - 1`), so reject it up front.
            if len == 0 {
                return Err(CogHttpError::Http("requested zero-length range".to_string()));
            }
            let end = start + len as u64 - 1;
            let range = format!("bytes={start}-{end}");
            let response = client
                .get(&url)
                .header(reqwest::header::RANGE, range)
                .send()
                .await
                .map_err(|e| CogHttpError::Http(e.to_string()))?;
            validate_range_response(response.status(), response.content_length(), len)?;
            let bytes = response.bytes().await.map_err(|e| CogHttpError::Http(e.to_string()))?;
            // A misbehaving server must not make us buffer more bytes than we asked
            // for. Cap the body to the requested length.
            let truncated = bytes.iter().copied().take(len).collect::<Vec<u8>>();
            Ok(truncated)
        })
    }

    fn content_length(&self) -> RangeSize {
        let client = self.client.clone();
        let url = self.url.clone();
        Box::pin(
            async move { client.head(&url).send().await.ok().and_then(|r| r.content_length()) },
        )
    }
}

/// A [`RangeTransport`] backed by an in-memory buffer. Used for tests and as a
/// streaming benchmark harness, and handy for reading a COG that is already held
/// in memory while still exercising the tile-by-tile fetch path.
///
/// With [`MemoryTransport::with_log`], every fetched range is recorded so a test
/// can assert that only the needed bytes were read.
pub struct MemoryTransport {
    data: Arc<Vec<u8>>,
    #[allow(clippy::type_complexity)]
    log: Option<Arc<Mutex<Vec<(u64, usize)>>>>,
}

impl MemoryTransport {
    /// Builds a transport over an owned buffer.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data: Arc::new(data), log: None }
    }

    /// Builds a transport that records every fetched `(offset, len)` into the
    /// returned handle.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn with_log(data: Vec<u8>) -> (Self, Arc<Mutex<Vec<(u64, usize)>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        (Self { data: Arc::new(data), log: Some(Arc::clone(&log)) }, log)
    }
}

impl RangeTransport for MemoryTransport {
    fn fetch_range(&self, start: u64, len: usize) -> RangeFetch {
        let data = Arc::clone(&self.data);
        let log = self.log.as_ref().map(Arc::clone);
        Box::pin(async move {
            let file_len = data.len() as u64;
            if start >= file_len {
                return Err(CogHttpError::Tiff(GeoTiffError::Truncated {
                    offset: start,
                    len,
                    file_len,
                }));
            }
            let end = (start + len as u64).min(file_len) as usize;
            let bytes = data[start as usize..end].to_vec();
            if let Some(log) = log {
                log.lock().unwrap().push((start, bytes.len()));
            }
            Ok(bytes)
        })
    }

    fn content_length(&self) -> RangeSize {
        let len = self.data.len() as u64;
        Box::pin(async move { Some(len) })
    }
}

/// A cache of byte ranges already fetched from the remote file, keyed by the
/// start offset. A stored entry covers `[start, start + bytes.len())`.
#[derive(Default, Clone)]
struct RangeCache {
    ranges: HashMap<u64, Vec<u8>>,
}

impl RangeCache {
    /// Records a fetched range.
    fn insert(&mut self, start: u64, bytes: Vec<u8>) {
        self.ranges.insert(start, bytes);
    }

    /// Returns the bytes for `[offset, offset + len)` if they are fully covered by
    /// a stored range.
    fn get(&self, offset: u64, len: usize) -> Option<Vec<u8>> {
        for (start, bytes) in &self.ranges {
            let end = start + bytes.len() as u64;
            if offset >= *start && offset + len as u64 <= end {
                let lo = (offset - start) as usize;
                return Some(bytes[lo..lo + len].to_vec());
            }
        }
        None
    }

    /// Whether the leading `len` bytes are already cached.
    fn has_prefix(&self, len: usize) -> bool {
        self.get(0, len).is_some()
    }
}

/// A streaming reader for a remote Cloud Optimized GeoTIFF.
///
/// Construct with [`CogRangeReader::new`] (HTTP) or
/// [`CogRangeReader::with_transport`] (any [`RangeTransport`]), then read a region
/// (or the whole image) with [`CogRangeReader::read_region`]. Each call downloads
/// only the bytes it needs.
pub struct CogRangeReader {
    transport: Box<dyn RangeTransport>,
    cache: Arc<Mutex<RangeCache>>,
}

impl CogRangeReader {
    /// Creates a reader that fetches the COG at `url` over HTTP Range requests.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                // A builder with only timeouts cannot fail to configure; fall back
                // to a default client rather than panicking on the off chance.
                let _ = e;
                reqwest::Client::new()
            });
        Self::with_transport(HttpTransport { client, url: url.into() })
    }

    /// Creates a reader over an arbitrary [`RangeTransport`] (in-memory buffer,
    /// object store, …). This is the seam that makes the reader testable and lets
    /// it run against any byte source.
    #[must_use]
    pub fn with_transport(transport: impl RangeTransport + 'static) -> Self {
        Self { transport: Box::new(transport), cache: Arc::new(Mutex::new(RangeCache::default())) }
    }

    /// Ensures the leading `PREFIX` bytes (or the whole file if smaller) are
    /// cached.
    async fn ensure_prefix(&self) -> Result<(), CogHttpError> {
        let size =
            self.transport.content_length().await.unwrap_or(PREFIX as u64).min(PREFIX as u64);
        let size = size.max(16) as usize;
        let need = !self.cache.lock().unwrap().has_prefix(size);
        if need {
            let data = self.transport.fetch_range(0, size).await?;
            self.cache.lock().unwrap().insert(0, data);
        }
        Ok(())
    }

    /// Parses the remote COG's layout (size, tiling) after fetching the prefix.
    ///
    /// # Errors
    /// Returns a [`CogHttpError`] if the prefix cannot be fetched or parsed.
    pub async fn layout(&self) -> Result<ImageLayout, CogHttpError> {
        self.ensure_prefix().await?;
        let prefix = self.cache.lock().unwrap().get(0, PREFIX).unwrap_or_default();
        let tiff = GeoTiff::parse(&prefix)?;
        Ok(tiff.layout(0)?)
    }

    /// Reads the entire image, fetching every tile. Prefer
    /// [`CogRangeReader::read_region`] when only part of the image is needed.
    ///
    /// # Errors
    /// Returns a [`CogHttpError`] on transport or decode failure.
    pub async fn read(&self) -> Result<AnyBand, CogHttpError> {
        let layout = self.layout().await?;
        self.read_region(0, 0, layout.width, layout.height).await
    }

    /// Reads only the sub-window `[x0, x1) x [y0, y1)` (cell coordinates),
    /// fetching just the tiles it overlaps.
    ///
    /// # Errors
    /// Returns a [`CogHttpError`] on transport or decode failure.
    pub async fn read_region(
        &self,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) -> Result<AnyBand, CogHttpError> {
        self.ensure_prefix().await?;
        let prefix = self.cache.lock().unwrap().get(0, PREFIX).unwrap_or_default();
        let tiff = GeoTiff::parse(&prefix)?;
        let layout = tiff.layout(0)?;
        if !layout.tiled {
            return Err(CogHttpError::Tiff(GeoTiffError::Unsupported(
                "image is not tiled; cannot stream by tile",
            )));
        }

        let tw = layout.tile_width.max(1);
        let th = layout.tile_height.max(1);
        let (tx0, ty0, tx1, ty1) = tile_window(x0, y0, x1, y1, tw, th, layout.width, layout.height);

        // Fetch exactly the tiles in the window.
        let chunks = tiff.chunk_table(0)?;
        let tiles_x = layout.width.div_ceil(tw);
        let mut missing: Vec<(u64, u64)> = Vec::new();
        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                let index = ty * tiles_x + tx;
                if let Some(&(offset, count)) = chunks.get(index) {
                    let already = self.cache.lock().unwrap().get(offset, count as usize).is_some();
                    if !already {
                        missing.push((offset, count));
                    }
                }
            }
        }
        for (offset, count) in missing {
            let data = self.transport.fetch_range(offset, count as usize).await?;
            self.cache.lock().unwrap().insert(offset, data);
        }

        // Build a synchronous fetch closure over the cache and run the decoder.
        let cache = Arc::clone(&self.cache);
        let fetch: Box<dyn Fn(u64, usize) -> Result<Vec<u8>, GeoTiffError> + Send + Sync> =
            Box::new(move |offset, len| {
                cache.lock().unwrap().get(offset, len).ok_or(GeoTiffError::NotFetched(offset))
            });
        let total = self.transport.content_length().await.unwrap_or(u64::MAX);
        let tiff = GeoTiff::from_fetch(fetch, total)?;
        Ok(tiff.read_region(0, x0, y0, x1, y1)?)
    }
}

/// Computes the inclusive tile-index window covering `[x0, x1) x [y0, y1)`.
#[allow(clippy::too_many_arguments)]
fn tile_window(
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    tw: usize,
    th: usize,
    width: usize,
    height: usize,
) -> (usize, usize, usize, usize) {
    let x0 = x0.min(x1).min(width);
    let y0 = y0.min(y1).min(height);
    let x1 = x1.max(x0).min(width);
    let y1 = y1.max(y0).min(height);
    (x0 / tw, y0 / th, x1.div_ceil(tw), y1.div_ceil(th))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geotiff::testsupport::make_cog;
    use reqwest::StatusCode;
    #[test]
    fn streams_only_the_tiles_a_region_needs() {
        // 2048x2048, 256x256 tiles -> 64 tiles, ~4 MiB. The 1 MiB prefix covers the
        // header and the first ~16 tiles; the requested region is the bottom-right
        // tile, far beyond the prefix.
        let width = 2048u16;
        let height = 2048u16;
        let bytes =
            make_cog(width, height, 256, 256, |c, r| ((c as u32 * 7 + r as u32 * 13) % 251) as u8);
        let (transport, log) = MemoryTransport::with_log(bytes.clone());
        let reader = CogRangeReader::with_transport(transport);

        let region = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(reader.read_region(1536, 1536, 1792, 1792))
            .unwrap();
        assert_eq!((region.width(), region.height()), (256, 256));
        let band = region.as_u8().unwrap();
        for (dx, dy) in [(0usize, 0usize), (10, 20), (255, 255)] {
            let col = 1536 + dx;
            let row = 1536 + dy;
            assert_eq!(band.get(dx, dy), Some(((col as u32 * 7 + row as u32 * 13) % 251) as u8));
        }

        // Streaming proof: only the prefix plus the single needed tile were fetched,
        // and the total is a small fraction of the whole file.
        let log = log.lock().unwrap();
        let total: usize = log.iter().map(|(_, len)| *len).sum();
        assert!(
            log.len() <= 2,
            "expected at most a prefix + 1 tile fetch, got {} requests",
            log.len()
        );
        assert!(
            total < bytes.len(),
            "fetched {total} of {} bytes — that is not a streaming read",
            bytes.len()
        );
    }

    #[test]
    fn range_response_must_be_206_not_200() {
        // A Range-unaware origin that answers `200 OK` with the whole file must be
        // rejected: silently downloading and truncating the whole file defeats the
        // module's "read without downloading the whole file" promise.
        assert!(validate_range_response(StatusCode::OK, Some(9999), 512).is_err());
        assert!(
            validate_range_response(StatusCode::from_u16(200).unwrap(), Some(512), 512).is_err()
        );
    }

    #[test]
    fn range_response_206_within_requested_length_is_ok() {
        assert!(validate_range_response(StatusCode::PARTIAL_CONTENT, Some(512), 512).is_ok());
        assert!(validate_range_response(StatusCode::PARTIAL_CONTENT, Some(100), 512).is_ok());
        // Missing Content-Length header is tolerated (defense-in-depth only).
        assert!(validate_range_response(StatusCode::PARTIAL_CONTENT, None, 512).is_ok());
    }

    #[test]
    fn range_response_206_exceeding_requested_length_is_rejected() {
        assert!(validate_range_response(StatusCode::PARTIAL_CONTENT, Some(513), 512).is_err());
    }
}
