use std::slice::Iter;
use crate::{Channels, Colorspace, ImgMetadata, Operation, Pixel, QOI_OP_DIFF, QOI_OP_INDEX, QOI_OP_LUMA, QOI_OP_RGB, QOI_OP_RGBA, QOI_OP_RUN};

pub fn decode(bytes: &Vec<u8>) -> (ImgMetadata, Vec<u8>) {

    verify_ending(&bytes);

    let mut iter = bytes[0..bytes.len()-8].iter();
    let metadata = parse_metadata(&mut iter);

    let channels_per_pixel = match metadata.channels {
        Channels::RGB => {3}
        Channels::RGBA => {4}
    };
    let total_pixels: usize = (metadata.width * metadata.height) as usize;
    let mut decoded: Vec<u8> = Vec::with_capacity(total_pixels * channels_per_pixel);

    let include_alpha = match metadata.channels {
        Channels::RGB => {false}
        Channels::RGBA => {true}
    };
    if total_pixels > 0 {

        let num_pixels_parsed = parse_chunks(&mut iter, &mut decoded, include_alpha);
        assert_eq!(total_pixels, num_pixels_parsed);

    }


    return (metadata, decoded);
}

fn parse_metadata(iter: &mut Iter<u8>) -> ImgMetadata {
    assert!(iter.len() >= 14);
    let magic: String = iter.take(4).map(|x| char::from(*x)).collect();
    assert_eq!(magic, "qoif");

    let width: u32 = parse_u32(iter);
    let height: u32 = parse_u32(iter);

    let channels = match iter.next().unwrap() {
        &3 => { Channels::RGB},
        &4 => {Channels::RGBA},
        &_ => {panic!("Invalid Header: Channels")}
    };
    let colorspace = match iter.next().unwrap() {
        &0 => { Colorspace::SrgbLinearAlpha},
        &1 => { Colorspace::AllLinearAlpha},
        &_ => {panic!("Invalid Header: Colorspace")}
    };

    ImgMetadata {
        width,
        height,
        channels,
        colorspace,
    }
}

fn parse_u32(iter: &mut Iter<u8>) -> u32 {
    let mut n: u32 = *iter.next().unwrap() as u32;
    n = n << 8;
    n += *iter.next().unwrap() as u32;
    n = n << 8;
    n += *iter.next().unwrap() as u32;
    n = n << 8;
    n += *iter.next().unwrap() as u32;
    n
}

