//! Geometry object

use serde::de::{Deserialize, Deserializer, Error, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor};
use std::fmt;

/// The Geometry object specified in [The GeoJSON Format (RFC7946)](https://tools.ietf.org/html/rfc7946).
// https://tools.ietf.org/html/rfc7946#section-3.1
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(Position),
    MultiPoint(Vec<Position>),
    LineString(LineString),
    MultiLineString(Vec<LineString>),
    Polygon(Polygon),
    MultiPolygon(Vec<Polygon>),
    // GeometryCollection(Vec<Geometry>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Position(
    /// Longitude
    pub f64,
    /// Latitude
    pub f64,
    // Twitter API does not provide altitudes
);

pub type LineString = Vec<Position>;
pub type Polygon = Vec<LinearRing>;
pub type LinearRing = LineString;

impl<'x> Deserialize<'x> for Geometry {
    fn deserialize<D: Deserializer<'x>>(d: D) -> Result<Self, D::Error> {
        struct GeometryVisitor;

        impl<'x> Visitor<'x> for GeometryVisitor {
            type Value = Geometry;

            fn visit_map<V: MapAccess<'x>>(self, mut v: V) -> Result<Geometry, V::Error> {
                enum Coordinates {
                    F64(f64),
                    Dim0(Position), // Point
                    Dim1(Vec<Position>), // MultiPoint, LineString
                    Dim2(Vec<Vec<Position>>), // MultiLineString, Polygon
                    Dim3(Vec<Vec<Vec<Position>>>), // MultiPolygon
                }

                impl<'x> Deserialize<'x> for Coordinates {
                    fn deserialize<D: Deserializer<'x>>(d: D) -> Result<Self, D::Error> {
                        struct CoordinatesVisitor;

                        impl<'x> Visitor<'x> for CoordinatesVisitor {
                            type Value = Coordinates;

                            fn visit_f64<E>(self, v: f64) -> Result<Coordinates, E> {
                                Ok(Coordinates::F64(v))
                            }

                            fn visit_i64<E>(self, v: i64) -> Result<Coordinates, E> {
                                Ok(Coordinates::F64(v as _))
                            }

                            fn visit_u64<E>(self, v: u64) -> Result<Coordinates, E> {
                                Ok(Coordinates::F64(v as _))
                            }

                            fn visit_seq<V: SeqAccess<'x>>(self, mut v: V) -> Result<Coordinates, V::Error> {
                                macro_rules! match_val {
                                    (
                                        $C:ident,
                                        $($V:ident => $R:ident,)*
                                    ) => {
                                        match v.next_element()? {
                                            Some($C::F64(v1)) => {
                                                let v2 = match v.next_element()? {
                                                    Some(val) => val,
                                                    None => return Err(V::Error::invalid_length(1, &self)),
                                                };
                                                while v.next_element::<IgnoredAny>()?.is_some() {}
                                                Ok($C::Dim0(Position(v1, v2)))
                                            },
                                            $(Some($C::$V(val)) => {
                                                let mut ret = v.size_hint().map_or_else(Vec::new, Vec::with_capacity);
                                                ret.push(val);
                                                while let Some(val) = v.next_element()? {
                                                    ret.push(val);
                                                }
                                                Ok($C::$R(ret))
                                            },)*
                                            Some($C::Dim3(_)) => Err(V::Error::invalid_type(Unexpected::Seq, &self)),
                                            None => Ok($C::Dim1(Vec::new())),
                                        }
                                    };
                                }

                                match_val! {
                                    Coordinates,
                                    Dim0 => Dim1,
                                    Dim1 => Dim2,
                                    Dim2 => Dim3,
                                }
                            }

                            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                                write!(f,
                                    "a floating point number \
                                    or an array of floating point number with a depth of 4 or lower"
                                )
                            }
                        }

