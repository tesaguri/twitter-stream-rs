pub use http::Method as RequestMethod;
pub use http::Uri;

use std::mem;
use std::slice;

use static_assertions::{assert_eq_align, assert_eq_size};

/// A `BoundingBox` is a rectangular area on the globe specified by coordinates of
/// the southwest and northeast edges in decimal degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct BoundingBox {
    /// Longitude of the west side of the bounding box.
    pub west_longitude: f64,
    /// Latitude of the south side of the bounding box.
    pub south_latitude: f64,
    /// Longitude of the east side of the bounding box.
    pub east_longitude: f64,
    /// Latitude of the north side of the bounding box.
    pub north_latitude: f64,
}

impl BoundingBox {
    /// Creates a `BoundingBox` with the longitudes and latitudes of its sides.
    ///
    /// # Example
    ///
    /// ```rust
    /// use twitter_stream::builder::BoundingBox;
    ///
    /// // Examples taken from Twitter's documentation.
    /// BoundingBox::new(-122.75, 36.8, -121.75, 37.8); // San Francisco
    /// BoundingBox::new(-74.0, 40.0, -73.0, 41.0); // New York City
    /// ```
    pub const fn new(
        west_longitude: f64,
        south_latitude: f64,
        east_longitude: f64,
        north_latitude: f64,
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }

    /// Creates a slice of `BoundingBox`-es from a slice of arrays of
    /// `[west_longitude, south_latitude, east_longitude, north_latitude]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use twitter_stream::builder::BoundingBox;
    ///
    /// #[derive(serde::Deserialize)]
    /// struct Config {
    ///     locations: Vec<[f64; 4]>,
    /// }
    ///
    /// let config = r#"{"locations": [[-122.75, 36.8, -121.75, 37.8]]}"#;
    /// let config: Config = serde_json::from_str(config).unwrap();
    ///
    /// let locations = BoundingBox::unflatten_slice(&config.locations);
    /// assert_eq!(locations, [BoundingBox::new(-122.75, 36.8, -121.75, 37.8)]);
    /// ```
    pub fn unflatten_slice(slice: &[[f64; 4]]) -> &[Self] {
        unsafe {
            // Safety: the `#[repr(C)]` on `BoundingBox` guarantees the soundness of the conversion.
            // We are checking the size and alignment of `BoundingBox` just to be sure.

            // Also, the attribute ensures that the fields are placed in
            // `[west_longitude, south_latitude, east_longitude, north_latitude]` order.

            assert_eq_size!(BoundingBox, [f64; 4]);
            assert_eq_align!([f64; 4], BoundingBox);

            let ptr: *const [f64; 4] = slice.as_ptr();
            let len = <[[f64; 4]]>::len(slice);

            slice::from_raw_parts(ptr as *const BoundingBox, len)
        }
    }

    /// Converts a slice of `BoundingBox`-es into a slice of arrays of
    /// `[west_longitude, south_latitude, east_longitude, north_latitude]`.
    pub fn flatten_slice(bboxes: &[Self]) -> &[[f64; 4]] {
        unsafe {
            // Safety: see the safety note in the `unflatten_slice` function.

            assert_eq_size!(BoundingBox, [f64; 4]);
            assert_eq_align!([f64; 4], BoundingBox);

            let ptr: *const BoundingBox = bboxes.as_ptr();
            let len = <[BoundingBox]>::len(bboxes);

            slice::from_raw_parts(ptr as *const [f64; 4], len)
        }
    }

    /// Creates a vector of `BoundingBox`-es from a vector of arrays of
    /// `[west_longitude, south_latitude, east_longitude, north_latitude]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use twitter_stream::builder::BoundingBox;
    ///
    /// #[derive(serde::Deserialize)]
    /// struct Config {
    ///     locations: Vec<[f64; 4]>,
    /// }
    ///
    /// let config = r#"{"locations": [[-122.75, 36.8, -121.75, 37.8]]}"#;
    /// let config: Config = serde_json::from_str(config).unwrap();
    ///
    /// let locations = BoundingBox::unflatten_vec(config.locations);
    /// assert_eq!(locations, [BoundingBox::new(-122.75, 36.8, -121.75, 37.8)]);
    pub fn unflatten_vec(vec: Vec<[f64; 4]>) -> Vec<Self> {
        unsafe {
            // Safety: the `#[repr(C)]` on `BoundingBox` guarantees the soundness of the conversion.
            // We are checking the size and alignment of `BoundingBox` just to be sure.
            // Also, the `ManuallyDrop` prevents the original vector from running its destructor.

            assert_eq_size!(BoundingBox, [f64; 4]);
            assert_eq_align!([f64; 4], BoundingBox);

            let vec = mem::ManuallyDrop::new(vec);

            let ptr: *const [f64; 4] = vec.as_ptr();
            let length = Vec::<[f64; 4]>::len(&vec);
            let capacity = Vec::<[f64; 4]>::capacity(&vec);

            Vec::<BoundingBox>::from_raw_parts(ptr as *mut BoundingBox, length, capacity)
        }
    }

