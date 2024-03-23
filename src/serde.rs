pub trait Encode {
    fn encode(&self) -> Result<Vec<u8>, std::io::Error>;
}

pub trait Decode
where
    Self: Sized,
{
    fn decode(bytes: &[u8]) -> Result<Self, std::io::Error>;
}

pub mod codec {
    use std::{io, marker::PhantomData, pin::Pin};

    use ::tokio_serde::{Deserializer, Serializer};
    use bytes::{Bytes, BytesMut};
    use educe::Educe;

    use super::{Decode, Encode};

    #[derive(Educe)]
    #[educe(Debug)]
    pub struct Tuber<Item, SinkItem> {
        #[educe(Debug(ignore))]
        ghost: PhantomData<(Item, SinkItem)>,
    }

    impl<Item, SinkItem> Default for Tuber<Item, SinkItem> {
        fn default() -> Self {
            Tuber { ghost: PhantomData }
        }
    }

    pub type SymmetricalTuber<T> = Tuber<T, T>;

    impl<Item, SinkItem> Deserializer<Item> for Tuber<Item, SinkItem>
    where
        Item: Decode,
    {
        type Error = io::Error;

        fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> Result<Item, Self::Error> {
            Item::decode(src)
        }
    }

    impl<Item, SinkItem> Serializer<SinkItem> for Tuber<Item, SinkItem>
    where
        SinkItem: Encode,
    {
        type Error = io::Error;

        fn serialize(self: Pin<&mut Self>, item: &SinkItem) -> Result<Bytes, Self::Error> {
            item.encode().map(Into::into)
        }
    }
}

#[cfg(all(feature = "bincode", not(feature = "bitcode")))]
mod bin {
    use super::*;
    use serde::{de::DeserializeOwned, Serialize};
    use std::io;

    impl<T> Encode for T
    where
        T: Serialize,
    {
        fn encode(&self) -> Result<Vec<u8>, std::io::Error> {
            ::bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
    }

    impl<T> Decode for T
    where
        T: DeserializeOwned,
    {
        fn decode(bytes: &[u8]) -> Result<Self, std::io::Error> {
            ::bincode::deserialize(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
    }
}

#[cfg(feature = "bitcode")]
mod bit {
    use super::*;
    use std::io;

    impl<T> Encode for T
    where
        T: bitcode::Encode,
    {
        fn encode(&self) -> Result<Vec<u8>, std::io::Error> {
            Ok(bitcode::encode(self))
        }
    }

    impl<T> Decode for T
    where
        T: bitcode::DecodeOwned,
    {
        fn decode(bytes: &[u8]) -> Result<Self, std::io::Error> {
            bitcode::decode(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
    }
}
