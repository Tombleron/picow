macro_rules! define_commands {
    (
        $(
            $(#[$attr:meta])*
            $name:ident = $code:expr, $size:expr $(,)?
        )*
    ) => {
        #[derive(Debug, Clone, Copy)]
        #[repr(u8)]
        pub enum CommandType {
            $(
                $(#[$attr])*
                $name = $code,
            )*
        }

        impl CommandType {
            const fn max_payload_size(&self) -> u8 {
                match self {
                    $(
                        Self::$name => $size,
                    )*
                }
            }
        }

        impl TryFrom<u8> for CommandType {
            type Error = ();

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $(
                        $code => Ok(Self::$name),
                    )*
                    _ => Err(()),
                }
            }
        }

        impl defmt::Format for CommandType {
            fn format(&self, fmt: defmt::Formatter) {
                match self {
                    $(
                        Self::$name => defmt::write!(fmt, "{}", stringify!($name)),
                    )*
                }
            }
        }
    };
}

pub(crate) use define_commands;
