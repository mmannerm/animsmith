//! Bounded, index-only raw animation/channel inventory for engine predictions.
//!
//! This is deliberately separate from normalized [`crate::Document`] tracks.
//! One flat candidate sequence gives deserialization a single global N+1
//! budget while retaining source-order animation and channel identities.

use crate::bounded_deserialize::{CappedSequence, deserialize_capped_sequence};
use crate::prediction::RawSourceSetCoverageV1;
use crate::source_facts::SourceFactsViewV1;
use crate::{InputIdentity, SourceFormatV1};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// Immutable raw animation/channel inventory contract identity.
pub const RAW_ANIMATION_CHANNEL_INVENTORY_V1_ID: &str =
    "urn:animsmith:raw-animation-channel-inventory:1";
/// The N+1 candidate prefix retained for one track-support prediction.
pub const RAW_ANIMATION_CHANNEL_INVENTORY_V1_MAX_CANDIDATES: usize = 4_097;

fn deserialize_rows<'de, D>(deserializer: D) -> Result<Vec<RawAnimationChannelRowV1>, D::Error>
where
    D: Deserializer<'de>,
{
    let values: CappedSequence<RawAnimationChannelRowV1> = deserialize_capped_sequence(
        deserializer,
        RAW_ANIMATION_CHANNEL_INVENTORY_V1_MAX_CANDIDATES,
    )?;
    if values.overflowed {
        return Err(D::Error::custom(
            "raw animation/channel inventory exceeded its global candidate bound",
        ));
    }
    Ok(values.values)
}

/// One row in canonical animation-then-channel candidate order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawAnimationChannelRowV1 {
    /// One source animation and its independently observed channel coverage.
    Animation {
        /// Zero-based source animation-array index.
        source_animation_index: u64,
        /// Coverage of this animation's source channel array.
        channel_coverage: RawSourceSetCoverageV1,
    },
    /// One source channel belonging to the immediately preceding animation.
    AnimationChannel {
        /// Zero-based source animation-array index.
        source_animation_index: u64,
        /// Zero-based source channel-array index.
        source_channel_index: u64,
    },
}

impl RawAnimationChannelRowV1 {
    /// Source animation-array index.
    pub const fn source_animation_index(&self) -> u64 {
        match self {
            Self::Animation {
                source_animation_index,
                ..
            }
            | Self::AnimationChannel {
                source_animation_index,
                ..
            } => *source_animation_index,
        }
    }
    /// Source channel-array index for a channel row.
    pub const fn source_channel_index(&self) -> Option<u64> {
        match self {
            Self::Animation { .. } => None,
            Self::AnimationChannel {
                source_channel_index,
                ..
            } => Some(*source_channel_index),
        }
    }
    /// Independent channel coverage carried by an animation row.
    pub const fn channel_coverage(&self) -> Option<RawSourceSetCoverageV1> {
        match self {
            Self::Animation {
                channel_coverage, ..
            } => Some(*channel_coverage),
            Self::AnimationChannel { .. } => None,
        }
    }
}

/// Bounded same-load raw animation/channel inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAnimationChannelInventoryV1 {
    schema: String,
    primary_input: InputIdentity,
    source_format: SourceFormatV1,
    animation_coverage: RawSourceSetCoverageV1,
    source_coverage_complete: bool,
    candidate_prefix_saturated: bool,
    #[serde(deserialize_with = "deserialize_rows")]
    rows: Vec<RawAnimationChannelRowV1>,
}

