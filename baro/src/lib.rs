//! 바로보기 코어. JPG·PNG 디코드는 시작 때 호출하지 않고, 파일을 열 때만 탄다.
//! SVG·GIF 함수도 그 형식을 열기 전에는 실행되지 않는다.

mod convert;
mod decode;
mod exif;
mod kind;
mod scan;

pub use convert::{convert, OutFormat};
pub use decode::{decode_gif, decode_jpeg, decode_png, decode_still, raster_svg, visit_gif, GifFrame, Still};
pub use exif::extract_jpeg_thumbnail;
pub use kind::{kind_from_name, sniff, Kind};
pub use scan::{list_images, load_thumb};

pub type Result<T> = std::result::Result<T, String>;
