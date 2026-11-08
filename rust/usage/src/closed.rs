//! `closed!`: a closed enum whose values are fixed snake_case names, the
//! only strings a packet carries. Each gets `ALL`, `name`, `from_name`
//! and serde as its name; an unknown name never deserializes into one.

macro_rules! closed {
    ($(#[$m:meta])* pub enum $name:ident { $($(#[$vm:meta])* $v:ident = $s:literal,)+ }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$vm])* $v,)+
        }

        impl $name {
            /// Every value, in declaration order.
            pub const ALL: &'static [$name] = &[$($name::$v,)+];

            /// Its fixed name, what the packet carries.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$v => $s,)+
                }
            }

            /// The value of a name, None for any other string.
            pub fn from_name(s: &str) -> Option<$name> {
                Self::ALL.iter().copied().find(|k| k.name() == s)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.name())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<$name, D::Error> {
                let s = String::deserialize(d)?;
                $name::from_name(&s).ok_or_else(|| serde::de::Error::custom(concat!("not a ", stringify!($name))))
            }
        }
    };
}
