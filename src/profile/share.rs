//! Share links (`easytierui://import?v=1&c=<base64url(TOML)>`) and their QR codes.
//!
//! A link carries the rendered config; the network secret travels only when the caller asks for it, and a link
//! that includes it is a [`Secret`] (shown or copied only on a person's request). Parsing a link goes through the
//! same untrusted-input import as a pasted file and never applies anything. QR codes are encoded to a module matrix
//! the UI draws, and decoded from the bytes of a picked image (PNG or JPEG): decoding is CPU-heavy, so the store
//! calls it through `core::work::off_thread` where threads exist.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use qrcodegen::{QrCode, QrCodeEcc};
use url::Url;

use super::{
    error::ProfileError,
    import::{ImportError, Imported, import_toml_with_id},
    model::{NetworkProfile, ProfileId},
};
use crate::core::{acl::GroupSecrets, secret::Secret};

/// The start of every share link.
pub const LINK_PREFIX: &str = "easytierui://import";
/// The only link format version this app writes and reads.
pub const LINK_VERSION: &str = "1";
/// The longest link accepted: a QR code holds under 3 KB, a pasted link is never close to this.
pub const MAX_LINK_BYTES: usize = 64 * 1024;
/// Quiet-zone width, in modules, the UI should leave around a drawn code.
pub const QR_QUIET_ZONE: usize = 4;
/// The most pixels a picked image may have (a 25-megapixel photo).
pub const MAX_IMAGE_PIXELS: usize = 25_000_000;
/// The longest side, in pixels.
const MAX_IMAGE_SIDE: usize = 10_000;

/// Why a link or a QR code could not be made or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShareError {
    /// The profile could not be rendered.
    #[error(transparent)]
    Profile(#[from] ProfileError),
    /// The text is not an `easytierui://import` link.
    #[error("this is not an EasyTierUI share link")]
    NotAShareLink,
    /// The link is from a newer (or unknown) format.
    #[error("this share link uses a format this version does not understand")]
    UnsupportedVersion,
    /// The link has no `c` payload.
    #[error("the share link carries no config")]
    MissingPayload,
    /// The payload is not base64url text of UTF-8.
    #[error("the share link's config is damaged")]
    BadPayload,
    /// The link is longer than [`MAX_LINK_BYTES`].
    #[error("the share link is larger than {max} bytes")]
    TooLarge {
        /// The limit.
        max: usize,
    },
    /// The config in the link was rejected.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// The link is too long for a QR code; share the link as text instead.
    #[error("the link is too long for a QR code")]
    QrTooLong,
    /// The picked file is neither a PNG nor a JPEG.
    #[error("the picked file is not a PNG or JPEG image")]
    UnsupportedImage,
    /// The image is larger than the limit.
    #[error("the image is larger than {max} pixels")]
    ImageTooLarge {
        /// The limit, in pixels.
        max: usize,
    },
    /// The image is damaged or uses an unsupported variant.
    #[error("the image could not be decoded")]
    ImageDecode,
    /// No QR code was found in the image.
    #[error("no QR code was found in the image")]
    NoQrCode,
    /// A QR code was found but could not be read (blurred, cropped, damaged).
    #[error("the QR code could not be read")]
    QrUnreadable,
}

/// Makes the share link of `profile`.
///
/// The network secret and the ACL group secrets are part of the link only when `include_secret` is `true` (the
/// network secret also needs `secret` to be given); otherwise the link names the network and the declared groups but
/// no secrets, and whoever imports it must enter them (the import preview counts the groups without one). The
/// result is a [`Secret`] either way: it is a copy of the profile's config.
///
/// # Errors
///
/// [`ShareError::Profile`] when the profile does not render (managed, invalid, or secrets are included and a
/// declared group has none in `groups`).
pub fn share_link(
    profile: &NetworkProfile,
    secret: Option<&Secret>,
    groups: &GroupSecrets,
    include_secret: bool,
) -> Result<Secret, ShareError> {
    let none = Secret::default();
    let no_groups = GroupSecrets::default();
    let (secret, groups) = if include_secret {
        (secret.unwrap_or(&none), groups)
    } else {
        (&none, &no_groups)
    };
    let toml = profile.render_with(secret, groups, include_secret)?;
    let capacity = LINK_PREFIX
        .len()
        .saturating_add(toml.expose().len().saturating_mul(2));
    let mut link = String::with_capacity(capacity);
    link.push_str(LINK_PREFIX);
    link.push_str("?v=");
    link.push_str(LINK_VERSION);
    link.push_str("&c=");
    URL_SAFE_NO_PAD.encode_string(toml.expose().as_bytes(), &mut link);
    Ok(Secret::new(link))
}

