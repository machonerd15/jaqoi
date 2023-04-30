use std::u8;

use crate::{Channels, Colorspace, ImgMetadata};

#[derive(Eq, PartialEq, Debug)]
enum Operation {
    QoiOpRgb,
    QoiOpRgba,
    QoiOpIndex,
    QoiOpDiff,
    QoiOpLuma,
    QoiOpRun,
}

const QOI_OP_RGB: u8 = 0b11111110;
const QOI_OP_RGBA: u8 = 0b11111111;
const QOI_OP_INDEX: u8 = 0b00;
const QOI_OP_DIFF: u8 = 0b01;
const QOI_OP_LUMA: u8 = 0b10;
const QOI_OP_RUN: u8 = 0b11;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a:u8,
}

pub(crate) fn add_header(bytes: &mut Vec<u8>, metadata: &ImgMetadata) {
    assert_eq!(bytes.len(), 0);

    let magic = vec![b'q', b'o', b'i', b'f'];
    bytes.extend(magic);

    let width = metadata.width;
    let height = metadata.height;
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());

    match metadata.channels {
        Channels::RGB => {bytes.push(3)}
        Channels::RGBA => {bytes.push(4)}
    }

    match metadata.colorspace {
        Colorspace::SrgbLinearAlpha => {bytes.push(0)}
        Colorspace::AllLinearAlpha => {bytes.push(1)}
    }

}

