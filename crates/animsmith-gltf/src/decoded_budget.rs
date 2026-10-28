//! Per-load admission for decoded accessor arrays, independent of source bytes.

use crate::LoadError;

/// Engineering safety ceilings, not format limits or a bound on total RSS.
/// Counting elements also bounds sparse iteration when most values are implicit.
const MAX_ELEMENTS: usize = 16_777_216;
const MAX_BYTES: usize = 256 * 1024 * 1024;

pub(crate) struct DecodedAccessorBudget {
    elements: usize,
    bytes: usize,
    max_elements: usize,
    max_bytes: usize,
}

impl Default for DecodedAccessorBudget {
    fn default() -> Self {
        Self::new(MAX_ELEMENTS, MAX_BYTES)
    }
}

impl DecodedAccessorBudget {
    pub(crate) fn new(max_elements: usize, max_bytes: usize) -> Self {
        Self {
            elements: 0,
            bytes: 0,
            max_elements,
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
        let elements = self.elements.checked_add(count);
        let total_bytes = bytes.and_then(|bytes| self.bytes.checked_add(bytes));
        match (elements, total_bytes, bytes) {
            (Some(elements), Some(total), Some(bytes))
                if elements <= self.max_elements
                    && total <= self.max_bytes
                    && bytes <= isize::MAX as usize =>
            {
                self.elements = elements;
                self.bytes = total;
                Ok(())
            }
            _ => Err(LoadError::DecodedAccessorBudget {
                accessor,
                location: location.to_owned(),
                problem: format!(
                    "requested {count} elements of {} decoded bytes each after {} elements / {} bytes; limits {} elements / {} bytes (checked allocation arithmetic)",
                    std::mem::size_of::<T>(),
                    self.elements,
                    self.bytes,
                    self.max_elements,
                    self.max_bytes,
                ),
            }),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
