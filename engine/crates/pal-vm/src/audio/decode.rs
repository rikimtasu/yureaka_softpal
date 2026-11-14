//! Desktop audio uses one codec pass for low startup latency. Memory-limited
//! console hosts retain the two-pass final-allocation path.
use super::*;
use symphonia::core::{audio::SampleBuffer, errors::Error, io::MediaSourceStream};

pub(super) fn decode_static(bytes: &[u8]) -> anyhow::Result<StaticSoundData> {
    #[cfg(not(any(target_os = "vita", target_os = "horizon")))]
    {
        let mut output = Vec::new();
        let sample_rate = visit_frames(Arc::from(bytes), |frames| {
            output.len().checked_add(frames.len()).and_then(|n| n.checked_mul(std::mem::size_of::<Frame>()))
                .filter(|&bytes| bytes <= 256 * 1024 * 1024)
                .ok_or_else(|| anyhow::anyhow!("decoded audio exceeds the allocation limit"))?;
            output.extend_from_slice(frames);
            Ok(())
        })?;
        anyhow::ensure!(!output.is_empty(), "audio decode produced no samples");
        return Ok(StaticSoundData { sample_rate, frames: output.into(), settings: StaticSoundSettings::new(), slice: None });
    }
    #[cfg(any(target_os = "vita", target_os = "horizon"))]
    decode_two_pass(bytes)
}

#[cfg(any(test, target_os = "vita", target_os = "horizon"))]
fn decode_two_pass(bytes: &[u8]) -> anyhow::Result<StaticSoundData> {
    let encoded: Arc<[u8]> = Arc::from(bytes);
    let mut count = 0usize;
    let sample_rate = visit_frames(encoded.clone(), |frames| {
        count = count.checked_add(frames.len())
            .ok_or_else(|| anyhow::anyhow!("decoded audio length overflow"))?;
        Ok(())
    })?;
    anyhow::ensure!(count != 0, "audio decode produced no samples");
    count.checked_mul(std::mem::size_of::<Frame>())
        .ok_or_else(|| anyhow::anyhow!("decoded audio allocation overflow"))?;
    let mut output = Arc::<[Frame]>::new_uninit_slice(count);
    let storage = Arc::get_mut(&mut output).expect("new audio allocation is unique");
    let mut written = 0usize;
    let second_rate = visit_frames(encoded, |frames| {
        let end = written.checked_add(frames.len())
            .ok_or_else(|| anyhow::anyhow!("decoded audio length overflow"))?;
        anyhow::ensure!(end <= storage.len(), "audio length changed between decode passes");
        for (dst, frame) in storage[written..end].iter_mut().zip(frames) {
            dst.write(*frame);
        }
        written = end;
        Ok(())
    })?;
    anyhow::ensure!(written == count && sample_rate == second_rate,
                    "audio format changed between decode passes");
    // Both passes use the same immutable stream. Every Frame was initialized
    // above, and a failed or inconsistent decode drops only MaybeUninit values.
    let frames = unsafe { output.assume_init() };
    Ok(StaticSoundData { sample_rate, frames, settings: StaticSoundSettings::new(), slice: None })
}

pub(super) fn visit_frames(encoded: Arc<[u8]>, mut visit: impl FnMut(&[Frame]) -> anyhow::Result<()>)
    -> anyhow::Result<u32>
{
    let stream = MediaSourceStream::new(Box::new(Cursor::new(encoded)), Default::default());
    let mut format = symphonia::default::get_probe()
        .format(&Default::default(), stream, &Default::default(), &Default::default())?.format;
    let track = format.default_track().ok_or_else(|| anyhow::anyhow!("no default audio track"))?;
    let track_id = track.id;
    let rate = track.codec_params.sample_rate.ok_or_else(|| anyhow::anyhow!("unknown sample rate"))?;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;
    let mut frames = Vec::new();
    let mut samples: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };
        if packet.track_id() != track_id { continue; }
        let decoded = decoder.decode(&packet)?;
        let channels = decoded.spec().channels.count();
        anyhow::ensure!(matches!(channels, 1 | 2), "unsupported audio channel configuration");
        if samples.as_ref().is_none_or(|samples| samples.capacity() < decoded.capacity() * channels) {
            samples = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec()));
        }
        let samples = samples.as_mut().unwrap();
        samples.copy_interleaved_ref(decoded);
        frames.clear();
        frames.extend(samples.samples().chunks_exact(channels).map(|sample| {
            Frame::new(sample[0], if channels == 1 { sample[0] } else { sample[1] })
        }));
        visit(&frames)?;
    }
    Ok(rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave() -> Vec<u8> {
        let samples: Vec<i16> = (0..4096).map(|i| (i * 137) as i16).collect();
        let length = samples.len() as u32 * 2;
        let mut bytes = b"RIFF".to_vec();
        bytes.extend((36 + length).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(44100u32.to_le_bytes());
        bytes.extend(88200u32.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(length.to_le_bytes());
        for sample in samples { bytes.extend(sample.to_le_bytes()); }
        bytes
    }
    #[test]
    fn single_pass_pcm_matches_two_pass_and_cache_shares_only_original_frames() {
        let mut clips = vec![wave()];
        if let Ok(root) = std::env::var("PAL_AUDIO_REGRESSION_ROOT") {
            let mut resources = ResourceManager::bootstrap(root, pal_asset::Nls::ShiftJis).unwrap();
            for name in ["VO01_00000.OGG", "VO01_00001.OGG", "VO01_00002.OGG"] {
                clips.push(resources.open(name).unwrap().bytes);
            }
        }
        let mut audio = AudioSystem::new(AudioConfig { enabled: false }).unwrap();
        for bytes in clips {
            let reference = decode_two_pass(&bytes).unwrap();
            let current = decode_static(&bytes).unwrap();
            assert_eq!(reference.sample_rate, current.sample_rate);
            assert_eq!(reference.frames.len(), current.frames.len());
            for (old, new) in reference.frames.iter().zip(current.frames.iter()) {
                assert_eq!((old.left.to_bits(), old.right.to_bits()), (new.left.to_bits(), new.right.to_bits()));
            }
            let (mut first, format) = audio.decode_cached(&bytes, &mut |_| anyhow::bail!("no MIX")).unwrap();
            let (second, second_format) = audio.decode_cached(&bytes, &mut |_| anyhow::bail!("no MIX")).unwrap();
            assert!(Arc::ptr_eq(&first.frames, &second.frames));
            assert_eq!(format, second_format);
            Arc::make_mut(&mut first.frames)[0] = Frame::new(9.0, 9.0);
            assert_ne!(first.frames[0].left.to_bits(), second.frames[0].left.to_bits());
        }
    }
}
