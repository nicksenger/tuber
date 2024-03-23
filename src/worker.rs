use web_sys::MessagePort;

use crate::{error::HostRuntimeError, Error, Tube, Encode, Decode};

pub struct Builder {
    name: String,
}

impl Builder {
    pub fn with_name(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    pub fn build<In, Out>(self) -> Result<Tube<In, Out>, Error>
    where
        In: Encode + Decode,
        Out: Encode + Decode + 'static,
    {
        crate::task::initialize()?;

        let message_port = unsafe {
            crate::util::js::from_global::<MessagePort>(&format!(
                "__tuber_message_port_{}",
                self.name
            ))?
        };
        let tube = Tube::<In, Out>::try_from_message_port_spawned(&message_port)?;

        Ok(tube)
    }
}