fn parse_chunks(mut iter: &mut Iter<u8>, mut bytes: &mut Vec<u8>, include_alpha: bool) -> usize {
    let mut pixels_seen: usize = 0;

    let mut prev_pixel = Pixel {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    let zero_pixel = Pixel {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    let mut index: [Option<Pixel>; 64] = [None; 64];
    index[super::encoder::calculate_index(&prev_pixel)] = Some(prev_pixel.clone());
    index[super::encoder::calculate_index(&zero_pixel)] = Some(zero_pixel.clone());



    while let Some(tag) = iter.next() {
        let operation = parse_operation(&tag);

        let current_pixel: Pixel;

        match operation {
            Operation::QoiOpRgb => {current_pixel = write_op_rgb(&mut bytes, &mut iter, &prev_pixel.a, include_alpha);}
            Operation::QoiOpRgba => {current_pixel = write_op_rgba(&mut bytes, &mut iter, include_alpha);}
            Operation::QoiOpIndex => {current_pixel = write_op_index(&mut bytes, &tag, &index, include_alpha);}
            Operation::QoiOpDiff => {current_pixel = write_op_diff(&mut bytes, &tag, &prev_pixel, include_alpha);}
            Operation::QoiOpLuma => {current_pixel = write_op_luma(&mut bytes, &tag, &mut iter, &prev_pixel, include_alpha);}
            Operation::QoiOpRun => {
                let run_len = write_op_run(&mut bytes, &tag, &prev_pixel, include_alpha);
                current_pixel = prev_pixel;
                pixels_seen += run_len - 1;
            }
        }

        index[super::encoder::calculate_index(&current_pixel)] = Some(current_pixel.clone());
        prev_pixel = current_pixel;

        pixels_seen += 1;
    }

    pixels_seen
}

fn parse_operation(tag: &u8) -> Operation {
    let tag_2 = tag >> 6;
    match *tag {
        QOI_OP_RGB => {Operation::QoiOpRgb}
        QOI_OP_RGBA => { Operation::QoiOpRgba}
        _ => {
            match tag_2 {
                QOI_OP_INDEX => {Operation::QoiOpIndex},
                QOI_OP_DIFF => {Operation::QoiOpDiff},
                QOI_OP_LUMA => {Operation::QoiOpLuma},
                QOI_OP_RUN => {Operation::QoiOpRun}
                _ => {panic!("All 2 bit options are covered")}
            }
        }
    }
}

fn verify_ending(vec: &Vec<u8>) {
    assert!(vec.len() >= 8);

    let len = vec.len();

    for i in 0..7 {
        assert_eq!(0, vec[len - 8 + i]);
    }

    assert_eq!(1, vec[len - 1]);
}

fn write_op_rgb(bytes: &mut Vec<u8>, iter: &mut Iter<u8>, alpha: &u8, include_alpha: bool) -> Pixel {
    let r = iter.next().unwrap();
    let g = iter.next().unwrap();
    let b = iter.next().unwrap();
    let a = alpha;

    bytes.push(*r);
    bytes.push(*g);
    bytes.push(*b);

    if include_alpha { bytes.push(*a);};

    Pixel {
        r: *r,
        g: *g,
        b: *b,
        a: *a,
    }
}

fn write_op_rgba(bytes: &mut Vec<u8>, iter: &mut Iter<u8>, include_alpha: bool) -> Pixel {
    let r = iter.next().unwrap();
    let g = iter.next().unwrap();
    let b = iter.next().unwrap();
    let a = iter.next().unwrap();

    bytes.push(*r);
    bytes.push(*g);
    bytes.push(*b);

    if include_alpha { bytes.push(*a);};

    Pixel {
        r: *r,
        g: *g,
        b: *b,
        a: *a,
    }
}

fn write_op_index(bytes: &mut Vec<u8>, tag: &u8, index: &[Option<Pixel>], include_alpha: bool) -> Pixel {
    let pixel = index[*tag as usize].unwrap();
    bytes.push(pixel.r);
    bytes.push(pixel.g);
    bytes.push(pixel.b);

    if include_alpha {
        bytes.push(pixel.a);
    }
    pixel
}

fn write_op_diff(bytes: &mut Vec<u8>, tag: &u8, prev_pixel: &Pixel, include_alpha: bool) -> Pixel {
    let mut current_pixel = prev_pixel.clone();

    let dr = (0b_00_11_00_00 & *tag) >> 4;
    let dg = (0b_00_00_11_00 & *tag) >> 2;
    let db = 0b_00_00_00_11 & *tag;

    let r = u8::wrapping_add(prev_pixel.r, dr);
    let g = u8::wrapping_add(prev_pixel.g, dg);
    let b = u8::wrapping_add(prev_pixel.b, db);

    current_pixel.r = u8::wrapping_sub(r, 2);
    current_pixel.g = u8::wrapping_sub(g, 2);
    current_pixel.b = u8::wrapping_sub(b, 2);

    bytes.push(current_pixel.r);
    bytes.push(current_pixel.g);
    bytes.push(current_pixel.b);

    if include_alpha { bytes.push(current_pixel.a);}

    current_pixel
}

fn write_op_luma(bytes: &mut Vec<u8>, tag: &u8, iter: &mut Iter<u8>, prev_pixel: &Pixel, include_alpha: bool) -> Pixel{
    let dg = *tag & 0b00_111111;

    let byte2 = iter.next().unwrap();
    let dr_dg = (*byte2 & 0b_1111_0000) >> 4;
    let db_dg = *byte2 & 0b_0000_1111;


    let dg = u8::wrapping_sub(dg, 32);
    let dr_dg = u8::wrapping_sub(dr_dg, 8);
    let db_dg = u8::wrapping_sub(db_dg, 8);


    let dr = u8::wrapping_add(dr_dg, dg);
    let db = u8::wrapping_add(db_dg, dg);


    let r = u8::wrapping_add(prev_pixel.r, dr);
    let g = u8::wrapping_add(prev_pixel.g, dg);
    let b = u8::wrapping_add(prev_pixel.b, db);
    let a = prev_pixel.a;

    bytes.push(r);
    bytes.push(g);
    bytes.push(b);
    if include_alpha { bytes.push(a);}

    Pixel {
        r,
        g,
        b,
        a,
    }

}

fn write_op_run(bytes: &mut Vec<u8>, tag: &u8, prev_pixel: &Pixel, include_alpha: bool) -> usize {
    let run_len = (*tag & 0b_00_111111) + 1;

    for _ in 0..run_len {
        bytes.push(prev_pixel.r);
        bytes.push(prev_pixel.g);
        bytes.push(prev_pixel.b);
        if include_alpha { bytes.push(prev_pixel.a);}
    }

    run_len as usize
}

#[cfg(test)]
mod tests {
    use crate::{Channels, Colorspace, Operation, QOI_OP_DIFF, QOI_OP_INDEX, QOI_OP_LUMA, QOI_OP_RGB, QOI_OP_RGBA};
    use crate::encoder::calculate_index;
    use super::*;

    #[test]
    fn decode_empty() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 0,
            height: 0,
            channels: Channels::RGB,
            colorspace: Colorspace::SrgbLinearAlpha,
        };
        crate::encoder::add_header(&mut bytes, &metadata);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);
        assert_eq!(encoded, vec![]);
    }

    #[test]
    fn parse_filled_metadata() {
        let metadata = ImgMetadata {
            width: 800,
            height: 600,
            channels: Channels::RGBA,
            colorspace: Colorspace::AllLinearAlpha,
        };

        let mut bytes = Vec::new();
        crate::encoder::add_header(&mut bytes, &metadata);

        let mut iter = bytes.iter();

        let returned_metadata = parse_metadata(&mut iter);

        assert_eq!(metadata, returned_metadata);
    }

    #[test]
    fn parse_u32_test() {
        let mut v: Vec<u8> = Vec::with_capacity(4);
        let num: u32 = 3150664367;

        let bits = ((num & (0b11111111 << 24)) >> 24) as u8;
        v.push(bits);

        let bits = ((num & (0b11111111 << 16)) >> 16) as u8;
        v.push(bits);

        let bits = ((num & (0b11111111 << 8)) >> 8) as u8;
        v.push(bits);

        let bits = (num & 0b11111111) as u8;
        v.push(bits);

        assert_eq!(num, parse_u32(&mut v.iter()));
    }

    #[test]
    fn parse_op_rgb_chunk() {
        let op = vec![QOI_OP_RGB, 50, 80, 23,
                      QOI_OP_RGB, 100, 17, 88];
        let expected = vec![50, 80, 23,
                            100, 17, 88];

        let mut bytes = Vec::new();
        parse_chunks(&mut op.iter(), &mut bytes, false);

        assert_eq!(expected, bytes)
    }

    #[test]
    fn parse_op_rgba_chunk() {
        let op = vec![QOI_OP_RGBA, 50, 80, 23, 200,
                      QOI_OP_RGB, 100, 17, 88];
        let expected = vec![50, 80, 23, 200,
                            100, 17, 88, 200];

        let mut bytes = Vec::new();
        parse_chunks(&mut op.iter(), &mut bytes, true);

        assert_eq!(expected, bytes)
    }

    #[test]
    fn parse_op_index_chunk() {
        let pixel = Pixel {
            r: 50,
            g: 80,
            b: 23,
            a: 200,
        };
        let index = calculate_index(&pixel) as u8;

        let op = vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a,
                      QOI_OP_RGBA, 100, 180, 0, 55,
                      index];
        let expected = vec![50, 80, 23, 200,
                            100, 180, 0, 55,
                            50, 80, 23, 200];

        let mut bytes = Vec::new();
        parse_chunks(&mut op.iter(), &mut bytes, true);

        assert_eq!(expected, bytes)
    }

    #[test]
    fn parse_op_diff_chunk() {
        let pixel = Pixel {
            r: 50,
            g: 80,
            b: 23,
            a: 200,
        };

        //op_diff, dr: -1, dg: -2, db: 1
        let diff_op = 0b01_01_00_11u8;

        let op = vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a,
                      diff_op];
        let expected = vec![50, 80, 23, 200,
                            49, 78, 24, 200];

        let mut bytes = Vec::new();
        parse_chunks(&mut op.iter(), &mut bytes, true);

        assert_eq!(expected, bytes)
    }

    #[test]
    fn parse_op_luma_chunk() {
        let pixel = Pixel {
            r: 230,
            g: 255,
            b: 23,
            a: 200,
        };

        let dg: u8 = 25 + 32;
        let luma_op = (QOI_OP_LUMA << 6) + dg;

        let dr_dg = (-5 + 8) as u8;
        let db_dg = 0 + 8;
        let byte2 = (dr_dg << 4) + db_dg;

        let op = vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a,
                      luma_op, byte2];
        let expected = vec![230, 255, 23, 200,
                            250, 24, 48, 200];

        let mut bytes = Vec::new();
        parse_chunks(&mut op.iter(), &mut bytes, true);

        assert_eq!(expected, bytes)
    }

    #[test]
    fn parse_op_run_chunk() {
        let pixel = Pixel {
            r: 230,
            g: 255,
            b: 23,
            a: 200,
        };

        let op_run = (QOI_OP_RUN << 6) + 4;

        let op = vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a,
                      op_run];
        let mut expected = Vec::new();

        for _ in 0..6 {
            expected.push(pixel.r);
            expected.push(pixel.g);
            expected.push(pixel.b);
            expected.push(pixel.a);
        }

        let mut bytes = Vec::new();
        parse_chunks(&mut op.iter(), &mut bytes, true);

        assert_eq!(expected, bytes)
    }

    #[test]
    fn decode_op_rgb_chunk() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 2,
            height: 1,
            channels: Channels::RGBA,
            colorspace: Colorspace::SrgbLinearAlpha,
        };
        crate::encoder::add_header(&mut bytes, &metadata);
        bytes.extend(vec![QOI_OP_RGB, 17, 18, 200]);
        bytes.extend(vec![QOI_OP_RGB, 6, 100, 50]);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);
        //alpha is unchanged from default 255
        assert_eq!(encoded, vec![17, 18, 200, 255, 6, 100, 50, 255]);
    }

    #[test]
    fn decode_op_rgba_chunk() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 2,
            height: 2,
            channels: Channels::RGBA,
            colorspace: Colorspace::SrgbLinearAlpha,
        };
        crate::encoder::add_header(&mut bytes, &metadata);
        bytes.extend(vec![QOI_OP_RGB, 17, 18, 200]);
        bytes.extend(vec![QOI_OP_RGBA, 6, 100, 50, 77]);
        bytes.extend(vec![QOI_OP_RGB, 55, 77, 22]);
        bytes.extend(vec![QOI_OP_RGBA, 7, 101, 51, 80]);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);
        //alpha's original default value is 255
        assert_eq!(encoded, vec![17, 18, 200, 255,
                                 6, 100, 50, 77,
                                 55, 77, 22, 77,
                                 7, 101, 51, 80]);
    }

    #[test]
    fn decode_op_index_chunk() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 2,
            height: 2,
            channels: Channels::RGBA,
            colorspace: Colorspace::SrgbLinearAlpha,
        };

        let pixel = Pixel {
            r: 6,
            g: 100,
            b: 50,
            a: 77,
        };
        let index = calculate_index(&pixel) as u8;
        crate::encoder::add_header(&mut bytes, &metadata);
        bytes.extend(vec![QOI_OP_RGB, 17, 18, 200]);
        bytes.extend(vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a]);
        bytes.extend(vec![QOI_OP_RGB, 55, 77, 22]);
        bytes.push(index);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);
        //alpha's original default value is 255
        assert_eq!(encoded, vec![17, 18, 200, 255,
                                 6, 100, 50, 77,
                                 55, 77, 22, 77,
                                 6, 100, 50, 77]);
    }

    #[test]
    fn decode_op_diff_chunk() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 3,
            height: 1,
            channels: Channels::RGBA,
            colorspace: Colorspace::SrgbLinearAlpha,
        };

        let pixel = Pixel {
            r: 0,
            g: 1,
            b: 50,
            a: 77,
        };

        let diff_op = 0b_01_00_00_11u8;

        crate::encoder::add_header(&mut bytes, &metadata);
        bytes.extend(vec![QOI_OP_RGB, 17, 18, 200]);
        bytes.extend(vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a]);
        bytes.push(diff_op);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);
        //alpha's original default value is 255
        assert_eq!(encoded, vec![17, 18, 200, 255,
                                 0, 1, 50, 77,
                                 254, 255, 51, 77]);
    }

    #[test]
    fn decode_op_luma_chunk() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 3,
            height: 1,
            channels: Channels::RGBA,
            colorspace: Colorspace::SrgbLinearAlpha,
        };

        let pixel = Pixel {
            r: 0,
            g: 1,
            b: 50,
            a: 77,
        };

        let dg: u8 = 10 + 32;
        let luma_op = (QOI_OP_LUMA << 6) + dg;

        let dr_dg = (2 + 8) as u8;
        let db_dg = 3 + 8;
        let byte2 = (dr_dg << 4) + db_dg;


        crate::encoder::add_header(&mut bytes, &metadata);
        bytes.extend(vec![QOI_OP_RGB, 17, 18, 200]);
        bytes.extend(vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a]);
        bytes.extend(vec![luma_op, byte2]);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);
        //alpha's original default value is 255
        assert_eq!(encoded, vec![17, 18, 200, 255,
                                 0, 1, 50, 77,
                                 12, 11, 63, 77]);
    }

    #[test]
    fn decode_op_run_chunk() {
        let mut bytes = Vec::new();
        let metadata = ImgMetadata {
            width: 4,
            height: 4,
            channels: Channels::RGBA,
            colorspace: Colorspace::SrgbLinearAlpha,
        };

        let pixel = Pixel {
            r: 0,
            g: 1,
            b: 50,
            a: 77,
        };

        let op_run = (QOI_OP_RUN << 6) + 13;


        crate::encoder::add_header(&mut bytes, &metadata);
        bytes.extend(vec![QOI_OP_RGB, 17, 18, 200]);
        bytes.extend(vec![QOI_OP_RGBA, pixel.r, pixel.g, pixel.b, pixel.a]);
        bytes.push(op_run);
        crate::encoder::add_end_marker(&mut bytes);

        let (returned_metadata, encoded) = decode(&bytes);

        assert_eq!(metadata, returned_metadata);

        let mut expected = vec![17, 18, 200, 255,
                                0, 1, 50, 77];
        for _ in 0 .. 14 {
            expected.extend(vec![0, 1, 50, 77]);
        }

        assert_eq!(encoded, expected);
    }

    #[test]
    fn parse_op_rgb() {
        assert_eq!(Operation::QoiOpRgb, parse_operation(&QOI_OP_RGB));
    }

    #[test]
    fn parse_op_rgba() {
        assert_eq!(Operation::QoiOpRgba, parse_operation(&QOI_OP_RGBA));
    }

    #[test]
    fn parse_op_index() {
        assert_eq!(Operation::QoiOpIndex, parse_operation(&((QOI_OP_INDEX << 6) + 50)));
    }

    #[test]
    fn parse_op_diff() {
        assert_eq!(Operation::QoiOpDiff, parse_operation(&((QOI_OP_DIFF << 6) + 23)));
    }

    #[test]
    fn parse_op_luma() {
        assert_eq!(Operation::QoiOpLuma, parse_operation(&((QOI_OP_LUMA << 6) + 17)));
    }

    #[test]
    fn parse_op_run() {
        assert_eq!(Operation::QoiOpRun, parse_operation(&((QOI_OP_RUN << 6) + 5)));
    }

    #[test]
    fn verify_ending_success() {
        let v: Vec<u8> = vec![12, 8, 7, 0, 0, 0, 0, 0, 0, 0, 1];
        verify_ending(&v);
    }

    #[test]
    #[should_panic]
    fn verify_ending_fail() {
        let v: Vec<u8> = vec![12, 8, 7, 0, 0, 0, 0, 5, 0, 0, 0];
        verify_ending(&v);
    }

    #[test]
    #[should_panic]
    fn verify_ending_too_short() {
        let v: Vec<u8> = vec![0, 0, 0, 1];
        verify_ending(&v);
    }

    #[test]
    fn pixels_seen() {
        //todo - verify parse_chunks sees correct number of pixels
    }

    #[test]
    fn write_op_rgb_with_alpha() {
        let mut bytes = Vec::new();
        //some extra values are appended to test RGB only picks up the first 3 after op code
        let op = vec![10, 18, 200, 40, 7];

        let expected = vec![10, 18, 200, 50];

        let expected_pixel = Pixel {
            r: 10,
            g: 18,
            b: 200,
            a: 50,
        };

        let current_pixel = write_op_rgb(&mut bytes, &mut op.iter(), &50, true);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_rgb_without_alpha() {
        let mut bytes = Vec::new();
        //some extra values are appended to test RGB only picks up the first 3 after op code
        let op = vec![10, 18, 200, 40, 7];

        let expected = vec![10, 18, 200];

        let expected_pixel = Pixel {
            r: 10,
            g: 18,
            b: 200,
            a: 50,
        };

        let current_pixel = write_op_rgb(&mut bytes, &mut op.iter(), &50, false);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_rgba_with_alpha() {
        let mut bytes = Vec::new();
        //some extra values are appended to test RGBA only picks up the first 4 after op code
        let op = vec![10, 18, 200, 40, 7, 60, 22];

        let expected = vec![10, 18, 200, 40];

        let expected_pixel = Pixel {
            r: 10,
            g: 18,
            b: 200,
            a: 40,
        };

        let current_pixel = write_op_rgba(&mut bytes, &mut op.iter(), true);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_rgba_without_alpha() {
        let mut bytes = Vec::new();
        //some extra values are appended to test RGBA only picks up the first 4 after op code
        let op = vec![10, 18, 200, 40, 7, 60, 22];

        let expected = vec![10, 18, 200];

        let expected_pixel = Pixel {
            r: 10,
            g: 18,
            b: 200,
            a: 40,
        };

        let current_pixel = write_op_rgba(&mut bytes, &mut op.iter(), false);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_index_with_alpha() {
        let mut bytes = Vec::new();

        let expected = vec![10, 18, 200, 40];

        let expected_pixel = Pixel {
            r: 10,
            g: 18,
            b: 200,
            a: 40,
        };

        let mut index: [Option<Pixel>; 64] = [None; 64];
        let i = calculate_index(&expected_pixel);
        index[i] = Some(expected_pixel.clone());

        //since QOI_OP_INDEX's 2 bit tag is 0b00, the entire instruction is simply the index number
        let tag = i as u8;

        let current_pixel = write_op_index(&mut bytes, &tag, &index, true);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_index_without_alpha() {
        let mut bytes = Vec::new();

        let expected = vec![10, 18, 200];

        let expected_pixel = Pixel {
            r: 10,
            g: 18,
            b: 200,
            a: 40,
        };

        let mut index: [Option<Pixel>; 64] = [None; 64];
        let i = calculate_index(&expected_pixel);
        index[i] = Some(expected_pixel.clone());

        //since QOI_OP_INDEX's 2 bit tag is 0b00, the entire instruction is simply the index number
        let tag = i as u8;

        let current_pixel = write_op_index(&mut bytes, &tag, &index, false);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_diff_with_alpha() {
        let mut bytes = Vec::new();

        let previous_pixel = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };

        let expected_pixel = Pixel {
            r: 11,
            g: 18,
            b: 30,
            a: 40,
        };
        let expected = vec![11, 18, 30, 40];

        //QOI_OP_DIFF: 01-tag, dr: 1 dg: -2 db: 0
        let op: u8 = 0b01_11_00_10;

        let current_pixel = write_op_diff(&mut bytes, &op, &previous_pixel, true);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_diff_without_alpha() {
        let mut bytes = Vec::new();

        let previous_pixel = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };

        let expected_pixel = Pixel {
            r: 11,
            g: 18,
            b: 30,
            a: 40,
        };
        let expected = vec![11, 18, 30];

        //QOI_OP_DIFF: 01-tag, dr: 1 dg: -2 db: 0
        let op: u8 = 0b01_11_00_10;

        let current_pixel = write_op_diff(&mut bytes, &op, &previous_pixel, false);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_luma_with_alpha() {
        let mut bytes = Vec::new();

        let previous_pixel = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };

        let expected_pixel = Pixel {
            r: 35,
            g: 40,
            b: 48,
            a: 40,
        };
        let expected = vec![35, 40, 48, 40];

        //QOI_OP_LUMA:
        let dg = u8::wrapping_sub(expected_pixel.g, previous_pixel.g);
        let dr = u8::wrapping_sub(expected_pixel.r, previous_pixel.r);
        let db = u8::wrapping_sub(expected_pixel.b, previous_pixel.b);

        let dr_dg = u8::wrapping_sub(dr, dg);
        let db_dg = u8::wrapping_sub(db, dg);

        let dg = u8::wrapping_add(dg, 32);
        let dr_dg = u8::wrapping_add(dr_dg, 8);
        let db_dg = u8::wrapping_add(db_dg, 8);

        let op: u8 = (QOI_OP_LUMA << 6) + dg;
        let byte2: u8 =(dr_dg << 4) + db_dg;

        let ops = vec![byte2];

        let current_pixel = write_op_luma(&mut bytes, &op, &mut ops.iter(), &previous_pixel, true);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_luma_without_alpha() {
        let mut bytes = Vec::new();

        let previous_pixel = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };

        let expected_pixel = Pixel {
            r: 35,
            g: 40,
            b: 48,
            a: 40,
        };
        let expected = vec![35, 40, 48];

        //QOI_OP_LUMA:
        let dg = u8::wrapping_sub(expected_pixel.g, previous_pixel.g);
        let dr = u8::wrapping_sub(expected_pixel.r, previous_pixel.r);
        let db = u8::wrapping_sub(expected_pixel.b, previous_pixel.b);

        let dr_dg = u8::wrapping_sub(dr, dg);
        let db_dg = u8::wrapping_sub(db, dg);

        let dg = u8::wrapping_add(dg, 32);
        let dr_dg = u8::wrapping_add(dr_dg, 8);
        let db_dg = u8::wrapping_add(db_dg, 8);

        let op: u8 = (QOI_OP_LUMA << 6) + dg;
        let byte2: u8 =(dr_dg << 4) + db_dg;

        let ops = vec![byte2];

        let current_pixel = write_op_luma(&mut bytes, &op, &mut ops.iter(), &previous_pixel, false);

        assert_eq!(bytes, expected);
        assert_eq!(current_pixel, expected_pixel);
    }

    #[test]
    fn write_op_run_with_alpha() {
        let mut bytes = Vec::new();

        let previous_pixel = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let expected = vec![10, 20, 30, 40,
                            10, 20, 30, 40,
                            10, 20, 30, 40];


        let op = (QOI_OP_RUN << 6) + 2;

        let run_len = write_op_run(&mut bytes, &op, &previous_pixel, true);

        assert_eq!(bytes, expected);
        assert_eq!(run_len, 3);
    }

    #[test]
    fn write_op_run_without_alpha() {
        let mut bytes = Vec::new();

        let previous_pixel = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let expected = vec![10, 20, 30,
                            10, 20, 30,
                            10, 20, 30];


        let op = (QOI_OP_RUN << 6) + 2;

        let run_len = write_op_run(&mut bytes, &op, &previous_pixel, false);

        assert_eq!(bytes, expected);
        assert_eq!(run_len, 3);
    }
}