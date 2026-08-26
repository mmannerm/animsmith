use serde::de::{DeserializeSeed, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::marker::PhantomData;

struct CappedStringVisitor {
    limit: usize,
}

impl CappedStringVisitor {
    fn retain<E>(self, value: &str) -> Result<String, E>
    where
        E: serde::de::Error,
    {
        if value.len() > self.limit {
            return Err(E::custom(format!(
                "string exceeds bounded UTF-8 byte limit of {}",
                self.limit
            )));
        }
        Ok(value.to_owned())
    }

    fn retain_owned<E>(self, value: String) -> Result<String, E>
    where
        E: serde::de::Error,
    {
        if value.len() > self.limit {
            return Err(E::custom(format!(
                "string exceeds bounded UTF-8 byte limit of {}",
                self.limit
            )));
        }
        Ok(value)
    }
}

impl<'de> Visitor<'de> for CappedStringVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a UTF-8 string with at most {} bytes",
            self.limit
        )
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.retain(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.retain(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.retain_owned(value)
    }
}

pub(crate) fn deserialize_capped_string<'de, D>(
    deserializer: D,
    limit: usize,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(CappedStringVisitor { limit })
}

pub(crate) fn deserialize_capped_option_string<'de, D>(
    deserializer: D,
    limit: usize,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalCappedStringVisitor {
        limit: usize,
    }

    impl<'de> Visitor<'de> for OptionalCappedStringVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "null or a UTF-8 string with at most {} bytes",
                self.limit
            )
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_capped_string(deserializer, self.limit).map(Some)
        }
    }

    deserializer.deserialize_option(OptionalCappedStringVisitor { limit })
}

#[derive(Debug)]
pub(crate) struct CappedSequence<T> {
    pub(crate) values: Vec<T>,
    pub(crate) overflowed: bool,
}

struct CappedSequenceVisitor<T> {
    limit: usize,
    element: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for CappedSequenceVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = CappedSequence<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence with at most {} elements", self.limit)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.limit));
        while values.len() < self.limit {
            let Some(value) = sequence.next_element()? else {
                return Ok(CappedSequence {
                    values,
                    overflowed: false,
                });
            };
            values.push(value);
        }
        let overflowed = sequence.next_element::<IgnoredAny>()?.is_some();
        if overflowed {
            while sequence.next_element::<IgnoredAny>()?.is_some() {}
        }
        Ok(CappedSequence { values, overflowed })
    }
}

pub(crate) fn deserialize_capped_sequence<'de, D, T>(
    deserializer: D,
    limit: usize,
) -> Result<CappedSequence<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(CappedSequenceVisitor {
        limit,
        element: PhantomData,
    })
}

pub(crate) struct CappedSequenceSeed<T> {
    pub(crate) limit: usize,
    pub(crate) element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for CappedSequenceSeed<T>
where
    T: Deserialize<'de>,
{
    type Value = CappedSequence<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_capped_sequence(deserializer, self.limit)
    }
}

#[derive(Debug)]
pub(crate) struct RowBudget {
    limit: usize,
    retained: usize,
    overflowed: bool,
}

pub(crate) fn consume_ignored_tail<'de, A>(
    sequence: &mut A,
    mut seen: usize,
    local_limit: usize,
) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    while seen < local_limit {
        if sequence.next_element::<IgnoredAny>()?.is_none() {
            return Ok(false);
        }
        seen += 1;
    }
    let overflowed = sequence.next_element::<IgnoredAny>()?.is_some();
    if overflowed {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
    }
    Ok(overflowed)
}

impl RowBudget {
    pub(crate) const fn new(limit: usize) -> Self {
        Self {
            limit,
            retained: 0,
            overflowed: false,
        }
    }

    pub(crate) fn admit(&mut self) -> bool {
        if self.overflowed {
            return false;
        }
        if self.retained == self.limit {
            self.overflowed = true;
            return false;
        }
        self.retained += 1;
        true
    }

    pub(crate) const fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub(crate) const fn found(&self) -> usize {
        if self.overflowed {
            self.limit + 1
        } else {
            self.retained
        }
    }
}

enum BudgetedElement<T> {
    Value(T),
    Skipped,
}

struct BudgetedElementSeed<'a, T> {
    budget: &'a mut RowBudget,
    element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for BudgetedElementSeed<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = BudgetedElement<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.budget.admit() {
            T::deserialize(deserializer).map(BudgetedElement::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| BudgetedElement::Skipped)
        }
    }
}

pub(crate) struct BudgetedCappedSequenceSeed<'a, T> {
    pub(crate) budget: &'a mut RowBudget,
    pub(crate) local_limit: usize,
    pub(crate) element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for BudgetedCappedSequenceSeed<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = CappedSequence<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BudgetedSequenceVisitor<'a, T> {
            budget: &'a mut RowBudget,
            local_limit: usize,
            element: PhantomData<fn() -> T>,
        }

        impl<'de, T> Visitor<'de> for BudgetedSequenceVisitor<'_, T>
        where
            T: Deserialize<'de>,
        {
            type Value = CappedSequence<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    "a sequence with at most {} elements",
                    self.local_limit
                )
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values =
                    Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.local_limit));
                let mut seen = 0usize;
                while seen < self.local_limit {
                    let Some(element) = sequence.next_element_seed(BudgetedElementSeed {
                        budget: self.budget,
                        element: PhantomData,
                    })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        BudgetedElement::Value(value) => values.push(value),
                        BudgetedElement::Skipped => {
                            let overflowed =
                                consume_ignored_tail(&mut sequence, seen, self.local_limit)?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed = consume_ignored_tail(&mut sequence, seen, self.local_limit)?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(BudgetedSequenceVisitor {
            budget: self.budget,
            local_limit: self.local_limit,
            element: PhantomData,
        })
    }
}
