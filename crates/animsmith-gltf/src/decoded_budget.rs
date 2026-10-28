//! Per-load admission for decoded accessor arrays, independent of source bytes.

use crate::LoadError;

/// Provisional engineering ceiling, not a format limit or total RSS bound.
/// Every decoded element type used here is at least four bytes, so this also
/// bounds aggregate iteration to 67,108,864 elements (below sparse u32 counters).
/// The ceiling has not been validated against licensed production assets.
const MAX_BYTES: usize = 256 * 1024 * 1024;

pub(crate) struct DecodedAccessorBudget {
    bytes: usize,
    max_bytes: usize,
}

impl Default for DecodedAccessorBudget {
    fn default() -> Self {
        Self::new(MAX_BYTES)
    }
}

impl DecodedAccessorBudget {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            bytes: 0,
            max_bytes,
        }
    }

    /// Charge before iterating/collecting or cloning a decoded array. Optional
    /// readers may first establish availability with constant-space slice checks.
    /// Use the destination type: normalized integers may widen on decoding.
    /// Charges are cumulative, including repeated references and discarded data.
    pub(crate) fn admit<T>(
        &mut self,
        accessor: usize,
        count: usize,
        location: &str,
    ) -> Result<(), LoadError> {
        let bytes = count.checked_mul(std::mem::size_of::<T>());
        let total_bytes = bytes.and_then(|bytes| self.bytes.checked_add(bytes));
        match (total_bytes, bytes) {
            (Some(total), Some(bytes))
                if total <= self.max_bytes && bytes <= isize::MAX as usize =>
            {
                self.bytes = total;
                Ok(())
            }
            _ => Err(LoadError::DecodedAccessorBudget {
                accessor,
                location: location.to_owned(),
                problem: format!(
                    "requested {count} elements of {} decoded bytes each after {} bytes; limit {} bytes (checked allocation arithmetic)",
                    std::mem::size_of::<T>(),
                    self.bytes,
                    self.max_bytes,
                ),
            }),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
