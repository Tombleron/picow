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
    };
}

pub(crate) use define_commands;