/// Reads a share link as a new profile with a fresh id. Nothing is applied: the caller shows
/// [`Imported::preview`] first.
///
/// # Errors
///
/// [`ShareError`]: not a link, a version this app does not know, a damaged payload, or [`ShareError::Import`] with
/// the field the config was rejected for.
pub fn parse_share_link(text: &str) -> Result<Imported, ShareError> {
    parse_share_link_with_id(text, ProfileId::generate()?)
}

/// Reads a share link as a profile with the id `id`.
///
/// # Errors
///
/// As [`parse_share_link`].
pub fn parse_share_link_with_id(text: &str, id: ProfileId) -> Result<Imported, ShareError> {
    let text = text.trim();
    if text.len() > MAX_LINK_BYTES {
        return Err(ShareError::TooLarge {
            max: MAX_LINK_BYTES,
        });
    }
    let url = Url::parse(text).map_err(|_| ShareError::NotAShareLink)?;
    if url.scheme() != "easytierui"
        || url.host_str() != Some("import")
        || !matches!(url.path(), "" | "/")
    {
        return Err(ShareError::NotAShareLink);
    }
    let mut version = None;
    let mut payload = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "v" if version.is_none() => version = Some(value),
            "c" if payload.is_none() => payload = Some(value),
            "v" | "c" => return Err(ShareError::BadPayload),
            _ => {}
        }
    }
    if version.as_deref() != Some(LINK_VERSION) {
        return Err(ShareError::UnsupportedVersion);
    }
    let payload = payload.ok_or(ShareError::MissingPayload)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| ShareError::BadPayload)?;
    let toml = Secret::new(String::from_utf8(bytes).map_err(|_| ShareError::BadPayload)?);
    Ok(import_toml_with_id(toml.expose(), id)?)
}

/// A QR code as dark / light modules, without the quiet zone.
#[derive(Clone, PartialEq, Eq)]
pub struct QrMatrix {
    size: usize,
    modules: Vec<bool>,
}

impl std::fmt::Debug for QrMatrix {
    /// The size only: the modules spell the text, which can hold a secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QrMatrix({0}x{0})", self.size)
    }
}

impl QrMatrix {
    /// Modules per side.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Whether the module at column `x`, row `y` is dark; `false` outside the code (so the quiet zone reads light).
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        if x >= self.size || y >= self.size {
            return false;
        }
        y.checked_mul(self.size)
            .and_then(|row| row.checked_add(x))
            .and_then(|index| self.modules.get(index))
            .copied()
            .unwrap_or(false)
    }
}

/// Encodes `text` as a QR code: medium error correction, low if the text only fits that way.
///
/// # Errors
///
/// [`ShareError::QrTooLong`] when even low error correction cannot hold the text.
pub fn qr_matrix(text: &str) -> Result<QrMatrix, ShareError> {
    let code = QrCode::encode_text(text, QrCodeEcc::Medium)
        .or_else(|_| QrCode::encode_text(text, QrCodeEcc::Low))
        .map_err(|_| ShareError::QrTooLong)?;
    let size = usize::try_from(code.size()).map_err(|_| ShareError::QrTooLong)?;
    let side = i32::try_from(size).map_err(|_| ShareError::QrTooLong)?;
    let mut modules = Vec::with_capacity(size.saturating_mul(size));
    for y in 0..side {
        for x in 0..side {
            modules.push(code.get_module(x, y));
        }
    }
    Ok(QrMatrix { size, modules })
}

