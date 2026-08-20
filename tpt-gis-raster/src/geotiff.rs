//! A from-scratch (Geo)TIFF reader — classic TIFF, BigTIFF, strips and tiles,
//! uncompressed / PackBits / DEFLATE / LZW, with the horizontal and floating-point
//! predictors — and a GeoTIFF georeferencing layer that turns the geo-keys into a
//! [`GeoTransform`](crate::GeoTransform).
//!
//! The reader is *synchronous* and reads every byte through a `fetch` closure
//! (`offset`, `len` -> bytes). That single seam lets the same decoder serve a
//! file loaded into memory ([`read`]) and a remote COG read tile-by-tile over HTTP
//! range requests ([`crate::http`] builds a `fetch` closure from the bytes it has
//! fetched so far). The output is a single [`AnyBand`](crate::AnyBand) — the
//! dynamically-typed band a file reader must return, since the sample type lives
//! in the file header rather than in the caller's types.
//!
//! # What is and isn't supported
//!
//! * **Single-band** images only (`SamplesPerPixel == 1`). Multi-sample (RGB,
//!   CMYK, …) rasters are rejected: this crate models one cell type per band, and
//!   a colour image is several bands, not one.
//! * **Planar configuration 1** (contiguous) only.
//! * Bits-per-sample of 8/16/32/64 with a `SampleFormat` of unsigned (1), signed
//!   (2), or floating point (3). The 1/4-bit packed pixel formats are not worth
//!   the complexity for a GIS raster.
//! * Floating-point predictor (3) and horizontal predictor (2).
//! * Rotated (non-north-up) transforms are not modelled: the georeferencing layer
//!   reads the axis-aligned coefficients and ignores any rotation, exactly as
//!   [`GeoTransform`](crate::GeoTransform) does.
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use tpt_gis_raster::geotiff::read;
//! // `bytes` is the contents of a .tif / .tiff file, however obtained.
//! # let bytes = tpt_gis_raster::geotiff::testsupport::make_uncompressed_tiff();
//! let band = read(&bytes).expect("a readable band");
//! assert_eq!(band.width(), 2);
//! assert_eq!(band.height(), 2);
//! # }
//! ```

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::band::{AnyBand, Band, GeoTransform};
use crate::cell::CellType;
use crate::error::RasterError;
use crate::grid::Grid;

/// An error produced while parsing or decoding a (Geo)TIFF.
#[derive(Debug, Clone, PartialEq)]
pub enum GeoTiffError {
    /// The first two bytes were neither `II` (little-endian) nor `MM`
    /// (big-endian).
    BadByteOrder,
    /// The magic number was neither 42 (classic TIFF) nor 43 (BigTIFF).
    BadMagic(u16),
    /// A tag referenced bytes past the end of the available data.
    Truncated {
        /// Byte offset that was requested.
        offset: u64,
        /// Number of bytes that were wanted.
        len: usize,
        /// Total size in bytes of the source.
        file_len: u64,
    },
    /// A tag's type code did not correspond to a known TIFF field type.
    UnknownType(u16),
    /// The image declared a sample size, layout, or format this reader rejects.
    Unsupported(&'static str),
    /// More than one sample per pixel was declared; only single-band rasters are
    /// supported.
    MultiSample(usize),
    /// A strip/tile was larger (before decompression) than its declared byte
    /// count, or vice versa.
    SizeMismatch(&'static str),
    /// A decompression backend failed.
    Decompress(&'static str),
    /// The georeferencing tags disagreed (e.g. a tiepoint without a pixel scale).
    BadGeoreferencing(&'static str),
    /// A byte range that should already have been fetched was not present (used by
    /// the HTTP reader when a tile it expected to need was not requested).
    NotFetched(u64),
}

impl fmt::Display for GeoTiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoTiffError::BadByteOrder => write!(f, "TIFF byte order must be 'II' or 'MM'"),
            GeoTiffError::BadMagic(value) => write!(f, "unknown TIFF magic number {value}"),
            GeoTiffError::Truncated { offset, len, file_len } => {
                write!(
                    f,
                    "TIFF references {len} bytes at offset {offset}, past source end {file_len}"
                )
            }
            GeoTiffError::UnknownType(value) => write!(f, "unknown TIFF field type code {value}"),
            GeoTiffError::Unsupported(what) => write!(f, "unsupported TIFF feature: {what}"),
            GeoTiffError::MultiSample(count) => {
                write!(f, "multi-sample ({count}) rasters are not supported")
            }
            GeoTiffError::SizeMismatch(what) => write!(f, "TIFF size mismatch: {what}"),
            GeoTiffError::Decompress(what) => write!(f, "TIFF decompression failed: {what}"),
            GeoTiffError::BadGeoreferencing(what) => {
                write!(f, "bad GeoTIFF georeferencing: {what}")
            }
            GeoTiffError::NotFetched(offset) => {
                write!(f, "TIFF byte range at offset {offset} was not fetched")
            }
        }
    }
}

impl core::error::Error for GeoTiffError {}

/// A `Result` using [`GeoTiffError`].
pub type Result<T> = core::result::Result<T, GeoTiffError>;

/// TIFF field type codes and their byte widths.
const TYPE_BYTE: u16 = 1;
const TYPE_ASCII: u16 = 2;
const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
const TYPE_RATIONAL: u16 = 5;
const TYPE_SBYTE: u16 = 6;
const TYPE_UNDEFINED: u16 = 7;
const TYPE_SSHORT: u16 = 8;
const TYPE_SLONG: u16 = 9;
const TYPE_SRATIONAL: u16 = 10;
const TYPE_FLOAT: u16 = 11;
const TYPE_DOUBLE: u16 = 12;
const TYPE_LONG8: u16 = 13;
const TYPE_SLONG8: u16 = 14;
const TYPE_IFD8: u16 = 15;

/// Byte width of a TIFF field type, or `None` if the code is unknown.
const fn type_size(code: u16) -> Option<usize> {
    match code {
        TYPE_BYTE | TYPE_ASCII | TYPE_SBYTE | TYPE_UNDEFINED => Some(1),
        TYPE_SHORT | TYPE_SSHORT => Some(2),
        TYPE_LONG | TYPE_SLONG | TYPE_FLOAT | TYPE_IFD8 => Some(4),
        TYPE_RATIONAL | TYPE_SRATIONAL | TYPE_DOUBLE | TYPE_LONG8 | TYPE_SLONG8 => Some(8),
        _ => None,
    }
}

/// Compression scheme codes.
const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_LZW: u16 = 5;
const COMPRESSION_PACKBITS: u16 = 32773;
const COMPRESSION_DEFLATE: u16 = 8;
const COMPRESSION_DEFLATE_PK: u16 = 32946;

/// SampleFormat codes.
const SAMPLEFORMAT_UINT: u16 = 1;
const SAMPLEFORMAT_INT: u16 = 2;
const SAMPLEFORMAT_FLOAT: u16 = 3;

/// Layout / georeferencing relevant tag codes.
const TAG_TILE_WIDTH: u16 = 322;
const TAG_TILE_HEIGHT: u16 = 323;
const TAG_MODEL_PIXEL_SCALE: u16 = 33550;
const TAG_MODEL_TIEPOINT: u16 = 33922;
const TAG_MODEL_TRANSFORMATION: u16 = 34264;
const TAG_GEO_KEY_DIRECTORY: u16 = 34735;
const TAG_SUB_IFDS: u16 = 330;
const TAG_GDAL_NODATA: u16 = 42113;

/// GeoKey IDs of interest.
const GEOKEY_GEOGRAPHIC_TYPE: u16 = 2048;
const GEOKEY_PROJECTED_TYPE: u16 = 3072;

/// A parsed IFD entry, with its bytes read out (whether they were inlined in the
/// entry or stored at an offset elsewhere in the file).
#[derive(Debug, Clone)]
struct Entry {
    type_code: u16,
    /// The raw, dereferenced value bytes (already fetched from the source).
    value: Vec<u8>,
}

impl Entry {
    fn as_u32(&self, little: bool) -> Option<u32> {
        match self.type_code {
            TYPE_SHORT | TYPE_SSHORT => u16::from_bytes(&self.value, little).map(u32::from),
            TYPE_LONG | TYPE_SLONG => u32::from_bytes(&self.value, little),
            TYPE_LONG8 | TYPE_SLONG8 => u64::from_bytes(&self.value, little).map(|v| v as u32),
            _ => None,
        }
    }