    /// Converts a vector of `BoundingBox`-es into a vector of arrays of
    /// `[west_longitude, south_latitude, east_longitude, north_latitude]`.
    pub fn flatten_vec(vec: Vec<Self>) -> Vec<[f64; 4]> {
        unsafe {
            // Safety: see the safety note in the `unflatten_vec` function.

            assert_eq_size!(BoundingBox, [f64; 4]);
            assert_eq_align!([f64; 4], BoundingBox);

            let vec = mem::ManuallyDrop::new(vec);

            let ptr: *const BoundingBox = vec.as_ptr();
            let length = Vec::<BoundingBox>::len(&vec);
            let capacity = Vec::<BoundingBox>::capacity(&vec);

            Vec::<[f64; 4]>::from_raw_parts(ptr as *mut [f64; 4], length, capacity)
        }
    }
}

impl AsRef<[f64; 4]> for BoundingBox {
    fn as_ref(&self) -> &[f64; 4] {
        &BoundingBox::flatten_slice(slice::from_ref(self))[0]
    }
}

impl AsRef<BoundingBox> for [f64; 4] {
    fn as_ref(&self) -> &BoundingBox {
        &BoundingBox::unflatten_slice(slice::from_ref(self))[0]
    }
}

impl From<[f64; 4]> for BoundingBox {
    fn from([west_longitude, south_latitude, east_longitude, north_latitude]: [f64; 4]) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

impl From<BoundingBox> for [f64; 4] {
    fn from(bbox: BoundingBox) -> Self {
        [
            bbox.west_longitude,
            bbox.south_latitude,
            bbox.east_longitude,
            bbox.north_latitude,
        ]
    }
}

impl From<(f64, f64, f64, f64)> for BoundingBox {
    fn from(
        (west_longitude, south_latitude, east_longitude, north_latitude): (f64, f64, f64, f64),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

impl From<((f64, f64), (f64, f64))> for BoundingBox {
    /// Creates a `BoundingBox` with two `(longitude, latitude)` pairs.
    ///
    /// The first argument specifies the southwest edge and the latter specifies the northeast edge.
    fn from(
        ((west_longitude, south_latitude), (east_longitude, north_latitude)): (
            (f64, f64),
            (f64, f64),
        ),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

#[cfg(test)]
mod tests {
    mod soundness {
        use slice_of_array::SliceNestExt;

        use super::super::*;

        #[test]
        fn flatten() {
            macro_rules! test {
            ($([$w:expr, $s:expr, $e:expr, $n:expr]),*$(,)?) => {{
                let bb = vec![$(BoundingBox::new($w, $s, $e, $n)),*];
                let ary = vec![$([$w, $s, $e, $n]),*];

                assert_eq!(*BoundingBox::unflatten_slice(&ary), *bb, "unflatten_slice(&{:?})", ary);
                assert_eq!(*BoundingBox::flatten_slice(&bb), *ary, "flatten_slice(&{:?})", bb);

                assert_eq!(BoundingBox::unflatten_vec(ary.clone()), bb, "flatten_vec({:?})", ary);
                assert_eq!(BoundingBox::flatten_vec(bb.clone()), ary, "flatten_vec({:?})", bb);
            }};
        }

            test!();
            test!([-122.75, 36.8, -121.75, 37.8]);
            test!([-122.75, 36.8, -121.75, 37.8], [-74.0, 40.0, -73.0, 41.0]);
        }

        #[test]
        fn unflatten_alignment() {
            static ARRAY: [f64; 5] = [1., 2., 3., 4., 5.];
            assert_eq!(
                BoundingBox::unflatten_slice(&ARRAY[..4].nest()),
                [BoundingBox::new(1., 2., 3., 4.)],
            );
            assert_eq!(
                BoundingBox::unflatten_slice(&ARRAY[1..].nest()),
                [BoundingBox::new(2., 3., 4., 5.)],
            );
        }
    }
}
