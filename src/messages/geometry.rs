use serde::de::{Deserialize, Deserializer, Error, MapVisitor, SeqVisitor, Type, Visitor};
use serde::de::impls::IgnoredAny;

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

impl Deserialize for Geometry {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        struct GeometryVisitor;

        impl Visitor for GeometryVisitor {
            type Value = Geometry;

            fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<Geometry, V::Error> {
                enum Coordinates {
                    F64(f64),
                    Dim0(Position), // Point
                    Dim1(Vec<Position>), // MultiPoint, LineString
                    Dim2(Vec<Vec<Position>>), // MultiLineString, Polygon
                    Dim3(Vec<Vec<Vec<Position>>>), // MultiPolygon
                }

                impl Deserialize for Coordinates {
                    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
                        struct CoordinatesVisitor;

                        impl Visitor for CoordinatesVisitor {
                            type Value = Coordinates;

                            fn visit_f64<E>(&mut self, v: f64) -> Result<Coordinates, E> {
                                Ok(Coordinates::F64(v))
                            }

                            fn visit_seq<V: SeqVisitor>(&mut self, mut v: V) -> Result<Coordinates, V::Error> {
                                macro_rules! match_val {
                                    (
                                        $C:ident,
                                        $($V:ident => $R:ident,)*
                                    ) => {
                                        match v.visit()? {
                                            Some($C::F64(v1)) => {
                                                let v2 = match v.visit()? {
                                                    Some(val) => val,
                                                    None => return Err(V::Error::invalid_length(1)),
                                                };
                                                while let Some(_) = v.visit::<IgnoredAny>()? {}
                                                v.end()?;
                                                Ok($C::Dim0(Position(v1, v2)))
                                            },
                                            $(Some($C::$V(val)) => {
                                                let mut ret = Vec::with_capacity(v.size_hint().0 + 1);
                                                ret.push(val);
                                                while let Some(val) = v.visit()? {
                                                    ret.push(val);
                                                }
                                                v.end()?;
                                                Ok($C::$R(ret))
                                            },)*
                                            Some($C::Dim3(_)) => Err(V::Error::invalid_type(Type::Seq)),
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
                        }

                        d.deserialize_seq(CoordinatesVisitor)
                    }
                }

                let mut c = None;

                use self::Geometry::*;

                macro_rules! end {
                    () => {{
                        while let Some(_) = v.visit::<IgnoredAny, IgnoredAny>()? {}
                        v.end()?;
                    }};
                }

                while let Some(k) = v.visit_key::<String>()? {
                    match k.as_str() {
                        "type" => {
                            let t = v.visit_value::<String>()?;
                            macro_rules! match_type {
                                ($C:ident, $($($V:ident($typ:expr))|* => $D:ident,)*) => {
                                    match t.as_str() {
                                        $($($typ => match c {
                                            Some($C::$D(val)) => {
                                                end!();
                                                return Ok($V(val));
                                            },
                                            None => {
                                                while let Some(k) = v.visit_key::<String>()? {
                                                    match k.as_str() {
                                                        "coordinates" => {
                                                            end!();
                                                            return Ok($V(v.visit_value()?));
                                                        },
                                                        _ => { v.visit_value::<IgnoredAny>()?; },
                                                    }
                                                }
                                                return Err(V::Error::missing_field("coordinates"));
                                            },
                                            _ => return Err(V::Error::custom("invalid coordinates type")),
                                        },)*)*
                                        s => return Err(V::Error::unknown_variant(&s)),
                                    }
                                };
                            }
                            match_type! {
                                Coordinates,
                                Point("Point") => Dim0,
                                MultiPoint("MultiPoint") | LineString("LineString") => Dim1,
                                MultiLineString("MultiLineString") | Polygon("Polygon") => Dim2,
                                MultiPolygon("MultiPolygon") => Dim3,
                            }
                        },
                        "coordinates" => c = Some(v.visit_value()?),
                        _ => { v.visit_value::<IgnoredAny>()?; },
                    }
                }

                v.missing_field("type")
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
    fn deserialize() {
        assert_eq!(
            Geometry::Point(Position(-75.14310264, 40.05701649)),
            json::from_str("{\"coordinates\":[-75.14310264,40.05701649],\"type\":\"Point\"}").unwrap()
        );
        assert_eq!(
            Geometry::Polygon(vec![vec![Position(2.2241006,48.8155414), Position(2.4699099,48.8155414), 
                Position(2.4699099,48.9021461), Position(2.2241006,48.9021461)]]),
            json::from_str("{\"coordinates\":[
                [[2.2241006,48.8155414],[2.4699099,48.8155414],[2.4699099,48.9021461],[2.2241006,48.9021461]]
                ],\"type\":\"Polygon\"}").unwrap()
        );
    }
}