    fn as_f64s(&self, little: bool) -> Option<Vec<f64>> {
        if self.type_code != TYPE_DOUBLE {
            return None;
        }
        let mut out = Vec::with_capacity(self.value.len() / 8);
        for chunk in self.value.chunks_exact(8) {
            out.push(f64::from_bytes(chunk, little)?);
        }
        Some(out)
    }

    fn as_u16s(&self, little: bool) -> Option<Vec<u16>> {
        match self.type_code {
            TYPE_SHORT | TYPE_SSHORT => {
                let mut out = Vec::with_capacity(self.value.len() / 2);
                for chunk in self.value.chunks_exact(2) {
                    out.push(u16::from_bytes(chunk, little)?);
                }
                Some(out)
            }
            _ => None,
        }
    }

    fn as_str(&self) -> Option<alloc::string::String> {
        if self.type_code != TYPE_ASCII && self.type_code != TYPE_UNDEFINED {
            return None;
        }
        let end = self.value.iter().position(|&b| b == 0).unwrap_or(self.value.len());
        alloc::string::String::from_utf8(self.value[..end].to_vec()).ok()
    }
}

/// A minimal ordered map keyed by TIFF tag, used for IFD entries.
#[derive(Debug, Clone, Default)]
struct EntryMap {
    items: Vec<(u16, Entry)>,
}

impl EntryMap {
    fn insert(&mut self, key: u16, value: Entry) {
        if let Some(slot) = self.items.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.items.push((key, value));
        }
    }

    fn get(&self, key: u16) -> Option<&Entry> {
        self.items.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
}

/// The concrete layout an image can be stored in.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Layout {
    Strips { rows_per_strip: u32 },
    Tiles { tile_width: u32, tile_height: u32 },
}

/// Everything the decoder needs to know about one image to turn its bytes into a
/// band.
#[derive(Debug, Clone)]
struct ImageInfo {
    width: u32,
    height: u32,
    bits_per_sample: u16,
    sample_format: u16,
    compression: u16,
    predictor: u16,
    layout: Layout,
    chunks: Vec<(u64, u64)>,
    geo_transform: Option<GeoTransform>,
    epsg: Option<u32>,
    nodata: Option<f64>,
    subifds: Vec<u64>,
    internal_tiling: bool,
    last_data_offset: u64,
}

/// The maximum number of decoded bytes a single `decode_window` call is allowed to
/// allocate. This bounds the impact of a hostile TIFF whose header declares an
/// enormous `ImageWidth`/`ImageLength`/`BitsPerSample` (otherwise `vec![0; w*h*bpp]`
/// would attempt an unbounded allocation).
const MAX_DECODE_BYTES: usize = 1 << 30; // 1 GiB

/// The maximum number of *raw* (still-compressed) bytes a single strip/tile chunk
/// is allowed to declare via its `StripByteCounts`/`TileByteCounts` IFD tag. This
/// is checked in [`GeoTiff::image_info_from`] — which both the synchronous
/// `decode_window` path and the HTTP `read_region` prefetch loop pass through —
/// before any `fetch` of the untrusted byte count is issued, closing a path where
/// a crafted COG could trigger a huge fetch (and, over HTTP, a real Range request)
/// without first being rejected.
const MAX_RAW_CHUNK_BYTES: u64 = 1 << 30; // 1 GiB

/// A parsed (Geo)TIFF. All byte access goes through `fetch`, so the same struct
/// serves an in-memory file and (via a closure backed by HTTP range requests) a
/// remote COG.
pub struct GeoTiff {
    fetch: Box<dyn Fn(u64, usize) -> Result<Vec<u8>> + Send + Sync>,
    file_len: u64,
    little: bool,
    bigtiff: bool,
    ifd_offsets: Vec<u64>,
}

