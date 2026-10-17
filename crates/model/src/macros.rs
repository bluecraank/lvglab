/// Declares a plain enum that mirrors a family of LVGL C constants.
///
/// Every variant maps to a constant suffix (e.g. `CENTER` for
/// `LV_ALIGN_CENTER`) and a human readable label. The enum serializes as its
/// suffix, which keeps project files readable and stable.
macro_rules! lv_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident : $prefix:literal {
            $($(#[$vmeta:meta])* $variant:ident = $suffix:literal, $label:literal;)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis enum $name { $($(#[$vmeta])* $variant),+ }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const C_PREFIX: &'static str = $prefix;

            /// The C constant without its family prefix.
            pub const fn suffix(self) -> &'static str {
                match self { $(Self::$variant => $suffix),+ }
            }

            pub const fn label(self) -> &'static str {
                match self { $(Self::$variant => $label),+ }
            }

            /// The full C constant, e.g. `LV_ALIGN_CENTER`.
            pub fn c_const(self) -> String {
                format!("{}{}", $prefix, self.suffix())
            }

            /// Parses a suffix or a full constant, ignoring ASCII case.
            pub fn from_suffix(text: &str) -> Option<Self> {
                let text = text.trim();
                let text = text.strip_prefix($prefix).unwrap_or(text);
                Self::ALL.iter().copied().find(|v| v.suffix().eq_ignore_ascii_case(text))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.label())
            }
        }

        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.suffix())
            }
        }

        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = <String as $crate::__private::serde::Deserialize>::deserialize(d)?;
                Self::from_suffix(&text).ok_or_else(|| {
                    <D::Error as $crate::__private::serde::de::Error>::custom(
                        format!(concat!("unknown ", stringify!($name), " `{}`"), text),
                    )
                })
            }
        }
    };
}