                        d.deserialize_any(CoordinatesVisitor)
                    }
                }

                let mut c = None;

                use self::Geometry::*;

                macro_rules! end {
                    () => {{
                        while v.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                    }};
                }

                while let Some(k) = v.next_key::<String>()? {
                    match k.as_str() {
                        "type" => {
                            let t = v.next_value::<String>()?;
                            macro_rules! match_type {
                                ($C:ident, $($($V:ident($typ:expr))|* => $D:ident,)*) => {{
                                    const EXPECTED: &'static [&'static str] = &[$($($typ),*),*];
                                    match t.as_str() {
                                        $($($typ => match c {
                                            Some($C::$D(val)) => {
                                                end!();
                                                return Ok($V(val));
                                            },
                                            None => {
                                                while let Some(k) = v.next_key::<String>()? {
                                                    if "coordinates" == k.as_str() {
                                                        let c = v.next_value()?;
                                                        end!();
                                                        return Ok($V(c));
                                                    } else {
                                                        v.next_value::<IgnoredAny>()?;
                                                    }
                                                }
                                                return Err(V::Error::missing_field("coordinates"));
                                            },
                                            _ => return Err(V::Error::custom("invalid coordinates type")),
                                        },)*)*
                                        s => return Err(V::Error::unknown_variant(&s, EXPECTED)),
                                    }
                                }};
                            }
                            match_type! {
                                Coordinates,
                                Point("Point") => Dim0,
                                MultiPoint("MultiPoint") | LineString("LineString") => Dim1,
                                MultiLineString("MultiLineString") | Polygon("Polygon") => Dim2,
                                MultiPolygon("MultiPolygon") => Dim3,
                            }
                        },
                        "coordinates" => c = Some(v.next_value()?),
                        _ => { v.next_value::<IgnoredAny>()?; },
                    }
                }

                Err(V::Error::missing_field("type"))
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map with `type` and `coordinate` fields")
            }
        }

        d.deserialize_map(GeometryVisitor)
    }
}

#[cfg(test)]
mod tests {
    use json;
    use super::*;

    #[test]
    fn deserialize_pass() {
        macro_rules! assert_deserialize_to {
            ($typ:ident($($inner:tt)*), $c:expr) => {{
                let geo = Geometry::$typ($($inner)*);
                let c = format!("\"coordinates\":{}", $c);
                let t = format!("\"type\":\"{}\"", stringify!($typ));
                assert_eq!(geo, json::from_str(&format!("{{{},{}}}", c, t)).unwrap());
                assert_eq!(geo, json::from_str(&format!("{{{},{}}}", t, c)).unwrap());
                assert_eq!(geo, json::from_str(&format!("{{\"1\":0,{},\"2\":[],{},\"3\":null}}", c, t)).unwrap());
                assert_eq!(geo, json::from_str(&format!("{{\"1\":0,{},\"2\":[],{},\"3\":null}}", t, c)).unwrap());
            }};
        }

        assert_deserialize_to!(
            Point(Position(-75.14310264, 40.05701649)),
            "[-75.14310264,40.05701649]"
        );
        assert_deserialize_to!(
            Polygon(vec![vec![Position(2.2241006,48.8155414), Position(2.4699099,48.8155414),
                Position(2.4699099,48.9021461), Position(2.2241006,48.9021461)]]),
            "[[[2.2241006,48.8155414],[2.4699099,48.8155414],[2.4699099,48.9021461],[2.2241006,48.9021461]]]"
        );
    }

    #[test]
    fn deserialize_fail() {
        macro_rules! assert_fail {
            ($json:expr) => {{
                json::from_str::<::serde::de::IgnoredAny>($json)
                    .expect("invalid JSON: this is a test bug");
                json::from_str::<Geometry>($json).unwrap_err();
            }};
        }

        assert_fail!("{}");
        assert_fail!("[0,1]");
        assert_fail!("{\"coordinates\":[1],\"type\":\"Point\"}");
        assert_fail!("{\"coordinates\":[1,2],\"type\":\"MultiPoint\"}");
        assert_fail!("{\"coordinates\":[[[[[0,0]]]]],\"type\":\"MultiPolygon\"}");
        assert_fail!("{\"coordinates\":[],\"type\":\"Foo\"}");
    }
}