impl GeoTiff {
    /// Parses the file header and walks the chain of IFDs, recording each image's
    /// offset. No pixel data is fetched yet.
    ///
    /// # Errors
    /// Returns a [`GeoTiffError`] if the header is malformed or any IFD cannot be
    /// read.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let owned = data.to_vec();
        let file_len = owned.len() as u64;
        let fetch: Box<dyn Fn(u64, usize) -> Result<Vec<u8>> + Send + Sync> =
            Box::new(move |offset, len| slice_at(&owned, offset, len));
        Self::from_fetch(fetch, file_len)
    }

    /// Builds a `GeoTiff` from a byte-fetching closure and the known source size.
    /// The closure is the single seam the HTTP reader plugs into.
    pub fn from_fetch(
        fetch: Box<dyn Fn(u64, usize) -> Result<Vec<u8>> + Send + Sync>,
        file_len: u64,
    ) -> Result<Self> {
        if file_len < 8 {
            return Err(GeoTiffError::Truncated { offset: 0, len: 8, file_len });
        }
        let header = fetch(0, 8)?;
        let little = match &header[0..2] {
            b"II" => true,
            b"MM" => false,
            _ => return Err(GeoTiffError::BadByteOrder),
        };
        let (bigtiff, first_ifd) = if u16::from_bytes(&header[2..4], little) == Some(42) {
            (false, u32::from_bytes(&header[4..8], little).unwrap() as u64)
        } else if u16::from_bytes(&header[2..4], little) == Some(43) {
            if file_len < 16 {
                return Err(GeoTiffError::Truncated { offset: 0, len: 16, file_len });
            }
            let head2 = fetch(0, 16)?;
            if u16::from_bytes(&head2[4..6], little) != Some(8) {
                return Err(GeoTiffError::Unsupported("BigTIFF with non-8-byte offsets"));
            }
            (true, u64::from_bytes(&head2[8..16], little).unwrap())
        } else {
            return Err(GeoTiffError::BadMagic(u16::from_bytes(&header[2..4], little).unwrap()));
        };

        let mut offsets = Vec::new();
        let mut current = Some(first_ifd);
        while let Some(offset) = current {
            if offset == 0 || offset >= file_len {
                break;
            }
            let parsed = Self::read_ifd(&fetch, offset, little, bigtiff)?;
            offsets.push(offset);
            current = parsed.next_ifd;
        }

        Ok(Self { fetch, file_len, little, bigtiff, ifd_offsets: offsets })
    }

    /// Number of images (IFDs) in the file's main chain.
    #[must_use]
    pub fn image_count(&self) -> usize {
        self.ifd_offsets.len()
    }

    /// The byte offset of image `index`'s IFD, for callers (e.g. the COG
    /// validator) that reason about file layout.
    #[must_use]
    pub fn ifd_offset(&self, index: usize) -> Option<u64> {
        self.ifd_offsets.get(index).copied()
    }

    /// The source size in bytes.
    #[must_use]
    pub fn file_len(&self) -> u64 {
        self.file_len
    }

    /// The `(offset, byte_count)` pairs for every strip/tile of image `index`, in
    /// file order. Used by the HTTP streaming reader to fetch only the tiles it
    /// needs.
    ///
    /// # Errors
    /// Returns a [`GeoTiffError`] if the IFD cannot be read.
    pub fn chunk_table(&self, index: usize) -> Result<Vec<(u64, u64)>> {
        Ok(self.image_info(index)?.chunks)
    }

    /// Decodes image `index` into a dynamically-typed [`AnyBand`].
    ///
    /// # Errors
    /// Returns a [`GeoTiffError`] if the image is unsupported or decoding fails.
    pub fn read_image(&self, index: usize) -> Result<AnyBand> {
        let info = self.image_info(index)?;
        self.decode_full(&info)
    }

    /// Decodes the first image. See [`GeoTiff::read_image`].
    ///
    /// # Errors
    /// As [`GeoTiff::read_image`].
    pub fn read(&self) -> Result<AnyBand> {
        self.read_image(0)
    }

    /// Decodes only the sub-window `[x0, x1) x [y0, y1)` (in cell coordinates) of
    /// image `index`, returning a band covering just that window. This is what the
    /// HTTP streaming reader uses so it never fetches tiles outside the request.
    ///
    /// # Errors
    /// Returns a [`GeoTiffError`] if the image is unsupported or decoding fails.
    pub fn read_region(
        &self,
        index: usize,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) -> Result<AnyBand> {
        let info = self.image_info(index)?;
        let width = info.width as usize;
        let height = info.height as usize;
        let (x0, y0, x1, y1) = clamp_window(x0, y0, x1, y1, width, height);
        if x1 <= x0 || y1 <= y0 {
            return Err(GeoTiffError::SizeMismatch("empty region"));
        }
        self.decode_window(&info, x0, y0, x1, y1)
    }

    /// Reports the layout (size, tiling, sub-IFDs) of image `index` without
    /// decoding any pixels. Used by the COG validator.
    ///
    /// # Errors
    /// Returns a [`GeoTiffError`] if the IFD cannot be read.
    pub fn layout(&self, index: usize) -> Result<ImageLayout> {
        let offset = self
            .ifd_offsets
            .get(index)
            .copied()
            .ok_or(GeoTiffError::Unsupported("image index out of range"))?;
        let parsed = Self::read_ifd(&self.fetch, offset, self.little, self.bigtiff)?;
        let info = self.image_info_from(&parsed.ifd, offset)?;
        Ok(self.build_layout(&info))
    }

    /// Reports the layout of the IFD at an arbitrary `offset` (e.g. an overview
    /// SubIFD, which is not on the main IFD chain).
    ///
    /// # Errors
    /// Returns a [`GeoTiffError`] if the IFD cannot be read.
    pub fn layout_at(&self, offset: u64) -> Result<ImageLayout> {
        let parsed = Self::read_ifd(&self.fetch, offset, self.little, self.bigtiff)?;
        let info = self.image_info_from(&parsed.ifd, offset)?;
        Ok(self.build_layout(&info))
    }

    /// Builds the public [`ImageLayout`] view from decoded parameters.
    fn build_layout(&self, info: &ImageInfo) -> ImageLayout {
        let (tiled, tile_width, tile_height) = match info.layout {
            Layout::Strips { .. } => (false, 0, 0),
            Layout::Tiles { tile_width, tile_height } => (true, tile_width, tile_height),
        };
        ImageLayout {
            width: info.width as usize,
            height: info.height as usize,
            tiled,
            tile_width: tile_width as usize,
            tile_height: tile_height as usize,
            subifds: info.subifds.clone(),
            epsg: info.epsg,
            has_geo_transform: info.geo_transform.is_some(),
            bits_per_sample: info.bits_per_sample,
            sample_format: info.sample_format,
            internal_tiling: info.internal_tiling,
            last_data_offset: info.last_data_offset,
        }
    }

    fn image_info(&self, index: usize) -> Result<ImageInfo> {
        let offset = self
            .ifd_offsets
            .get(index)
            .copied()
            .ok_or(GeoTiffError::Unsupported("image index out of range"))?;
        let parsed = Self::read_ifd(&self.fetch, offset, self.little, self.bigtiff)?;
        self.image_info_from(&parsed.ifd, offset)
    }

    /// Reads one IFD: its entries and the offset of the next IFD in the chain.
    fn read_ifd(
        fetch: &dyn Fn(u64, usize) -> Result<Vec<u8>>,
        offset: u64,
        little: bool,
        bigtiff: bool,
    ) -> Result<ParsedIfd> {
        let inline = if bigtiff { 8 } else { 4 };
        let entry_size = if bigtiff { 20 } else { 12 };
        let count_bytes = if bigtiff { 8 } else { 2 };
        let header = fetch(offset, count_bytes)?;
        let count = if bigtiff {
            u64::from_bytes(&header, little).unwrap()
        } else {
            u16::from_bytes(&header, little).unwrap() as u64
        };
        let mut entries = EntryMap::default();
        let mut pos = offset + count_bytes as u64;
        for _ in 0..count {
            let entry = fetch(pos, entry_size)?;
            let tag = u16::from_bytes(&entry[0..2], little).unwrap();
            let type_code = u16::from_bytes(&entry[2..4], little).unwrap();
            let value_count = if bigtiff {
                u64::from_bytes(&entry[4..12], little).unwrap()
            } else {
                u32::from_bytes(&entry[4..8], little).unwrap() as u64
            };
            let size = type_size(type_code).ok_or(GeoTiffError::UnknownType(type_code))?;
            let value_bytes = value_count
                .checked_mul(size as u64)
                .ok_or(GeoTiffError::SizeMismatch("entry value too large"))?;
            let field = &entry[entry_size - inline..entry_size];
            let bytes = if value_bytes <= inline as u64 {
                field[..value_bytes as usize].to_vec()
            } else {
                let target = if bigtiff {
                    u64::from_bytes(field, little).unwrap()
                } else {
                    u32::from_bytes(field, little).unwrap() as u64
                };
                fetch(target, value_bytes as usize)?
            };
            entries.insert(tag, Entry { type_code, value: bytes });
            pos += entry_size as u64;
        }
        let next_pos = pos;
        let next_bytes = if bigtiff { 8 } else { 4 };
        let next = fetch(next_pos, next_bytes)?;
        let next_ifd = if bigtiff {
            u64::from_bytes(&next, little)
        } else {
            u32::from_bytes(&next, little).map(|v| v as u64)
        };
        Ok(ParsedIfd { ifd: entries, next_ifd })
    }

    /// Turns a parsed IFD into full decoding parameters.
    fn image_info_from(&self, ifd: &EntryMap, _ifd_offset: u64) -> Result<ImageInfo> {
        let width = ifd
            .get(256)
            .and_then(|e| e.as_u32(self.little))
            .ok_or(GeoTiffError::Unsupported("ImageWidth missing"))?;
        let height = ifd
            .get(257)
            .and_then(|e| e.as_u32(self.little))
            .ok_or(GeoTiffError::Unsupported("ImageLength missing"))?;

        let samples_per_pixel =
            ifd.get(277).and_then(|e| e.as_u32(self.little)).unwrap_or(1) as u16;
        if samples_per_pixel != 1 {
            return Err(GeoTiffError::MultiSample(samples_per_pixel as usize));
        }

        let bits = ifd
            .get(258)
            .and_then(|e| e.as_u32(self.little))
            .ok_or(GeoTiffError::Unsupported("BitsPerSample missing"))? as u16;
        let sample_format = ifd.get(339).and_then(|e| e.as_u32(self.little)).unwrap_or(1) as u16;
        let compression = ifd.get(259).and_then(|e| e.as_u32(self.little)).unwrap_or(1) as u16;
        let predictor = ifd.get(317).and_then(|e| e.as_u32(self.little)).unwrap_or(1) as u16;
        let planar_config = ifd.get(284).and_then(|e| e.as_u32(self.little)).unwrap_or(1) as u16;
        if planar_config != 1 {
            return Err(GeoTiffError::Unsupported("planar configuration 2"));
        }

        let layout = if let (Some(tw), Some(th)) = (
            ifd.get(TAG_TILE_WIDTH).and_then(|e| e.as_u32(self.little)),
            ifd.get(TAG_TILE_HEIGHT).and_then(|e| e.as_u32(self.little)),
        ) {
            Layout::Tiles { tile_width: tw, tile_height: th }
        } else {
            let rps = ifd.get(278).and_then(|e| e.as_u32(self.little)).unwrap_or(height);
            Layout::Strips { rows_per_strip: rps }
        };

        let offsets = ifd.get(324).or_else(|| ifd.get(273));
        let counts = ifd.get(325).or_else(|| ifd.get(279));
        let (offsets, counts) = match (offsets, counts) {
            (Some(o), Some(c)) => (o, c),
            _ => return Err(GeoTiffError::Unsupported("missing offsets/byte counts")),
        };
        let off_vals = integer_list(offsets, self.little);
        let cnt_vals = integer_list(counts, self.little);
        if off_vals.len() != cnt_vals.len() {
            return Err(GeoTiffError::SizeMismatch("offset/byte-count count mismatch"));
        }
        let chunks: Vec<(u64, u64)> = off_vals.into_iter().zip(cnt_vals).collect();

        // Reject any chunk whose declared (uncompressed) byte count is absurd
        // before a fetch of that count is ever issued. This guards both the local
        // `decode_window` path and the HTTP `read_region` prefetch loop, which call
        // `fetch_range` directly with these same untrusted counts.
        for &(_, count) in &chunks {
            if count > MAX_RAW_CHUNK_BYTES {
                return Err(GeoTiffError::SizeMismatch("chunk byte count exceeds raw size limit"));
            }
        }

        let (geo_transform, epsg) = read_georeferencing(ifd, self.little)?;
        let nodata = ifd
            .get(TAG_GDAL_NODATA)
            .and_then(|e| e.as_str())
            .and_then(|s| s.trim().parse::<f64>().ok());

        let subifds =
            ifd.get(TAG_SUB_IFDS).map(|e| integer_list(e, self.little)).unwrap_or_default();

        let internal_tiling = chunks.windows(2).all(|pair| pair[0].0 < pair[1].0);
        // Use `checked_add` so a hostile `(offset, byte_count)` pair cannot wrap and
        // panic; an overflowing chunk has already been rejected by the
        // `MAX_RAW_CHUNK_BYTES` cap above, but the offset itself is attacker-controlled
        // and must not produce a bogus `last_data_offset` used by layout validation.
        let last_data_offset =
            chunks.iter().map(|(o, c)| o.checked_add(*c).unwrap_or(u64::MAX)).max().unwrap_or(0);

        Ok(ImageInfo {
            width,
            height,
            bits_per_sample: bits,
            sample_format,
            compression,
            predictor,
            layout,
            chunks,
            geo_transform,
            epsg,
            nodata,
            subifds,
            internal_tiling,
            last_data_offset,
        })
    }

    /// Decodes the complete image described by `info`.
    fn decode_full(&self, info: &ImageInfo) -> Result<AnyBand> {
        let width = info.width as usize;
        let height = info.height as usize;
        self.decode_window(info, 0, 0, width, height)
    }

    /// Decodes a window `[x0, x1) x [y0, y1)` of the image.
    fn decode_window(
        &self,
        info: &ImageInfo,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) -> Result<AnyBand> {
        let bytes_per_sample = ((info.bits_per_sample as usize).div_ceil(8)).max(1);
        if info.bits_per_sample == 0 || info.bits_per_sample > 64 {
            return Err(GeoTiffError::Unsupported("bits per sample out of range"));
        }
        let width = info.width as usize;
        let height = info.height as usize;

        let (tiles_x, tiles_y, chunk_w, chunk_h, chunk_count) = match info.layout {
            Layout::Strips { rows_per_strip } => {
                let rps = rows_per_strip.max(1) as usize;
                let n = height.div_ceil(rps);
                (1, n, width, rps.min(height), n)
            }
            Layout::Tiles { tile_width, tile_height } => {
                let tw = tile_width.max(1) as usize;
                let th = tile_height.max(1) as usize;
                let tx = width.div_ceil(tw);
                let ty = height.div_ceil(th);
                (tx, ty, tw, th, tx * ty)
            }
        };
        if chunk_count == 0 || info.chunks.len() != chunk_count {
            return Err(GeoTiffError::SizeMismatch("chunk count mismatch"));
        }

        // The decoded sub-window, row-major, in file byte order, before cell
        // interpretation.
        let out_w = x1 - x0;
        let out_h = y1 - y0;
        let total_bytes = out_w
            .checked_mul(out_h)
            .and_then(|v| v.checked_mul(bytes_per_sample))
            .ok_or(GeoTiffError::SizeMismatch("decoded window dimensions overflow"))?;
        if total_bytes > MAX_DECODE_BYTES {
            return Err(GeoTiffError::SizeMismatch("decoded window exceeds size limit"));
        }
        let mut buffer = vec![0u8; total_bytes];

        let tx0 = x0 / chunk_w;
        let ty0 = y0 / chunk_h;
        let tx1 = x1.div_ceil(chunk_w);
        let ty1 = y1.div_ceil(chunk_h);

        for ty in ty0..ty1.min(tiles_y) {
            for tx in tx0..tx1.min(tiles_x) {
                let chunk_index = ty * tiles_x + tx;
                let (offset, byte_count) = info.chunks[chunk_index];
                let raw = (self.fetch)(offset, byte_count as usize)?;
                let expected = chunk_w
                    .checked_mul(chunk_h)
                    .and_then(|v| v.checked_mul(bytes_per_sample))
                    .ok_or(GeoTiffError::SizeMismatch("tile dimensions overflow"))?;
                if expected > MAX_DECODE_BYTES {
                    return Err(GeoTiffError::SizeMismatch("tile exceeds decode size limit"));
                }
                let mut decoded = decompress(&raw, info.compression, expected)?;

                let cw = chunk_w.min(width.saturating_sub(tx * chunk_w));
                let ch = chunk_h.min(height.saturating_sub(ty * chunk_h));
                if cw == 0 || ch == 0 {
                    continue;
                }
                undo_predictor(info.predictor, &mut decoded, cw, ch, bytes_per_sample)?;

                // Which columns/rows of this chunk fall inside the window.
                let col_lo = x0.max(tx * chunk_w);
                let col_hi = x1.min((tx + 1) * chunk_w);
                let row_lo = y0.max(ty * chunk_h);
                let row_hi = y1.min((ty + 1) * chunk_h);
                // The decoded tile is a full `chunk_w x chunk_h` block, so its row
                // stride is `chunk_w` and column indices are relative to the tile's
                // own origin — not the clamped cell width `cw` or absolute columns.
                let src_col0 = col_lo - tx * chunk_w;
                let src_col1 = col_hi - tx * chunk_w;
                for row in row_lo..row_hi {
                    let src_row = row - ty * chunk_h;
                    let src_start = (src_row * chunk_w + src_col0) * bytes_per_sample;
                    let src =
                        &decoded[src_start..src_start + (src_col1 - src_col0) * bytes_per_sample];
                    let dest_row = row - y0;
                    let dest_col = col_lo - x0;
                    let dest_start = (dest_row * out_w + dest_col) * bytes_per_sample;
                    buffer[dest_start..dest_start + src.len()].copy_from_slice(src);
                }
            }
        }

        let transform =
            info.geo_transform.unwrap_or_else(|| GeoTransform::new(0.0, (y1) as f64, 1.0, 1.0));
        // Shift the origin so the window's top-left corner keeps its world
        // position from the full image.
        let transform = shift_transform(transform, x0, y0);
        let epsg = info.epsg;
        let nodata = info.nodata;
        build_band(
            buffer,
            out_w,
            out_h,
            bytes_per_sample,
            self.little,
            info.sample_format,
            info.bits_per_sample,
            transform,
            epsg,
            nodata,
        )
    }
}

