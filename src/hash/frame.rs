use core::fmt;

/// Largest frame accepted, in pixels. A frame is hashed through two `f32` planes of this many
/// elements, so this bounds memory at about 8 bytes per pixel on top of the frame itself.
pub const MAX_FRAME_PIXELS: usize = 1 << 24;

/// A decoded image: packed RGB8, row-major, no padding, with explicit dimensions.
///
/// This is the only image input to the crate. The length is checked at construction, so a frame
/// whose rows were read with the wrong stride (for example a rotated image whose width and
/// height were swapped but whose byte length still matches) is a host bug the type cannot see:
/// the host must pass the dimensions of the buffer it actually decoded.
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    rgb: &'a [u8],
    width: u32,
    height: u32,
}

/// Why a [`Frame`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameError {
    /// Width or height is zero.
    ZeroDimension,
    /// `width * height` exceeds [`MAX_FRAME_PIXELS`] (or overflows).
    TooLarge,
    /// The buffer is not `width * height * 3` bytes.
    LengthMismatch {
        /// Bytes required by the dimensions.
        expected: usize,
        /// Bytes supplied.
        actual: usize,
    },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::ZeroDimension => f.write_str("frame width and height must be non-zero"),
            FrameError::TooLarge => write!(f, "frame exceeds {MAX_FRAME_PIXELS} pixels"),
            FrameError::LengthMismatch { expected, actual } => {
                write!(
                    f,
                    "frame buffer is {actual} bytes, dimensions need {expected}"
                )
            }
        }
    }
}

impl<'a> Frame<'a> {
    /// Wraps a packed RGB8 buffer.
    pub fn new(rgb: &'a [u8], width: u32, height: u32) -> Result<Self, FrameError> {
        if width == 0 || height == 0 {
            return Err(FrameError::ZeroDimension);
        }
        let pixels = (width as usize)
            .checked_mul(height as usize)
            .filter(|p| *p <= MAX_FRAME_PIXELS)
            .ok_or(FrameError::TooLarge)?;
        let expected = pixels * 3;
        if rgb.len() != expected {
            return Err(FrameError::LengthMismatch {
                expected,
                actual: rgb.len(),
            });
        }
        Ok(Frame { rgb, width, height })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn rgb(&self) -> &'a [u8] {
        self.rgb
    }
}

// Pixel data must never reach logs or error text (design invariant I6): print the shape only.
impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Frame {{ width: {}, height: {}, bytes: {} }}",
            self.width,
            self.height,
            self.rgb.len()
        )
    }
}