pub(crate) fn add_chunks(mut bytes: &mut Vec<u8>, pixels: &Vec<u8>, alpha_included: bool) -> Result<(),()>{
    // println!("Adding chunks for: {:?}", pixels);
    let expected_values_per_pixel = match alpha_included {
        true => {4}
        false => {3}
    };

    // println!("Expected pixels: {expected_values_per_pixel}");
    //todo - better error messaging
    if pixels.len() % expected_values_per_pixel != 0 {return Err(())};

    // println!("Didn't return an error");

    let mut index: [Option<Pixel>; 64] = [None; 64];

    let mut previous_pixel = Pixel{
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    let mut run_count: u8 = 0;

    let mut pixel_iter = pixels.iter();

    while pixel_iter.len() >= expected_values_per_pixel {
        // println!("Entering Loop");
        let pixel = Pixel{
            r: *pixel_iter.next().unwrap(),
            g: *pixel_iter.next().unwrap(),
            b: *pixel_iter.next().unwrap(),
            a: match alpha_included {
                true => {*pixel_iter.next().unwrap()}
                false => {255}
            },
        };

        let operation = find_operation(&previous_pixel, &pixel, &index);

        // println!("Got operation {:?}", operation);

        if operation != Operation::QoiOpRun && run_count > 0 {
            push_run(&mut bytes, run_count);
            run_count=0;
        }

        match operation {
            Operation::QoiOpRgb => {push_rgb(&pixel, bytes)}
            Operation::QoiOpRgba => {push_rgba(&pixel, bytes)}
            Operation::QoiOpIndex => {push_index(&pixel, bytes)}
            Operation::QoiOpDiff => {push_diff(&pixel, &previous_pixel, bytes)}
            Operation::QoiOpLuma => {push_luma(&pixel, &previous_pixel, bytes)}
            Operation::QoiOpRun => {
                run_count += 1;
                if run_count >= 63 {
                    push_run(&mut bytes, 62);
                    run_count -= 62;
                }
            }
        }

        index[calculate_index(&pixel)] = Some(pixel.clone());
        previous_pixel = pixel;

    }

    if run_count > 0 {
        push_run(&mut bytes, run_count);
    }

    Ok(())
}

pub(crate) fn add_end_marker(bytes: &mut Vec<u8>) {
    for _ in 0..7 {
        bytes.push(0);
    }
    bytes.push(1);
}

fn tag_byte(tag: u8, lower_bits: u8) -> u8 {
    assert!(tag < 4);
    assert!(lower_bits < 64);

    (tag << 6) + lower_bits
}

fn calculate_index(pixel: &Pixel) -> usize {
    let index:u32 = (pixel.r as u32) * 3 + (pixel.g as u32) * 5 + (pixel.b as u32) * 7 + (pixel.a as u32) * 11;
    (index % 64) as usize
}

fn create_diff(curr: &Pixel, prev: &Pixel) -> u8 {
    let dr = u8::wrapping_sub(curr.r, prev.r);
    let dg = u8::wrapping_sub(curr.g, prev.g);
    let db = u8:: wrapping_sub(curr.b, prev.b);

    let dr= u8::wrapping_add(dr, 2);
    let dg = u8::wrapping_add(dg, 2);
    let db= u8::wrapping_add(db, 2);

    assert!(dr < 4);
    assert!(dg < 4);
    assert!(db < 4);

    let mut lower_bits: u8 = dr;
    lower_bits = (lower_bits << 2) + dg;
    lower_bits = (lower_bits << 2) + db;

    tag_byte(QOI_OP_DIFF, lower_bits)
}

fn create_diff_luma(curr: &Pixel, prev: &Pixel) -> Vec<u8> {
    let dr = u8::wrapping_sub(curr.r, prev.r);
    let dg = u8::wrapping_sub(curr.g, prev.g);
    let db = u8::wrapping_sub(curr.b, prev.b);

    let dr_dg = u8::wrapping_sub(dr, dg);
    let db_dg = u8::wrapping_sub(db, dg);

    let dg = u8::wrapping_add(dg, 32);
    let dr_dg = u8::wrapping_add(dr_dg, 8);
    let db_dg = u8::wrapping_add(db_dg, 8);

    assert!(dg < 64);
    assert!(dr_dg < 16);
    assert!(db_dg < 16);

    let mut bytes = Vec::new();
    bytes.push(tag_byte(QOI_OP_LUMA, dg));
    let mut byte2 = dr_dg;
    byte2 = (byte2 << 4) + db_dg;
    bytes.push(byte2);
    bytes
}

fn find_operation(prev_pixel: &Pixel, curr_pixel: &Pixel, index: &[Option<Pixel>]) -> Operation {
    // println!("pp: {:?} cp: {:?}", prev_pixel, curr_pixel);
    if prev_pixel.a != curr_pixel.a { return Operation::QoiOpRgba; };
    // println!("Not alpha diff");
    if *prev_pixel == *curr_pixel {return Operation::QoiOpRun;}
    if let Some(index_pixel) = index[calculate_index(curr_pixel)] {
        if index_pixel == *curr_pixel {return Operation::QoiOpIndex;}
    }

    let dr = u8::wrapping_sub(curr_pixel.r, prev_pixel.r);
    let dg = u8::wrapping_sub(curr_pixel.g, prev_pixel.g);
    let db = u8::wrapping_sub(curr_pixel.b, prev_pixel.b);


    let dr_2 = u8::wrapping_add(dr, 2);
    let dg_2 = u8::wrapping_add(dg, 2);
    let db_2 = u8::wrapping_add(db, 2);

    // println!("dr_2: {dr_2} dg_2: {dg_2} db_2: {db_2}");

    if dr_2 < 4 && dg_2 < 4 && db_2 < 4 {
        return Operation::QoiOpDiff;
    }

    let dr_dg = u8::wrapping_sub(dr, dg);
    // println!("dr_dg: {dr_dg}");
    let dr_dg_8 = u8::wrapping_add(dr_dg, 8);
    // println!("dr_dg_8: {dr_dg_8}");

    let db_dg = u8::wrapping_sub(db, dg);
    // println!("db_dg: {db_dg}");
    let db_dg_8 = u8::wrapping_add(db_dg, 8);
    // println!("db_dg_8: {db_dg_8}");

    let dg_32 = u8::wrapping_add(dg, 32);
    // println!("dg_32: {dg_32}");

    if  dg_32 < 64 && dr_dg_8 < 16 && db_dg_8 < 16{
        return Operation::QoiOpLuma;
    }

    Operation::QoiOpRgb
}

fn push_run(bytes: &mut Vec<u8>, run_length: u8) {
    assert!(run_length > 0 && run_length < 63);
    bytes.push(tag_byte(QOI_OP_RUN, run_length - 1));
}

fn push_rgb(pixel: &Pixel, bytes: &mut Vec<u8>) {
    bytes.push(QOI_OP_RGB);
    bytes.push(pixel.r);
    bytes.push(pixel.g);
    bytes.push(pixel.b);
}

fn push_rgba(pixel: &Pixel, bytes: &mut Vec<u8>) {
    bytes.push(QOI_OP_RGBA);
    bytes.push(pixel.r);
    bytes.push(pixel.g);
    bytes.push(pixel.b);
    bytes.push(pixel.a);
}

fn push_index(pixel: &Pixel, bytes: &mut Vec<u8>) {
    bytes.push(tag_byte(QOI_OP_INDEX, calculate_index(&pixel).try_into().unwrap()));
}

fn push_diff(curr: &Pixel, prev: &Pixel, bytes: &mut Vec<u8>) {
    bytes.push(create_diff(&curr, &prev));
}

fn push_luma(curr: &Pixel, prev: &Pixel, bytes: &mut Vec<u8>) {
    bytes.extend(create_diff_luma(&curr, &prev));
}


#[cfg(test)]
mod tests {
    use crate::{Channels, Colorspace};

    use super::*;

    #[test]
    fn end_marker() {
        let mut vec = Vec::new();
        add_end_marker(&mut vec);
        assert_eq!(vec![0,0,0,0,0,0,0,1], vec);
    }

    #[test]
    fn header_rgb_srgb() {
        let metadata = ImgMetadata {
            width: 10,
            height: 20,
            channels: Channels::RGB,
            colorspace: Colorspace::SrgbLinearAlpha,
        };
        let mut header: Vec<u8> = Vec::new();
        add_header(&mut header, &metadata);
        let expected_header: Vec<u8> = vec![b'q', b'o', b'i', b'f', 0, 0, 0, 10, 0, 0, 0, 20, 3, 0];
        
        assert_eq!(header, expected_header);

    }
    
    #[test]
    fn header_rgba_all() {
        let metadata = ImgMetadata {
            width: u32::MAX,
            height: u32::MAX,
            channels: Channels::RGBA,
            colorspace: Colorspace::AllLinearAlpha,
        };
        let mut header: Vec<u8> = Vec::new();
        add_header(&mut header, &metadata);
        let expected_header: Vec<u8> = vec![b'q', b'o', b'i', b'f', 255, 255, 255, 255, 255, 255, 255, 255, 4, 1];

        assert_eq!(header, expected_header);
    }

    #[test]
    fn chunk_rgb() {
        let pixel = vec![50, 50, 50];
        let mut bytes = Vec::new();
        add_chunks(&mut bytes, &pixel, false).unwrap();
        assert_eq!(bytes, vec![QOI_OP_RGB, 50, 50, 50]);
    }

    #[test]
    fn chunk_rgba() {
        let pixel = vec![50, 50, 50, 50];
        let mut bytes = Vec::new();
        add_chunks(&mut bytes, &pixel, true).unwrap();
        assert_eq!(bytes, vec![QOI_OP_RGBA, 50, 50, 50, 50]);
    }

    #[test]
    fn chunk_rgba_unchanged_alpha() {
        let pixel = vec![50, 50, 50, 255];
        let mut bytes = Vec::new();
        add_chunks(&mut bytes, &pixel, true).unwrap();
        assert_eq!(bytes, vec![QOI_OP_RGB, 50, 50, 50]);
    }

    #[test]
    fn tag_op_index() {
        let tag = QOI_OP_INDEX;
        let index: u8 = 0b00101011;

        let expected: u8 = 0b00101011;

        assert_eq!(tag_byte(tag, index), expected);
    }

    #[test]
    fn tag_op_luma() {
        let tag = QOI_OP_LUMA;
        let diff_green: u8 = 0b00110011;

        let expected: u8 = 0b10110011;

        assert_eq!(tag_byte(tag, diff_green), expected);
    }

    #[test]
    fn index_calculation() {
        let pixel = Pixel{
            r: 5,
            g: 107,
            b: 203,
            a: 251,
        };
        assert_eq!(calculate_index(&pixel), 60);
    }

    #[test]
    fn index() {
        let pixel_1 = vec![50, 50, 50];
        let pixel_2 = vec![255,255,255];
        let mut pixels = Vec::new();
        pixels.extend(&pixel_1);
        pixels.extend(&pixel_2);
        pixels.extend(&pixel_1);

        let mut bytes = Vec::new();
        add_chunks(&mut bytes, &pixels, false).unwrap();

        let op1 = vec![QOI_OP_RGB, 50, 50, 50];
        let op2 = vec![QOI_OP_RGB, 255, 255, 255];

        let pixel = Pixel{r: 50, g: 50, b: 50, a: 255};

        let op3 = vec![tag_byte(QOI_OP_INDEX, calculate_index(&pixel).try_into().unwrap())];

        let mut expected = Vec::new();
        expected.extend(op1);
        expected.extend(op2);
        expected.extend(op3);

        assert_eq!(bytes, expected);
    }

    #[test]
    fn diff_fn() {
        let prev = Pixel {
            r: 10,
            g: 5,
            b: 88,
            a: 0,
        };
        let curr = Pixel {
            r: 8,
            g: 4,
            b: 88,
            a: 0,
        };
        assert_eq!(create_diff(&curr, &prev), tag_byte(QOI_OP_DIFF, 0b00000110));
    }

    //todo - test to make sure diff and luma properly handle wraparounds
    #[test]
    fn diff() {
        let pixel_1 = Pixel {
            r: 50,
            g: 50,
            b: 50,
            a: 0,
        };
        let pixel_2 = Pixel {
            r: 51,
            g: 48,
            b: 49,
            a: 0,
        };
        let mut pixels = Vec::new();
        pixels.extend(vec![pixel_1.r, pixel_1.g, pixel_1.b]);
        pixels.extend(vec![pixel_2.r, pixel_2.g, pixel_2.b]);

        let mut bytes = Vec::new();

        add_chunks(&mut bytes, &pixels, false).unwrap();

        let mut op = vec![QOI_OP_RGB, 50, 50, 50];
        op.push(create_diff(&pixel_2, &pixel_1));

        assert_eq!(bytes, op);

    }

    #[test]
    fn luma_fn() {
        let p1 = Pixel {
            r: 50,
            g: 50,
            b: 50,
            a: 0,
        };
        let p2 = Pixel {
            r: 87,
            g: 80,
            b: 72,
            a: 0,
        };
        let op = create_diff_luma(&p2, &p1);

        let header = tag_byte(QOI_OP_LUMA, 62);
        let byte2 = ((0b1111 << 4) + 0b0000) as u8;
        assert_eq!(op, vec![header, byte2]);
    }

    #[test]
    fn luma() {
        let pixel_1 = Pixel {
            r: 50,
            g: 50,
            b: 50,
            a: 0,
        };
        let pixel_2 = Pixel {
            r: 73,
            g: 70,
            b: 65,
            a: 0,
        };
        let mut pixels = Vec::new();
        pixels.extend(vec![pixel_1.r, pixel_1.g, pixel_1.b]);
        pixels.extend(vec![pixel_2.r, pixel_2.g, pixel_2.b]);

        let mut bytes = Vec::new();

        add_chunks(&mut bytes, &pixels, false).unwrap();

        let mut expected = vec![QOI_OP_RGB, 50, 50, 50];
        let diff_luma = create_diff_luma(&pixel_2, &pixel_1);
        expected.extend(diff_luma);

        assert_eq!(bytes, expected);

    }

    #[test]
    fn op_run() {
        let px_0 = vec![0, 0, 0];
        let mut pixels = Vec::new();
        for _ in 0..3 {
            pixels.extend(&px_0);
        }
        let px_50 = vec![50, 50, 50];
        for _ in 0..5 {
            pixels.extend(&px_50);
        }
        pixels.extend(vec![100, 100, 100]);
        pixels.extend(&px_50);
        pixels.extend(&px_50);

        let mut bytes = Vec::new();
        add_chunks(&mut bytes, &pixels, false).unwrap();

        //QOI_OP_RUN lower bits have a bias of -1, so the lower bits are 1 lower than the run
        let mut expected = vec![tag_byte(QOI_OP_RUN, 2)];
        expected.extend(vec![QOI_OP_RGB, 50, 50, 50]);
        //this is 2 lower since the first px_50 was a QOI_OP_RGB
        expected.push(tag_byte(QOI_OP_RUN, 3));
        expected.extend(vec![QOI_OP_RGB, 100, 100, 100]);
        let p_50_px = Pixel {
            r: px_50[0],
            g: px_50[1],
            b: px_50[2],
            a: 255,
        };
        //"A valid encoder must not issue 2 or more consecutive QOI_OP_INDEX
        // chunks to the same index. QOI_OP_RUN should be used instead. "
        expected.push(tag_byte(QOI_OP_INDEX, calculate_index(&p_50_px).try_into().unwrap()));
        expected.push(tag_byte(QOI_OP_RUN, 0));

        assert_eq!(bytes, expected);
    }

    #[test]
    fn run_63(){
        let pixel = Pixel {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        let mut pixels = Vec::new();
        for _ in 0..63 {
            pixels.extend(vec![pixel.r, pixel.g, pixel.b]);
        }

        let mut bytes = Vec::new();
        add_chunks(&mut bytes, &pixels, false).unwrap();

        let mut expected = vec![tag_byte(QOI_OP_RUN, 61)];
        expected.push(tag_byte(QOI_OP_RUN, 0));

        assert_eq!(bytes, expected);

    }


    #[test]
    fn find_operation_rgb() {
        let pp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let cp = Pixel {
            r: 200,
            g: 100,
            b: 50,
            a: 40,
        };
        let index: [Option<Pixel>; 64] = [None; 64];

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpRgb);
    }

    #[test]
    fn find_operation_rgba() {
        let pp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let cp = Pixel {
            r: 200,
            g: 100,
            b: 50,
            a: 41,
        };
        let index: [Option<Pixel>; 64] = [None; 64];

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpRgba);
    }

    #[test]
    fn find_operation_index() {
        let ip = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let pp = Pixel {
            r: 100,
            g: 20,
            b: 30,
            a: 40,
        };
        let cp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let mut index: [Option<Pixel>; 64] = [None; 64];
        index[calculate_index(&ip)] = Some(ip.clone());

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpIndex);
    }

    #[test]
    fn find_operation_diff() {
        let pp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let cp = Pixel {
            r: 11,
            g: 20,
            b: 28,
            a: 40,
        };
        let index: [Option<Pixel>; 64] = [None; 64];

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpDiff);
    }

    #[test]
    fn find_operation_diff_wraparound() {
        let pp = Pixel {
            r: 0,
            g: 255,
            b: 0,
            a: 40,
        };
        let cp = Pixel {
            r: 255,
            g: 0,
            b: 254,
            a: 40,
        };
        let index: [Option<Pixel>; 64] = [None; 64];

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpDiff);
    }

    #[test]
    fn find_operation_luma() {
        let pp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let cp = Pixel {
            r: 37,
            g: 40,
            b: 42,
            a: 40,
        };
        let index: [Option<Pixel>; 64] = [None; 64];

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpLuma);
    }

    #[test]
    fn find_operation_luma_wraparound() {
        let pp = Pixel {
            r: 0,
            g: 255,
            b: 0,
            a: 40,
        };
        let cp = Pixel {
            r: 250,
            g: 1,
            b: 2,
            a: 40,
        };
        let index: [Option<Pixel>; 64] = [None; 64];

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpLuma);
    }

    #[test]
    fn find_operation_run() {
        let pp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let cp = Pixel {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        };
        let mut index: [Option<Pixel>; 64] = [None; 64];
        index[calculate_index(&pp)] = Some(pp.clone());

        assert_eq!(find_operation(&pp, &cp, &index), Operation::QoiOpRun);
    }

    #[test]
    fn push_run_test() {
        let mut bytes = Vec::new();

        push_run(&mut bytes, 10);

        let expected = vec![tag_byte(QOI_OP_RUN, 9)];

        assert_eq!(&bytes, &expected);
    }

    #[test]
    #[should_panic]
    fn push_run_63() {
        let mut bytes = Vec::new();

        push_run(&mut bytes, 63);
    }

    #[test]
    fn push_rgb_test() {
        let mut bytes = Vec::new();

        let pixel = Pixel {
            r: 77,
            g: 82,
            b: 51,
            a: 2,
        };

        push_rgb(&pixel, &mut bytes);

        let expected = vec![QOI_OP_RGB, 77, 82, 51];

        assert_eq!(&bytes, &expected);
    }

    #[test]
    fn push_rgba_test() {
        let mut bytes = Vec::new();

        let pixel = Pixel {
            r: 77,
            g: 82,
            b: 51,
            a: 2,
        };

        push_rgba(&pixel, &mut bytes);

        let expected = vec![QOI_OP_RGBA, 77, 82, 51, 2];

        assert_eq!(&bytes, &expected);
    }

    #[test]
    fn push_index_test() {
        let mut bytes = Vec::new();

        let pixel = Pixel {
            r: 77,
            g: 82,
            b: 51,
            a: 2,
        };
        push_index(&pixel, &mut bytes);

        let expected = vec![tag_byte(QOI_OP_INDEX, calculate_index(&pixel) as u8)];

        assert_eq!(&bytes, &expected);
    }

    #[test]
    fn push_diff_test() {
        let mut bytes = Vec::new();

        let prev = Pixel {
            r: 77,
            g: 82,
            b: 51,
            a: 2,
        };
        let curr = Pixel {
            r: 78,
            g: 81,
            b: 51,
            a: 2,
        };

        push_diff(&curr, &prev, &mut bytes);

        let expected = vec![create_diff(&curr, &prev)];

        assert_eq!(&bytes, &expected);
    }

    #[test]
    fn push_luma_test() {
        let mut bytes = Vec::new();

        let prev = Pixel {
            r: 88,
            g: 92,
            b: 60,
            a: 2,
        };
        let curr = Pixel {
            r: 77,
            g: 82,
            b: 51,
            a: 2,
        };

        push_luma(&curr, &prev, &mut bytes);

        let expected = create_diff_luma(&curr, &prev);

        assert_eq!(&bytes, &expected);
    }

}