/// Clamps a requested window into `[0, width) x [0, height)` and orders it.
fn clamp_window(
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    width: usize,
    height: usize,
) -> (usize, usize, usize, usize) {
    let x0 = x0.min(x1).min(width);
    let y0 = y0.min(y1).min(height);
    let x1 = x1.max(x0).min(width);
    let y1 = y1.max(y0).min(height);
    (x0, y0, x1, y1)
}

/// Offsets a transform by `dx`/`dy` cells so a sub-window keeps the georeferencing
/// of the full image.
fn shift_transform(t: GeoTransform, dx: usize, dy: usize) -> GeoTransform {
    let (wx, wy) = t.cell_to_world(dx as f64, dy as f64);
    GeoTransform::try_new(wx, wy, t.pixel_width(), t.pixel_height()).unwrap_or(t)
}

/// A parsed IFD plus the offset chain pointer.
struct ParsedIfd {
    ifd: EntryMap,
    next_ifd: Option<u64>,
}

/// The layout of one image, without its pixels.
#[derive(Debug, Clone)]
pub struct ImageLayout {
    /// Width in cells.
    pub width: usize,
    /// Height in cells.
    pub height: usize,
    /// Whether the image is tiled (required for a COG).
    pub tiled: bool,
    /// Tile width in cells (0 if stripped).
    pub tile_width: usize,
    /// Tile height in cells (0 if stripped).
    pub tile_height: usize,
    /// Offsets of any SubIFDs (overviews), in order.
    pub subifds: Vec<u64>,
    /// EPSG code, if a geo-key declared one.
    pub epsg: Option<u32>,
    /// Whether a model pixel scale / tiepoint / transformation was present.
    pub has_geo_transform: bool,
    /// Bits per cell.
    pub bits_per_sample: u16,
    /// Sample format (1 = uint, 2 = int, 3 = float).
    pub sample_format: u16,
    /// Whether tile/strip offsets are strictly increasing, so a reader can stream
    /// the image sequentially (a COG requirement).
    pub internal_tiling: bool,
    /// The highest byte offset covered by this image's pixel data (the end of the
    /// last tile/strip). Used to check the data lies inside the file.
    pub last_data_offset: u64,
}