/// Reads the QR code in a picked image (the encoded PNG or JPEG bytes) and returns its text. The text can hold a
/// secret, hence [`Secret`].
///
/// # Errors
///
/// [`ShareError::UnsupportedImage`], [`ShareError::ImageTooLarge`], [`ShareError::ImageDecode`],
/// [`ShareError::NoQrCode`] or [`ShareError::QrUnreadable`].
pub fn decode_qr_image(bytes: &[u8]) -> Result<Secret, ShareError> {
    let gray = if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        gray_from_png(bytes)?
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        gray_from_jpeg(bytes)?
    } else {
        return Err(ShareError::UnsupportedImage);
    };
    let (width, height) = (gray.width, gray.height);
    let mut image = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| {
        y.checked_mul(width)
            .and_then(|row| row.checked_add(x))
            .and_then(|index| gray.pixels.get(index))
            .copied()
            .unwrap_or(u8::MAX)
    });
    let grids = image.detect_grids();
    if grids.is_empty() {
        return Err(ShareError::NoQrCode);
    }
    for grid in grids {
        if let Ok((_meta, content)) = grid.decode() {
            return Ok(Secret::new(content));
        }
    }
    Err(ShareError::QrUnreadable)
}

/// Reads a share link out of a picked image of its QR code, as a new profile. Nothing is applied.
///
/// # Errors
///
/// As [`decode_qr_image`] and [`parse_share_link`].
pub fn import_qr_image(bytes: &[u8]) -> Result<Imported, ShareError> {
    let text = decode_qr_image(bytes)?;
    parse_share_link(text.expose())
}

/// An 8-bit grayscale image.
struct Gray {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// Checks the dimensions against the limits and returns them as `usize`.
fn check_size(width: u32, height: u32) -> Result<(usize, usize), ShareError> {
    let too_large = ShareError::ImageTooLarge {
        max: MAX_IMAGE_PIXELS,
    };
    let width = usize::try_from(width).map_err(|_| too_large.clone())?;
    let height = usize::try_from(height).map_err(|_| too_large.clone())?;
    if width == 0 || height == 0 {
        return Err(ShareError::ImageDecode);
    }
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return Err(too_large);
    }
    match width.checked_mul(height) {
        Some(area) if area <= MAX_IMAGE_PIXELS => Ok((width, height)),
        _ => Err(too_large),
    }
}

/// Brightness of an RGB pixel, 0 to 255 (Rec. 601 weights).
fn luma(r: u8, g: u8, b: u8) -> u8 {
    let sum = u32::from(r)
        .saturating_mul(299)
        .saturating_add(u32::from(g).saturating_mul(587))
        .saturating_add(u32::from(b).saturating_mul(114));
    u8::try_from(sum / 1000).unwrap_or(u8::MAX)
}

/// A pixel of brightness `value` and opacity `alpha`, laid over white.
fn over_white(value: u8, alpha: u8) -> u8 {
    let kept = u32::from(value).saturating_mul(u32::from(alpha));
    let white = 255_u32.saturating_mul(u32::from(255_u8.saturating_sub(alpha)));
    u8::try_from(kept.saturating_add(white) / 255).unwrap_or(u8::MAX)
}

