use byteorder::{BigEndian, ByteOrder};

pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn read(&mut self, n: usize) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            let byte_idx = self.pos >> 3;
            let bit_idx = 7 - (self.pos & 7);
            let bit = if byte_idx < self.data.len() {
                ((self.data[byte_idx] >> bit_idx) & 1) as u32
            } else {
                0
            };
            v = (v << 1) | bit;
            self.pos += 1;
        }
        v
    }
}

pub struct BitWriter {
    out: Vec<u8>,
    acc: u8,
    n: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }

    pub fn write(&mut self, v: u32, nbits: usize) {
        for i in (0..nbits).rev() {
            self.acc = (self.acc << 1) | (((v >> i) & 1) as u8);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc);
                self.acc = 0;
                self.n = 0;
            }
        }
    }

    pub fn into_bytes(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc << (8 - self.n));
        }
        self.out
    }
}

#[derive(Debug, Clone)]
pub struct Mp3Frame<'a> {
    pub header: u32,
    pub side_info: &'a [u8],
    pub main_data: &'a [u8],
}

pub fn parse_mp3_frames<'a>(mp3: &'a [u8]) -> Vec<Mp3Frame<'a>> {
    let mut i = 0usize;
    if mp3.starts_with(b"ID3") && mp3.len() >= 10 {
        let id3_size = (((mp3[6] & 0x7F) as usize) << 21)
            | (((mp3[7] & 0x7F) as usize) << 14)
            | (((mp3[8] & 0x7F) as usize) << 7)
            | ((mp3[9] & 0x7F) as usize);
        i = 10 + id3_size;
    }

    let kbps = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    let sample_rates = [44100, 48000, 32000];

    let mut frames = Vec::new();
    while i + 4 <= mp3.len() {
        let h = BigEndian::read_u32(&mp3[i..i + 4]);
        if (h >> 21) != 0x7FF {
            i += 1;
            continue;
        }
        let bitrate_idx = ((h >> 12) & 0xF) as usize;
        let srate_idx = ((h >> 10) & 0x3) as usize;
        if bitrate_idx == 0 || bitrate_idx >= 15 || srate_idx >= 3 {
            i += 1;
            continue;
        }

        let padding = ((h >> 9) & 1) as usize;
        let frame_size = (144000 * kbps[bitrate_idx]) / sample_rates[srate_idx] + padding;
        let channel_mode = (h >> 6) & 3;
        let side_len = if channel_mode == 3 { 17 } else { 32 };

        if i + frame_size > mp3.len() {
            break;
        }

        let side_info = &mp3[i + 4..i + 4 + side_len];
        let main_data = &mp3[i + 4 + side_len..i + frame_size];
        frames.push(Mp3Frame {
            header: h,
            side_info,
            main_data,
        });
        i += frame_size;
    }
    frames
}