/// Reads a `len`-byte subslice at `offset` from an in-memory buffer.
fn slice_at(data: &[u8], offset: u64, len: usize) -> Result<Vec<u8>> {
    let start = offset as usize;
    let end = start.checked_add(len).ok_or(GeoTiffError::Truncated {
        offset,
        len,
        file_len: data.len() as u64,
    })?;
    data.get(start..end).map(|s| s.to_vec()).ok_or(GeoTiffError::Truncated {
        offset,
        len,
        file_len: data.len() as u64,
    })
}

/// Reads an entry's value as a list of `u64` (offsets / byte counts).
fn integer_list(entry: &Entry, little: bool) -> Vec<u64> {
    match entry.type_code {
        TYPE_SHORT | TYPE_SSHORT => entry
            .value
            .chunks_exact(2)
            .filter_map(|c| u16::from_bytes(c, little).map(u64::from))
            .collect(),
        TYPE_LONG | TYPE_SLONG => entry
            .value
            .chunks_exact(4)
            .filter_map(|c| u32::from_bytes(c, little).map(u64::from))
            .collect(),
        TYPE_LONG8 | TYPE_SLONG8 => {
            entry.value.chunks_exact(8).filter_map(|c| u64::from_bytes(c, little)).collect()
        }
        _ => Vec::new(),
    }
}

/// Extracts the [`GeoTransform`] and EPSG code from the GeoTIFF geo-key tags.
fn read_georeferencing(
    ifd: &EntryMap,
    little: bool,
) -> Result<(Option<GeoTransform>, Option<u32>)> {
    let mut epsg = None;
    if let Some(dir) = ifd.get(TAG_GEO_KEY_DIRECTORY) {
        if let Some(shorts) = dir.as_u16s(little) {
            let mut i = 4;
            while i + 4 <= shorts.len() {
                let key = shorts[i];
                let location = shorts[i + 1];
                let value_offset = shorts[i + 3];
                if (key == GEOKEY_GEOGRAPHIC_TYPE || key == GEOKEY_PROJECTED_TYPE)
                    && location == 0
                    && value_offset != 0
                {
                    epsg = Some(value_offset as u32);
                }
                i += 4;
            }
        }
    }

    if let Some(transform) = ifd.get(TAG_MODEL_TRANSFORMATION) {
        let m = transform
            .as_f64s(little)
            .ok_or(GeoTiffError::BadGeoreferencing("ModelTransformationTag is not 16 doubles"))?;
        if m.len() < 16 {
            return Err(GeoTiffError::BadGeoreferencing("truncated ModelTransformationTag"));
        }
        let origin_x = m[3];
        let origin_y = m[7];
        let pixel_width = m[0];
        let pixel_height = -m[5];
        let gt = GeoTransform::try_new(origin_x, origin_y, pixel_width, pixel_height)
            .map_err(|_| GeoTiffError::BadGeoreferencing("non-positive pixel size in transform"))?;
        return Ok((Some(gt), epsg));
    }

    let scale = ifd.get(TAG_MODEL_PIXEL_SCALE).and_then(|e| e.as_f64s(little));
    let tiepoint = ifd.get(TAG_MODEL_TIEPOINT).and_then(|e| e.as_f64s(little));
    match (scale, tiepoint) {
        (Some(scale), Some(tie)) if scale.len() >= 2 && tie.len() >= 6 => {
            let sx = scale[0];
            let sy = scale[1];
            let (i, j, x, y) = (tie[0], tie[1], tie[3], tie[4]);
            let origin_x = x - i * sx;
            let origin_y = y + j * sy;
            let gt = GeoTransform::try_new(origin_x, origin_y, sx, sy)
                .map_err(|_| GeoTiffError::BadGeoreferencing("non-positive pixel scale"))?;
            Ok((Some(gt), epsg))
        }
        _ => Ok((None, epsg)),
    }
}

/// Builds the right [`AnyBand`] variant for the declared cell type and wraps the
/// georeferencing metadata.
#[allow(clippy::too_many_arguments)]
fn build_band(
    buffer: Vec<u8>,
    width: usize,
    height: usize,
    bytes_per_sample: usize,
    little: bool,
    sample_format: u16,
    bits: u16,
    geo: GeoTransform,
    epsg: Option<u32>,
    nodata: Option<f64>,
) -> Result<AnyBand> {
    let nodata_cell = nodata.and_then(|v| nodata_to_cell(v, sample_format, bits));

    macro_rules! finish {
        ($ty:ty, $kind:ident) => {{
            let cells = cells_of_type::<$ty>(&buffer, bytes_per_sample, little);
            let grid = Grid::new(width, height, cells).map_err(grid_err)?;
            let band = attach(Band::new(grid, geo), epsg, nodata_cell.map(|v| v as $ty));
            Ok(AnyBand::$kind(band))
        }};
    }

    match (bits, sample_format) {
        (8, _) => finish!(u8, U8),
        (16, SAMPLEFORMAT_INT) => finish!(i16, I16),
        (16, _) => finish!(u16, U16),
        (32, SAMPLEFORMAT_INT) => finish!(i32, I32),
        (32, SAMPLEFORMAT_UINT) => finish!(u32, U32),
        (32, SAMPLEFORMAT_FLOAT) => finish!(f32, F32),
        (64, SAMPLEFORMAT_FLOAT) => finish!(f64, F64),
        _ => Err(GeoTiffError::Unsupported("unsupported bits/sample-format combination")),
    }
}

fn attach<T: CellType>(band: Band<T>, epsg: Option<u32>, nodata: Option<T>) -> Band<T> {
    let mut band = band;
    if let Some(epsg) = epsg {
        band = band.with_epsg(epsg);
    }
    if let Some(nodata) = nodata {
        band = band.with_nodata(nodata);
    }
    band
}

fn grid_err(e: RasterError) -> GeoTiffError {
    match e {
        RasterError::CellCountMismatch { .. } => GeoTiffError::SizeMismatch("cell count mismatch"),
        _ => GeoTiffError::SizeMismatch("grid error"),
    }
}

/// Interprets `buffer` as cells of type `T`.
fn cells_of_type<T: CellType>(buffer: &[u8], bytes_per_sample: usize, little: bool) -> Vec<T> {
    let step = bytes_per_sample.max(1);
    let mut cells = Vec::with_capacity(buffer.len() / step);
    let mut i = 0;
    while i + step <= buffer.len() {
        let value = if little {
            T::from_le_bytes_slice(&buffer[i..i + step])
        } else {
            T::from_be_bytes_slice(&buffer[i..i + step])
        };
        if let Some(value) = value {
            cells.push(value);
        }
        i += step;
    }
    cells
}

/// Maps a parsed nodata `f64` to a value of the cell type.
fn nodata_to_cell(value: f64, sample_format: u16, bits: u16) -> Option<f64> {
    match (bits, sample_format) {
        (64, SAMPLEFORMAT_FLOAT) | (32, SAMPLEFORMAT_FLOAT) => Some(value),
        _ => Some(value),
    }
}

/// Decompresses one strip/tile's raw bytes, given the compression scheme, and
/// bounds the output to at most `expected` bytes — the size the header declares
/// this strip/tile should decode to. This is the guard against a decompression
/// bomb: a tiny hostile payload that inflates to gigabytes of output can no longer
/// exhaust memory, because the decoder is allowed to produce at most `expected`
/// bytes (plus one, to detect the over-long case and error rather than silently
/// truncate).
fn decompress(raw: &[u8], compression: u16, expected: usize) -> Result<Vec<u8>> {
    match compression {
        COMPRESSION_NONE => {
            if raw.len() != expected {
                return Err(GeoTiffError::SizeMismatch("uncompressed size mismatch"));
            }
            Ok(raw.to_vec())
        }
        COMPRESSION_PACKBITS => decode_packbits(raw, expected),
        COMPRESSION_DEFLATE | COMPRESSION_DEFLATE_PK => decode_deflate(raw, expected),
        COMPRESSION_LZW => decode_lzw(raw, expected),
        _ => Err(GeoTiffError::Unsupported("compression scheme")),
    }
}

