use fo3_server::tools::ealayer3::parse_mp3_frames;
use std::env;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--selftest") {
        // Minimal MP3 frame header: sync 0xFFE or 0xFFF
        // 0xFF 0xFB 0x90 0x64 (MPEG-1 Layer 3, 128 kbps, 44100 Hz, padding=0, stereo)
        let mut test_frame = vec![0xFF, 0xFB, 0x90, 0x64];
        test_frame.resize(417, 0); // 144 * 128000 / 44100 = 417 bytes frame size
        let frames = parse_mp3_frames(&test_frame);
        assert_eq!(frames.len(), 1);
        println!("ealayer3 selftest OK");
        return Ok(());
    }

    if args.len() < 2 {
        eprintln!("Usage: ealayer3 <file.mp3_or_sps> [--selftest]");
        std::process::exit(1);
    }

    let file_path = &args[1];
    let data = fs::read(file_path)?;
    let frames = parse_mp3_frames(&data);

    println!("File: {} ({} bytes)", file_path, data.len());
    println!("Parsed MP3 frames: {}", frames.len());

    if !frames.is_empty() {
        let first = &frames[0];
        let h = first.header;
        let bitrate_idx = ((h >> 12) & 0xF) as usize;
        let srate_idx = ((h >> 10) & 0x3) as usize;
        let kbps = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
        let srates = [44100, 48000, 32000];

        let rate = if srate_idx < srates.len() { srates[srate_idx] } else { 0 };
        let br = if bitrate_idx < kbps.len() { kbps[bitrate_idx] } else { 0 };
        let duration_secs = (frames.len() as f64 * 1152.0) / (rate as f64).max(1.0);

        println!("Bitrate: {} kbps", br);
        println!("Sample rate: {} Hz", rate);
        println!("Estimated duration: {:.2}s", duration_secs);
    }

    Ok(())
}
