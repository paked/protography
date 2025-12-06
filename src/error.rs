use crate::pmtiles::PmtilesError;

pub type Result<T> = std::result::Result<T, ProtographyError>;

#[derive(Debug)]
pub enum ProtographyError {
    GenericError,
    PmtilesError(PmtilesError),
}

impl From<PmtilesError> for ProtographyError {
    fn from(value: PmtilesError) -> Self {
        ProtographyError::PmtilesError(value)
    }
}