/// Inflates a DEFLATE stream using the pure-Rust `miniz_oxide` backend of
/// `flate2` (no C library, per the project's zero-FFI rule). The decoder is wrapped
/// in a `Take` so it can read at most `expected + 1` bytes, bounding the allocated
/// output even when the stream is maliciously over-long.
fn decode_deflate(raw: &[u8], expected: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let decoder = flate2::read::DeflateDecoder::new(raw);
    let mut limited = decoder.take((expected as u64).saturating_add(1));
    let mut out = Vec::with_capacity(expected.min(1 << 20));
    limited.read_to_end(&mut out).map_err(|_| GeoTiffError::Decompress("DEFLATE"))?;
    if out.len() > expected {
        return Err(GeoTiffError::SizeMismatch("DEFLATE output exceeds declared size"));
    }
    Ok(out)
}

/// Decodes a TIFF LZW stream (early-change variant) via `weezl`. The decoded length
/// is checked against `expected` so an over-long stream cannot masquerade as valid
/// pixel data.
fn decode_lzw(raw: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut decoder = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
    let decoded = decoder.decode(raw).map_err(|_| GeoTiffError::Decompress("LZW"))?;
    if decoded.len() > expected {
        return Err(GeoTiffError::SizeMismatch("LZW output exceeds declared size"));
    }
    Ok(decoded)
}

/// Decodes a PackBits run-length-encoded strip/tile. PackBits output is inherently
/// bounded by roughly 128× the input, but the result is still checked against
/// `expected`.
fn decode_packbits(raw: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let header = raw[i] as i8;
        i += 1;
        if header >= 0 {
            let count = header as usize + 1;
            if i + count > raw.len() {
                return Err(GeoTiffError::SizeMismatch("PackBits literal run"));
            }
            out.extend_from_slice(&raw[i..i + count]);
            i += count;
        } else if header != -128 {
            let count = (1 - header as i32) as usize;
            if i >= raw.len() {
                return Err(GeoTiffError::SizeMismatch("PackBits repeat"));
            }
            let byte = raw[i];
            out.extend(core::iter::repeat(byte).take(count));
            i += 1;
        }
    }
    if out.len() > expected {
        return Err(GeoTiffError::SizeMismatch("PackBits output exceeds declared size"));
    }
    Ok(out)
}

/// Undoes the horizontal (2) or floating-point (3) predictor on a decoded
/// `chunk_w x chunk_h` block of `bytes_per_sample`-wide cells.
fn undo_predictor(
    predictor: u16,
    data: &mut [u8],
    chunk_w: usize,
    chunk_h: usize,
    bytes_per_sample: usize,
) -> Result<()> {
    if predictor == 1 {
        return Ok(());
    }
    if bytes_per_sample == 0 || data.len() < chunk_w * chunk_h * bytes_per_sample {
        return Err(GeoTiffError::SizeMismatch("predictor block too small"));
    }
    let row_bytes = chunk_w * bytes_per_sample;
    match predictor {
        2 => {
            for row in 0..chunk_h {
                let base = row * row_bytes;
                for p in 1..chunk_w {
                    let cur = base + p * bytes_per_sample;
                    let prev = base + (p - 1) * bytes_per_sample;
                    for k in 0..bytes_per_sample {
                        data[cur + k] = data[cur + k].wrapping_add(data[prev + k]);
                    }
                }
            }
            Ok(())
        }
        3 => {
            for row in 0..chunk_h {
                let base = row * row_bytes;
                for p in 1..chunk_w {
                    for k in 0..bytes_per_sample {
                        let cur = base + p * bytes_per_sample + k;
                        let (src_p, src_k) =
                            if k == 0 { (p - 1, bytes_per_sample - 1) } else { (p - 1, k - 1) };
                        let prev = base + src_p * bytes_per_sample + src_k;
                        data[cur] = data[cur].wrapping_add(data[prev]);
                    }
                }
            }
            Ok(())
        }
        _ => Err(GeoTiffError::Unsupported("predictor scheme")),
    }
}

// --- Convenience top-level API ------------------------------------------------

/// Reads the first image of a (Geo)TIFF held in memory.
///
/// # Errors
/// Returns a [`GeoTiffError`] if the file is malformed or the image is
/// unsupported.
pub fn read(data: &[u8]) -> Result<AnyBand> {
    GeoTiff::parse(data)?.read()
}

#[cfg(feature = "std")]
/// Reads the first image of a (Geo)TIFF file from disk.
///
/// # Errors
/// Returns a [`GeoTiffError`] on parse/decode failure, or `std::io::Error` if the
/// file cannot be read.
pub fn read_file(path: &std::path::Path) -> core::result::Result<AnyBand, GeoTiffOrIo> {
    let data = std::fs::read(path)?;
    Ok(GeoTiff::parse(&data)?.read()?)
}

/// The error type for [`read_file`]: either an IO error or a parse error.
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum GeoTiffOrIo {
    /// A filesystem error.
    Io(std::io::Error),
    /// A parse/decode error.
    Tiff(GeoTiffError),
}

#[cfg(feature = "std")]
impl fmt::Display for GeoTiffOrIo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoTiffOrIo::Io(e) => write!(f, "io error: {e}"),
            GeoTiffOrIo::Tiff(e) => write!(f, "{e}"),
        }
    }
}

#[cfg(feature = "std")]
impl core::error::Error for GeoTiffOrIo {}

#[cfg(feature = "std")]
impl From<std::io::Error> for GeoTiffOrIo {
    fn from(e: std::io::Error) -> Self {
        GeoTiffOrIo::Io(e)
    }
}

#[cfg(feature = "std")]
impl From<GeoTiffError> for GeoTiffOrIo {
    fn from(e: GeoTiffError) -> Self {
        GeoTiffOrIo::Tiff(e)
    }
}

mod byte_ops {
    /// Endian-aware integer/float reading helpers used throughout the parser.
    pub trait FromBytes: Sized {
        /// Reads the value from `bytes` (which must be exactly the type's width),
        /// interpreting them as little- or big-endian.
        fn from_bytes(bytes: &[u8], little: bool) -> Option<Self>;
    }

    impl FromBytes for u16 {
        fn from_bytes(bytes: &[u8], little: bool) -> Option<Self> {
            let array: [u8; 2] = bytes.get(..2)?.try_into().ok()?;
            Some(if little { u16::from_le_bytes(array) } else { u16::from_be_bytes(array) })
        }
    }
    impl FromBytes for u32 {
        fn from_bytes(bytes: &[u8], little: bool) -> Option<Self> {
            let array: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
            Some(if little { u32::from_le_bytes(array) } else { u32::from_be_bytes(array) })
        }
    }
    impl FromBytes for u64 {
        fn from_bytes(bytes: &[u8], little: bool) -> Option<Self> {
            let array: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
            Some(if little { u64::from_le_bytes(array) } else { u64::from_be_bytes(array) })
        }
    }
    impl FromBytes for f32 {
        fn from_bytes(bytes: &[u8], little: bool) -> Option<Self> {
            let array: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
            Some(if little { f32::from_le_bytes(array) } else { f32::from_be_bytes(array) })
        }
    }
    impl FromBytes for f64 {
        fn from_bytes(bytes: &[u8], little: bool) -> Option<Self> {
            let array: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
            Some(if little { f64::from_le_bytes(array) } else { f64::from_be_bytes(array) })
        }
    }
}

use byte_ops::FromBytes;

/// Helpers for building in-memory TIFF fixtures, used by the crate's own tests
/// and by documentation examples. Not part of the supported API surface.
pub mod testsupport {
    use super::*;

    /// Builds a minimal, uncompressed, little-endian, single-band 2x2 `u8` TIFF,
    /// entirely in memory, to exercise the parser end to end.
    pub fn make_uncompressed_tiff() -> Vec<u8> {
        let mut out = Vec::new();
        let width = 2u16;
        let height = 2u16;
        let bits = 8u16;
        let compression = COMPRESSION_NONE;
        let photometric = 1u16;
        let strip_offset = 8u32 + 2 + 12 * 7 + 4; // header + count + entries + next-ifd
        let pixel_data: [u8; 4] = [10, 20, 30, 40];

        macro_rules! entry {
            ($tag:expr, $tc:expr, $count:expr, $val:expr) => {{
                out.extend_from_slice(&($tag as u16).to_le_bytes());
                out.extend_from_slice(&($tc as u16).to_le_bytes());
                out.extend_from_slice(&($count as u32).to_le_bytes());
                let mut field = [0u8; 4];
                field[..($val).len()].copy_from_slice($val);
                out.extend_from_slice(&field);
            }};
        }

        out.extend_from_slice(b"II"); // little-endian
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes()); // IFD offset