fn gray_from_png(bytes: &[u8]) -> Result<Gray, ShareError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // Palettes, tRNS and low bit depths expanded; 16-bit samples cut to 8: every pixel is 1 to 4 bytes.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|_| ShareError::ImageDecode)?;
    let (width, height) = check_size(reader.info().width, reader.info().height)?;
    let size = reader.output_buffer_size().ok_or(ShareError::ImageDecode)?;
    if size > MAX_IMAGE_PIXELS.saturating_mul(4) {
        return Err(ShareError::ImageTooLarge {
            max: MAX_IMAGE_PIXELS,
        });
    }
    let mut buffer = vec![0_u8; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|_| ShareError::ImageDecode)?;
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(ShareError::ImageDecode),
    };
    if frame.line_size == 0 {
        return Err(ShareError::ImageDecode);
    }
    let mut pixels = Vec::with_capacity(width.saturating_mul(height));
    for row in buffer.chunks_exact(frame.line_size).take(height) {
        for pixel in row.chunks_exact(channels).take(width) {
            pixels.push(match *pixel {
                [gray] => gray,
                [gray, alpha] => over_white(gray, alpha),
                [r, g, b] => luma(r, g, b),
                [r, g, b, alpha] => over_white(luma(r, g, b), alpha),
                _ => u8::MAX,
            });
        }
    }
    if pixels.len() != width.saturating_mul(height) {
        return Err(ShareError::ImageDecode);
    }
    Ok(Gray {
        width,
        height,
        pixels,
    })
}