impl RawAnimationChannelInventoryV1 {
    /// Capture the canonical N+1 subject prefix and aggregate source coverage.
    pub fn from_source(facts: SourceFactsViewV1<'_>) -> Self {
        let mut rows = Vec::new();
        let mut candidate_prefix_saturated = false;
        let mut source_coverage_complete =
            facts.clips().coverage().state() == crate::SourceSetCoverageStateV1::Complete;
        'animations: for animation in facts.clips().rows() {
            if !source_coverage_complete {
                break;
            }
            let channel_coverage: RawSourceSetCoverageV1 = animation.channels().coverage().into();
            source_coverage_complete &=
                channel_coverage.state() == crate::RawSourceSetCoverageStateV1::Complete;
            rows.push(RawAnimationChannelRowV1::Animation {
                source_animation_index: animation.source_clip_index() as u64,
                channel_coverage,
            });
            if rows.len() == RAW_ANIMATION_CHANNEL_INVENTORY_V1_MAX_CANDIDATES {
                candidate_prefix_saturated = true;
                break;
            }
            if !source_coverage_complete {
                break;
            }
            for channel in animation.channels().rows() {
                rows.push(RawAnimationChannelRowV1::AnimationChannel {
                    source_animation_index: animation.source_clip_index() as u64,
                    source_channel_index: channel.source_channel_index() as u64,
                });
                if rows.len() == RAW_ANIMATION_CHANNEL_INVENTORY_V1_MAX_CANDIDATES {
                    candidate_prefix_saturated = true;
                    break 'animations;
                }
            }
        }
        Self {
            schema: RAW_ANIMATION_CHANNEL_INVENTORY_V1_ID.into(),
            primary_input: facts.primary_identity().clone(),
            source_format: facts.format(),
            animation_coverage: facts.clips().coverage().into(),
            source_coverage_complete,
            candidate_prefix_saturated,
            rows,
        }
    }

    /// Validate coverage states, bounded work, and contiguous source ordering.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != RAW_ANIMATION_CHANNEL_INVENTORY_V1_ID
            || self.rows.len() > RAW_ANIMATION_CHANNEL_INVENTORY_V1_MAX_CANDIDATES
            || self.candidate_prefix_saturated
                != (self.rows.len() == RAW_ANIMATION_CHANNEL_INVENTORY_V1_MAX_CANDIDATES)
            || !valid_coverage(self.animation_coverage)
        {
            return Err("invalid raw animation/channel inventory header or bound");
        }
        let mut expected_animation = 0u64;
        let mut current_animation = None;
        let mut expected_channel = 0u64;
        for row in &self.rows {
            match row {
                RawAnimationChannelRowV1::Animation {
                    source_animation_index,
                    channel_coverage,
                } => {
                    if *source_animation_index != expected_animation
                        || !valid_coverage(*channel_coverage)
                    {
                        return Err("raw animation rows are not contiguous or valid");
                    }
                    expected_animation = expected_animation.saturating_add(1);
                    current_animation = Some(*source_animation_index);
                    expected_channel = 0;
                }
                RawAnimationChannelRowV1::AnimationChannel {
                    source_animation_index,
                    source_channel_index,
                } => {
                    if current_animation != Some(*source_animation_index)
                        || *source_channel_index != expected_channel
                    {
                        return Err("raw channel rows are not contiguous or attached");
                    }
                    expected_channel = expected_channel.saturating_add(1);
                }
            }
        }
        let retained_coverage_complete = self.animation_coverage.state()
            == crate::RawSourceSetCoverageStateV1::Complete
            && self.rows.iter().all(|row| {
                row.channel_coverage().is_none_or(|coverage| {
                    coverage.state() == crate::RawSourceSetCoverageStateV1::Complete
                })
            });
        if self.source_coverage_complete != retained_coverage_complete {
            return Err("raw animation/channel aggregate coverage is contradictory");
        }
        Ok(())
    }

    /// Exact primary input identity.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }
    /// Exact source format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }
    /// Animation inventory coverage.
    pub const fn animation_coverage(&self) -> RawSourceSetCoverageV1 {
        self.animation_coverage
    }
    /// Number of retained animation/channel candidates.
    pub fn candidate_count(&self) -> u64 {
        self.rows.len() as u64
    }
    /// Whether source demand exceeded the retained N+1 prefix.
    pub const fn candidate_overflow(&self) -> bool {
        self.candidate_prefix_saturated
    }
    /// Canonical flat candidate prefix.
    pub fn rows(&self) -> &[RawAnimationChannelRowV1] {
        &self.rows
    }
    /// Whether complete coverage proves the source has no animation subjects.
    pub fn is_complete_empty(&self) -> bool {
        self.source_coverage_complete && self.rows.is_empty()
    }
    /// Whether every source animation/channel inventory was observed completely.
    pub const fn source_coverage_complete(&self) -> bool {
        self.source_coverage_complete
    }
}

fn valid_coverage(coverage: RawSourceSetCoverageV1) -> bool {
    matches!(
        (coverage.state(), coverage.reason()),
        (crate::RawSourceSetCoverageStateV1::Complete, None)
            | (crate::RawSourceSetCoverageStateV1::Partial, Some(_))
            | (crate::RawSourceSetCoverageStateV1::Unavailable, Some(_))
    )
}
