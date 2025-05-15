use anyhow::Result;
use erased_serde::Serialize as ErasedSerialize;
use serde::Serialize;

pub trait SerializeAsAny: ErasedSerialize {}
impl<T: ErasedSerialize> SerializeAsAny for T {}

#[derive(Serialize)]
pub struct ErrorOutput {
    pub error: String,
}

pub fn serialize_output(data: &dyn SerializeAsAny) -> Result<String> {
    let mut buf = Vec::new();
    {
        let mut serializer = serde_json::Serializer::pretty(&mut buf);
        let mut erased_serializer = <dyn erased_serde::Serializer>::erase(&mut serializer);
        data.erased_serialize(&mut erased_serializer)?;
    }
    Ok(String::from_utf8(buf)?)
}

pub fn serialize_error(e: &anyhow::Error) -> String {
    let error = e.to_string();
    let error_response = ErrorOutput { error };
    serialize_output(&error_response).unwrap_or_else(|_| format!("{{\"error\": \"{}\"}}", e))
}