fn gray_from_jpeg(bytes: &[u8]) -> Result<Gray, ShareError> {
    use zune_jpeg::{
        JpegDecoder,
        zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
    };
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::Luma)
        .set_max_width(MAX_IMAGE_SIDE)
        .set_max_height(MAX_IMAGE_SIDE);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|_| ShareError::ImageDecode)?;
    let info = decoder.info().ok_or(ShareError::ImageDecode)?;
    let (width, height) = check_size(u32::from(info.width), u32::from(info.height))?;
    let pixels = decoder.decode().map_err(|_| ShareError::ImageDecode)?;
    if pixels.len() != width.saturating_mul(height) {
        return Err(ShareError::ImageDecode);
    }
    Ok(Gray {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{
        model::{IpMode, Socks5},
        testing,
    };

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn id() -> ProfileId {
        ProfileId::parse(ID).unwrap()
    }

    fn secret() -> Secret {
        Secret::new("example-secret-do-not-use")
    }

    fn profile() -> NetworkProfile {
        let mut p = NetworkProfile::new(id(), "example-net", "example-net");
        p.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        p.peers = vec!["tcp://203.0.113.7:11010".parse().unwrap()];
        p.socks5 = Some(Socks5::local(1080));
        p
    }

    #[test]
    fn links_round_trip_with_the_secret() {
        let link = share_link(&profile(), Some(&secret()), &GroupSecrets::default(), true).unwrap();
        assert!(link.expose().starts_with("easytierui://import?v=1&c="));
        assert_eq!(format!("{link:?}"), "Secret(***)");
        let imported = parse_share_link_with_id(link.expose(), id()).unwrap();
        assert_eq!(imported.profile, profile());
        assert_eq!(imported.secret, Some(secret()));
        assert!(imported.preview.has_secret);
    }

    #[test]
    fn links_leave_the_secret_out_unless_asked() {
        for (given, include) in [(Some(secret()), false), (None, false), (None, true)] {
            let link = share_link(
                &profile(),
                given.as_ref(),
                &GroupSecrets::default(),
                include,
            )
            .unwrap();
            let imported = parse_share_link_with_id(link.expose(), id()).unwrap();
            assert_eq!(imported.profile, profile());
            assert_eq!(imported.secret, None);
            assert!(!link.expose().contains("example-secret"));
            // The payload is encoded, so check the decoded config as well.
            let payload = link.expose().split("&c=").nth(1).unwrap();
            let decoded = String::from_utf8(URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap();
            assert!(!decoded.contains("network_secret"));
        }
    }

    #[test]
    fn group_secrets_travel_exactly_when_the_network_secret_does() {
        let (full, groups) = testing::full_profile().unwrap();
        let id = full.id.clone();
        let with = share_link(&full, Some(&secret()), &groups, true).unwrap();
        let imported = parse_share_link_with_id(with.expose(), id.clone()).unwrap();
        assert_eq!(imported.profile, full);
        assert_eq!(imported.secret, Some(secret()));
        assert_eq!(imported.group_secrets, groups);
        assert_eq!(
            imported
                .preview
                .acl
                .as_ref()
                .map(|acl| acl.groups_without_secret),
            Some(0)
        );

        let without = share_link(&full, Some(&secret()), &groups, false).unwrap();
        let payload = without.expose().split("&c=").nth(1).unwrap();
        let decoded = String::from_utf8(URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap();
        assert!(!decoded.contains("example-group-secret"));
        assert!(!decoded.contains("network_secret"));
        let imported = parse_share_link_with_id(without.expose(), id).unwrap();
        assert_eq!(imported.profile.acl, full.acl);
        assert!(imported.group_secrets.is_empty());
        assert_eq!(
            imported
                .preview
                .acl
                .as_ref()
                .map(|acl| acl.groups_without_secret),
            Some(1)
        );

        // Asking for secrets without having a group's secret is an error, not a link with a hole in it.
        assert!(matches!(
            share_link(&full, Some(&secret()), &GroupSecrets::default(), true),
            Err(ShareError::Profile(ProfileError::Acl(_)))
        ));
    }

    #[test]
    fn links_survive_whitespace_padding_and_extra_parameters() {
        let link = share_link(&profile(), Some(&secret()), &GroupSecrets::default(), true).unwrap();
        let padded = format!("  {}&utm=1\n", link.expose());
        assert_eq!(
            parse_share_link_with_id(&padded, id()).unwrap().profile,
            profile()
        );
        let with_padding = format!("{}==", link.expose());
        assert!(parse_share_link_with_id(&with_padding, id()).is_ok());
    }

    #[test]
    fn bad_links_are_rejected_with_a_reason() {
        let parse = |text: &str| parse_share_link_with_id(text, id()).unwrap_err();
        assert_eq!(parse("https://example.com"), ShareError::NotAShareLink);
        assert_eq!(
            parse("easytierui://export?v=1&c=AAAA"),
            ShareError::NotAShareLink
        );
        assert_eq!(parse("hello"), ShareError::NotAShareLink);
        assert_eq!(
            parse("easytierui://import?v=2&c=AAAA"),
            ShareError::UnsupportedVersion
        );
        assert_eq!(
            parse("easytierui://import?c=AAAA"),
            ShareError::UnsupportedVersion
        );
        assert_eq!(parse("easytierui://import?v=1"), ShareError::MissingPayload);
        assert_eq!(
            parse("easytierui://import?v=1&c=***"),
            ShareError::BadPayload
        );
        assert_eq!(
            parse("easytierui://import?v=1&c=AAAA&c=BBBB"),
            ShareError::BadPayload
        );
        // base64url of invalid UTF-8.
        assert_eq!(
            parse("easytierui://import?v=1&c=_-8"),
            ShareError::BadPayload
        );
        assert_eq!(
            parse(&format!(
                "easytierui://import?v=1&c={}",
                "A".repeat(MAX_LINK_BYTES)
            )),
            ShareError::TooLarge {
                max: MAX_LINK_BYTES
            }
        );
    }

    #[test]
    fn a_link_with_a_bad_config_names_the_field() {
        let toml = "dhcp = true\nlisteners = []\n[network_identity]\nnetwork_name = \"n\"\n[[peer]]\nuri = \"ftp://x.example.com\"\n";
        let link = format!("easytierui://import?v=1&c={}", URL_SAFE_NO_PAD.encode(toml));
        match parse_share_link_with_id(&link, id()).unwrap_err() {
            ShareError::Import(error) => assert_eq!(error.field(), Some("peer[0].uri")),
            other => panic!("expected an import error, got {other:?}"),
        }
    }

    #[test]
    fn managed_profiles_have_nothing_to_share() {
        let managed = NetworkProfile::managed(id(), "Office");
        assert_eq!(
            share_link(&managed, None, &GroupSecrets::default(), false),
            Err(ShareError::Profile(ProfileError::Managed))
        );
    }

    /// Draws a QR matrix as an 8-bit grayscale PNG: `scale` pixels per module and a quiet zone around it.
    fn render_png(matrix: &QrMatrix, scale: usize) -> Vec<u8> {
        let total = matrix
            .size()
            .saturating_add(QR_QUIET_ZONE.saturating_mul(2));
        let dark =
            |x: usize, y: usize| match (x.checked_sub(QR_QUIET_ZONE), y.checked_sub(QR_QUIET_ZONE))
            {
                (Some(x), Some(y)) => matrix.is_dark(x, y),
                _ => false,
            };
        let mut pixels = Vec::new();
        for y in 0..total {
            let row: Vec<u8> = (0..total)
                .flat_map(|x| std::iter::repeat_n(if dark(x, y) { 0_u8 } else { 255_u8 }, scale))
                .collect();
            for _ in 0..scale {
                pixels.extend_from_slice(&row);
            }
        }
        let side = u32::try_from(total.saturating_mul(scale)).unwrap();
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, side, side);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&pixels).unwrap();
        }
        out
    }

    #[test]
    fn qr_codes_encode_and_decode_from_a_png() {
        let link = share_link(&profile(), Some(&secret()), &GroupSecrets::default(), true).unwrap();
        let matrix = qr_matrix(link.expose()).unwrap();
        assert!(matrix.size() >= 21);
        assert!(!format!("{matrix:?}").contains("easytierui"));
        assert!(!matrix.is_dark(matrix.size(), 0));
        let png = render_png(&matrix, 4);
        let decoded = decode_qr_image(&png).unwrap();
        assert_eq!(decoded.expose(), link.expose());
        let imported = import_qr_image(&png).unwrap();
        assert_eq!(imported.secret, Some(secret()));
        assert_eq!(imported.profile.network_name, "example-net");
    }

    #[test]
    fn qr_codes_without_the_secret_decode_without_it() {
        let link =
            share_link(&profile(), Some(&secret()), &GroupSecrets::default(), false).unwrap();
        let png = render_png(&qr_matrix(link.expose()).unwrap(), 3);
        let imported = import_qr_image(&png).unwrap();
        assert_eq!(imported.secret, None);
    }

    #[test]
    fn qr_text_too_long_is_reported() {
        assert_eq!(qr_matrix(&"x".repeat(4000)), Err(ShareError::QrTooLong));
    }

    #[test]
    fn images_without_a_readable_code_are_reported() {
        // A blank white PNG.
        let blank = {
            let mut out = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut out, 64, 64);
                encoder.set_color(png::ColorType::Grayscale);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().unwrap();
                writer.write_image_data(&[255_u8; 64 * 64]).unwrap();
            }
            out
        };
        assert_eq!(decode_qr_image(&blank), Err(ShareError::NoQrCode));
        assert_eq!(
            decode_qr_image(b"GIF89a not an image"),
            Err(ShareError::UnsupportedImage)
        );
        assert_eq!(decode_qr_image(&[]), Err(ShareError::UnsupportedImage));
        assert_eq!(decode_qr_image(&blank[..40]), Err(ShareError::ImageDecode));
        assert_eq!(
            decode_qr_image(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 1, 2, 3]),
            Err(ShareError::ImageDecode)
        );
    }

    #[test]
    fn a_qr_code_that_is_not_a_share_link_is_not_imported() {
        let png = render_png(&qr_matrix("https://example.com").unwrap(), 4);
        assert_eq!(
            decode_qr_image(&png).unwrap().expose(),
            "https://example.com"
        );
        assert_eq!(
            import_qr_image(&png).unwrap_err(),
            ShareError::NotAShareLink
        );
    }

    #[test]
    fn pixel_math_stays_in_range() {
        assert_eq!(luma(255, 255, 255), 255);
        assert_eq!(luma(0, 0, 0), 0);
        assert_eq!(over_white(0, 0), 255);
        assert_eq!(over_white(0, 255), 0);
        assert_eq!(over_white(100, 255), 100);
    }
}
