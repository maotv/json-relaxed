//! Fault tolerant handcrafted Json Deserialization



use serde_json::{json, Value, value::Index};
use std::{error::Error, fmt};

macro_rules! s {
    // use s! instead of String::from
    ($expression:expr) => {
        String::from($expression)
    };
}

#[derive(Debug)]
pub struct FromJsonError {
    msg: String
}


impl FromJsonError {
    pub fn with_message(message: &str) -> Self {
        FromJsonError {
            msg: String::from(message)
        }
    }
    fn unexpected() -> Self {
        FromJsonError {
            msg: String::from("unexpected error")
        }
    }
}

impl Error for FromJsonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}



impl fmt::Display for FromJsonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.msg)
    }
}


pub trait TryFromJson: Sized {
    fn try_from_json(value: &Value) -> Result<Self,FromJsonError>;
}

pub trait MaybeValue {

    fn maybe_bool<I: Index>(&self, key: I) -> Maybe<bool>;
    fn maybe_int<I: Index>(&self, key: I) -> Maybe<i64>;
    fn maybe_uint<I: Index>(&self, key: I) -> Maybe<u64>;
    fn maybe_string<I: Index>(&self, key: I) -> Maybe<String>;
    fn maybe_array<T: TryFromJson, I: Index>(&self, key: I) -> Maybe<Vec<T>>;
    fn maybe_object<T: TryFromJson, I: Index>(&self, key: I) -> Maybe<T>;

}
/// The result of getting a typed value from a json array or object
pub enum Maybe<T> 
{
    /// The value does not exist or is explicitly set to null
    Null,
    /// The value is available and is given strictly as a type T
    Strict(T),
    /// The value can be interpreted as type T, eg 1 for true or "23" for 23
    Relaxed(T),
    /// The value could not be read (parse error, array for bool, ...) 
    Error(FromJsonError),
//    Debug(T)
}

impl <T> Maybe<T> 
{
    pub fn strict(self) -> Option<T>{
        match self {
            Maybe::Strict(v) => Some(v),
            _ => None
        }
    }

    pub fn strict_ok(self) -> Result<T,FromJsonError>{
        match self {
            Maybe::Strict(v) => Ok(v),
            Maybe::Error(e) => Err(e),
            _ => Err(FromJsonError::with_message("no strict value"))
        }
    }

    pub fn relaxed(self) -> T 
        where T: Default
    {
        match self {
            Maybe::Null => Default::default(),
            Maybe::Error(_) => Default::default(),
            Maybe::Strict(v) => v,
            Maybe::Relaxed(v) => v
        }
    }

    pub fn default(self, dflt: impl Into<T>) -> T {
        match self {
            Maybe::Null => dflt.into(),
            Maybe::Error(_) => dflt.into(),
            Maybe::Strict(v) => v,
            Maybe::Relaxed(v) => v
        }
    }

    pub fn default_for_null(self, dflt: impl Into<T>) -> Option<T> {
        match self {
            Maybe::Strict(v) => Some(v),
            Maybe::Null => Some(dflt.into()),
            _ => None
        }
    }

}






impl MaybeValue for Value {



    fn maybe_object<T: TryFromJson, I: Index>(&self, key: I) -> Maybe<T> {

        match self.get(key) {

            Some(v) => {
                let t: Result<T,_> = T::try_from_json(v);
                match t {
                    Ok(tv) => Maybe::Strict(tv),
                    Err(e) => Maybe::Error(e)
                }
            },

            None => Maybe::Null

        }
    }


    fn maybe_array<T: TryFromJson, I: Index>(&self, key: I) -> Maybe<Vec<T>> {
        
        match self.get(key) {
            Some(Value::Array(a)) => {

                // Maybe::Null
                let mut collect: Vec<T> = Vec::new();
                let mut clean = true;
                for i in a.iter().map(|i| T::try_from_json(i)) {
                    match i {
                        Ok(v) => collect.push(v),
                        Err(_) => {
                            clean = false;
                        }
                    }
                }

                match clean {
                    true => Maybe::Strict(collect),
                    false => Maybe::Relaxed(collect)
                }

            },
            Some(v) => {
                match T::try_from_json(v) {
                    Ok(t) => Maybe::Relaxed(vec!(t)),
                    Err(e) => Maybe::Error(e)
                }
            },
            None => {
                Maybe::Null
            }
        }



    }

