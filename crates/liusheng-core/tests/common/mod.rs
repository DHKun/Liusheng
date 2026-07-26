// 各集成测试二进制分别编译本模块，未用到全部辅助函数属正常
#![allow(dead_code)]

use std::path::Path;

/// 写一个测试用 wav：立体声、每帧两声道同值的锯齿序列 offset..offset+frames。
pub fn write_ramp_wav16(path: &Path, rate: u32, frames: usize, offset: i32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        let v = (offset + i as i32) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

pub fn write_ramp_wav24(path: &Path, rate: u32, frames: usize, offset: i32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        // 左移 4 位让数值超出 16 位范围，确保 24 位路径真正被走到
        let v = (offset + i as i32) << 4;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}
