use crate::message::{MessageOutput, MessageSegment, RenderOutput, SegmentType};

use super::super::RenderOptions;

pub(super) fn build_messages(
    header_text: String,
    segments: Vec<MessageSegment>,
    options: &RenderOptions,
) -> Vec<RenderOutput> {
    if should_bundle(&header_text, &segments, options) {
        return build_bundled_messages(header_text, segments);
    }

    let mut messages = vec![RenderOutput::Message(MessageOutput::from_text(header_text))];
    messages.extend(
        segments
            .into_iter()
            .map(|segment| RenderOutput::Message(MessageOutput::from_segment(segment))),
    );
    messages
}

fn should_bundle(header_text: &str, segments: &[MessageSegment], options: &RenderOptions) -> bool {
    if options.forward_contents {
        return true;
    }

    // Thresholds are character counts (see `split_text_segments`), not byte
    // lengths — byte counting would trip the threshold ~3x early for CJK.
    let ordered_text_len: usize = segments
        .iter()
        .filter(|segment| matches!(segment.segment_type, SegmentType::Text))
        .map(|segment| {
            segment
                .text
                .as_deref()
                .map_or(0, |text| text.chars().count())
        })
        .sum();
    let total_text_len = header_text.chars().count() + ordered_text_len;
    let non_text_segments = segments
        .iter()
        .filter(|segment| !matches!(segment.segment_type, SegmentType::Text))
        .count();

    (options.forward_text_threshold > 0 && total_text_len > options.forward_text_threshold)
        || (options.forward_media_threshold > 0
            && non_text_segments > options.forward_media_threshold)
}

fn build_bundled_messages(header_text: String, segments: Vec<MessageSegment>) -> Vec<RenderOutput> {
    let mut bundle = vec![MessageOutput::from_text(header_text.clone())];

    // Audio/video must stay as standalone messages; everything else joins
    // the bundle.
    let mut deferred = Vec::new();
    for segment in segments {
        if segment.is_failure_notice
            || matches!(
                segment.segment_type,
                SegmentType::Audio | SegmentType::Video
            )
        {
            deferred.push(segment);
            continue;
        }

        bundle.push(MessageOutput::from_segment(segment));
    }

    let mut messages = if bundle.len() == 1 {
        // A bundle of one message is pointless; send it plainly.
        vec![RenderOutput::Message(MessageOutput::from_text(header_text))]
    } else {
        vec![RenderOutput::Bundle(bundle)]
    };
    messages.extend(
        deferred
            .into_iter()
            .map(|segment| RenderOutput::Message(MessageOutput::from_segment(segment))),
    );
    messages
}