    fn maybe_string<I: Index>(&self, key: I) -> Maybe<String> {


        match self.get(key) {
            Some(Value::Null) => Maybe::Null,
            Some(Value::Bool(b)) => Maybe::Relaxed(b.to_string()),
            Some(Value::Number(n)) => Maybe::Relaxed(n.to_string()),
            Some(Value::String(s)) => Maybe::Strict(s!(s)),
            Some(Value::Array(_)) => Maybe::Error(FromJsonError::with_message("type mismatch: array")),
            Some(Value::Object(_)) => Maybe::Error(FromJsonError::with_message("type mismatch: object")),
            None => Maybe::Null
        }
    
    
    }
    fn maybe_bool<I: Index>(&self, key: I) -> Maybe<bool> {
        match self.get(key) {
            Some(Value::Null) => Maybe::Null,
            Some(Value::Bool(b)) => Maybe::Strict(*b),
            Some(Value::Number(n)) => {
                if n.is_i64() {
                    Maybe::Relaxed(n.as_i64().expect("checked above") != 0)
                } else if  n.is_u64() {
                    Maybe::Relaxed(n.as_u64().expect("checked above") > 0)
                } else if n.is_f64() {
                    Maybe::Relaxed(n.as_f64().expect("checked above") != 0.0)
                } else {
                    Maybe::Error(FromJsonError::unexpected())
                }
            },
            Some(Value::String(s)) => {
                Maybe::Relaxed( s != "" && s != "0" && s.to_lowercase() != "false" )
            },
            Some(Value::Array(_)) => Maybe::Error(FromJsonError::with_message("type mismatch: array")),
            Some(Value::Object(_)) => Maybe::Error(FromJsonError::with_message("type mismatch: object")),
            None => Maybe::Null
        }
    }


    fn maybe_uint<I: Index>(&self, key: I) -> Maybe<u64> {
        match self.maybe_int(key) {
            Maybe::Strict(n) => Maybe::Strict(n as u64),
            Maybe::Relaxed(n) => Maybe::Relaxed(n as u64),
            Maybe::Error(e) => Maybe::Error(e),
            Maybe::Null => Maybe::Null,
        }
    }

    fn maybe_int<I: Index>(&self, key: I) -> Maybe<i64> {
        match self.get(key) {
            Some(Value::Null) => Maybe::Null,
            Some(Value::Bool(b)) => {
                match b {
                    true  => Maybe::Relaxed(1),
                    false => Maybe::Relaxed(0)
                }
            },
            Some(Value::Number(n)) => {
                if n.is_i64() {
                    Maybe::Strict(n.as_i64().expect("checked above"))
                } else if  n.is_u64() {
                    Maybe::Strict(n.as_u64().expect("checked above") as i64)
                } else if n.is_f64() {
                    Maybe::Relaxed(n.as_f64().expect("checked above") as i64)
                } else {
                    Maybe::Error(FromJsonError::unexpected())
                }
            },
            Some(Value::String(s)) => {
                let n = s.parse::<i64>();
                match n {
                    Ok(i) => Maybe::Relaxed(i),
                    Err(_) => Maybe::Error(FromJsonError::with_message("parseIntError"))
                }
            },
            Some(Value::Array(_)) => Maybe::Error(FromJsonError::with_message("type mismatch: array")),
            Some(Value::Object(_)) => Maybe::Error(FromJsonError::with_message("type mismatch: object")),
            None => Maybe::Null
        }
    }
}



// struct DummyData {
//     index: i64,
//     name: String
// }


// fn decode_setup_data(data: &Value) -> Result<(),DecypherError> {

//     let x = DummyData {
//         index: data.maybe_int("index").default(12),
//         name: data.maybe_string("name").relaxed()
//     };

//     Ok(())
// }



#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{FromJsonError, MaybeValue};

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn basic_test() -> Result<(),FromJsonError> {

        let json = json!(
            {
                "foo": 23,
                "bar": "42"
            }
        );

        assert_eq!(json.maybe_int("foo").strict(), Some(23));
        assert_eq!(json.maybe_string("foo").relaxed(), "23");
        assert_eq!(json.maybe_int("bar").relaxed(), 42);
        assert_eq!(json.maybe_string("bar").relaxed(), "42");
        Ok(())
    }


}