        out.extend_from_slice(&7u16.to_le_bytes());
        entry!(256, TYPE_SHORT, 1, &width.to_le_bytes());
        entry!(257, TYPE_SHORT, 1, &height.to_le_bytes());
        entry!(258, TYPE_SHORT, 1, &bits.to_le_bytes());
        entry!(259, TYPE_SHORT, 1, &compression.to_le_bytes());
        entry!(262, TYPE_SHORT, 1, &photometric.to_le_bytes());
        entry!(273, TYPE_LONG, 1, &strip_offset.to_le_bytes());
        entry!(279, TYPE_LONG, 1, &(pixel_data.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // next IFD = none

        out.extend_from_slice(&pixel_data);
        out
    }

    /// Builds a stripped, little-endian, single-band `u8` TIFF whose single strip
    /// is encoded with the given compression scheme, so the decoder's
    /// decompression paths can be exercised.
    pub fn make_compressed_tiff(
        compression: u16,
        pixels: &[u8],
        width: u16,
        height: u16,
    ) -> Vec<u8> {
        let encoded = match compression {
            COMPRESSION_NONE => pixels.to_vec(),
            COMPRESSION_DEFLATE | COMPRESSION_DEFLATE_PK => deflate_encode(pixels),
            COMPRESSION_LZW => lzw_encode(pixels),
            COMPRESSION_PACKBITS => packbits_encode(pixels),
            other => panic!("fixture does not know compression {other}"),
        };

        let mut out = Vec::new();
        let bits = 8u16;
        let photometric = 1u16;
        let strip_offset = 8u32 + 2 + 12 * 7 + 4;
        let pixel_data = encoded;

        macro_rules! entry {
            ($tag:expr, $tc:expr, $count:expr, $val:expr) => {{
                out.extend_from_slice(&($tag as u16).to_le_bytes());
                out.extend_from_slice(&($tc as u16).to_le_bytes());
                out.extend_from_slice(&($count as u32).to_le_bytes());
                let mut field = [0u8; 4];
                field[..($val).len()].copy_from_slice($val);
                out.extend_from_slice(&field);
            }};
        }

        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());

        out.extend_from_slice(&7u16.to_le_bytes());
        entry!(256, TYPE_SHORT, 1, &width.to_le_bytes());
        entry!(257, TYPE_SHORT, 1, &height.to_le_bytes());
        entry!(258, TYPE_SHORT, 1, &bits.to_le_bytes());
        entry!(259, TYPE_SHORT, 1, &compression.to_le_bytes());
        entry!(262, TYPE_SHORT, 1, &photometric.to_le_bytes());
        entry!(273, TYPE_LONG, 1, &strip_offset.to_le_bytes());
        entry!(279, TYPE_LONG, 1, &(pixel_data.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());

        out.extend_from_slice(&pixel_data);
        out
    }

    fn deflate_encode(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn lzw_encode(data: &[u8]) -> Vec<u8> {
        weezl::encode::Encoder::new(weezl::BitOrder::Msb, 8).encode(data).expect("lzw encode")
    }

    fn packbits_encode(data: &[u8]) -> Vec<u8> {
        // Emit the whole buffer as a single literal run so the decoder's literal
        // path is exercised. A PackBits literal header of `n` means "copy the next
        // n + 1 bytes unchanged".
        let header = (data.len().saturating_sub(1)) as u8;
        let mut out = vec![header];
        out.extend_from_slice(data);
        out
    }

    /// Builds a tiled, single-band, uncompressed `u8` Cloud Optimized GeoTIFF
    /// entirely in memory, so the COG layout validator and the streaming reader can
    /// be exercised end to end without touching disk or the network.
    ///
    /// The image is `width x height` cells in `tile_w x tile_h` tiles, laid out with
    /// *internal tiling* (tile data in raster order, strictly increasing offsets) and
    /// georeferenced north-up with a `1 x 1` pixel size and EPSG:3857, so it passes
    /// [`crate::cog::validate`] as a valid COG. `pix(col, row)` supplies each cell's
    /// value.
    pub fn make_cog<F>(width: u16, height: u16, tile_w: u16, tile_h: u16, mut pix: F) -> Vec<u8>
    where
        F: FnMut(usize, usize) -> u8,
    {
        let width = width as usize;
        let height = height as usize;
        let tile_w = tile_w as usize;
        let tile_h = tile_h as usize;
        let bytes_per = 1usize;
        let tiles_x = (width + tile_w - 1) / tile_w.max(1);
        let tiles_y = (height + tile_h - 1) / tile_h.max(1);
        let tile_count = tiles_x * tiles_y;
        let tile_bytes = tile_w * tile_h * bytes_per;

        // Pixel data for every tile, in raster order (row-major tiles, row-major
        // cells within each tile), so the offsets are already strictly increasing.
        let mut tile_data = Vec::with_capacity(tile_count * tile_bytes);
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let col0 = tx * tile_w;
                let row0 = ty * tile_h;
                for r in 0..tile_h {
                    let row = (row0 + r).min(height - 1);
                    for c in 0..tile_w {
                        let col = (col0 + c).min(width - 1);
                        tile_data.push(pix(col, row));
                    }
                }
            }
        }

        // GeoTIFF georeferencing blobs.
        let mut pixel_scale = Vec::new();
        for value in [1.0f64, 1.0, 0.0] {
            pixel_scale.extend_from_slice(&value.to_le_bytes());
        }
        // Tiepoint maps raster (0, 0) to world (0, height): north-up, top-left origin.
        let mut tiepoint = Vec::new();
        for value in [0.0f64, 0.0, 0.0, 0.0, height as f64, 0.0] {
            tiepoint.extend_from_slice(&value.to_le_bytes());
        }
        // GeoKeyDirectory: one key, ProjectedCSTypeGeoKey = 3857.
        let mut geokey = Vec::new();
        for value in [1u16, 1, 0, 1, 3072, 0, 1, 3857] {
            geokey.extend_from_slice(&value.to_le_bytes());
        }
        let nodata = b"-9999\0".to_vec();

        // Lay out the file: header, IFD, external fields, then tile data in order.
        const TAG_TILE_WIDTH: u16 = 322;
        const TAG_TILE_HEIGHT: u16 = 323;
        const TAG_TILE_OFFSETS: u16 = 324;
        const TAG_TILE_BYTECOUNTS: u16 = 325;
        const TAG_MODEL_PIXEL_SCALE: u16 = 33550;
        const TAG_MODEL_TIEPOINT: u16 = 33922;
        const TAG_GEO_KEY_DIRECTORY: u16 = 34735;
        const TAG_GDAL_NODATA: u16 = 42113;

        let entries: &[(u16, u16, u32, u32)] = &[
            (256, TYPE_SHORT, 1, width as u32),
            (257, TYPE_SHORT, 1, height as u32),
            (258, TYPE_SHORT, 1, 8),
            (259, TYPE_SHORT, 1, COMPRESSION_NONE as u32),
            (262, TYPE_SHORT, 1, 1),
            (277, TYPE_SHORT, 1, 1),
            (TAG_TILE_WIDTH, TYPE_SHORT, 1, tile_w as u32),
            (TAG_TILE_HEIGHT, TYPE_SHORT, 1, tile_h as u32),
            (339, TYPE_SHORT, 1, SAMPLEFORMAT_UINT as u32),
        ];
        let external: &[(u16, u16, u32)] = &[
            (TAG_TILE_OFFSETS, TYPE_LONG, tile_count as u32),
            (TAG_TILE_BYTECOUNTS, TYPE_LONG, tile_count as u32),
            (TAG_MODEL_PIXEL_SCALE, TYPE_DOUBLE, 3),
            (TAG_MODEL_TIEPOINT, TYPE_DOUBLE, 6),
            (TAG_GEO_KEY_DIRECTORY, TYPE_SHORT, geokey.len() as u32 / 2),
            (TAG_GDAL_NODATA, TYPE_ASCII, nodata.len() as u32),
        ];
        let count_n = (entries.len() + external.len()) as u16;

        let ifd_offset = 8u32;
        let ifd_size = 2 + 12 * entries.len() + 12 * external.len() + 4;
        let mut pos = 8 + ifd_size;
        let pixel_scale_pos = pos;
        pos += pixel_scale.len();
        let tiepoint_pos = pos;
        pos += tiepoint.len();
        let geokey_pos = pos;
        pos += geokey.len();
        let tile_offsets_pos = pos;
        pos += tile_count * 4;
        let tile_bytecounts_pos = pos;
        pos += tile_count * 4;
        let nodata_pos = pos;
        pos += nodata.len();

        let mut tile_positions = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            tile_positions.push(pos);
            pos += tile_bytes;
        }
        let file_len = pos;

