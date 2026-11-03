use crate::video::HardwareCodec;

#[derive(Debug)]
pub struct KeyframeCadence {
    interval_frames: u32,
    frames_since_keyframe: u32,
}

impl KeyframeCadence {
    pub fn new(interval_frames: u32) -> Self {
        Self {
            interval_frames: interval_frames.max(1),
            frames_since_keyframe: 0,
        }
    }

    pub fn note_submitted(&mut self) -> bool {
        self.frames_since_keyframe = self.frames_since_keyframe.saturating_add(1);
        // Half an interval of slack keeps encoders that honour the GOP from getting a second
        // keyframe while their own one is still in the encoder's output queue.
        if self.frames_since_keyframe <= self.interval_frames + self.interval_frames / 2 {
            return false;
        }
        self.frames_since_keyframe = 0;
        true
    }

    pub fn note_keyframe(&mut self) {
        self.frames_since_keyframe = 0;
    }
}

// Some drivers emit IDR frames without MFSampleExtension_CleanPoint, so the NAL type is read too.
pub fn starts_with_random_access_picture(codec: HardwareCodec, payload: &[u8]) -> bool {
    let Some(header) = first_picture_nal_header(codec, payload) else {
        return false;
    };
    match codec {
        HardwareCodec::H264 => header & 0x1F == 5,
        // BLA, IDR and CRA pictures; Rewa encodes without B-frames, so a CRA has no skipped leading pictures.
        HardwareCodec::Hevc => (16..=21).contains(&((header >> 1) & 0x3F)),
        HardwareCodec::Av1 => false,
    }
}

fn first_picture_nal_header(codec: HardwareCodec, payload: &[u8]) -> Option<u8> {
    let mut rest = payload;
    while let Some(start) = find_start_code(rest) {
        rest = &rest[start..];
        let header = *rest.first()?;
        let is_picture = match codec {
            HardwareCodec::H264 => (1..=5).contains(&(header & 0x1F)),
            HardwareCodec::Hevc => ((header >> 1) & 0x3F) <= 31,
            HardwareCodec::Av1 => return None,
        };
        if is_picture {
            return Some(header);
        }
    }
    None
}

fn find_start_code(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(3)
        .position(|window| window == [0, 0, 1])
        .map(|position| position + 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H264_IDR: &[u8] = &[
        0, 0, 0, 1, 0x67, 0x42, 0, 0, 0, 1, 0x68, 0xCE, 0, 0, 1, 0x06, 5, 0, 0, 1, 0x65, 0x88,
    ];
    const H264_P: &[u8] = &[0, 0, 0, 1, 0x09, 0x10, 0, 0, 0, 1, 0x41, 0x9A];
    const HEVC_IDR: &[u8] = &[
        0, 0, 0, 1, 0x40, 0x01, 0, 0, 0, 1, 0x42, 0x01, 0, 0, 0, 1, 0x44, 0x01, 0, 0, 1, 0x26, 0x01,
    ];
    const HEVC_CRA: &[u8] = &[0, 0, 0, 1, 0x2A, 0x01, 0xAF];
    const HEVC_TRAIL: &[u8] = &[0, 0, 0, 1, 0x46, 0x01, 0x50, 0, 0, 1, 0x02, 0x01];

    #[test]
    fn recognises_h264_idr_slices_behind_parameter_sets() {
        assert!(starts_with_random_access_picture(
            HardwareCodec::H264,
            H264_IDR
        ));
        assert!(!starts_with_random_access_picture(
            HardwareCodec::H264,
            H264_P
        ));
    }

    #[test]
    fn recognises_hevc_random_access_pictures() {
        assert!(starts_with_random_access_picture(
            HardwareCodec::Hevc,
            HEVC_IDR
        ));
        assert!(starts_with_random_access_picture(
            HardwareCodec::Hevc,
            HEVC_CRA
        ));
        assert!(!starts_with_random_access_picture(
            HardwareCodec::Hevc,
            HEVC_TRAIL
        ));
    }

    #[test]
    fn rejects_payloads_without_a_picture() {
        for payload in [
            &[][..],
            &[0, 0, 1][..],
            &[0, 0, 0, 1, 0x67, 0x42][..],
            &[0x65; 8][..],
        ] {
            assert!(!starts_with_random_access_picture(
                HardwareCodec::H264,
                payload
            ));
        }
        assert!(!starts_with_random_access_picture(
            HardwareCodec::Av1,
            H264_IDR
        ));
    }

    #[test]
    fn forces_a_keyframe_only_after_the_encoder_misses_its_interval() {
        let mut cadence = KeyframeCadence::new(4);
        let forced: Vec<usize> = (0..14).filter(|_| cadence.note_submitted()).collect();
        assert_eq!(forced, [6, 13]);

        let mut honoured = KeyframeCadence::new(4);
        for _ in 0..40 {
            assert!(!honoured.note_submitted());
            assert!(!honoured.note_submitted());
            assert!(!honoured.note_submitted());
            assert!(!honoured.note_submitted());
            honoured.note_keyframe();
        }
    }
}