        let mut out = vec![0u8; file_len];
        out[0..2].copy_from_slice(b"II");
        out[2..4].copy_from_slice(&42u16.to_le_bytes());
        out[4..8].copy_from_slice(&ifd_offset.to_le_bytes());

        let mut p = 8;
        out[p..p + 2].copy_from_slice(&count_n.to_le_bytes());
        p += 2;

        // Inline (<= 4 byte) fields store their value left-aligned in the 4-byte slot.
        for &(tag, tc, count, value) in entries {
            out[p..p + 2].copy_from_slice(&tag.to_le_bytes());
            out[p + 2..p + 4].copy_from_slice(&tc.to_le_bytes());
            out[p + 4..p + 8].copy_from_slice(&count.to_le_bytes());
            out[p + 8..p + 12].copy_from_slice(&value.to_le_bytes());
            p += 12;
        }
        // External fields store their byte offset in the 4-byte slot.
        // Must stay in the same order as the `external` entry array below.
        let ext_positions = [
            tile_offsets_pos,
            tile_bytecounts_pos,
            pixel_scale_pos,
            tiepoint_pos,
            geokey_pos,
            nodata_pos,
        ];
        let mut ext_offsets = ext_positions.iter().copied();
        for &(tag, tc, count) in external {
            let offset = ext_offsets.next().unwrap();
            out[p..p + 2].copy_from_slice(&tag.to_le_bytes());
            out[p + 2..p + 4].copy_from_slice(&tc.to_le_bytes());
            out[p + 4..p + 8].copy_from_slice(&count.to_le_bytes());
            out[p + 8..p + 12].copy_from_slice(&(offset as u32).to_le_bytes());
            p += 12;
        }
        out[p..p + 4].copy_from_slice(&0u32.to_le_bytes());

        out[pixel_scale_pos..pixel_scale_pos + pixel_scale.len()].copy_from_slice(&pixel_scale);
        out[tiepoint_pos..tiepoint_pos + tiepoint.len()].copy_from_slice(&tiepoint);
        out[geokey_pos..geokey_pos + geokey.len()].copy_from_slice(&geokey);
        for (i, &offset) in tile_positions.iter().enumerate() {
            out[tile_offsets_pos + i * 4..tile_offsets_pos + i * 4 + 4]
                .copy_from_slice(&(offset as u32).to_le_bytes());
            out[tile_bytecounts_pos + i * 4..tile_bytecounts_pos + i * 4 + 4]
                .copy_from_slice(&(tile_bytes as u32).to_le_bytes());
        }
        out[nodata_pos..nodata_pos + nodata.len()].copy_from_slice(&nodata);
        for (i, &offset) in tile_positions.iter().enumerate() {
            let src = &tile_data[i * tile_bytes..i * tile_bytes + tile_bytes];
            out[offset..offset + tile_bytes].copy_from_slice(src);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellKind;

    fn pixels() -> Vec<u8> {
        // A 4x3 ramp with a couple of repeated values to exercise run-length.
        vec![1, 2, 2, 3, 4, 4, 4, 5, 6, 7, 7, 8]
    }

    #[test]
    fn reads_an_uncompressed_single_band_tiff() {
        let bytes = testsupport::make_uncompressed_tiff();
        let tiff = GeoTiff::parse(&bytes).unwrap();
        assert_eq!(tiff.image_count(), 1);
        let band = tiff.read().unwrap();
        assert_eq!(band.width(), 2);
        assert_eq!(band.height(), 2);
        assert_eq!(band.kind(), CellKind::U8);
        assert_eq!(band.as_u8().unwrap().grid().cells(), &[10u8, 20, 30, 40]);
    }

    #[test]
    fn reads_compressed_strip_variants() {
        let expected = pixels();
        for (name, compression) in [
            ("none", COMPRESSION_NONE),
            ("deflate", COMPRESSION_DEFLATE),
            ("lzw", COMPRESSION_LZW),
            ("packbits", COMPRESSION_PACKBITS),
        ] {
            let bytes = testsupport::make_compressed_tiff(compression, &expected, 4, 3);
            let band = GeoTiff::parse(&bytes).unwrap().read().unwrap();
            assert_eq!(band.kind(), CellKind::U8, "{name} kind");
            assert_eq!(band.as_u8().unwrap().grid().cells(), &expected[..], "{name} pixels");
        }
    }

    #[test]
    fn reads_a_sub_window_of_the_image() {
        let expected = pixels();
        let bytes = testsupport::make_compressed_tiff(COMPRESSION_NONE, &expected, 4, 3);
        let tiff = GeoTiff::parse(&bytes).unwrap();
        // The 2x2 window at columns [1,3), rows [1,3) is rows 1-2, cols 1-2:
        //   row 1: [4,4]  row 2: [7,7]
        let band = tiff.read_region(0, 1, 1, 3, 3).unwrap();
        assert_eq!(band.width(), 2);
        assert_eq!(band.height(), 2);
        assert_eq!(band.as_u8().unwrap().grid().cells(), &[4u8, 4, 7, 7]);
    }

    #[test]
    fn rejects_a_truly_malformed_header() {
        assert!(GeoTiff::parse(b"XX\x00\x00").is_err());
        assert!(GeoTiff::parse(&[0u8; 4]).is_err());
    }

    #[test]
    fn reads_a_tiled_cog_full_image_and_region() {
        // 4x4 image, 2x2 tiles, value = col + row.
        let bytes = testsupport::make_cog(4, 4, 2, 2, |c, r| (c + r) as u8);
        let tiff = GeoTiff::parse(&bytes).unwrap();
        assert_eq!(tiff.image_count(), 1);

        let full = tiff.read().unwrap();
        assert_eq!(full.width(), 4);
        assert_eq!(full.height(), 4);
        let full = full.as_u8().unwrap();
        assert_eq!(full.get(0, 0), Some(0));
        assert_eq!(full.get(3, 3), Some(6));
        assert_eq!(full.get(2, 1), Some(3));

        // A region starting mid-tile (top-left 2x2 window of cells [1,3)x[1,3)) must
        // extract the right sub-block; this exercises the tile-relative stride.
        let region = tiff.read_region(0, 1, 1, 3, 3).unwrap();
        assert_eq!(region.width(), 2);
        assert_eq!(region.height(), 2);
        let region = region.as_u8().unwrap();
        assert_eq!(region.get(0, 0), Some(2)); // col 1, row 1
        assert_eq!(region.get(1, 1), Some(4)); // col 2, row 2
    }

    #[test]
    fn rejects_a_chunk_with_an_absurd_byte_count() {
        // A crafted COG may declare a per-tile byte count far larger than the real
        // tile size, reaching a (possibly very large over HTTP) fetch before any
        // size check. `MAX_RAW_CHUNK_BYTES` must reject it in `image_info_from`
        // before a fetch is issued.
        let mut bytes = testsupport::make_cog(4, 4, 2, 2, |c, r| (c + r) as u8);
        // Locate the TileByteCounts (tag 325) external array via the IFD and inflate
        // the first chunk's count past the cap.
        let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
        let mut p = ifd + 2;
        for _ in 0..count {
            let tag = u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap());
            if tag == 325 {
                let value_off =
                    u32::from_le_bytes(bytes[p + 8..p + 12].try_into().unwrap()) as usize;
                let huge = MAX_RAW_CHUNK_BYTES as u32 + 1;
                bytes[value_off..value_off + 4].copy_from_slice(&huge.to_le_bytes());
                break;
            }
            p += 12;
        }
        let tiff = GeoTiff::parse(&bytes).unwrap();
        assert!(tiff.read_image(0).is_err());
    }
}
